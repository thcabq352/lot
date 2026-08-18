#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::Value;

#[tauri::command]
fn status() -> Value {
    lot_ui::status()
}

#[tauri::command]
fn create(path: String, name: Option<String>) -> Value {
    lot_ui::create(&path, name.as_deref())
}

#[tauri::command]
fn open(path: String) -> Value {
    lot_ui::open(&path)
}

#[tauri::command]
fn school_set(enabled: Option<bool>, path: Option<String>, amount: Option<String>) -> Value {
    lot_ui::school_set(enabled, path.as_deref(), amount.as_deref())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![status, create, open, school_set])
        .run(tauri::generate_context!())
        .expect("no lot-ui — window failed to start");
}
