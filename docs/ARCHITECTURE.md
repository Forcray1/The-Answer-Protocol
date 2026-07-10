# The Answer Protocol — Architecture & Onboarding Guide

> Purpose of this file: if you put the project down, learn some Rust, and come
> back in a month, this document lets you re-find yourself in the code. It
> explains **what each piece does**, **which Rust mechanic makes it work**, and
> **how a message flows through the whole program**.
>
> It describes the code *as it exists today*. When you change the code, update
> this file too.

---

## 1. The 30-second mental model

```
                      ONE TCP server, MANY clients
                      one message = one line ending in '\n'

   ┌────────────┐   text line: "MOVE north\n"      ┌──────────────────┐
   │ CLI client │ ───────────────────────────────► │                  │
   │ (client/)  │ ◄─────────────────────────────── │                  │
   └────────────┘   text line: "S: OK room-loc.X\n"│                  │
                                                   │   SERVER         │
   ┌────────────┐                                  │   (serveur/)     │
   │ GUI client │ ───────────────────────────────► │                  │
   │(client_bevy)│◄─────────────────────────────── │  authoritative   │
   └────────────┘                                  │  game state      │
                                                   └────────┬─────────┘
                                                            │ persists on disconnect
                                                            ▼
                                                   ┌──────────────────┐
                                                   │  SQLite game.db  │
                                                   │  (database/)     │
                                                   └──────────────────┘
```

**The one rule that explains the whole design:** the **server is the single
source of truth**. Clients are "dumb terminals" — they send text commands and
display text replies. They never decide game logic. The room you are in, the
items on the floor, an NPC's HP — all of that lives *only* on the server. This
is why a Pokémon-style GUI only renders things; it never owns game state.

---

## 2. The project is a Cargo *workspace* (4 crates)

`Cargo.toml` at the root is not a normal package — it is a **workspace** that
groups four independent crates that compile together:

```
The-Answer-Protocol/
├── Cargo.toml          ← workspace: lists the 4 members below
├── world.yaml          ← the game world (rooms, items, npcs, quests) as data
├── game.db             ← SQLite file (auto-created) holding saved players
├── logs.json           ← structured JSON logs (only when run with --logs)
│
├── serveur/            ← CRATE 1: the TCP server + all game logic
│   └── src/
│       ├── main.rs        network loop, one task per client, logging
│       ├── commands.rs    text line  ->  GameCommand enum (the parser)
│       ├── world.rs       YAML  ->  Rust structs (the static catalog)
│       ├── state.rs       the live, mutable game state in RAM
│       └── handlers.rs    one fn per command: the actual rules
│
├── client/             ← CRATE 2: the CLI client (raw protocol in/out)
│   └── src/main.rs
│
├── client_bevy/        ← CRATE 3: the GUI client (Bevy game engine)
│   └── src/main.rs
│
└── database/           ← CRATE 4: SQLite persistence (a reusable library)
    ├── src/lib.rs         register / verify / load / save player
    └── migrations/        SQL that creates the tables on first run
```

**Rust mechanic — workspace + library crate:** `serveur`, `client`, and
`client_bevy` are *binary* crates (they have a `main()`). `database` is a
*library* crate (no `main`, just functions). The server depends on it and calls
`database::init_db(...)`, `database::save_player(...)`, etc. That is why you see
`database::` (with no `crate::`) used inside the server — it is an external crate
from the server's point of view, even though it lives in the same repo.

---

## 3. The three layers of "state" (the most important idea)

Beginners get lost here, so internalize this. The same item (say
`epee_rouillee`) exists in **three different forms** at three different layers:

