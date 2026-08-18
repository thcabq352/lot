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

#[tauri::command]
fn section(phase: Option<String>) -> Value {
    lot_ui::section(phase.as_deref())
}

#[tauri::command]
fn breakdown_parse() -> Value {
    lot_ui::breakdown_parse()
}

#[tauri::command]
fn wall_add(act: Option<String>, text: String) -> Value {
    lot_ui::wall_add(act.as_deref(), &text)
}

#[tauri::command]
fn wall_update(id: String, text: Option<String>, act: Option<String>) -> Value {
    lot_ui::wall_update(&id, text.as_deref(), act.as_deref())
}

#[tauri::command]
fn wall_remove(id: String) -> Value {
    lot_ui::wall_remove(&id)
}

#[tauri::command]
fn picture_lock(shot: String) -> Value {
    lot_ui::picture_lock(&shot)
}

#[tauri::command]
fn picture_unlock(shot: String) -> Value {
    lot_ui::picture_unlock(&shot)
}

#[tauri::command]
fn picture_ref(shot: String, file: String, note: Option<String>, size: Option<String>) -> Value {
    lot_ui::picture_ref(&shot, &file, note.as_deref(), size.as_deref())
}

#[tauri::command]
fn handoff(commit: Option<bool>) -> Value {
    lot_ui::handoff(commit.unwrap_or(false))
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
            writer_unlock,
            section,
            breakdown_parse,
            wall_add,
            wall_update,
            wall_remove,
            picture_lock,
            picture_unlock,
            picture_ref,
            handoff
        ])
        .run(tauri::generate_context!())
        .expect("no lot-ui — window failed to start");
}
