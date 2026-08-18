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
                if let Some(w) = current_writer() {
                    obj.insert("writer".into(), w);
                }
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

pub fn writer_brief(text: &str) -> Value {
    match lot_core::set_brief(text) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "brief": show.writer.brief }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn writer_style(
    genre: Option<&str>,
    living: Option<&str>,
    canon: Option<&str>,
    format: Option<&str>,
) -> Value {
    let genres = csv_list(genre);
    let living = csv_list(living);
    let canon = csv_list(canon);
    let format = format.map(str::trim).filter(|s| !s.is_empty());
    match lot_core::set_style(
        genres.as_deref(),
        living.as_deref(),
        canon.as_deref(),
        format,
    ) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({
                "genres": show.writer.genres,
                "styles_living": show.writer.styles_living,
                "styles_canon": show.writer.styles_canon,
                "format": show.writer.format,
            }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn writer_cast(
    name: Option<&str>,
    function: Option<&str>,
    look: Option<&str>,
    must_not: Option<&str>,
    from_json: Option<&str>,
) -> Value {
    let name = name.map(str::trim).filter(|s| !s.is_empty());
    if from_json.is_some() && name.is_some() {
        return err("cast: use name or json, not both");
    }
    let result = if let Some(raw) = from_json {
        lot_core::replace_cast_json(raw)
    } else if let Some(n) = name {
        lot_core::upsert_cast(n, function, look, must_not)
    } else {
        return err("cast needs name or json");
    };
    match result {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "cast": show.writer.cast }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn writer_draft() -> Value {
    match lot_core::draft_screenplay() {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({
                "draft": show.writer.draft_path,
                "provenance": show.writer.draft_provenance,
            }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn writer_revise(notes: &str) -> Value {
    if notes.trim().is_empty() {
        return err("notes is required");
    }
    match lot_core::revise_screenplay(notes) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({
                "draft": show.writer.draft_path,
                "provenance": show.writer.draft_provenance,
            }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn writer_lock() -> Value {
    match lot_core::lock_writer() {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "locked": show.writer.locked }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn writer_unlock() -> Value {
    match lot_core::unlock_writer() {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            json!({ "locked": show.writer.locked }),
        )),
        Err(e) => err(&e.to_string()),
    }
}

fn current_writer() -> Option<Value> {
    let p = lot_core::current_show_path().ok().flatten()?;
    let show = lot_core::read_show(&p).ok()?;
    serde_json::to_value(&show.writer).ok()
}

fn csv_list(raw: Option<&str>) -> Option<Vec<String>> {
    let parts: Vec<String> = raw?
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
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

    fn assert_no_lesson(v: &Value) {
        let obj = v.as_object().unwrap();
        for forbidden in ["lesson", "quiz", "theory", "school_note", "rubric"] {
            assert!(
                !obj.contains_key(forbidden),
                "school off must not leak {forbidden} in {v}"
            );
        }
    }

    #[test]
    fn empty_brief_refuses_draft() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("draft");
        let created = create(&dir.to_string_lossy(), Some("NoBrief"));
        assert_eq!(created["ok"], true, "{created}");
        let drafted = writer_draft();
        assert_eq!(drafted["ok"], false, "{drafted}");
        assert_eq!(drafted["door"], DOOR);
        assert!(
            drafted["error"].as_str().unwrap_or("").contains("no brief"),
            "{drafted}"
        );
        assert_no_lesson(&drafted);
        let st = status();
        assert_eq!(st["writer"]["brief"], "");
        assert!(st["writer"]["draft_path"].is_null(), "{st}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writer_confirm_school_off_has_no_lesson_keys() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("writer");
        let created = create(&dir.to_string_lossy(), Some("Desk"));
        assert_eq!(created["ok"], true, "{created}");

        let brief = writer_brief("Ada waits by the tent.");
        assert_eq!(brief["ok"], true, "{brief}");
        assert_eq!(brief["door"], DOOR);
        assert_eq!(brief["brief"], "Ada waits by the tent.");
        assert_no_lesson(&brief);

        let style = writer_style(Some("drama"), None, None, Some("ad"));
        assert_eq!(style["ok"], true, "{style}");
        assert_eq!(style["format"], "advertisement");
        assert_eq!(style["genres"][0], "drama");
        assert_no_lesson(&style);

        let cast = writer_cast(Some("Ada"), Some("lead"), None, None, None);
        assert_eq!(cast["ok"], true, "{cast}");
        assert_eq!(cast["cast"][0]["name"], "Ada");
        assert_eq!(cast["cast"][0]["function"], "lead");
        assert_no_lesson(&cast);

        let locked = writer_lock();
        assert_eq!(locked["ok"], true, "{locked}");
        assert_eq!(locked["locked"], true);
        assert_no_lesson(&locked);
        let unlocked = writer_unlock();
        assert_eq!(unlocked["ok"], true, "{unlocked}");
        assert_eq!(unlocked["locked"], false);

        let st = status();
        assert_eq!(st["writer"]["brief"], "Ada waits by the tent.");
        assert_eq!(st["writer"]["format"], "advertisement");
        assert_eq!(st["writer"]["cast"][0]["name"], "Ada");
        assert_eq!(st["writer"]["locked"], false);
        assert_no_lesson(&st);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
