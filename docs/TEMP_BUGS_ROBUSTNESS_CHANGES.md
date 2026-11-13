# Temporary report — Bugs & robustness pass (2026-11-13)

This temporary file lists the changes applied for the **"Bugs and robustness"** section and the implementation methods used.

## 1) Cancel-safe command reading

### What changed
- Reworked the server connection loop in `serveur/src/main.rs`.
- Replaced `read_line` inside `tokio::select!` with:
  - a dedicated reader task (`BufReader::read_line` loop),
  - forwarding complete lines through `tokio::sync::mpsc::unbounded_channel`.
- Main loop now selects on:
  - incoming command lines from channel,
  - broadcast events.

### Method used
- Producer/consumer split to avoid cancellation hazards with `read_line` in `select!`.

---

## 2) Avoid awaiting while holding global state lock

### What changed
- `CONNECT` path was split:
  - new `handlers::process_connect(...)` handles DB/auth work **outside** lock,
  - state mutation happens in a short lock section.
- `handlers::process_command(...)` is now synchronous for non-CONNECT commands.
- Disconnect/QUIT/shutdown save paths now:
  - remove/copy player data under lock,
  - drop lock,
  - then await DB save.

### Method used
- Two-phase command handling: async I/O first, locked state mutation second.

---

## 3) Timer-based mob respawn

### What changed
- Added a periodic respawn task in `serveur/src/main.rs` using `tokio::time::interval`.
- Respawn checks no longer depend on user command traffic.
- Emitted respawn events (`MOB_SPAWN` / `BOARD_SPAWN`) from that timer task.

### Method used
- Background scheduler task + batched event emission.

---

## 4) DB default room mismatch fix

### What changed
- Added migration `database/migrations/0007_fix_default_room.sql`:
  - updates existing rows from `Oasis1` to `Start_oasis`.
- Added migration `database/migrations/0008_add_stat_points.sql`:
  - adds the missing `stat_points` column for existing databases.
- Updated `database/src/lib.rs` registration insert to explicitly set `current_room = 'Start_oasis'`.
- Added safe fallback in connect restore if a saved room does not exist in world data.

### Method used
- Forward schema correction + compatibility migration + runtime guard.

---

## 5) GUI input key consistency (WASD / ZQSD)

### What changed
- Updated movement input detection in `client_bevy/src/player.rs`:
  - North = `W` or `Z`,
  - West = `A` or `Q`,
  - South = `S`,
  - East = `D`.
- Updated README gameplay description to document both layouts.

### Method used
- Layout-agnostic key matching helpers (`is_pressed`, `is_just_pressed`).

---

## 6) Removed unwrap/expect hotspots in server + GUI

### What changed
- Replaced server-side `unwrap()` calls in group/quest/dialogue/connect flows with explicit `Option` handling and protocol errors.
- Replaced `expect("world.yaml error")` with explicit startup error handling in `serveur/src/main.rs`.
- Replaced `tokio::runtime::Runtime::new().unwrap()` in `client_bevy/src/net.rs` with graceful error path.
- Verified no remaining `unwrap()/expect()` in:
  - `serveur/src/**`
  - `client_bevy/src/**`

### Method used
- Early-return error handling (`let Some(...) = ... else { ... }`) and safe startup fallbacks.

---

## 7) Formatting & linting

### What changed
- Ran `cargo fmt --all`.
- Ran `cargo clippy --workspace --all-targets -- -D warnings`.
- Applied code updates so clippy passes with current workspace lint policy.

### Method used
- Iterative lint-fix loop: run → inspect output → patch → rerun until clean.

---

## Files touched (high level)

- `serveur/src/main.rs`
- `serveur/src/handlers.rs`
- `serveur/src/world.rs`
- `serveur/src/state.rs`
- `database/src/lib.rs`
- `database/migrations/0007_fix_default_room.sql`
- `database/migrations/0008_add_stat_points.sql`
- `client_bevy/src/net.rs`
- `client_bevy/src/player.rs`
- `client_bevy/src/main.rs`
- `domain/src/lib.rs`
- `domain/src/quest.rs`
- `README.md`
