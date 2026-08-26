#[derive(Debug, Clone)]
pub struct LayerBand {
    pub min_y: f32,
    pub max_y: f32,
    pub layer: i32,
}

#[derive(Debug, Clone, Default)]
pub struct LayerProfile {
    bands: Vec<LayerBand>,
}

impl LayerProfile {
    pub fn from_text(input: &str) -> Self {
        let mut bands = Vec::new();

        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((range_part, layer_part)) = line.split_once('=') else {
                continue;
            };
            let Some((a_str, b_str)) = range_part.trim().split_once(':') else {
                continue;
            };

            let a = match a_str.trim().parse::<f32>() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let b = match b_str.trim().parse::<f32>() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let layer = match layer_part.trim().parse::<i32>() {
                Ok(v) => v,
                Err(_) => continue,
            };

            let min_y = a.min(b);
            let max_y = a.max(b);
            bands.push(LayerBand { min_y, max_y, layer });
        }

        Self { bands }
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    pub fn get_layer(&self, y: f32) -> Option<i32> {
        self.bands
            .iter()
            .find(|b| y >= b.min_y && y <= b.max_y)
            .map(|b| b.layer)
    }
}

pub fn load_room_layers(asset_root: &str, room: &str) -> Option<LayerProfile> {
    let path = format!("{}/maps/{}/layers.txt", asset_root, room);
    let raw = std::fs::read_to_string(path).ok()?;
    let profile = LayerProfile::from_text(&raw);
    if profile.is_empty() {
        None
    } else {
        Some(profile)
    }
}
