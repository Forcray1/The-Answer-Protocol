//! Persistence layer of the game (SQLite).
//!
//! - `init_db`         : creates game.db and tables if they don't exist
//! - `register_player` : creates a player account
//! - `verify_player`   : checks the password on login
//! - `load_player`     : loads persisted player data
//! - `save_player`     : saves player data
//!
//! Persistence uses the strong types of the `domain` crate (Inventory, ItemId, QuestId,
//! RoomId). JSON columns store: the full inventory (3 bags) as one object, the equipment as
//! a slot -> id map, and the completed quests as an id list.

mod auth;
mod init;
mod model;
mod player;

pub use auth::{register_player, verify_player};
pub use init::init_db;
pub use model::PlayerData;
pub use player::{load_player, save_player};
