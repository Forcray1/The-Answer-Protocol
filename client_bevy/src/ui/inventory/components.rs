use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;

#[derive(Component)]
pub struct InventoryDescriptionText;

#[derive(Component, Clone)]
pub struct InventorySlotInfo {
    pub id: String,
    pub name: String,
    pub damage: u32,
    pub slot: String,
    pub is_equipped: bool,
    pub consumable: bool,
}

#[derive(Default, Clone, PartialEq, Eq)]
pub enum InventoryTab {
    #[default]
    All,
    Weapons,
    Armor,
    Medaillon,
    Consumables,
    Other,
    KeyItems,
}

impl InventoryTab {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            InventoryTab::All => "All",
            InventoryTab::Weapons => "Weapons",
            InventoryTab::Armor => "Armor",
            InventoryTab::Medaillon => "Medaillon",
            InventoryTab::Consumables => "Consumables",
            InventoryTab::Other => "Other",
            InventoryTab::KeyItems => "Key",
        }
    }
    pub(crate) fn matches(&self, slot: &str, consumable: bool) -> bool {
        match self {
            InventoryTab::All => true,
            InventoryTab::Weapons => slot == "weapon",
            InventoryTab::Armor => slot == "chest",
            InventoryTab::Medaillon => slot == "medaillon",
            InventoryTab::Consumables => consumable,
            InventoryTab::Other => {
                !consumable
                    && !matches!(slot, "weapon" | "chest" | "medaillon" | "key")
            }
            InventoryTab::KeyItems => slot == "key",
        }
    }
}

#[derive(Clone, Default)]
pub struct RawInvItem {
    pub id: String,
    pub count: u32,
    pub slot: String,
    pub name: String,
    pub damage: u32,
    pub sprite: String,
    pub is_equipped: bool,
    pub consumable: bool,
}

#[derive(Resource, Default)]
pub struct InventoryState {
    pub open: bool,
    pub active_tab: InventoryTab,
    pub cached_items: Vec<RawInvItem>,
    pub player_gold: i32,
    pub stat_points: i32,
}

#[derive(Component)]
pub(crate) struct InventoryUiRoot;

#[derive(Component)]
pub(crate) struct InventoryGrid;

#[derive(Component)]
pub struct EquipmentPanel;

#[derive(Component)]
pub struct EquipmentSlot(pub String);

#[derive(Component)]
pub struct InventoryActionBtn;

/// The "Drop" button in the inventory item detail panel.
#[derive(Component)]
pub struct InventoryDropBtn;

#[derive(Component)]
pub struct InventoryActionText;

#[derive(Component)]
pub struct StatValueText(pub usize);

/// Marks the gold display text in the inventory.
#[derive(Component)]
pub(crate) struct InventoryGoldText;

/// Marks the "N stat points" text.
#[derive(Component)]
pub(crate) struct StatPointsText;

/// "+ button" for a stat: contains the stat name to send to the server.
#[derive(Component)]
pub(crate) struct StatUpBtn(pub String);

/// Marks a "Use" button on a consumable item slot; contains the item id.
#[derive(Component)]
pub(crate) struct UseItemBtn(pub String);

/// Marks the center silhouette image in the equipment panel.
#[derive(Component)]
pub struct PlayerSilhouette;

/// Marks a tab button.
#[derive(Component)]
pub(crate) struct InventoryTabBtn(pub(crate) InventoryTab);

/// Marks the active-tab highlight bar.
#[derive(Component)]
pub(crate) struct ActiveTabUnderline;

#[derive(Resource, Default)]
pub struct SelectedItem {
    pub id: Option<String>,
    pub name: Option<String>,
    pub is_equipped: bool,
    pub consumable: bool,
    pub slot: String,
}

#[derive(Component)]
pub(crate) struct InventorySlot;
