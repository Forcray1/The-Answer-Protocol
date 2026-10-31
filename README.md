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

TAP is a small multiplayer text adventure in the spirit of the old MUDs. Several players connect to one server, explore a shared map, chat on three channels (global, room, group), pick up and drop items, talk to NPCs, complete quests and fight monsters.

The project delivers:

| Component | Crate | Description |
| --- | --- | --- |
| Server | `serveur` | Async TCP server (Tokio). Holds the live world state in memory and speaks the TAP protocol. |
| CLI client | `client` | Minimal terminal client. Shows server messages in real time while you type raw protocol commands. |
| GUI client | `client_bevy` | Graphical client built with Bevy and egui: 2D rooms, sprites, inventory, quest log, chat box, combat screen, minimap. |
| Domain | `domain` | Types shared by the whole workspace (ids, items, equipment, stats, quest stages, groups). |
| Database | `database` | SQLite persistence (accounts with Argon2 password hashes, progress, inventory, position). |
| World data | `world.yaml` | Static world: rooms, exits, NPCs, items and quests. |

All code is Rust, as required (C, C++, Rust, Go or Zig only).

## Instructions

Prerequisites:

- Rust **1.85 or newer** (`client_bevy` uses edition 2024), installed with [rustup](https://rustup.rs).
- GNU Make.
- On Linux, system libraries for Bevy (audio, udev, windowing). On Debian/Ubuntu run `make install-deps-linux`.

Quick start (three terminals, all from the repository root):

```bash
make install        # fetch dependencies
make run-server     # terminal 1
make run-client     # terminal 2 (CLI)
make run-client-gui # terminal 3 (GUI)
```

The server must be started **from the repository root** because it reads `./world.yaml` and creates `./game.db` and `./logs.json` in the current directory.

## Building and Running

The build tool is **Cargo** (workspace), wrapped by a **Makefile** that provides the standard targets.

| Target | What it does |
| --- | --- |
| `make install` | Check the toolchain and fetch all crates (`cargo fetch`). |
| `make install-deps-linux` | Debian/Ubuntu only: install the system libraries Bevy needs. |
| `make build` (or `make all`) | Build the whole workspace (server, CLI, GUI). |
| `make build-server` / `build-client` / `build-gui` | Build one component. |
| `make run-server` | Start the server on `127.0.0.1:4243` with JSON logging enabled. |
| `make run-client` | Start the CLI client. |
| `make run-client-gui` | Start the Bevy GUI client. |
| `make lint` | `cargo fmt --check` + `cargo clippy -D warnings`. Run `make fmt` first if formatting fails. |
| `make fmt` | Format the code with rustfmt. |
| `make test` | Run the unit tests. |
| `make clean` | Remove `target/`. |
| `make fclean` | `clean` + delete `game.db` and `logs.json` (**all accounts are lost**). |
| `make re` | `fclean` then `all`. |
| `make help` | List all targets. |

Variables:

- `PROFILE=debug` builds without `--release` (faster to compile, but the GUI runs slowly). Default: `release`.
- `LOGS=0` starts the server without the `--logs` flag. Default: `1`.

Examples:

```bash
make run-server LOGS=0
make run-client-gui PROFILE=debug
```

Both clients connect to `127.0.0.1:4243` (hard-coded constant, see [Developer notes](#developer-notes-audit-and-known-gaps)).

### Connecting

On connection the server sends a greeting, then you identify yourself:

```text
S: OK hello proto=1
C: CONNECT <username> <password>
S: OK connected skin=default name=<username> room=Start_oasis exits=north,east,south,west
```

If the username does not exist the account is created with the given password; otherwise the password is verified. Until `CONNECT` succeeds, game commands answer `ERR utilize_connect_first`.

### CLI client

The CLI client implements the first approach allowed by the subject: **what you type is sent unchanged to the server** (RFC syntax), and everything the server sends is printed immediately, including asynchronous events, because reading the socket and reading stdin run concurrently (`tokio::select!`). `QUIT` ends the program cleanly.

### GUI client

The GUI client logs in through a login menu, shows the current room as a 2D scene (map layers, collisions, other players, NPCs and mobs), and sends protocol commands on your behalf: movement between rooms, `POS` updates, `INTERACT` on NPCs (E key), `ATTACK`/`FLEE` in the combat screen, inventory and equipment actions, quest log (`QUESTS`) and a chat box with Global / Room / Group channels. A text console is also available for raw commands. A minimap shows the explored rooms.

## Architecture

**Concurrency model.** One Tokio task per TCP connection, all sharing:

- `Arc<WorldData>`: immutable world loaded from `world.yaml` at startup.
- `Arc<Mutex<ServerState>>`: all live state (players by socket address, items on the ground, NPC hit points, dead NPCs waiting for respawn, active combats, groups).
- `Arc<SqlitePool>`: persistence.
- A `tokio::sync::broadcast` channel of `GlobalEvent`s.

**Request path.** Each connection task `select!`s between (a) a line arriving from its socket and (b) an event arriving on the broadcast channel.

1. A line is parsed by `GameCommand::parse` (`serveur/src/commands.rs`) into a typed enum.
2. `handlers::process_command` (`serveur/src/handlers.rs`) is a **dispatcher/router**: a single `match` on the enum calls one `handle_*` function per command. Handlers mutate the state and return the reply text; the connection task writes it to the socket.
3. Handlers that must notify other players push a `GlobalEvent` on the broadcast channel.

**Event routing.** A `GlobalEvent` carries a message plus an optional target: one player (`target_player`), a room (`target_room`), a group (`target_group`) or everybody. Every connection task receives every event and decides whether to write it, by checking its own player's room/group. An event is never echoed to the player in `sender_addr` (for chat, `sender_addr` is `None` so the sender also sees their own message).

**Disconnects.** On `QUIT`, EOF or socket error the player is removed from the state first, saved to the database, and only then are the `PRESENCE LEAVE` and global chat events broadcast, so nobody can interact with a player who is already gone. A failed write to one client never stops delivery to the others.

**Anti-spam.** Per-player cooldowns: 500 ms between `MOVE`, 1 s between `ATTACK`, 2 s between `CHAT`.

**Crate layout.**

```text
Cargo.toml            workspace
serveur/src/main.rs   listener, connection loop, logging
serveur/src/commands.rs  command parser
serveur/src/handlers.rs  command handlers (dispatcher)
serveur/src/state.rs     live game state
serveur/src/world.rs     world.yaml data model
domain/               shared types (ids, items, equipment, quests, groups)
database/             SQLite access + migrations
client/               CLI client
client_bevy/          GUI client (Bevy + egui)
world.yaml            world data
```

## Protocol Implementation

Framing: TCP, UTF-8, one message per line terminated by `\n`. Server lines start with `S: `; replies begin with `OK` or `ERR`; asynchronous pushes begin with `EVT`.

### Commands implemented

| Command | Notes |
| --- | --- |
| `CONNECT <name> <password>` | Creates the account on first use. |
| `LOOK` | Room name, description, items on the ground, NPCs present. |
| `MOVE <north\|south\|east\|west>` | Replies `OK room-loc.<room> exits=...`. |
| `TAKE <id or name>` / `DROP <id or name>` | Items are matched on id or display name, case-insensitively; multi-word names work. |
| `INVENTORY` | Replies with a readable list plus `EVT INV_DATA`, `EVT EQUIP_DATA` and `EVT CHAR_STATS`. |
| `TALK <npc>` | Dialogue; also advances quests. |
| `ATTACK <enemy>` | See [Combat System](#combat-system). |
| `STATUS` | HP, level, XP, gold, location. |
| `QUESTS` | Replies with an `EVT QUEST_DATA` line. |
| `WHO` | `OK { "room": [...], "server": N }`. |
| `CHAT <GLOBAL\|ROOM\|GROUP> <text>` | Global, room and group channels. |
| `GROUP`, `GROUP CREATE`, `GROUP INVITE <player>`, `GROUP ACCEPT`, `GROUP LEAVE` | Group management. |
| `QUIT` | `OK goodbye`, then the connection is closed. |

### Events pushed by the server

`EVT GLOBAL CHAT`, `EVT ROOM <room> CHAT`, `EVT GROUP CHAT`, `EVT ROOM <room> PRESENCE ENTER|LEAVE|ROSTER`, `EVT ROOM <room> MOB_SPAWN|MOB_DESPAWN`, `EVT ROOM <room> POS`, `EVT COMBAT START|HIT|END`, `EVT PLAYER_STATS`, `EVT CHAR_STATS`, `EVT INV_DATA`, `EVT EQUIP_DATA`, `EVT QUEST_DATA`, `EVT QUEST_UPDATE`, `EVT QUEST_RESET`, `EVT DIALOGUE`, `EVT DIALOGUE_CHOICE`.

### Deviations and extensions (to be justified and kept up to date)

| Area | Behaviour here | Reason |
| --- | --- | --- |
| `CONNECT` | Requires a password (second argument). | The server has persistent accounts; the GUI has a login form. |
| Reply payloads | Several replies are human-readable text (`LOOK`, `INVENTORY`, `STATUS`, `TAKE`, `DROP`) instead of the structured examples of the subject. | Written for readability in the CLI first. |
| Presence events | `EVT ROOM <room> PRESENCE ENTER <name> <skin> <x> <y>` carries the room id, a skin and a position. | The GUI needs to place players in the room. |
| Extra commands | `POS <x> <y>`, `INTERACT <npc>`, `ANSWER <npc> <answer>`, `EQUIP`, `UNEQUIP`, `EQUIPMENT`, `INFO`, `FLEE`. | Needed by the GUI (position sync, NPC interaction, choice dialogues, equipment). |
| Extra events | `MOB_SPAWN`, `MOB_DESPAWN`, `POS`, `PLAYER_STATS`, `INV_DATA`, `EQUIP_DATA`, `CHAR_STATS`, `QUEST_DATA`, `QUEST_UPDATE`, `QUEST_RESET`, `DIALOGUE`, `DIALOGUE_CHOICE`, `COMBAT *`. | Same reason. A client that only knows the base RFC can ignore them. |
| `QUEST <id>` | Parsed but not yet implemented (answers `OK command received but not implemented yet`). | See the developer notes. |
| Errors | Error tokens are snake_case codes for protocol errors (`malformed_command`, `utilize_connect_first`, `unknown_channel`, ...) and English sentences for gameplay errors. | Historical; to be unified. |

## Combat System

Combat is resolved **server-side** when a player sends `ATTACK <enemy>` (or `INTERACT` on an enemy, which opens the fight). Only NPCs with `role: enemy` can be attacked, and only in the player's current room.

**Round structure.** Each `ATTACK` is one exchange: the player strikes first, and if the enemy survives it counter-attacks immediately.

**Damage formulas.**

```text
player_damage   = 10 + damage of the equipped weapon
damage_to_enemy = max(1, player_damage - enemy.defense)
enemy_damage    = enemy.damage (default 5)
damage_to_player = max(1, enemy_damage - total_defense of equipped armor)
```

- Players start with **100 HP** and enemies have their own `hp`, `damage`, `defense` and `exp_reward` in `world.yaml`.
- `STATUS` shows current HP, level, XP, gold and location.
- When the enemy reaches 0 HP: the player gains `exp_reward` XP (level-ups follow `4*level^2 + 56` XP per level), drops are rolled by percentage, kill-quest counters advance, `EVT ROOM <room> MOB_DESPAWN` is sent and the mob respawns after its `respawn_time` (default 30 s).
- When the player reaches 0 HP: the death is announced on the global chat and logged, the player is moved back to `Start_oasis`, and HP is restored.

**Combat commands.** `ATTACK <enemy>`, `FLEE` (aliases `ESCAPE`, `FUIR`) which ends the fight immediately, and `STATUS`. The server defines combat phases for a quick-time-event (QTE) mechanic that the GUI combat screen drives; the server does not yet enforce QTE timing.

**Limits (see developer notes):** no initiative order, no `DEFEND`, `FLEE` has no failure chance, and respawn restores full HP.

## Quest System

Quests are declared in `world.yaml` and driven by the server.

**Structure.** A quest has an id, a name, a description, a giver NPC, a `reward_exp`, an optional `reward_item`, and a list of **stages**. Each stage has one objective:

| Objective | Completed when |
| --- | --- |
| `talk_to_npc` | The player talks to the target NPC. |
| `fetch_item` | The player talks to the quest giver while carrying the item; the item is **consumed** from the inventory. |
| `kill_mob` (`count`) | The player kills the target NPC `count` times (a per-player counter is kept). |
| `answer_dialogue` | The player answers the NPC with the expected answer (`ANSWER <npc> <answer>`). A wrong answer can show a custom line and reset the quest. |

**Progression.** Talking to the giver starts the quest; every stage is validated server-side; the NPC dialogue changes with the stage. When the last stage is validated the quest is completed, moved to `completed_quests`, and the reward is granted (XP and/or an item), then `EVT QUEST_UPDATE` is sent. `QUESTS` returns the active quests as `id:name:description:objective:stage` entries separated by `|`.

**Quests shipped.**

1. **The Answer Protocol** (`main_quest`, giver: Queen): bring `urbosas_cimeter` (from the caves) to the Queen, then kill `Final_boss` in the temple. Reward: 1000 XP.
2. **Dory's Gift** (`quest_dory`, giver: Dory): answer Dory's question correctly (`yes`). Reward: 50 XP and the `magical_strength_medaillon`.

Quest items are flagged `type: quest`: they cannot be dropped, and picking one up is tracked **per player** (each player can take it once), so a quest item does not disappear for the other players.

## World Design

19 rooms in a hub-and-spoke layout around the spawn room `Start_oasis`, with exits `north/south/east/west`. Every exit is reciprocal.

| Room | Exits |
| --- | --- |
| `Start_oasis` (spawn) | north `Wild2`, east `Wild4`, south `Wild6`, west `Wild3` |
| `Wild1` | south `Wild3`, east `Wild2` |
| `Wild2` | west `Wild1`, south `Start_oasis` |
| `Wild3` | north `Wild1`, east `Start_oasis` |
| `Wild4` | north `City`, east `Wild5`, south `Oasis2`, west `Start_oasis` |
| `Wild5` | west `Wild4` |
| `Wild6` | north `Start_oasis`, west `Cave1` |
| `Oasis2` | north `Wild4`, east `Wild7` |
| `Wild7` | west `Oasis2` |
| `City` | south `Wild4`, east `Castle` |
| `Castle` | west `City`, east `Temple1` |
| `Cave1` | east `Wild6`, west `Cave2` |
| `Cave2` | east `Cave1`, south `Cave3` |
| `Cave3` | north `Cave2`, south `Cave4` |
| `Cave4` | north `Cave3` |
| `Temple1` – `Temple4` | a corridor: west back towards `Castle`, east towards `Temple4` (`Temple4` is the dead end) |

- **Loop:** `Start_oasis → Wild2 → Wild1 → Wild3 → Start_oasis` (full circuit).
- **Optional branches:** `Wild5` (dead end), `Oasis2 → Wild7` (dead end), `Wild6 → Cave1..4` (caves, quest item), `Wild4 → City → Castle → Temple1..4` (final boss).

| Zone | Rooms | Content |
| --- | --- | --- |
| Spawn | `Start_oasis` | Gragas, Jamato, Fatih; items `Excalibur`, `Dague` on the ground. |
| Wilds | `Wild1`–`Wild7` | `SandMonster` (enemy) in `Wild3`; Guard1/Guard2 in `Wild4`; Zek in `Wild5`; Mage in `Wild6`; Remy in `Wild7`. |
| Oasis | `Oasis2` | Anwar, Forgeon, Axel, Elyesa. |
| City | `City`, `Castle` | Guards, Dory, Oguzhan, Soufiane and the Queen. |
| Caves | `Cave1`–`Cave4` | `urbosas_cimeter` (quest item) in `Cave4`. |
| Temple | `Temple1`–`Temple4` | `Final_boss` (300 HP) in `Temple4`. |

**NPC roles:** enemies (`SandMonster`, `Final_boss`), NPCs flagged `Quest_giver` (Queen, Dory, Gragas, Soufiane, Jamato, Zek, Remy; only the Queen and Dory currently give a quest) and dialogue-only NPCs (guards, villagers). **Items:** 4 (two weapons lying in the spawn room, one quest weapon in the caves, one quest-reward medallion).

## Server Logging

Logging is enabled with the `--logs` flag (`make run-server` passes it by default). Each event is one **JSON object per line**, written to **stdout** and appended to **`logs.json`**:

```json
{"timestamp":"2026-10-31T14:02:11","event":"CONNECT","player":"alice","details":{"ip":"127.0.0.1:51234","type":"new"}}
```

Events currently logged: `CONNECT` (with IP and address), `DISCONNECT` (`QUIT` or `connection_lost`), `AUTH_FAIL`, `TAKE`, `DROP`, `DEATH`. Database errors go to stderr (`[DB] ...`).

**Monitoring.** Follow the log live with `tail -f logs.json`; filter with `grep '"event":"AUTH_FAIL"' logs.json`, or `jq 'select(.event=="DEATH")' logs.json`. Repeated `AUTH_FAIL` from one IP indicates password guessing. Command flooding is limited by the cooldowns described in [Architecture](#architecture), which answer `ERR movement_cooldown_too_fast`, `ERR attack_cooldown` and `ERR chat_spam_forbidden`.

The full logging requirements of the subject (log levels, every command and response, quest and NPC events, abuse detection) are tracked in the developer notes below.

## Testing

All tests are manual; start the server (`make run-server`) and open clients with `make run-client` (or `nc 127.0.0.1 4243`). Note that `MOVE` has a 500 ms cooldown, so wait between moves when pasting scripts.

**Multiplayer.** Open two CLI clients.

```text
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

**Combat.** Go to the sand monster: from `Start_oasis`, `MOVE west` reaches `Wild3`. `LOOK` shows `SandMonster`. Send `ATTACK SandMonster` repeatedly (at least 1 s apart). Check the damage in each reply, `STATUS`, the XP gain and the `MOB_DESPAWN` event on kill. Let the monster beat you to check the death/respawn path. Wait for `respawn_time` seconds and send another command to see the monster respawn.

**Quests.**

- *Dory's Gift:* `Start_oasis → east (Wild4) → north (City)`, `TALK Dory`, then `ANSWER Dory yes`. Expect the quest completion message, +50 XP and the medallion in `INVENTORY`. Replay with a wrong answer to check the reset dialogue.
- *The Answer Protocol:* `TALK Queen` in `City`; from `Start_oasis` go `south, west, west, south, south` to `Cave4` and `TAKE urbosas_cimeter`; walk back and `TALK Queen` again (the item is handed over and stage 2 starts); then go `City → east (Castle) → east` to `Temple1` ... `Temple4` and defeat `Final_boss`. Check `QUESTS` after each step.

**Persistence.** Quit, restart the server and `CONNECT` again with the same name and password: level, XP, inventory, room and position are restored; a wrong password answers `ERR bad_credentials`.

## Group Contributions

Based on the development history (see [Timeline](#timeline)):

| Member | Responsibilities |
| --- | --- |
| **avauclai** | Core architecture (async TCP server, command parser), world loading from YAML, navigation and chat, GUI work: login menu, inventory UI, quest UI, health/XP bars, chat box, room transitions, mob placement, `E` interaction, combat system and its QTE design. |
| **mlorenzo** | Domain crate and shared types, inventory and quest delivery, SQLite persistence, sprites/maps/layers/collisions/minimap for the Bevy client, player movement and animation, world design (`world.yaml`), NPC dialogues and characters, multi-stage quest system. |

## Resources

References:

- The Rust Programming Language: <https://doc.rust-lang.org/book/>
- Tokio tutorial and API (TCP, `select!`, `broadcast`): <https://tokio.rs/tokio/tutorial>
- Bevy engine and book: <https://bevyengine.org/learn/> and `bevy_egui`
- `serde`, `serde_yaml`, `serde_json`, `sqlx` (SQLite), `argon2` crate documentation
- RFC 9293 (TCP) and the attached RFC 42TAP
- Background on MUDs (Multi-User Dungeons)

**How AI was used.**


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
````

TODO:
	- Add the collisions [DONE] and interactions
	- finish the world.yaml with all the informations
	- Make all of the combat system
	- Logs not working for bevy client (ah ouais ?)
	- Upgrade the UI (chatbox disapear after 5s), Create Group UI and main menu UI with leave option.
	- Fix bug respawn mob system, they respawn only if the players moves
	- Help Command
	- Stats Point system

- Combat system
- Inventory
- PNJ (quest giver, dialogue, item giver)
- Quests and boardquest
- Statistics windows
- Items

## Developer notes: audit and known gaps

Internal checklist written after reviewing the code against the subject (v1.4). Tick items as they are fixed, and update the [Protocol Implementation](#protocol-implementation) table when behaviour changes. Items marked *(verify)* could not be confirmed by reading the code alone. Nothing was compiled or run during the audit, so run `make lint` and `make test` first.

### Blocking: likely evaluation failures

- [ ] **Hello line typo.** `serveur/src/main.rs` sends `S: DK hello proto=1`; it must be `S: OK hello proto=1`.
- [ ] **`CONNECT <name>` without password** answers `ERR password_required`, so a client following the RFC cannot log in (clients must be interchangeable between groups). Make the password optional or document the deviation prominently.
- [ ] **Reply formats vs the subject.** Subject examples: `OK room=loc.bakery`, `OK taken=item.herbs`, `OK dropped=item.herbs`, JSON for `LOOK`/`INVENTORY`/`WHO`, `EVT ROOM PRESENCE LEAVE alice`. Code sends `OK room-loc.X`, prose for `TAKE`/`DROP`/`LOOK`/`INVENTORY`, and `EVT ROOM <room> PRESENCE ...`. Align with RFC 42TAP, or list each difference in "Protocol Implementation".
- [ ] **`QUEST <id>`** is parsed but falls into the catch-all arm (`not implemented yet`). Implement it.
- [ ] **`QUESTS`** only replies with an `EVT QUEST_DATA` line and no `OK`/`ERR`; add a proper reply.
- [ ] **World validation is missing.** `WorldData::load_from_file` only deserializes. Validate at startup: every exit target exists, every NPC/item id referenced by a room, quest, drop or reward exists, the start room exists, and fail with a clear message. Known bad reference: `Chungus` in `City` (no such NPC).
- [ ] **Combat requirements.** Subject asks for turn-based combat with initiative order, `DEFEND`/`FLEE` style commands, respawn at a safe place with **reduced** health, and combat results **broadcast** to relevant players. Currently: one exchange per `ATTACK`, no initiative, no `DEFEND`, `FLEE` always succeeds, death restores full HP, hits are not broadcast to the room. On death the player is moved to `Start_oasis` without `PRESENCE LEAVE/ENTER` events, so other clients keep a ghost.
- [ ] **Logging requirements.** Needed: always on (not only with `--logs`), levels INFO/WARN/ERROR, every command received (with player and parameters), every response and error code, NPC interactions, quest progress/completion, abuse patterns (command flooding, rapid connections) and precise timestamps. Currently only `CONNECT`, `DISCONNECT`, `AUTH_FAIL`, `TAKE`, `DROP`, `DEATH` are logged, `logs.json` is re-opened on every call, and cooldown rejections are not logged.
- [ ] **TAKE/DROP do not notify the room**, so other players' views go stale. Subject: room view must update after TAKE/DROP. Emit an event to the room.
- [ ] **GUI required controls.** Subject lists buttons for LOOK, MOVE, TAKE, DROP, TALK, ATTACK, STATUS, QUEST, QUESTS, WHO, GROUP, QUIT, plus counters for players in the room and on the server. No `LOOK`, `TAKE`, `TALK`, `STATUS`, `WHO` or `QUIT` is sent by a button *(verify in the UI)*, and no counters were found. Picking up ground items seems possible only through the text console *(verify)*.
- [ ] **Quest variety.** Both shipped quests are `talk_to_npc`-based at the top level (stages add fetch/kill/answer). Fine for the count, but make sure the README demonstrates a fetch and a defeat quest.

### Bugs and robustness

- [ ] **`read_line` inside `tokio::select!` is not cancel-safe** and `line.clear()` runs at the top of each loop iteration: a partially received command is dropped whenever a broadcast event wins the race. Use a reader task that forwards complete lines through a channel, or keep the buffer across iterations.
- [ ] **Global mutex held across slow work.** `main.rs` locks the state and then awaits `process_command`, which includes SQLite calls and Argon2 hashing/verification in `handle_connect`. One login blocks every player and the Tokio worker. Do the DB and hashing outside the lock, and use `tokio::task::spawn_blocking` for Argon2.
- [ ] **Broadcast capacity is 32** and `POS` events are frequent: slow receivers will lag and lose events silently. Increase it and handle `RecvError::Lagged`.
- [ ] **Hard-coded address** `127.0.0.1:4243` in server, CLI and GUI. Add CLI args or env vars (and bind `0.0.0.0` if peers on other machines must connect).
- [ ] **Mob respawn only runs when some command is processed** (`update_respawns` is called in the command path). Use a timer task (already in the TODO list).
- [ ] **DB default room mismatch.** Migration `0001` defaults `current_room` to `'Oasis1'`, which does not exist; the spawn room is `Start_oasis`. A player saved with the default would load into a missing room (`room_error`).
- [ ] **Plain-text passwords on the wire.** Acceptable for a school project, but state it as a known limitation.
- [ ] **Leftover French.** `handle_quests` still emits French objective text (`Parler a`, `Recuperer`, `Livrer`...), while the changelog says everything was translated. Code comments in `state.rs` and other files are in French too.
- [ ] **`QUEST` parsing** keeps only `parts[1]`; `CHAT <channel>` with an empty message returns `malformed_command`; `MOVE` accepts only full direction names (`north`, not `n`). Check these against the RFC.
- [ ] **GUI input keys.** Code uses WASD/ZQSD detection in `player.rs`; keep the controls documented consistently.
- [ ] **`unwrap()`/`expect()`** (about 15 in server and GUI), including `world.yaml error` on load. Replace with proper error messages.
- [ ] **Clippy and rustfmt** were not run. Expect `make lint` to fail until `make fmt` has been run once and clippy warnings are fixed.

### World and balance

- [ ] `Excalibur` (damage 999) lies in the spawn room, which trivialises every fight including `Final_boss` (300 HP). Move it, nerf it, or gate it behind a quest.
- [ ] Many NPCs have `role: "None"` (the string). Use real roles (`dialogue`, `merchant`, `quest_giver`, `enemy`) so the "3 distinct NPC roles" requirement is clearly met, and remove the `Quest_giver` role from NPCs that give no quest, or give them one.
- [ ] Only 2 enemies exist (`SandMonster` with 1 damage, `Final_boss`). Add a few mobs on the main paths.
- [ ] Only 4 items; add consumables (the subject example has an `ale` that restores energy) and a command to use them.

### Repository hygiene

- [ ] Commit `Cargo.lock` (binaries, and the Makefile uses `cargo fetch --locked` when it exists).
- [ ] Add a `.gitignore` for `target/`, `game.db*`, `logs.json`.
- [ ] Make sure the sprite/asset folder is committed where the GUI expects it (Bevy loads `assets/` relative to the crate, i.e. `client_bevy/assets`; `make run-client-gui` prints a warning when the folder is missing).
- [ ] Both members need commits and meaningful commit messages (subject VII.2); keep the Group Contributions table accurate.
- [ ] Fill in the "How AI was used" paragraph in Resources, and make sure each of you can explain the `select!` loop, the broadcast routing in `main.rs` and the quest-stage code during the evaluation (a small live modification may be requested).
- [ ] Remove or keep this section as you prefer once everything is ticked; the Timeline below is the original development log and TODO list, kept unchanged.