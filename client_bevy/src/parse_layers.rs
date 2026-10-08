#[derive(Debug, Clone)]
pub struct LayerBand {
    pub min_y: f32,
    pub max_y: f32,
    // None = this band applies at any x (old behavior, unchanged).
    pub min_x: Option<f32>,
    pub max_x: Option<f32>,
    pub layer: i32,
}

impl LayerBand {
    fn matches(&self, x: f32, y: f32) -> bool {
        if y < self.min_y || y > self.max_y {
            return false;
        }
        match (self.min_x, self.max_x) {
            (Some(min_x), Some(max_x)) => x >= min_x && x <= max_x,
            _ => true,
        }
    }
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

            // "y_min:y_max" or "y_min:y_max @ x_min:x_max"
            let mut parts = range_part.trim().splitn(2, '@');
            let Some(y_part) = parts.next() else { continue };
            let x_part = parts.next();

            let Some((a_str, b_str)) = y_part.trim().split_once(':') else {
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
            let min_y = a.min(b);
            let max_y = a.max(b);

            // Only set if an "@ x_min:x_max" part is present and valid;
            // otherwise the band stays x-unconstrained, same as before.
            let (min_x, max_x) = match x_part {
                Some(x_str) => match x_str.trim().split_once(':') {
                    Some((xa_str, xb_str)) => {
                        let xa = xa_str.trim().parse::<f32>().ok();
                        let xb = xb_str.trim().parse::<f32>().ok();
                        match (xa, xb) {
                            (Some(xa), Some(xb)) => (Some(xa.min(xb)), Some(xa.max(xb))),
                            _ => (None, None),
                        }
                    }
                    None => (None, None),
                },
                None => (None, None),
            };

            let layer = match layer_part.trim().parse::<i32>() {
                Ok(v) => v,
                Err(_) => continue,
            };

            bands.push(LayerBand { min_y, max_y, min_x, max_x, layer });
        }

        Self { bands }
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    // x is only used against bands that declared an "@ x_min:x_max" range;
    // bands without one match regardless of x, exactly like before.
    pub fn get_layer(&self, x: f32, y: f32) -> Option<i32> {
        self.bands
            .iter()
            .find(|b| b.matches(x, y))
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