| Layer | Lives in | Form of `epee_rouillee` | Mutable? | Survives restart? |
|---|---|---|---|---|
| **Static catalog** | `world.yaml` → `world.rs` structs | the *definition*: name, +10 dmg, slot=weapon | no (read-only) | yes (it's a file) |
| **Runtime state** | `state.rs` (`ServerState`) | *where the instance is right now* (room floor / a player's inventory) | yes (changes constantly) | **no** (RAM only) |
| **Persistence** | `database/` → `game.db` | a player's saved inventory as JSON text | yes | yes |

Read that table twice. Almost every handler in `handlers.rs` works by
**looking up the definition in the catalog** (`world.world.items.iter().find(...)`)
and then **mutating the runtime state** (move it from `room_items` into
`player.inventory`). On disconnect, the runtime state is **flattened into JSON
and written to SQLite**. On reconnect, JSON is read back and rebuilt into runtime
state.

- **Catalog** answers *"what is this thing?"* → `serveur/src/world.rs`
- **Runtime** answers *"where is it / what's its HP now?"* → `serveur/src/state.rs`
- **Persistence** answers *"what did this player have when they left?"* → `database/src/lib.rs`

This is exactly the "definition vs. placement" distinction we discussed for
items, generalized to the entire game.

---

## 4. Concepts & the Rust mechanic behind each

This section is a tour. For each concept: what it is, **where** it is in your
code, and the Rust feature that powers it.

### 4.1 Async / `tokio` / `.await` — doing many things at once
A game server must talk to many players "simultaneously" without one slow client
freezing the others. Rust does this with **async tasks** managed by the `tokio`
runtime.

- `#[tokio::main]` on `serveur/src/main.rs:52` turns `main` into an async program.
- `tokio::spawn(async move { ... })` at `main.rs:90` launches **one lightweight
  task per connected client**. Thousands can run on a few OS threads.
- `.await` means *"this might take time (network, disk); let other tasks run
  while we wait."* You'll see it on every network read, DB query, etc.

> Mental shortcut: a `fn` that is `async` returns a *promise of a value later*.
> You must `.await` it to get the value. If you forget `.await`, nothing happens.

### 4.2 TCP + line framing — the wire format
- `TcpListener::bind("127.0.0.1:4243")` (`main.rs:78`) opens the port.
- `listener.accept().await` (`main.rs:82`) blocks until a client connects, giving
  back a `socket` and the client's address `addr`.
- **Framing rule (from the subject):** one message per line, `\n`-terminated,
  UTF-8. `BufReader::read_line(&mut line)` reads exactly up to the next `\n`.
  That's why every reply string in `handlers.rs` ends in `\n`.

### 4.3 `SocketAddr` as the player's identity
Every player is keyed by their network address `addr` (`std::net::SocketAddr`).
Look at `ServerState.players: HashMap<SocketAddr, Player>` (`state.rs:73`). When a
command arrives, the server uses `addr` to find *which* player sent it. This is
the thread connecting the network layer to the game layer.

### 4.4 `Arc` — sharing one thing across many tasks
Each client task needs access to the **same** world, the **same** state, the
**same** database pool. Rust forbids two owners of one value, so we wrap shared
things in `Arc<T>` (Atomically Reference-Counted pointer):

```rust
let shared_world = Arc::new(world_data);      // main.rs:68
...
let world = Arc::clone(&shared_world);        // main.rs:84  (cheap: bumps a counter)
```

`Arc::clone` does **not** copy the world — it just makes another handle to the
same data and increments a counter. When the last handle drops, the data is
freed. This is how all client tasks read the same map.

### 4.5 `Mutex` — safely *changing* shared state
`Arc` alone only allows *reading*. To **mutate** shared state from many tasks
without data races, you wrap it: `Arc<Mutex<ServerState>>` (`main.rs:73`).

```rust
let mut guard = state.lock().await;   // handlers run here: main.rs:120
// ... mutate guard ...
// guard is dropped at end of scope -> lock released automatically
```

`lock().await` waits its turn, hands you exclusive access (`guard`), and the lock
is released the moment `guard` goes out of scope. Note the server locks the state
**only for the duration of processing one command** (`main.rs:119-131`), then
releases — so other players aren't blocked while one client is idle.

> The DB pool is `Arc<SqlitePool>` with **no** `Mutex` (`main.rs:22`). Why? Because
> `SqlitePool` is already internally thread-safe (it manages its own connections).
> Wrapping it in a Mutex would needlessly serialize all DB access.

### 4.6 `broadcast` channel — pushing events to everyone
Chat, deaths, joins/leaves must reach **other** clients asynchronously. That's a
`tokio::sync::broadcast` channel (`main.rs:75`). One sender, many receivers:

- Any handler can `tx.send(GlobalEvent { sender_addr, message })` (e.g. chat,
  `handlers.rs:508`).
- Every client task holds a receiver `rx` and listens for these events
  (`main.rs:150`). The check `if event.sender_addr != addr` means *"don't echo my
  own message back to me."*

`GlobalEvent` (`main.rs:24`) is just `{ who sent it, the text line to forward }`.

### 4.7 `tokio::select!` — read network OR receive an event, whichever first
This is the heart of the per-client loop (`main.rs:100`). Each client task must do
two things at once:
1. wait for **the client to send a command** (`reader.read_line`), and
2. wait for **a broadcast event to forward** (`rx.recv`).

`tokio::select!` waits on both branches simultaneously and runs whichever fires
first. This is exactly the subject's requirement that clients "remain responsive
while receiving asynchronous events." The CLI client uses the same pattern
(`client/src/main.rs:21`): select between *server sent me a line* and *user typed
a line*.

### 4.8 `serde` + `Deserialize` — turning YAML into Rust structs
`world.rs` is almost entirely **struct definitions with `#[derive(Deserialize)]`**.
That derive teaches `serde` how to fill a struct from a file. The whole load is
three lines (`world.rs:111`):

```rust
let content = std::fs::read_to_string(path)?;          // YAML text
let data: WorldData = serde_yaml::from_str(&content)?;  // text -> structs
```

The field names in the structs must match the YAML keys. Useful attributes you
already use:
- `#[serde(default)]` — if the YAML omits this field, use the type's default
  (e.g. `damage: Option<i32>` becomes `None`). This is why optional NPC fields
  work.
- `#[serde(default = "default_item_type")]` (`world.rs:68`) — call a function for
  the default; this is how an item with no `type:` becomes `Standard`.
- `#[serde(rename_all = "lowercase")]` on the `ItemType` enum — YAML `"quest"`
  maps to the `Quest` variant.
- `#[serde(tag = "type", ...)]` on `QuestObjective` (`world.rs:90`) — an *enum
  driven by a field*: `type: fetch_item` picks the `FetchItem` variant. This is
  how one `quests:` list can hold different objective shapes.
- `r#type` (`world.rs:69`) — `type` is a reserved Rust keyword, so `r#` lets you
  use it as a field name to match the YAML key `type`.

### 4.9 Enums + `match` — the command parser
`GameCommand` (`commands.rs:4`) is an **enum**: a closed list of every command the
game understands, each carrying its own data (`Move(String)`,
`Chat { channel, message }`, ...). `GameCommand::parse` (`commands.rs:28`) turns a
raw line into one of these:

```
"TAKE Rusty Sword"  -> split -> ["TAKE","Rusty","Sword"]
                    -> match "TAKE" -> Take("Rusty Sword")   // parts[1..].join(" ")
```

Then `handlers::process_command` (`handlers.rs:20`) `match`es on the enum and calls
the right handler. **The `match` is exhaustive** — Rust forces you to handle every
variant, which is why adding a command means touching both files (see §6).

> Note: multi-word names are supported by `parts[1..].join(" ")` — that is the
> subject's "multi-word item names" requirement, living at `commands.rs:50-51`.

### 4.10 `Option` / `Result` / `if let` — no nulls, no exceptions
Rust has **no null and no exceptions**. Instead:
- `Option<T>` is `Some(value)` or `None` — "maybe there's a value." Looking up a
  player returns `Option<&Player>` because the address might not be connected.
- `Result<T, E>` is `Ok(value)` or `Err(error)` — "this might fail." DB calls
  return `Result`.

You unwrap them with `if let` / `match`. This is why almost every handler is a
staircase like:

```rust
if let Some(player) = state.players.get(&addr) {     // is this addr a player?
    if let Some(loc) = world.world.locations.get(&player.current_room) { // room exists?
        ... // happy path
    } else { "S: ERR room_not_found\n".to_string() }
} else { "S: ERR utilize_connect_first\n".to_string() } // not connected
```

Read the `else` branches bottom-up and you get the error cases for free. The `?`
operator (e.g. `main.rs:61`) is shorthand: "if this `Result` is `Err`, return it
immediately."

### 4.11 Ownership, `&`, `&mut`, and `.clone()`
This is the Rust idea to actually learn before resuming. Rough rules as they show
up here:
- `&T` = borrow to **read** (`state: &ServerState` in `handle_look`).
- `&mut T` = borrow to **modify** (`state: &mut ServerState` in `handle_move`).
  Only one `&mut` can exist at a time — that's the borrow checker protecting you.
- `.clone()` = make an owned copy when you can't keep a borrow. You'll notice
  `player.current_room.clone()` a lot: the code copies the room id out so it can
  then take a *mutable* borrow of `state` without the compiler complaining that
  two borrows overlap. That's a very common beginner pattern and it's fine.
- `&mut *guard` (`main.rs:121`) = "reborrow the thing inside the Mutex guard as a
  plain `&mut ServerState`" so it can be passed to handlers.

### 4.12 Collections you use
- `HashMap<K, V>` — a dictionary. `players: HashMap<SocketAddr, Player>`,
  `inventory: HashMap<String, u32>` (item id → how many).
- `HashSet<T>` — a set of unique values. `collected_by: HashSet<String>` tracks
  *which usernames already grabbed a quest item* so a shared quest item can be
  taken once per player without being removed for others (`state.rs:22`,
  logic at `handlers.rs:295`).
- `Vec<T>` — a growable list. `room_items`, `dead_npcs`, `completed_quests`.

### 4.13 Persistence & hashing (the `database` crate)
- `init_db` (`lib.rs:27`) creates `game.db` if missing and runs the SQL in
  `migrations/` to build tables. `sqlx::migrate!` bakes the migrations into the
  binary at compile time.
- Passwords are **hashed with Argon2** (`lib.rs:65`), never stored in plaintext.
  `verify_player` re-hashes the input and compares.
- `inventory`, `equipment`, `completed_quests` are stored as **JSON strings** in
  single columns (`serde_json::to_string` on save, `from_str` on load). That's
  the bridge between runtime `HashMap`s and flat SQL columns.

---

## 5. Information flow — three commands traced end to end

### 5.1 A brand-new player connects: `CONNECT alice secret`
```
client types "CONNECT alice secret\n"
  → server task reads the line                                main.rs:101
  → GameCommand::parse  →  Connect{username:"alice", password:"secret"}  commands.rs:41
  → lock state                                                main.rs:120
  → process_command matches Connect  →  handle_connect        handlers.rs:29
       → database::load_player("alice")  →  Ok(None) (unknown) handlers.rs:95
       → database::register_player(...)  (hashes password)     handlers.rs:133
       → state.add_player(addr, "alice")  → Player at 100 HP, room "village_square"  state.rs:134
       → tx.send(GlobalEvent "... alice vient de se connecter") handlers.rs:139
  → reply "S: OK connected\n" written back to alice           main.rs:133
  → meanwhile every OTHER client's task wakes on rx.recv and
    forwards the join message                                 main.rs:150
```

### 5.2 Movement: `MOVE north`
```
"MOVE north\n"  →  parse  →  Move("north")                    commands.rs:47
  → handle_move                                               handlers.rs:194
      → cooldown check (500ms anti-spam) using player.last_move
      → look up current room in the CATALOG, read its exits   handlers.rs:198-199
      → if exit "north" exists: player.current_room = next    (mutate RUNTIME state)
      → reply "S: OK room-loc.<next>\n"
```
Notice: movement only changes `player.current_room` (a `String`). There are **no
tile coordinates** on the server — exactly why a Pokémon-style GUI keeps tile/
pixel movement client-side and only sends `MOVE <dir>` when the sprite crosses a
door. (See `docs/` notes / the GUI section.)

### 5.3 Taking an item: `TAKE Rusty Sword`
```
"TAKE Rusty Sword\n"  →  parse  →  Take("Rusty Sword")        commands.rs:50
  → handle_take                                               handlers.rs:286
      → find the item DEFINITION in the catalog by id OR name handlers.rs:289
      → find that item in room_items (RUNTIME floor list)     handlers.rs:291
      → if Standard: remove the instance from the room        handlers.rs:294
        if Quest:    record username in collected_by instead  handlers.rs:296-298
      → player.inventory[id] += 1                             handlers.rs:304
      → check quests: did taking this item complete a FetchItem quest? give EXP  handlers.rs:307
      → reply "S: OK Tu as ramassé : <name> [QUÊTE...]\n"
```
This is the catalog-vs-runtime dance in one function: **read the definition,
mutate the placement.**

### 5.4 What happens on disconnect (graceful OR a dropped connection)
Both paths converge on the same cleanup (`main.rs:103` for a lost socket,
`main.rs:135` for `QUIT`):
```
remove_player(addr) pulls the Player OUT of runtime state   state.rs:150
  → save_player_to_db: flatten inventory/equipment/quests to JSON, UPDATE game.db  handlers.rs:52
  → tx.send "... a quitté le monde"  (broadcast leave)       handlers.rs:140
  → break the loop, task ends, socket closes
```
This satisfies the subject's "remove player state before broadcasting leave" rule.

---

## 6. Where do I add things? (extension points)

**To add a new command** (say `DEFEND`):
1. `commands.rs` — add a variant `Defend` to the `GameCommand` enum, and a match
   arm in `parse` that recognizes the word `"DEFEND"`.
2. `handlers.rs` — add an arm in `process_command` and write `fn handle_defend(...)`.
3. If it changes saved data, update `state.rs` (the `Player` struct) and the
   `database` crate (column + load/save).

**To add world content** (rooms, items, npcs, quests): edit **`world.yaml` only**.
As long as the keys match the structs in `world.rs`, no Rust changes are needed.
If you add a *new field*, add it to the matching struct in `world.rs`.

**To change combat / quests:** it's all in `handlers.rs` — `handle_attack`
(damage = `(atk - def).max(1)`, `handlers.rs:440`), `handle_take`/`handle_talk`
for quest completion. Respawns live in `state.rs:update_respawns`.

**To build the Pokémon-style GUI:** all of it goes in `client_bevy/` and talks to
the server only through the existing text protocol. The server does **not** change.
Split `client_bevy/src/main.rs` into Bevy plugins (`net/`, `world/`, `player/`,
`ui/`). Player sprite/tile movement is purely local; cross a door tile → send
`MOVE <dir>` → on `S: OK room-loc.X` swap the rendered map. Other players appear
via `EVT ROOM PRESENCE ENTER/LEAVE`.

---

## 7. "Where am I?" cheat sheet

| I want to find... | Look in |
|---|---|
| How a typed line becomes a command | `serveur/src/commands.rs` |
| The rules of a specific command | `serveur/src/handlers.rs` (`handle_*`) |
| What a room / item / npc / quest *is* | `serveur/src/world.rs` + `world.yaml` |
| Live game state (who's where, floor items, HP) | `serveur/src/state.rs` |
| The network loop / one-task-per-client / events | `serveur/src/main.rs` |
| Saving & loading players, passwords | `database/src/lib.rs` |
| The raw text CLI | `client/src/main.rs` |
| The graphical client | `client_bevy/src/main.rs` |
| The actual map content | `world.yaml` (root) |

---

## 8. Glossary (Rust terms you'll re-Google)

- **crate** — a Rust package (a binary or a library). You have 4.
- **`Arc<T>`** — shared read-only handle to one heap value across tasks/threads.
- **`Mutex<T>`** — guards a value so only one task mutates it at a time.
- **`async` / `.await`** — cooperative concurrency; "pause here, let others run."
- **`tokio::spawn`** — start a concurrent task.
- **`tokio::select!`** — wait on several async things, act on the first ready.
- **broadcast channel** — one-to-many message bus (used for events/chat).
- **`Option<T>`** — `Some`/`None`, Rust's replacement for null.
- **`Result<T,E>`** — `Ok`/`Err`, Rust's replacement for exceptions; `?` propagates.
- **`derive(Deserialize)`** — auto-generate "fill this struct from a file" code.
- **borrow (`&` / `&mut`)** — temporary access without taking ownership.
- **`.clone()`** — make an owned copy to escape a borrow conflict.
```
