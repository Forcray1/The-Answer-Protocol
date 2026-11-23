*This project has been created as part of the 42 curriculum by avauclai, mlorenzo.*

# TAP — The Answer Protocol

A shared-world retro text adventure: a Rust TCP server, a command-line client and a graphical (Bevy) client that all speak the line-based **TAP** protocol (RFC 42TAP).

## Table of contents

- [Description](#description)
- [Instructions](#instructions)
- [Building and Running](#building-and-running)
- [Architecture](#architecture)
- [Protocol Implementation](#protocol-implementation)
- [Combat System](#combat-system)
- [Quest System](#quest-system)
- [World Design](#world-design)
- [Server Logging](#server-logging)
- [Testing](#testing)
- [Group Contributions](#group-contributions)
- [Resources](#resources)
- [Timeline](#timeline) (development history and open tasks)
- [Developer notes: audit and known gaps](#developer-notes-audit-and-known-gaps)

## Description

TAP is a small multiplayer text adventure in the spirit of the old MUDs. Several players connect to one server, explore a shared map, chat on three channels (global, room, group), pick up and drop items, talk to NPCs, complete quests, buy equipment and fight monsters.

The project delivers:

| Component  | Crate         | Description                                                                                                           |
| ---------- | ------------- | --------------------------------------------------------------------------------------------------------------------- |
| Server     | `server`      | Async TCP server (Tokio). Holds the live world state in memory and speaks the TAP protocol.                           |
| CLI client | `client`      | Minimal terminal client. Shows server messages in real time while you type raw protocol commands.                     |
| GUI client | `client_bevy` | Graphical client built with Bevy and egui: 2D rooms, sprites, inventory, quest log, chat box, combat screen, minimap. |
| Domain     | `domain`      | Types shared by the whole workspace (ids, items, equipment, stats, quest stages, groups).                             |
| Database   | `database`    | SQLite persistence (accounts with Argon2 password hashes, progress, inventory, position).                             |
| World data | `world.yaml`  | Static world: rooms, exits, NPCs, items and quests (`listing_items.md` lists the items and mobs).                     |

All code is Rust, as required (C, C++, Rust, Go or Zig only). No Python is used.

## Instructions

Prerequisites:

- Rust **1.85 or newer** (`client_bevy` uses edition 2024), installed with [rustup](https://rustup.rs).
- GNU Make.
- On Linux, system libraries for Bevy (audio, udev, windowing). On Debian/Ubuntu run `make install-deps-linux`.

Quick start (three terminals, all from the repository root):

```
make install        # fetch dependencies
make run-server     # terminal 1
make run-client     # terminal 2 (CLI)
make run-client-gui # terminal 3 (GUI)
```

The server must be started **from the repository root** because it reads `./world.yaml` and creates `./game.db` and `./logs.json` in the current directory.

## Building and Running

The build tool is **Cargo** (workspace), wrapped by a **Makefile** that provides the standard targets required by the subject (install dependencies, run-server, run-client, run-client-gui, lint, clean).

| Target                                             | What it does                                                                                 |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `make install`                                     | Check the toolchain and fetch all crates (`cargo fetch`).                                    |
| `make install-deps-linux`                          | Debian/Ubuntu only: install the system libraries Bevy needs.                                 |
| `make build` (or `make all`)                       | Build the whole workspace (server, CLI, GUI).                                                |
| `make build-server` / `build-client` / `build-gui` | Build one component.                                                                         |
| `make run-server`                                  | Start the server on `127.0.0.1:4243` (logs go to stdout and `logs.json`).                    |
| `make run-client`                                  | Start the CLI client.                                                                        |
| `make run-client-gui`                              | Start the Bevy GUI client.                                                                   |
| `make lint`                                        | `cargo fmt --check` + `cargo clippy -D warnings`. Run `make fmt` first if formatting fails.  |
| `make fmt`                                         | Format the code with rustfmt.                                                                |
| `make test`                                        | Run the unit tests.                                                                          |
| `make clean`                                       | Remove `target/`.                                                                            |
| `make fclean`                                      | `clean` + delete `game.db` and `logs.json` (**all accounts are lost**).                      |
| `make re`                                          | `fclean` then `all`.                                                                         |
| `make help`                                        | List all targets.                                                                            |

Variables:

- `PROFILE=debug` builds without `--release` (faster to compile, but the GUI runs slowly). Default: `release`.
- `LOGS=0` starts the server without the `--logs` flag. Default: `1`. (The server writes its logs in both cases, see [Server Logging](#server-logging).)

Examples:

```
make run-server LOGS=0
make run-client-gui PROFILE=debug
```

Both clients connect to `127.0.0.1:4243` (hard-coded constant, see [Developer notes](#developer-notes-audit-and-known-gaps)).

The code is linted without crate-level `#![allow(...)]` attributes: warnings must be fixed, not silenced.

### Connecting

On connection the server sends a greeting, then you identify yourself:

```
S: OK hello proto=1
C: CONNECT <username> <password>
S: OK connected skin=default name=<username> room=Start_oasis exits=north,east,south,west
```

If the username does not exist the account is created with the given password; otherwise the password is verified (`ERR bad_credentials` on failure). `CONNECT <username>` **without a password** opens a temporary *guest* session that is never saved. Until `CONNECT` succeeds, game commands answer `ERR utilize_connect_first`.

### CLI client

The CLI client implements the first approach allowed by the subject: **what you type is sent unchanged to the server** (RFC syntax), and everything the server sends is printed immediately, including asynchronous events, because reading the socket and reading stdin run concurrently (`tokio::select!`). `QUIT` ends the program cleanly. We chose this approach because it keeps the CLI a faithful, easy-to-debug view of the protocol; the friendly interface is the job of the GUI.

### GUI client

The GUI client logs in through a login menu, shows the current room as a 2D scene (map layers, collisions, other players, NPCs and mobs), and sends protocol commands on your behalf: movement between rooms, `POS` updates, `INTERACT` on NPCs (E key), the combat menu (attack / heavy / magic / flee / item), inventory and equipment actions (equip, use, drop, stat points), the shop and quest-board windows, the quest log (`QUESTS`) and a chat box with separate Global / Room / Group channels. The chat view is separate from the log view (the developer terminal, opened with F1, shows the raw server lines). The minimap shows the number of players in the room and on the server (`WHO`). Movement supports both **WASD** and **ZQSD** layouts. A text console is also available for raw commands.

## Architecture

**Concurrency model.** One Tokio task per TCP connection, all sharing:

- `Arc<WorldData>`: immutable world loaded from `world.yaml` at startup (the loader checks that every exit and reference points to something that exists).
- `Arc<Mutex<ServerState>>`: all live state (players by socket address, items on the ground, NPC hit points, dead NPCs waiting for respawn, active combats, groups).
- `Arc<SqlitePool>`: persistence.
- A `tokio::sync::broadcast` channel of `GlobalEvent`s.

A separate task ticks once per second to respawn dead NPCs, and another one saves everything on Ctrl-C.

**Request path (dispatcher/router).** Each connection task `select!`s between (a) a line arriving from its socket and (b) an event arriving on the broadcast channel.

1. A line is parsed by `GameCommand::parse` (`server/src/commands.rs`) into a typed enum.
2. `handlers::process_command` (`server/src/handlers/mod.rs`) is a **dispatcher/router**: a single `match` on the enum calls one `handle_*` function per command. Handlers mutate the state and return the reply text; the connection task writes it to the socket. We chose a dispatcher over inline handling so that each command lives in its own small function that can be read and tested on its own.
3. Handlers that must notify other players push a `GlobalEvent` with one of the helpers of `events.rs` (`to_player`, `to_room`, `to_group`, `to_all`).

**Event routing.** A `GlobalEvent` carries a message plus an optional target: one player (`target_player`), a room (`target_room`), a group (`target_group`) or everybody. Every connection task receives every event and decides whether to write it, by checking its own player's room/group. An event is never echoed to the player in `sender_addr`.

**Disconnects.** On `QUIT`, EOF or socket error the player is removed from the state first, saved to the database, and only then are the `PRESENCE LEAVE` and global chat events broadcast, so nobody can interact with a player who is already gone. A failed send to one client never stops delivery to the others.

**Anti-spam.** Per-player cooldowns: 500 ms between `MOVE`, 1 s between attacks, 2 s between `CHAT`. The server also logs flooding patterns (see [Server Logging](#server-logging)).

**Crate layout.**

```
Cargo.toml                   workspace
server/src/main.rs           startup: database, world, listener, tasks
server/src/connection.rs     one client: read lines, run commands, forward events
server/src/tasks.rs          background tasks: NPC respawn, shutdown save
server/src/events.rs         GlobalEvent and the to_player/to_room/to_group/to_all helpers
server/src/logging.rs        structured JSON logging
server/src/persistence.rs    ground items written back to world.yaml on shutdown
server/src/commands.rs       command parser
server/src/state.rs          live game state
server/src/world.rs          world.yaml data model and lookups
server/src/board_quests.rs   quest-board generator
server/src/handlers/         command handlers (mod.rs = dispatcher), one file per topic:
                             connect, movement, inventory, equip, shop, npc, quest_*,
                             board, combat, magic, heavy, outcome, chat, group, wire, util
domain/                      shared types (ids, items, equipment, quests, groups)
database/                    SQLite access (init, auth, player, model) + migrations
client/                      CLI client
client_bevy/src/             GUI client: main, net, menu, game, map, minimap, shop,
                             interaction, board_quest_ui, and the folders
                             ui/ (inventory, chat, quest, hud), combat/, player/, mob/
world.yaml                   world data
```

## Protocol Implementation

Framing: TCP, UTF-8, one message per line terminated by `\n`. Server lines start with `S: `; replies begin with `OK` or `ERR`; asynchronous pushes begin with `EVT`.

### Commands implemented

| Command                                                                         | Notes                                                                                                                  |
| ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `CONNECT <name> [password]`                                                     | Creates the account on first use. Without a password: temporary guest session.                                         |
| `LOOK`                                                                          | Room name, description, items on the ground, NPCs present.                                                             |
| `MOVE <north\|south\|east\|west>`                                               | Replies `OK room=loc.<room> exits=...`.                                                                                |
| `TAKE <id or name>` / `DROP <id or name>`                                       | Items are matched on id or display name, case-insensitively; multi-word names work.                                    |
| `INVENTORY`                                                                     | Replies with a readable list plus `EVT INV_DATA`, `EVT EQUIP_DATA` and `EVT CHAR_STATS`.                               |
| `TALK <npc>`                                                                    | Dialogue; also advances quests.                                                                                        |
| `ATTACK <enemy>`                                                                | See [Combat System](#combat-system).                                                                                  |
| `STATUS`                                                                        | HP, level, XP, gold, location and combat status.                                                                       |
| `QUEST [id or name]`                                                            | Without argument, returns details for the first active quest; with an id/name, returns detailed status for that quest. |
| `QUESTS`                                                                        | Replies with an `EVT QUEST_DATA` line.                                                                                 |
| `WHO`                                                                           | `OK { "room": [...], "server": N }`.                                                                                   |
| `CHAT <GLOBAL\|ROOM\|GROUP> <text>`                                             | Global, room and group channels.                                                                                       |
| `GROUP`, `GROUP CREATE`, `GROUP INVITE <player>`, `GROUP ACCEPT`, `GROUP LEAVE` | Group management.                                                                                                      |
| `QUIT`                                                                          | `OK goodbye`, then the connection is closed.                                                                           |

### Events pushed by the server

`EVT GLOBAL CHAT`, `EVT ROOM <room> CHAT`, `EVT GROUP CHAT`, `EVT GROUP CREATED|INVITED|JOIN|LEAVE|DISBAND`, `EVT ROOM <room> PRESENCE ENTER|LEAVE|ROSTER`, `EVT ROOM <room> MOB_SPAWN|MOB_DESPAWN|BOARD_SPAWN|ITEM_SPAWN|ITEM_DESPAWN|POS`, `EVT COMBAT START|HIT|BONUS_HIT|STUN|STUN_APPLIED|STUN_HIT|END`, `EVT PLAYER_STATS`, `EVT CHAR_STATS`, `EVT INV_DATA`, `EVT EQUIP_DATA`, `EVT GOLD`, `EVT STAT_POINTS`, `EVT SHOP`, `EVT BOARD_QUESTS`, `EVT QUEST_DATA`, `EVT QUEST_UPDATE`, `EVT QUEST_RESET`, `EVT DIALOGUE`, `EVT DIALOGUE_CHOICE`.

### Deviations and extensions

| Area            | Behaviour here                                                                                                                                                                                                                       | Reason                                                                                   |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| `CONNECT`       | Takes an optional password (second argument). Without it the session is a guest.                                                                                                                                                      | The server has persistent accounts; the GUI has a login form; guests ease testing.       |
| Reply payloads  | Several replies are human-readable text (`LOOK`, `INVENTORY`, `STATUS`, `TAKE`, `DROP`) instead of the structured examples of the subject.                                                                                            | Written for readability in the CLI first.                                                |
| Presence events | `EVT ROOM <room> PRESENCE ENTER <name> <skin> <x> <y>` carries the room id, a skin and a position.                                                                                                                                    | The GUI needs to place players in the room.                                              |
| Extra commands  | `POS <x> <y>`, `INTERACT <npc>`, `ANSWER <npc> <answer>`, `EQUIP`, `UNEQUIP`, `EQUIPMENT`, `INFO`, `USE <item>`, `STATUP <stat>`, `BUY <npc> <item>`, `BOARD_QUESTS`, `BOARD_TAKE <slot>`, `BOARD_ABANDON`, `FLEE` (aliases `ESCAPE`, `FUIR`), `ATTACK_MAGIC`, `ATTACK_HEAVY`, `ATTACK_MAGIC_HEAVY`. | Needed by the GUI (position sync, NPC interaction, equipment, shop, board) and by combat. |
| Extra events    | Every event of the list above that is not in the base RFC (`MOB_*`, `POS`, `*_DATA`, `GOLD`, `SHOP`, `COMBAT *`, `DIALOGUE*`, ...).                                                                                                    | Same reason. A client that only knows the base RFC can ignore them.                      |
| `QUEST`         | Implemented. Returns one quest detail object (`OK { ... }`) for a specific id/name or for the first active quest when called without an argument.                                                                                      | Keeps `QUESTS` as the multi-quest listing command.                                       |
| Errors          | Protocol errors are snake_case codes: `malformed_command`, `utilize_connect_first`, `already_logged_in`, `bad_credentials`, `unknown_direction`, `room_not_found`, `unknown_channel`, `chat_spam_forbidden`, `movement_cooldown_too_fast`, `attack_cooldown`, `item_unknown`, `item_not_in_inventory`, `npc_not_found`, `not_enough_gold`, `no_stat_points`, `temple_pass_required`, `internal_error`, ... Some gameplay errors are English sentences. | Historical; the code set should be unified (see developer notes).                        |

## Combat System

Combat is **turn-based and resolved server-side**. A player starts or continues a fight by sending `ATTACK <enemy>` (or `INTERACT` on an enemy, which opens the fight). Only NPCs with `role: enemy` can be attacked, and only in the player's current room.

**Initiative order.** There is no initiative roll: the player always acts first. Each attack command is one round: the player strikes, and if the enemy survives it counter-attacks immediately. We chose this because the server is request/response: the player's command is what advances the turn, and a fixed order keeps fights predictable for the GUI turn animation.

**Damage formulas.**

```
physical attack = stats.damage + weapon damage         (+20% with the strength medaillon)
magic attack    = stats.special_damage                 (+12 with the magic staff, +20% with the magical strength medaillon)
damage_to_enemy = max(1, attack - enemy.defense)       (enemy.special_defense for magic)
enemy_attack    = enemy.damage                         (default 5)
damage_to_player = max(1, enemy_attack - (defense + armor defense))   (special_defense instead of defense after a magic attack)
```

- Players start with **100 HP** (`max_hp`, raised by level-ups through stat points); enemies have their own `hp`, `damage`, `defense`, `special_defense` and `exp_reward` in `world.yaml`.
- `STATUS` shows current HP, level, XP, gold, location and whether the player is in combat.
- Every attack is rate-limited to one per second (`ERR attack_cooldown`).
- Combat results are broadcast: the replies and the `EVT COMBAT *` / `EVT PLAYER_STATS` events update the fighter, `EVT ROOM <room> MOB_DESPAWN` tells the room that a monster died, and fleeing and deaths are announced in the chat.

**Combat commands (our additions to the base `ATTACK`).**

| Command                                  | Effect                                                                                                                               |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `ATTACK_MAGIC <enemy>`                   | Same exchange, scaling on `special_damage` against `special_defense`.                                                                |
| `ATTACK_HEAVY <enemy>`                   | Physical hit followed by a bonus hit of 75% of its damage (total ×1.75). The player is then **stunned for one turn**: the enemy gets a free counter-attack (`EVT COMBAT STUN_*`). |
| `ATTACK_MAGIC_HEAVY <enemy>`             | Same heavy mechanic with the magic formula.                                                                                          |
| `FLEE` (aliases `ESCAPE`, `FUIR`)        | **75% chance** to escape: the fight ends (`EVT COMBAT END flee`). On failure the player stays in combat (`flee_failed`) and the turn is lost. |
| `USE <potion>`                           | Out of the attack menu: consumes a potion and heals a percentage of `max_hp`.                                                        |

There is no separate `DEFEND` command: defence is handled by the `defense` / `special_defense` stats, armor and potions, and by choosing `FLEE` when a fight goes badly. This is a conscious design choice (the combat stays centred on the stats the player invests in with `STATUP`).

**Rewards.** When the enemy reaches 0 HP the player gains `exp_reward` XP (plus the experience medaillon bonus), the loot table is rolled by percentage, kill-quest counters advance, and the mob respawns after its `respawn_time` (default 30 s). Each level costs `4*level^2 + 56` XP and gives **3 stat points**, spent with `STATUP <damage|special_damage|defense|special_defense|max_hp>` (`max_hp` gives +5 per point, the others +1).

**Death.** When the player reaches 0 HP the death is announced on the global chat and logged (`DEATH`), the player respawns in the safe start room `Start_oasis` with **30% of their maximum HP** (minimum 1), and the fight ends.

## Quest System

Quests are declared in `world.yaml` and driven by the server (`server/src/handlers/quest_*.rs` and `npc.rs`).

**Structure.** A quest has an id, a name, a description, a giver NPC, rewards (experience, gold and/or items) and a list of **stages**. Each stage has one objective:

| Objective            | Completed when                                                                                                                            |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `talk_to_npc`        | The player talks to the target NPC.                                                                                                       |
| `fetch_item`         | The player talks to the quest giver while carrying the item; the item is **consumed** from the inventory.                                 |
| `kill_mob` (`count`) | The player kills the target NPC `count` times (a per-player counter is kept).                                                             |
| `answer_dialogue`    | The player answers the NPC with the expected answer (`ANSWER <npc> <answer>`). A wrong answer can show a custom line and reset the quest. |

**Progression and validation.** Talking to the giver starts the quest; every stage is validated server-side (the client never says "I did it"), and the NPC dialogue changes with the stage. When the last stage is validated the quest is completed, moved to `completed_quests`, and the reward is granted, then `EVT QUEST_UPDATE` is sent. `QUESTS` returns the active quests as `id:name:description:objective:stage` entries separated by `|`; progress is saved with the account.

**Rewards.** Experience (which can level the player up), gold, and items. Rewards listing the same item several times give several units.

**Quests shipped.**

1. **The Answer Protocol** (`main_quest`, giver: Queen): bring `urbosas_cimeter` (from the caves) to the Queen, then kill `Final_boss` in the temple. The temple is locked: entering it from the castle needs the `temple_pass` item (`ERR temple_pass_required` otherwise).
2. **Dory's Gift** (`quest_dory`, giver: Dory): answer Dory's question correctly (`yes`). Reward: 50 XP and the `magical_strength_medaillon`.
3. **Board quests** (replayable): the notice boards in `Start_oasis` and `Oasis2` share a pool of 6 kill quests generated by the server (kill 2-10 of a given mob, 10-20 gold per required kill). `BOARD_QUESTS` lists them, `BOARD_TAKE <slot>` locks one to a player, `BOARD_ABANDON` gives it back, and a completed slot is regenerated.

Quest items are flagged `type: quest`: they cannot be dropped, and picking one up is tracked **per player** (each player can take it once), so a quest item does not disappear for the other players.

## World Design

19 rooms in a hub-and-spoke layout around the spawn room `Start_oasis`, with exits `north/south/east/west`. Every exit is reciprocal.

| Room                  | Exits                                                                                      |
| --------------------- | ------------------------------------------------------------------------------------------ |
| `Start_oasis` (spawn) | north `Wild2`, east `Wild4`, south `Wild6`, west `Wild3`                                   |
| `Wild1`               | south `Wild3`, east `Wild2`                                                                |
| `Wild2`               | west `Wild1`, south `Start_oasis`                                                          |
| `Wild3`               | north `Wild1`, east `Start_oasis`                                                          |
| `Wild4`               | north `City`, east `Wild5`, south `Oasis2`, west `Start_oasis`                             |
| `Wild5`               | west `Wild4`                                                                               |
| `Wild6`               | north `Start_oasis`, west `Cave1`                                                          |
| `Oasis2`              | north `Wild4`, east `Wild7`                                                                |
| `Wild7`               | west `Oasis2`                                                                              |
| `City`                | south `Wild4`, east `Castle`                                                               |
| `Castle`              | west `City`, east `Temple1`                                                                |
| `Cave1`               | east `Wild6`, west `Cave2`                                                                 |
| `Cave2`               | east `Cave1`, south `Cave3`                                                                |
| `Cave3`               | north `Cave2`, south `Cave4`                                                               |
| `Cave4`               | north `Cave3`                                                                              |
| `Temple1` – `Temple4` | a corridor: west back towards `Castle`, east towards `Temple4` (`Temple4` is the dead end) |

- **Loop:** `Start_oasis → Wild2 → Wild1 → Wild3 → Start_oasis` (full circuit).
- **Optional branches:** `Wild5` (dead end), `Oasis2 → Wild7` (dead end), `Wild6 → Cave1..4` (caves, quest item), `Wild4 → City → Castle → Temple1..4` (final boss).

| Zone   | Rooms               | Content                                                                                                       |
| ------ | ------------------- | ------------------------------------------------------------------------------------------------------------- |
| Spawn  | `Start_oasis`       | Gragas, Jamato, Fatih; items `Excalibur`, `Dague` on the ground; quest board.                                 |
| Wilds  | `Wild1`–`Wild7`     | `SandMonster` (enemy) in `Wild3`; Guard1/Guard2 in `Wild4`; Zek in `Wild5`; Mage in `Wild6`; Remy in `Wild7`. |
| Oasis  | `Oasis2`            | Anwar, Forgeon (merchant), Axel, Elyesa; quest board.                                                         |
| City   | `City`, `Castle`    | Guards, Dory, Oguzhan, Soufiane and the Queen.                                                                |
| Caves  | `Cave1`–`Cave4`     | `urbosas_cimeter` (quest item) in `Cave4`.                                                                    |
| Temple | `Temple1`–`Temple4` | `Final_boss` (300 HP) in `Temple4`.                                                                           |

**NPC roles:** enemies (`SandMonster`, `Final_boss`, and the mobs listed in `listing_items.md`), quest givers (the Queen and Dory give quests; the other NPCs flagged `Quest_giver` are reserved for future quests), a merchant (Forgeon, `BUY`) and dialogue-only NPCs (guards, villagers). **Items:** weapons, armor, medaillons (strength, magical strength, experience), consumables (healing potions), the temple pass and quest items. The world contains more than the 4 items required and several can be obtained in-world: two weapons lie in the spawn room, a quest weapon is in the caves, medaillons are quest rewards or shop goods, and monsters drop loot. The complete list is in `listing_items.md`.

## Server Logging

The server logs every event as **one JSON object per line**, with a timestamp, a level, the event name, the player and a `details` object. Lines are written to **stdout** and appended to **`logs.json`** (the file is created next to the server, i.e. in the repository root).

```
{"timestamp":"2026-11-20T14:02:11","level":"INFO","event":"CONNECT","player":"alice","details":{"ip":"127.0.0.1:51234","type":"new"}}
{"timestamp":"2026-11-20T14:02:15","level":"INFO","event":"COMMAND","player":"alice","details":{"raw":"ATTACK SandMonster"}}
{"timestamp":"2026-11-20T14:02:15","level":"INFO","event":"RESPONSE","player":"alice","details":{"raw":"ATTACK SandMonster","response":"S: OK ..."}}
```

**Levels.** `INFO` for normal events, `WARN` for `AUTH_FAIL` and `ABUSE_PATTERN`, `ERROR` for `INTERNAL_ERROR` and `DB_ERROR`. Database errors are also printed on stderr (`[DB] ...`).

**Events logged.**

| Event                                       | When                                                                                                        |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `CONNECT` / `DISCONNECT`                    | Login (with IP and address, account type new / returning / guest) and logout (`QUIT` or `connection_lost`). |
| `AUTH_FAIL`                                 | Wrong password.                                                                                             |
| `COMMAND`                                   | **Every** command received, with the player name (or `anonymous` plus the address before login).            |
| `RESPONSE`                                  | **Every** reply sent back, including error codes. Combat, quest and NPC results are visible in these texts.  |
| `TAKE`, `DROP`                              | Item movements between inventory and ground.                                                                |
| `DEATH`                                     | A player killed by a monster (with the killer).                                                             |
| `BOARD_TAKE`, `BOARD_ABANDON`, `BOARD_COMPLETE` | Quest-board progress.                                                                                   |
| `ABUSE_PATTERN`                             | Suspicious behaviour (see below).                                                                           |

The very frequent `POS` commands are not logged, to keep the log readable and the server fast. Writing one line per event is cheap and does not hold the game state lock.

**Abuse detection.** The server logs `ABUSE_PATTERN` (level `WARN`) in two cases: `rapid_connections` when one IP opens 5 or more connections within 10 seconds, and `command_flooding` when a connection sends 25 or more commands within 2 seconds (logged at most every 5 seconds). Spam is also limited by cooldowns (`ERR movement_cooldown_too_fast`, `ERR attack_cooldown`, `ERR chat_spam_forbidden`).

**Monitoring.** Follow the log live with `tail -f logs.json`; filter with `grep '"event":"AUTH_FAIL"' logs.json` or `jq 'select(.event=="DEATH")' logs.json`. Repeated `AUTH_FAIL` or `ABUSE_PATTERN` entries from one IP indicate guessing or flooding.

## Testing

All tests are manual; start the server (`make run-server`) and open clients with `make run-client` (or `nc 127.0.0.1 4243`). Note that `MOVE` has a 500 ms cooldown, so wait between moves when pasting scripts. `make lint` and `make test` check the code quality and the unit tests.

**Multiplayer.** Open two CLI clients.

```
(client 1) CONNECT alice pw1
(client 2) CONNECT bob pw2
(client 1)  <- EVT ROOM Start_oasis PRESENCE ENTER bob ...
(client 1) WHO                    -> OK { "room": ["alice", "bob"], "server": 2 }
(client 1) CHAT GLOBAL hello      -> OK, then both clients get EVT GLOBAL CHAT alice hello
(client 1) MOVE north             -> bob sees PRESENCE LEAVE alice
(client 2) CHAT ROOM hi           -> alice does NOT receive it (different rooms)
```

Test items by having alice `TAKE Dague` and then `LOOK` from bob's client: the item is gone for bob. After `DROP Dague` it is visible and takeable again. Test disconnects by killing one client with Ctrl-C: the other must see `PRESENCE LEAVE` and keep working.

**Groups.** `GROUP CREATE` (alice), `GROUP INVITE bob`, `GROUP ACCEPT` (bob), `CHAT GROUP hi`, `GROUP`, `GROUP LEAVE`.

**Combat.** Go to the sand monster: from `Start_oasis`, `MOVE west` reaches `Wild3`. `LOOK` shows `SandMonster`. Send `ATTACK SandMonster` repeatedly (at least 1 s apart; faster answers `ERR attack_cooldown`). Check the damage in each reply, `STATUS`, the XP gain and the `MOB_DESPAWN` event on kill. Also try `ATTACK_MAGIC`, `ATTACK_HEAVY` (the next turn the enemy hits for free) and `FLEE` (about 3 times out of 4 it succeeds). Let the monster beat you to check the death path: you must reappear in `Start_oasis` with 30% HP. Wait for the monster's `respawn_time` and send another command to see it respawn.

**Quests.**

- *Dory's Gift:* `Start_oasis → east (Wild4) → north (City)`, `TALK Dory`, then `ANSWER Dory yes`. Expect the quest completion message, +50 XP and the medaillon in `INVENTORY`. Replay with a wrong answer to check the reset dialogue.
- *The Answer Protocol:* `TALK Queen` in `City`; from `Start_oasis` go `south, west, west, south, south` to `Cave4` and `TAKE urbosas_cimeter`; walk back and `TALK Queen` again (the item is handed over and stage 2 starts); then go `City → east (Castle) → east` (needs `temple_pass`) to `Temple1` ... `Temple4` and defeat `Final_boss`. Check `QUESTS` after each step.
- *Board:* in `Start_oasis`, `BOARD_QUESTS`, `BOARD_TAKE 1`, kill the requested mobs, then check the reward and the regenerated slot; `BOARD_ABANDON` releases a taken quest.

**Persistence.** Quit, restart the server and `CONNECT` again with the same name and password: level, XP, inventory, room and position are restored; a wrong password answers `ERR bad_credentials`. A guest session (`CONNECT bob` without password) is not saved.

**Logging and abuse.** `tail -f logs.json` while playing: every command and response must appear. Open 5 connections within 10 seconds, or paste 25 commands at once, and look for `ABUSE_PATTERN`.

## Group Contributions

Based on the development history (see [Timeline](#timeline)):

| Member       | Responsibilities                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **avauclai** | Core architecture (async TCP server, command parser), world loading from YAML, navigation and chat, GUI work: login menu, inventory UI, quest UI, health/XP bars, chat box, room transitions, mob placement, `E` interaction, combat system and its QTE design.                                                                                                                                                |
| **mlorenzo** | Domain crate and shared types, inventory and quest delivery, SQLite persistence, sprites/maps/layers/collisions/minimap for the Bevy client, player movement and animation, world design (`world.yaml`), NPC dialogues and characters, multi-stage quest system, inventory UI redesign (silhouette/tabs/medaillon), shop system, stat points, item stat bonuses, combat turn-sequencing and sub-menu overhaul, quest board, logging, README, Makefile. |

By component: **server** and **CLI client** (both members), **GUI client** (both; avauclai for login/inventory/quests/chat/combat basics, mlorenzo for maps/minimap/player/shop/combat sequencing), **world design** (mlorenzo), **database** (mlorenzo).

## Resources

References:

- The Rust Programming Language: [https://doc.rust-lang.org/book/](https://doc.rust-lang.org/book/)
- Tokio tutorial and API (TCP, `select!`, `broadcast`): [https://tokio.rs/tokio/tutorial](https://tokio.rs/tokio/tutorial)
- Bevy engine and book: [https://bevyengine.org/learn/](https://bevyengine.org/learn/) and `bevy_egui`
- `serde`, `serde_yaml`, `serde_json`, `sqlx` (SQLite), `argon2` crate documentation
- RFC 9293 (TCP) and the attached RFC 42TAP
- Background on MUDs (Multi-User Dungeons)

**How AI was used.**
AI has been used to help with research, and to fasten the code, as in righting basic functions, and structs. All concepts have been learnt and understand, AI has been used for the sole purpose of doing repetitive tasks.

## Timeline:
````
09/06/2026:
    avauclai:
        - Core Architecture: Set up an async TCP server using tokio and a custom text-command parser.
        - World Generation: Implemented dynamic loading for rooms, items, and NPCs from a world.yaml file.
10/06/2026
	avauclai
    	- Navigation & Chat: Added room-to-room movement, area scanning (look), and proximity-filtered chat.
11/06/2026:
	mlorenzo:
		- Created the classes for all instances
        - Inventory & Quests: Created item management (take, drop) and an NPC quest delivery system (talk) with EXP/item rewards.
12/06/2026:
	mlorenzo:
		- Added a database in sql persistant throughought the restart and logs of the game/players
		- Adapted the serveur/main.rs and serveur/handlers.rs to match the database
16/06/2026:
	mlorenzo:
		- Organized all the sprites, items, pnj and zones requiered for the game
		- Adapted the classes for the missing variables
27/06/2026:
	mlorenzo:
		- Added a domain crate to hold all of the common classes and identification to specify them throughout the all project
		- Adapted the classes from classes.rs in the real program
		- Changed all of the returns from the program from french to english
06/07/2026:
	mlorenzo:
		- Added a first map of the oasis in the map.rs for the bevy client
07/07/2026:
	mlorenzo:
		- Added the sprite of the skin to the DB
		- Implemented a first version of the player for bevy client
10/07/2026:
	mlorenzo:
		- Added a toggle chat for textual commands on the bevy client
		- Added the ZQSD movement
		- Added animation based on the direction with basic sprites (to be modified)
		- Added dynamic map building from the actual room of the player
11/07/2026:
	mlorenzo:
		- Added the possibility to see other player connected in your room
		- Reorganisation of the files in bevy client
19/07/2026:
	mlorenzo:
		- Improved the world.yaml to make the move between zone possible
04/08/2026:
	mlorenzo:
		- Improved world.yaml
		- Checked up on the sprites and there composition to adapt to the code
05/08/2026:
	avauclai:
		- Added a first version of the inventory UI
		- Added first version of Login Menu
		- Correction BUG affichage position joueur client bevy
06/08/2026:
	avauclai:
		- fix du bug de teleportation
		- brouillon fonctionnel de l'inventaire UI
12/08/2026:
	mlorenzo:
		- Added the move between zones through the bevy client
18/08/2026:
	mlorenzo:
		- Added collision in the maps
		- Position of player showed
	avauclai:
		- Room Transition
		- Mob function placing
		- "E" Interaction Button
19/08/2026:
	avauclai
		- Quest UI
		- Health and XP Bar UI
21/08/2026:
	avauclai
		- Inventory UI V2
		- Close button
		- Weapon icon on inventory
25/08/2026:
	mlorenzo:
		- Edited the world.yaml for the last map zone to implement
		- linked the lasts map except for the cave3 and cave4
26/08/2026:
	mlorenzo:
		- Edited exit from maps to link, and corrected the issues with the temple link 
		- Dealt with the perspective issue
		- Did some layer files
10/09/2026:
	avauclai:
		- First reflection about the combat system and QTE, highlighting the problems encountered and solutions.
18/09/2026:
	mlorenzo:
		- Implemented the minimap system
		- Added x cords to layer files parser
04/10/2026:
	mlorenzo:
		- Added the collisions for all the maps
07/10/2026:
	mlorenzo:
		- Corrected the issue with the parser for the layers
		- Did some layers files
09/10/2026:
	mlorenzo:
		- Change the debug format for a more explicit display
		- Did layers files for all the maps except dungeons
	avauclai:
		- Added the UI chatbox
10/10/2026:
	mlorenzo:
		- Finished the mandatory exit point to not be stuck
		- Finished all of the perspectives
	avauclai:
		- Added the combat system
11/10/2026:
	mlorenzo:
		- write the characters
		- Made some sprites
		- Improved the interactions and talk with pnj
12/10/2026:
	avauclai:
		- Adjusted the position of NPCs in the combat system page
13/10/2026:
	mlorenzo:
		- Finished the implementation of the map with the shadows
		- edited the PNJ a bit
26/10/2026:
	mlorenzo:
		- Implemented the quest system
		- Added the dialogue of pnj depending on the stage of the quest
		- Edited the inventory setup for it to match the items
		- Added the main quest of the queen and the quest of dory to the world.yaml and in game
31/10/2026:
	mlorenzo:
		- Update README
		- Added Makefile
		- Listing items and mobs
		- Made the QUIT command get back to the login screen
01/11/2026:
	mlorenzo:
		- Done the positioning of all PNJ
		- First version of the shop
		- Quests fully done
		- Corrected some bugs in the UI
02/11/2026:
	mlorenzo:
		- Shop improved
03/11/2026:
	mlorenzo:
		- Inventory UI fully redesigned: silhouette panel, category tabs (All/Weapons/Armor/Medaillon/Consumables/Key Items), medaillon slot
		- Medaillon equip/unequip system with persistence in DB
		- Gold display and stat-points counter in inventory stats panel
		- Consumable tab with "Use" action button; healing potion restores % of max HP
		- Stat point allocation system: +3 points per level-up, spendable per stat via "+" buttons
		- Item stat bonuses applied in combat (damage/special_damage from equipped medaillon)
		- Combat charge animation: player and enemy lunge sprites with AtPeak pause and snap-back
		- Magic attack command (`ATTACK_MAGIC`) scaling on special_damage vs special_defense
		- Heavy attack (`ATTACK_HEAVY`, `ATTACK_MAGIC_HEAVY`): 1.75× damage, 1-turn cooldown
		- Shop system: Forgeon NPC, `BUY <npc> <item>` server command, shop window UI with item list and gold cost
04/11/2026:
	mlorenzo:
		- Combat overhauled to Pokemon-style turn sequencing: player lunge → 2 s damage display → enemy lunge → 2 s damage display → input
		- Main menu reduced to 3 options (Attack / Item / Flee); attack sub-menu has 4 types (Physical / Heavy Phys / Magical / Class1 Magic)
		- Button images swap dynamically between menus; no white text overlay on main menu (images carry baked-in labels)
		- Combat end (victory/defeat) now correctly triggers exit transition from within the turn sequence
09/11/2026:
	mlorenzo:
		- Done the items on the ground, and the drop/take both on terminal and bevy
		- Did the quest board entierly 
13/11/2026:
	mlorenzo:
		- Corrected the heavy attacks
		- Finished all of the bugs and robustness section
		- implemented sprite for all PNJ and Player
		- Respawn done
14/11/2026:
	mlorenzo:
		- Sprites for some items linked
		- XP medaillon implemented
		- world.yaml cleared
		- zone descriptions finished
		- rework medaillon strength and magic
15/11/2026:
	mlorenzo:
		- Locked the temple
		- improved world.yaml to add mobs and items
		- Corrected bugs
		- descriptions in the world.yaml
		- reworked the board quest and queen quest
		- requierments added
17/11/2026:
	mlorenzo:
		- added xp for eval/tests
		- corrected a lot of bugs who appeared last commit
		- cleaned the exit of a dialogue with E 
18/11/2026:
	mlorenzo:
		- quest implemented
		- Adjustment size sandmonster
		- Translated all french to english
		- added connect in temporary account without password
		- Cleaned the enter broadcast
		- Synced view of player between client_bevy and client
19/11/2026:
	mlorenzo:
		- Big rework and clean of the server and domain
20/11/2026:
	mlorenzo:
		- Rework database
		- Rework client_bevy
22/11/2026:
	mlorenzo:
		- corrections bevy_client and server
23/11/2026:
	mlorenzo:
		- Update README
````

TODO:
	- finish the world.yaml with all the informations
	- Help Command
	- Make a pannel for each command to be clickable (mandatory)
	- MAJ README
	- pass lint
