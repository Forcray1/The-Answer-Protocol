use bevy::prelude::*;

use crate::game::GameState;
use crate::AppState;

pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CollisionMask>()
            .add_systems(
                Update,
                load_mask_on_room_change.run_if(in_state(AppState::InGame)),
            );
    }
}

const ASSET_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../sprites");
const MASK_FILE: &str = "collision.png";

pub const MAP_WIDTH: f32 = 2560.0;
pub const MAP_HEIGHT: f32 = 1440.0;
pub const MAP_HALF_W: f32 = MAP_WIDTH / 2.0;
pub const MAP_HALF_H: f32 = MAP_HEIGHT / 2.0;

const ALPHA_THRESHOLD: u8 = 10;
const LUMA_THRESHOLD: u32 = 128;

#[derive(Resource, Default)]
pub struct CollisionMask {
    grid: Option<Mask>,
}

struct Mask {
    width: u32,
    height: u32,
    solid: Vec<bool>,
}

impl CollisionMask {
    pub fn blocks_box(&self, center: Vec2, half: Vec2) -> bool {
        let Some(m) = &self.grid else {
            return false;
        };
        let to_px = |x: f32| (x + MAP_HALF_W) / MAP_WIDTH * m.width as f32;
        let to_py = |y: f32| (MAP_HALF_H - y) / MAP_HEIGHT * m.height as f32;

        let clamp_x = |v: f32| v.floor().clamp(0.0, (m.width - 1) as f32) as u32;
        let clamp_y = |v: f32| v.floor().clamp(0.0, (m.height - 1) as f32) as u32;

        let px0 = clamp_x(to_px(center.x - half.x));
        let px1 = clamp_x(to_px(center.x + half.x));
        let py0 = clamp_y(to_py(center.y + half.y));
        let py1 = clamp_y(to_py(center.y - half.y));

        for py in py0..=py1 {
            for px in px0..=px1 {
                if m.solid[(py * m.width + px) as usize] {
                    return true;
                }
            }
        }
        false
    }
}

fn load_mask_on_room_change(
    game_state: Res<GameState>,
    mut mask: ResMut<CollisionMask>,
    mut current: Local<Option<String>>,
) {
    let room = &game_state.current_room;
    if room == "unknown" || current.as_deref() == Some(room.as_str()) {
        return;
    }
    *current = Some(room.clone());

    mask.grid = load_mask(room);
    match &mask.grid {
        Some(m) => println!(
            "[COLLISION] Masque chargé pour '{}' ({}x{}).",
            room, m.width, m.height
        ),
        None => println!(
            "[COLLISION] Aucun masque pour '{}' (déplacement libre).",
            room
        ),
    }
}

fn load_mask(room: &str) -> Option<Mask> {
    let path = format!("{}/maps/{}/{}", ASSET_ROOT, room, MASK_FILE);
    let img = match image::open(&path) {
        Ok(img) => img.to_rgba8(),
        Err(_) => return None,
    };
    let (width, height) = img.dimensions();
    let solid = img
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            a > ALPHA_THRESHOLD && luma < LUMA_THRESHOLD
        })
        .collect();
    Some(Mask {
        width,
        height,
        solid,
    })
}
