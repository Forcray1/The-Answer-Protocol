use bevy::prelude::*;
use crate::AppState;
use crate::combat::CombatResource;
use crate::net::{NetworkSender, ServerMessageEvent};
use crate::player::LocalPlayerName;

pub(crate) const BG_DARK: Color = Color::rgba(0.12, 0.09, 0.07, 0.97);

pub(crate) const BG_PANEL: Color = Color::rgba(0.18, 0.14, 0.10, 0.95);

pub(crate) const BORDER_GOLD: Color = Color::rgba(0.72, 0.58, 0.30, 1.0);

pub(crate) const BORDER_INNER: Color = Color::rgba(0.40, 0.32, 0.20, 0.8);

pub(crate) const SLOT_BG: Color = Color::rgba(0.22, 0.18, 0.14, 1.0);

pub(crate) const SLOT_BORDER: Color = Color::rgba(0.50, 0.42, 0.28, 0.7);

pub(crate) const TITLE_COLOR: Color = Color::rgba(0.90, 0.80, 0.55, 1.0);

pub(crate) const TEXT_LIGHT: Color = Color::rgba(0.85, 0.82, 0.75, 1.0);

pub(crate) const TEXT_DIM: Color = Color::rgba(0.55, 0.50, 0.42, 1.0);

pub(crate) const STAT_GREEN: Color = Color::rgba(0.40, 0.80, 0.35, 1.0);

pub(crate) const STAT_RED: Color = Color::rgba(0.85, 0.30, 0.25, 1.0);

pub(crate) const BTN_BG: Color = Color::rgba(0.35, 0.28, 0.18, 1.0);

pub(crate) const BTN_BORDER: Color = Color::rgba(0.60, 0.50, 0.30, 1.0);

pub(crate) const TAB_ACTIVE_BG: Color = Color::rgba(0.45, 0.36, 0.20, 1.0);

pub(crate) const TAB_IDLE_BG: Color = Color::rgba(0.22, 0.18, 0.14, 0.0);
