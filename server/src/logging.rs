use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::AtomicBool;

use chrono::Local;
use serde_json::json;

pub static LOG_MODE: AtomicBool = AtomicBool::new(false);

pub fn log_event(event_type: &str, player: &str, details: serde_json::Value) {
    let level = match event_type {
        "AUTH_FAIL" | "ABUSE_PATTERN" => "WARN",
        "INTERNAL_ERROR" | "DB_ERROR" => "ERROR",
        _ => "INFO",
    };
    let log_entry = json!({
        "timestamp": Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "level": level,
        "event": event_type,
        "player": player,
        "details": details
    });
    let log_string = log_entry.to_string();
    let suppress_pos_log = matches!(event_type, "COMMAND" | "RESPONSE")
        && log_entry["details"]["raw"]
            .as_str()
            .map(|raw| raw.trim_start().to_ascii_uppercase().starts_with("POS "))
            .unwrap_or(false);
    if suppress_pos_log {
        return;
    }
    println!("{}", log_string);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs.json")
    {
        let _ = writeln!(file, "{}", log_string);
    }
}
