use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct QuestState {
    pub open: bool,
}

#[derive(Resource, Default)]
pub(crate) struct SelectedQuest {
    pub(crate) id: Option<String>,
}

#[derive(Component)]
pub(crate) struct QuestUiRoot;

#[derive(Component)]
pub(crate) struct QuestListPanel;

#[derive(Component)]
pub(crate) struct QuestEmptyText;

#[derive(Component)]
pub(crate) struct QuestDetailTitle;

#[derive(Component)]
pub(crate) struct QuestDetailDescription;

#[derive(Component)]
pub(crate) struct QuestDetailObjective;

#[derive(Component)]
pub(crate) struct QuestEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) objective: String,
}

#[derive(Component)]
pub(crate) struct QuestEntrySlot;

// Quest Journal theme colours
pub(crate) const QUEST_BG: Color = Color::rgba(0.08, 0.07, 0.06, 0.96);

pub(crate) const QUEST_BORDER: Color = Color::rgba(0.55, 0.42, 0.18, 1.0);

pub(crate) const QUEST_TITLE_COLOR: Color = Color::rgba(0.90, 0.75, 0.35, 1.0);

pub(crate) const QUEST_ENTRY_BG: Color = Color::rgba(0.30, 0.24, 0.10, 0.92);

pub(crate) const QUEST_ENTRY_BORDER: Color = Color::rgba(0.50, 0.40, 0.15, 0.8);

pub(crate) const QUEST_ENTRY_TEXT: Color = Color::rgba(0.88, 0.76, 0.42, 1.0);

pub(crate) const QUEST_PARCHMENT: Color = Color::rgba(0.82, 0.75, 0.60, 0.92);

pub(crate) const QUEST_DETAIL_TITLE: Color = Color::rgba(0.15, 0.12, 0.08, 1.0);

pub(crate) const QUEST_DETAIL_TEXT: Color = Color::rgba(0.25, 0.22, 0.18, 1.0);

pub(crate) const QUEST_ENTRY_HOVER: Color = Color::rgba(0.40, 0.33, 0.14, 0.95);

pub(crate) const QUEST_ENTRY_SELECTED: Color = Color::rgba(0.48, 0.38, 0.16, 1.0);
