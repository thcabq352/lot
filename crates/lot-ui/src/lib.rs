//! Command layer for the human film-bay. Same lot-core functions as the CLI.
//! Tests call these with path strings — they do not open a window.

use serde_json::{json, Value};
use std::path::Path;

pub const DOOR: &str = "ui";

pub fn status() -> Value {
    let mut st = lot_core::Status::bootstrap();
    st.door = DOOR;
    match serde_json::to_value(&st) {
        Ok(mut v) => {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("phases".into(), json!(lot_core::HANDOFF_PHASES));
            }
            v
        }
        Err(e) => err(&e.to_string()),
    }
}

pub fn create(path: &str, name: Option<&str>) -> Value {
    let path = path.trim();
    if path.is_empty() {
        return err("path is required");
    }
    match lot_core::create_show(Path::new(path), name) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "id": show.id, "name": show.name }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn open(path: &str) -> Value {
    let path = path.trim();
    if path.is_empty() {
        return err("path is required");
    }
    match lot_core::open_show(Path::new(path)) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "id": show.id, "name": show.name }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn school_set(enabled: Option<bool>, path: Option<&str>, amount: Option<&str>) -> Value {
    match lot_core::school_set(enabled, path, None, amount, None) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "school": show.school }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

fn with_door(mut v: Value) -> Value {
    if let Some(obj) = v.as_object_mut() {
        obj.insert("door".into(), json!(DOOR));
    }
    v
}

fn err(msg: &str) -> Value {
    json!({ "ok": false, "error": msg, "door": DOOR })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_ENV: Mutex<()> = Mutex::new(());

    fn isolate() {
        std::env::remove_var("LOT_SHOW");
        std::env::remove_var("LOT_CAP");
        std::env::remove_var("LOT_AGENT");
        lot_core::clear_caps();
        lot_core::clear_agent();
        let tmp = std::env::temp_dir().join(format!(
            "lot-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("LOT_HOME", tmp.join("home"));
    }

    fn show_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lot-ui-show-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn create_and_open_take_path_strings() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("create");
        let path = dir.to_string_lossy();
        let created = create(&path, Some("Carnival"));
        assert_eq!(created["ok"], true, "{created}");
        assert_eq!(created["door"], DOOR);
        assert_eq!(created["name"], "Carnival");
        assert!(
            created["show"].as_str().is_some_and(|s| !s.is_empty()),
            "create must echo a path string {created}"
        );

        let opened = open(&path);
        assert_eq!(opened["ok"], true, "{opened}");
        assert_eq!(opened["door"], DOOR);
        assert_eq!(opened["name"], "Carnival");
        assert!(
            opened["show"].as_str().is_some_and(|s| !s.is_empty()),
            "open must echo a path string {opened}"
        );

        let empty = create("  ", None);
        assert_eq!(empty["ok"], false);
        assert!(
            empty["error"].as_str().unwrap_or("").contains("path"),
            "{empty}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_door_is_ui_and_lists_phases() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("status");
        let created = create(&dir.to_string_lossy(), Some("Rail"));
        assert_eq!(created["ok"], true, "{created}");
        let st = status();
        assert_eq!(st["ok"], true, "{st}");
        assert_eq!(st["door"], DOOR);
        assert_eq!(st["show_name"], "Rail");
        let phases: Vec<&str> = st["phases"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str())
            .collect();
        assert_eq!(phases, lot_core::HANDOFF_PHASES.to_vec());
        assert_eq!(st["phase"], "writer");
        assert!(st["last_event"]["kind"].as_str().is_some(), "{st}");
        assert_eq!(st["school"]["enabled"], false);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn school_off_payload_has_no_lesson_keys() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("school");
        let created = create(&dir.to_string_lossy(), Some("NoLesson"));
        assert_eq!(created["ok"], true, "{created}");
        let v = school_set(Some(false), None, None);
        assert_eq!(v["ok"], true, "{v}");
        assert_eq!(v["door"], DOOR);
        assert_eq!(v["school"]["enabled"], false);
        let obj = v.as_object().unwrap();
        for forbidden in ["lesson", "quiz", "theory", "school_note", "rubric"] {
            assert!(
                !obj.contains_key(forbidden),
                "school off must not leak {forbidden} in {v}"
            );
        }
        let st = status();
        let st_obj = st.as_object().unwrap();
        for forbidden in ["lesson", "quiz", "theory", "school_note", "rubric"] {
            assert!(
                !st_obj.contains_key(forbidden),
                "status school off must not leak {forbidden} in {st}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
