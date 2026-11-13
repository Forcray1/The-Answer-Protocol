use crate::AppState;
use crate::net::{NetworkSender, NetworkStatus, ServerMessageEvent};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

#[derive(Resource, Default)]
pub struct LoginData {
    pub username: String,
    pub password: String,
    pub error_msg: Option<String>,
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoginData>().add_systems(
            Update,
            (login_ui_system, handle_login_responses).run_if(in_state(AppState::MainMenu)),
        );
    }
}

fn login_ui_system(
    mut contexts: EguiContexts,
    mut login_data: ResMut<LoginData>,
    sender: Res<NetworkSender>,
    status: Res<NetworkStatus>,
) {
    let ctx = contexts.ctx_mut();

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_black_alpha(200)))
        .show(ctx, |_| {});

    egui::Window::new("Login")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.heading("The Answer Protocol");
                ui.add_space(20.0);

                if !status.connected {
                    ui.colored_label(egui::Color32::YELLOW, "Waiting for the server...");
                    ui.add_space(10.0);
                }

                if let Some(error) = &login_data.error_msg {
                    ui.colored_label(egui::Color32::RED, error);
                    ui.add_space(10.0);
                }

                egui::Grid::new("login_grid")
                    .num_columns(2)
                    .spacing([10.0, 10.0])
                    .show(ui, |ui| {
                        ui.label("Username:");
                        let response = ui.text_edit_singleline(&mut login_data.username);
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            send_login(&login_data, &sender);
                        }
                        ui.end_row();

                        ui.label("Password:");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut login_data.password).password(true),
                        );
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            send_login(&login_data, &sender);
                        }
                        ui.end_row();
                    });

                ui.add_space(20.0);
                if ui.button("Connect").clicked() {
                    send_login(&login_data, &sender);
                }
                ui.add_space(10.0);
            });
        });
}

fn send_login(login_data: &LoginData, sender: &NetworkSender) {
    if !login_data.username.is_empty() && !login_data.password.is_empty() {
        let _ = sender.0.send(format!(
            "CONNECT {} {}\n",
            login_data.username, login_data.password
        ));
    }
}

fn handle_login_responses(
    mut events: EventReader<ServerMessageEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    mut login_data: ResMut<LoginData>,
) {
    for ev in events.read() {
        let msg = &ev.0;
        if msg.starts_with("S: OK connected") {
            next_state.set(AppState::InGame);
        } else if msg.starts_with("S: ERR") {
            login_data.error_msg = Some(msg.clone());
        } else if msg == "Network system initialized." {
            // Connected (again): drop a stale "unreachable" error.
            if login_data.error_msg.as_deref() == Some("S: ERR Unable to reach the server.") {
                login_data.error_msg = None;
            }
        }
    }
}
