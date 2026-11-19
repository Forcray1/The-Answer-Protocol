use std::collections::HashMap;

use domain::RoomId;

use crate::state::RuntimeItem;

pub fn save_world_state(room_items: &HashMap<RoomId, Vec<RuntimeItem>>, world_path: &str) {
    let content = match std::fs::read_to_string(world_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[SAVE] Cannot read {}: {}", world_path, e);
            return;
        }
    };

    let mut value: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[SAVE] Cannot parse world.yaml: {}", e);
            return;
        }
    };

    if let Some(locations) = value
        .get_mut("world")
        .and_then(|w| w.get_mut("locations"))
        .and_then(|l| l.as_mapping_mut())
    {
        for (room_id, items) in room_items {
            let key = serde_yaml::Value::String(room_id.0.clone());
            if let Some(room_entry) = locations.get_mut(&key) {
                let item_ids: Vec<serde_yaml::Value> = items
                    .iter()
                    .map(|ri| serde_yaml::Value::String(ri.item_id.0.clone()))
                    .collect();
                room_entry["items"] = serde_yaml::Value::Sequence(item_ids);
            }
        }
    }

    match serde_yaml::to_string(&value) {
        Ok(output) => {
            if let Err(e) = std::fs::write(world_path, output) {
                eprintln!("[SAVE] Cannot write world.yaml: {}", e);
            } else {
                println!("[SAVE] world.yaml updated with current ground items.");
            }
        }
        Err(e) => eprintln!("[SAVE] Cannot serialize world.yaml: {}", e),
    }
}
