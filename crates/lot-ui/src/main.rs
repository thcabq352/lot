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

#[tauri::command]
fn writer_brief(text: String) -> Value {
    lot_ui::writer_brief(&text)
}

#[tauri::command]
fn writer_style(
    genre: Option<String>,
    living: Option<String>,
    canon: Option<String>,
    format: Option<String>,
) -> Value {
    lot_ui::writer_style(
        genre.as_deref(),
        living.as_deref(),
        canon.as_deref(),
        format.as_deref(),
    )
}

#[tauri::command]
fn writer_cast(
    name: Option<String>,
    function: Option<String>,
    look: Option<String>,
    must_not: Option<String>,
    from_json: Option<String>,
) -> Value {
    lot_ui::writer_cast(
        name.as_deref(),
        function.as_deref(),
        look.as_deref(),
        must_not.as_deref(),
        from_json.as_deref(),
    )
}

#[tauri::command]
fn writer_draft() -> Value {
    lot_ui::writer_draft()
}

#[tauri::command]
fn writer_revise(notes: String) -> Value {
    lot_ui::writer_revise(&notes)
}

#[tauri::command]
fn writer_lock() -> Value {
    lot_ui::writer_lock()
}

#[tauri::command]
fn writer_unlock() -> Value {
    lot_ui::writer_unlock()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            status,
            create,
            open,
            school_set,
            writer_brief,
            writer_style,
            writer_cast,
            writer_draft,
            writer_revise,
            writer_lock,
            writer_unlock
        ])
        .run(tauri::generate_context!())
        .expect("no lot-ui — window failed to start");
}
