mod bubbles;
mod components;
mod input;
mod messages;
mod setup;
mod state;
mod view;

pub(crate) use bubbles::*;
pub(crate) use input::*;
pub(crate) use messages::*;
pub(crate) use setup::*;
pub(crate) use state::*;
pub(crate) use view::*;

use bevy::prelude::*;
use crate::AppState;

pub(crate) const MAX_CHAT_LINES: usize = 10;

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatConsole>()
            .init_resource::<ChatHistory>()
            .init_resource::<ChatFadeState>()
            .add_systems(Startup, setup_chat_ui)
            .add_systems(OnEnter(AppState::InGame), on_enter_ingame)
            .add_systems(OnExit(AppState::InGame), on_exit_ingame)
            .add_systems(
                Update,
                (
                    toggle_chat,
                    toggle_terminal,
                    handle_inputs.after(toggle_chat),
                    handle_terminal_inputs.after(toggle_terminal),
                    handle_tab_clicks,
                    handle_input_clicks,
                    process_chat_events,
                    display_terminal_messages,
                    tick_chat_fade.after(process_chat_events),
                    update_chat_container_style.after(tick_chat_fade),
                    update_chat_tabs_ui.after(tick_chat_fade),
                    update_chat_input_ui.after(tick_chat_fade),
                    update_chat_messages_ui.after(tick_chat_fade),
                    spawn_chat_bubbles,
                    tick_chat_bubbles,
                    update_chat_combat_visibility,
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
