//! Command layer for the human film-bay. Same lot-core functions as the CLI.
//! Tests call these with path strings — they do not open a window.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const DOOR: &str = "ui";

pub fn status() -> Value {
    let mut st = lot_core::Status::bootstrap();
    st.door = DOOR;
    match serde_json::to_value(&st) {
        Ok(mut v) => {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("phases".into(), json!(lot_core::HANDOFF_PHASES));
                if let Some((dir, show)) = current_open() {
                    if let Ok(w) = serde_json::to_value(&show.writer) {
                        obj.insert("writer".into(), w);
                    }
                    let phase = show.phase.clone();
                    obj.insert("section".into(), section_view(&dir, &show, &phase));
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

pub fn section(phase: Option<&str>) -> Value {
    match lot_core::require_current() {
        Ok((dir, show)) => {
            let phase = normalize_phase(phase, &show.phase);
            let mut v = section_view(&dir, &show, &phase);
            if let Some(obj) = v.as_object_mut() {
                obj.insert("ok".into(), json!(true));
                obj.insert("show".into(), json!(dir.display().to_string()));
            }
            if let Ok((_, _, card)) = lot_core::resource_read("lot://show") {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("show_card".into(), card);
                }
            }
            with_door(v)
        }
        Err(e) => err(&e.to_string()),
    }
}

pub fn breakdown_parse() -> Value {
    match lot_core::breakdown_parse(None) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            lot_core::breakdown_summary(&show),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn wall_add(act: Option<&str>, text: &str) -> Value {
    match lot_core::wall_add(act, text) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            lot_core::wall_summary(&show),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn wall_update(id: &str, text: Option<&str>, act: Option<&str>) -> Value {
    let id = id.trim();
    if id.is_empty() {
        return err("id is required");
    }
    match lot_core::wall_update(id, text, act) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            lot_core::wall_summary(&show),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn wall_remove(id: &str) -> Value {
    let id = id.trim();
    if id.is_empty() {
        return err("id is required");
    }
    match lot_core::wall_remove(id) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            lot_core::wall_summary(&show),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn picture_lock(shot: &str) -> Value {
    let shot = shot.trim();
    if shot.is_empty() {
        return err("shot is required");
    }
    match lot_core::picture_lock(shot, true) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            picture_confirm(&show, shot, true),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn picture_unlock(shot: &str) -> Value {
    let shot = shot.trim();
    if shot.is_empty() {
        return err("shot is required");
    }
    match lot_core::picture_unlock(shot) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            picture_confirm(&show, shot, false),
        )),
        Err(e) => err(&e.to_string()),
    }
}

pub fn picture_ref(shot: &str, file: &str, note: Option<&str>, size: Option<&str>) -> Value {
    let shot = shot.trim();
    if shot.is_empty() {
        return err("shot is required");
    }
    let file = file.trim();
    if file.is_empty() {
        return err("path is required");
    }
    match lot_core::picture_ref(shot, Path::new(file), note, size) {
        Ok((dir, show)) => with_door(lot_core::mutation_json(
            &dir,
            &show,
            lot_core::picture_summary(&show),
        )),
        Err(e) => err(&e.to_string()),
    }
}

fn picture_confirm(show: &lot_core::Show, shot: &str, locked: bool) -> Value {
    let mut v = lot_core::picture_summary(show);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("shot".into(), json!(shot));
        obj.insert("locked".into(), json!(locked));
        obj.insert(
            "locked_count".into(),
            json!(show.shots.iter().filter(|s| s.locked).count()),
        );
    }
    v
}

pub fn handoff(commit: bool) -> Value {
    match lot_core::handoff(commit) {
        Ok((dir, show, report)) => {
            let extra = serde_json::to_value(&report).unwrap_or_else(|_| json!({}));
            with_door(lot_core::mutation_json(&dir, &show, extra))
        }
        Err(e) => err(&e.to_string()),
    }
}

fn current_open() -> Option<(PathBuf, lot_core::Show)> {
    let p = lot_core::current_show_path().ok().flatten()?;
    let show = lot_core::read_show(&p).ok()?;
    Some((p, show))
}

fn normalize_phase(raw: Option<&str>, fallback: &str) -> String {
    let t = raw.unwrap_or(fallback).trim().to_ascii_lowercase();
    if t.is_empty() {
        fallback.to_ascii_lowercase()
    } else {
        t
    }
}

fn section_view(dir: &Path, show: &lot_core::Show, phase: &str) -> Value {
    json!({
        "phase": phase,
        "card": card_for(dir, show, phase),
        "handoff": serde_json::to_value(lot_core::inspect_handoff(dir, show))
            .unwrap_or_else(|_| json!({})),
    })
}

fn card_for(dir: &Path, show: &lot_core::Show, phase: &str) -> Value {
    match phase {
        "writer" => json!({
            "brief": show.writer.brief,
            "draft": show.writer.draft_path,
            "locked": show.writer.locked,
            "cast": show.writer.cast.len(),
            "format": show.writer.format,
        }),
        "breakdown" => breakdown_card(dir, show),
        "wall" => lot_core::wall_summary(show),
        "picture" => lot_core::picture_summary(show),
        "stage" => stage_card(dir, show),
        "motion" => motion_card(dir, show),
        "board" => board_card(dir, show),
        "slate" => json!({
            "target": show.slate.default_target,
            "prompts": show.shots.iter().filter(|s| !s.prompt.trim().is_empty()).count(),
            "cards": show.shots.iter().map(|s| json!({
                "num": s.num,
                "name": s.name,
                "prompt": s.prompt,
                "targets": s.prompt_targets.keys().cloned().collect::<Vec<_>>(),
                "loras": s.loras.iter().map(|l| l.id.clone()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
        "dailies" => json!({
            "takes": show.takes.len(),
            "circled": show.takes.iter().filter(|t| t.circled).count(),
            "cards": show.takes.iter().map(|t| json!({
                "id": t.id,
                "filename": t.filename,
                "path": t.path,
                "shot_id": t.shot_id,
                "circled": t.circled,
            })).collect::<Vec<_>>(),
        }),
        "stems" => json!({
            "brief": show.stems.soundtrack_brief,
            "cue": show.stems.soundtrack_cue,
            "soundtrack": show.stems.soundtrack_path,
            "vo_text": show.stems.vo_text,
            "vo": show.stems.vo_path,
        }),
        "cut" => cut_card(dir, show),
        other => json!({ "phase": other }),
    }
}

fn breakdown_card(dir: &Path, show: &lot_core::Show) -> Value {
    let mut sum = lot_core::breakdown_summary(show);
    let (fountain, fountain_path) = fountain_info(dir, show);
    if let Some(obj) = sum.as_object_mut() {
        obj.insert("fountain".into(), json!(fountain));
        if let Some(p) = fountain_path {
            obj.insert("fountain_path".into(), json!(p));
        }
        obj.insert(
            "slugs".into(),
            json!(show
                .scenes
                .iter()
                .map(|s| json!({ "id": s.id, "num": s.num, "slug": s.slug }))
                .collect::<Vec<_>>()),
        );
    }
    sum
}

fn stage_card(dir: &Path, show: &lot_core::Show) -> Value {
    let block = dir.join("stage").join("block.json");
    let marks = show
        .shots
        .iter()
        .map(|s| s.stage_marks.len())
        .sum::<usize>();
    json!({
        "block": block.is_file(),
        "block_path": existing_path(&block),
        "marks": marks,
        "cards": show.shots.iter().map(|s| json!({
            "num": s.num,
            "name": s.name,
            "size": s.size,
            "angle": s.angle,
            "lens": s.lens,
            "move": s.move_kind,
            "marks": s.stage_marks.iter().map(|m| json!({
                "who": m.who,
                "mark": m.mark,
                "x": m.x,
                "z": m.z,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn motion_card(dir: &Path, show: &lot_core::Show) -> Value {
    let previs = dir.join("motion").join("previs.json");
    json!({
        "previs": previs.is_file(),
        "previs_path": existing_path(&previs),
        "plates": show.shots.iter().filter(|s| s.plate_path.is_some()).count(),
        "cards": show.shots.iter().map(|s| json!({
            "num": s.num,
            "name": s.name,
            "plate": s.plate_path,
            "mode": s.motion_mode,
            "move": s.motion_move,
            "notes": s.motion_notes,
        })).collect::<Vec<_>>(),
    })
}

fn board_card(dir: &Path, show: &lot_core::Show) -> Value {
    let board = dir.join("board").join("board.json");
    if board.is_file() {
        if let Some(pack) = read_json(&board) {
            let cards = pack
                .get("shots")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|sh| {
                            json!({
                                "num": sh.get("num"),
                                "name": sh.get("name"),
                                "still": sh.get("still"),
                                "backend": sh.get("backend"),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let stills = cards
                .iter()
                .filter(|c| c.get("still").and_then(|s| s.as_str()).is_some())
                .count();
            return json!({
                "board": true,
                "board_path": board.display().to_string(),
                "stills": stills,
                "cards": cards,
            });
        }
    }
    json!({
        "board": false,
        "stills": show.shots.iter().filter(|s| s.still_path.is_some()).count(),
        "cards": show.shots.iter().map(|s| json!({
            "num": s.num,
            "name": s.name,
            "still": s.still_path,
            "backend": s.still_backend,
        })).collect::<Vec<_>>(),
    })
}

fn cut_card(dir: &Path, show: &lot_core::Show) -> Value {
    let fcpxml = dir.join("export.fcpxml");
    let edl = dir.join("export.edl");
    json!({
        "circled": show.takes.iter().filter(|t| t.circled).count(),
        "fcpxml": fcpxml.is_file(),
        "fcpxml_path": existing_path(&fcpxml),
        "edl": edl.is_file(),
        "edl_path": existing_path(&edl),
        "finish": show.finish.path,
        "upscaled": show.finish.upscaled,
    })
}

fn fountain_info(dir: &Path, show: &lot_core::Show) -> (bool, Option<String>) {
    if let Some(p) = &show.writer.draft_path {
        if Path::new(p).is_file() {
            return (true, Some(p.clone()));
        }
    }
    let fallback = dir.join(lot_core::SCREENPLAY_FILE);
    if fallback.is_file() {
        (true, Some(fallback.display().to_string()))
    } else {
        (false, None)
    }
}

fn existing_path(p: &Path) -> Option<String> {
    p.is_file().then(|| p.display().to_string())
}

fn read_json(p: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&raw).ok()
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

    #[test]
    fn section_follows_phase_and_has_no_lesson_keys() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("section");
        let created = create(&dir.to_string_lossy(), Some("Bay"));
        assert_eq!(created["ok"], true, "{created}");
        let st = status();
        assert_eq!(st["section"]["phase"], "writer", "{st}");
        assert!(st["section"]["card"]["brief"].is_string(), "{st}");
        assert!(st["section"]["handoff"]["missing"].is_array(), "{st}");
        assert_no_lesson(&st);

        let sec = section(None);
        assert_eq!(sec["ok"], true, "{sec}");
        assert_eq!(sec["door"], DOOR);
        assert_eq!(sec["phase"], "writer");
        assert!(
            sec["show"].as_str().is_some_and(|s| !s.is_empty()),
            "section must echo a path string {sec}"
        );
        assert_eq!(sec["show_card"]["name"], "Bay");
        assert_no_lesson(&sec);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wall_picture_commands_are_honest() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("honest");
        let created = create(&dir.to_string_lossy(), Some("Honest"));
        assert_eq!(created["ok"], true, "{created}");
        std::fs::write(
            dir.join(lot_core::SCREENPLAY_FILE),
            "INT. TENT - NIGHT\n\nADA\nDon't put it on.\n",
        )
        .unwrap();
        assert_eq!(breakdown_parse()["ok"], true);

        let empty = wall_add(None, "");
        assert_eq!(empty["ok"], false, "{empty}");
        assert!(
            empty["error"].as_str().unwrap_or("").contains("no beat"),
            "{empty}"
        );

        let added = wall_add(Some("i"), "Ada waits.");
        assert_eq!(added["ok"], true, "{added}");
        assert!(
            added["show"].as_str().is_some_and(|s| !s.is_empty()),
            "{added}"
        );
        let removed = wall_remove("beat-1");
        assert_eq!(removed["ok"], true, "{removed}");
        assert_eq!(removed["beats"], 0);
        let missing = wall_remove("beat-1");
        assert_eq!(missing["ok"], false, "{missing}");
        assert!(
            missing["error"]
                .as_str()
                .unwrap_or("")
                .contains("unknown beat"),
            "{missing}"
        );

        let locked = picture_lock("01");
        assert_eq!(locked["ok"], true, "{locked}");
        let unlocked = picture_unlock("01");
        assert_eq!(unlocked["ok"], true, "{unlocked}");
        assert_eq!(unlocked["locked"], false);

        let gone = picture_ref("01", "no-such-ref.png", None, None);
        assert_eq!(gone["ok"], false, "{gone}");
        assert!(
            gone["error"].as_str().unwrap_or("").contains("not a file")
                || gone["error"].as_str().unwrap_or("").contains("no ref"),
            "{gone}"
        );

        let other = show_dir("honest-other");
        let _ = std::fs::remove_dir_all(&other);
        lot_core::create_show(&other, Some("Other")).unwrap();
        let stolen = other.join("secret.png");
        std::fs::write(&stolen, b"nope").unwrap();
        lot_core::open_show(&dir).unwrap();
        let jailed = picture_ref("01", &stolen.to_string_lossy(), None, None);
        assert_eq!(jailed["ok"], false, "{jailed}");
        assert!(
            jailed["error"].as_str().unwrap_or("").contains("jailed"),
            "{jailed}"
        );
        let _ = std::fs::remove_dir_all(&other);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn breakdown_parse_without_fountain_is_honest() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("nofountain");
        let created = create(&dir.to_string_lossy(), Some("Empty"));
        assert_eq!(created["ok"], true, "{created}");
        let parsed = breakdown_parse();
        assert_eq!(parsed["ok"], false, "{parsed}");
        assert_eq!(parsed["door"], DOOR);
        assert!(
            parsed["error"]
                .as_str()
                .unwrap_or("")
                .contains("no screenplay"),
            "{parsed}"
        );
        assert_no_lesson(&parsed);
        let sec = section(Some("breakdown"));
        assert_eq!(sec["ok"], true, "{sec}");
        assert_eq!(sec["card"]["fountain"], false);
        assert_eq!(sec["card"]["scenes"], 0);
        assert_no_lesson(&sec);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn section_viewers_and_thin_confirms_school_off() {
        let _g = TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = show_dir("pipeline");
        let created = create(&dir.to_string_lossy(), Some("Carnival"));
        assert_eq!(created["ok"], true, "{created}");
        std::fs::write(
            dir.join(lot_core::SCREENPLAY_FILE),
            "INT. TENT - NIGHT\n\nADA\nDon't put it on.\n",
        )
        .unwrap();

        let parsed = breakdown_parse();
        assert_eq!(parsed["ok"], true, "{parsed}");
        assert_eq!(parsed["door"], DOOR);
        assert_eq!(parsed["scenes"], 1);
        assert_no_lesson(&parsed);

        let breakdown = section(Some("breakdown"));
        assert_eq!(breakdown["ok"], true, "{breakdown}");
        assert!(
            breakdown["show"].as_str().is_some_and(|s| !s.is_empty()),
            "{breakdown}"
        );
        assert_eq!(breakdown["card"]["fountain"], true);
        assert!(
            breakdown["card"]["fountain_path"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{breakdown}"
        );
        assert_eq!(breakdown["card"]["scenes"], 1);
        assert_eq!(breakdown["card"]["slugs"][0]["slug"], "INT. TENT - NIGHT");
        let chars = breakdown["card"]["characters"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            chars.iter().any(|c| c.as_str() == Some("ADA")),
            "{breakdown}"
        );
        assert_no_lesson(&breakdown);

        let dry = handoff(false);
        assert_eq!(dry["ok"], true, "{dry}");
        assert_eq!(dry["ready"], true, "{dry}");
        assert_eq!(dry["dry_run"], true);
        assert_eq!(dry["next"], "wall");
        assert_no_lesson(&dry);

        let advanced = handoff(true);
        assert_eq!(advanced["ok"], true, "{advanced}");
        assert_eq!(advanced["committed"], true, "{advanced}");
        assert_eq!(advanced["phase"], "wall");
        assert_no_lesson(&advanced);

        let beat = wall_add(Some("i"), "Ada waits by the trunk.");
        assert_eq!(beat["ok"], true, "{beat}");
        assert_eq!(beat["beats"], 1);
        assert_eq!(beat["cards"][0]["text"], "Ada waits by the trunk.");
        assert_no_lesson(&beat);

        let revised = wall_update("beat-1", Some("Ada waits in the rain."), None);
        assert_eq!(revised["ok"], true, "{revised}");
        assert_eq!(revised["cards"][0]["text"], "Ada waits in the rain.");
        assert_no_lesson(&revised);

        let empty_beat = wall_add(None, "  ");
        assert_eq!(empty_beat["ok"], false, "{empty_beat}");
        assert!(
            empty_beat["error"]
                .as_str()
                .unwrap_or("")
                .contains("no beat"),
            "{empty_beat}"
        );

        let second = wall_add(Some("ii"), "She does not put it on.");
        assert_eq!(second["ok"], true, "{second}");
        let gone = wall_remove("beat-2");
        assert_eq!(gone["ok"], true, "{gone}");
        assert_eq!(gone["beats"], 1);
        assert_eq!(gone["cards"][0]["id"], "beat-1");
        assert_no_lesson(&gone);

        let missing = wall_remove("beat-9");
        assert_eq!(missing["ok"], false, "{missing}");
        assert!(
            missing["error"]
                .as_str()
                .unwrap_or("")
                .contains("unknown beat"),
            "{missing}"
        );

        let wall = section(Some("wall"));
        assert_eq!(wall["card"]["beats"], 1);
        assert_eq!(wall["card"]["cards"][0]["text"], "Ada waits in the rain.");
        assert_no_lesson(&wall);

        let empty_shot = picture_lock("  ");
        assert_eq!(empty_shot["ok"], false);
        assert!(
            empty_shot["error"].as_str().unwrap_or("").contains("shot"),
            "{empty_shot}"
        );

        let locked = picture_lock("01");
        assert_eq!(locked["ok"], true, "{locked}");
        assert_eq!(locked["locked"], true);
        assert_eq!(locked["locked_count"], 1);
        assert_no_lesson(&locked);

        let unlocked = picture_unlock("01");
        assert_eq!(unlocked["ok"], true, "{unlocked}");
        assert_eq!(unlocked["locked"], false);
        assert_eq!(unlocked["locked_count"], 0);
        assert_no_lesson(&unlocked);

        let relocked = picture_lock("01");
        assert_eq!(relocked["ok"], true, "{relocked}");

        let src = dir.join("media").join("tent.png");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"png-bytes").unwrap();
        let attached = picture_ref("01", &src.to_string_lossy(), None, None);
        assert_eq!(attached["ok"], true, "{attached}");
        assert!(
            attached["cards"][0]["ref"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{attached}"
        );
        assert!(src.is_file(), "source must stay");
        assert_no_lesson(&attached);

        let other = show_dir("other-ref");
        let _ = std::fs::remove_dir_all(&other);
        lot_core::create_show(&other, Some("Other")).unwrap();
        let stolen = other.join("secret.png");
        std::fs::write(&stolen, b"nope").unwrap();
        lot_core::open_show(&dir).unwrap();
        let jailed = picture_ref("01", &stolen.to_string_lossy(), None, None);
        assert_eq!(jailed["ok"], false, "{jailed}");
        assert!(
            jailed["error"].as_str().unwrap_or("").contains("jailed"),
            "{jailed}"
        );
        assert!(stolen.is_file(), "other-show source must stay");
        let _ = std::fs::remove_dir_all(&other);

        let picture = section(Some("picture"));
        assert_eq!(picture["card"]["locked"], 1);
        assert_eq!(picture["card"]["cards"][0]["num"], "01");
        assert_eq!(picture["card"]["cards"][0]["locked"], true);
        assert!(
            picture["card"]["cards"][0]["ref"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{picture}"
        );
        assert_no_lesson(&picture);

        lot_core::stage_place(
            "01",
            "Ada",
            Some("by the trunk"),
            Some("2"),
            Some("4"),
            None,
            None,
        )
        .unwrap();
        lot_core::stage_camera(
            "01",
            Some("WIDE"),
            Some("eye"),
            Some("35"),
            Some("dolly in"),
        )
        .unwrap();
        lot_core::stage_export().unwrap();
        let stage = section(Some("stage"));
        assert_eq!(stage["card"]["block"], true, "{stage}");
        assert!(
            stage["card"]["block_path"]
                .as_str()
                .is_some_and(|s| s.contains("block.json")),
            "{stage}"
        );
        assert!(stage["card"]["marks"].as_u64().unwrap_or(0) >= 1, "{stage}");
        assert_eq!(stage["card"]["cards"][0]["size"], "WIDE");
        assert_no_lesson(&stage);

        lot_core::motion_marks(
            "01",
            Some("dolly in"),
            Some("keep neon"),
            Some("camera_only"),
        )
        .unwrap();
        lot_core::motion_export().unwrap();
        let motion = section(Some("motion"));
        assert_eq!(motion["card"]["previs"], true, "{motion}");
        assert!(
            motion["card"]["previs_path"]
                .as_str()
                .is_some_and(|s| s.contains("previs.json")),
            "{motion}"
        );
        assert_eq!(motion["card"]["plates"], 0);
        assert_eq!(motion["card"]["cards"][0]["move"], "dolly in");
        assert_no_lesson(&motion);

        std::fs::create_dir_all(dir.join("board")).unwrap();
        std::fs::write(
            dir.join("board").join("board.json"),
            r#"{"show":"Carnival","shots":[{"num":"01","name":"INT. TENT - NIGHT","still":null}]}"#,
        )
        .unwrap();
        let board = section(Some("board"));
        assert_eq!(board["card"]["board"], true, "{board}");
        assert!(
            board["card"]["board_path"]
                .as_str()
                .is_some_and(|s| s.contains("board.json")),
            "{board}"
        );
        assert_eq!(board["card"]["stills"], 0);
        assert_eq!(board["card"]["cards"][0]["num"], "01");
        assert!(board["card"]["cards"][0]["still"].is_null(), "{board}");
        assert_no_lesson(&board);

        lot_core::slate_set("01", "wide tent, neon rain", None).unwrap();
        let slate = section(Some("slate"));
        assert_eq!(slate["card"]["prompts"], 1);
        assert_eq!(slate["card"]["cards"][0]["prompt"], "wide tent, neon rain");
        assert_no_lesson(&slate);

        let clip = dir.join("01-foo.mp4");
        std::fs::write(&clip, b"clip").unwrap();
        lot_core::dailies_ingest(Some(&clip), None).unwrap();
        lot_core::dailies_circle("tk-1").unwrap();
        let dailies = section(Some("dailies"));
        assert_eq!(dailies["card"]["takes"], 1);
        assert_eq!(dailies["card"]["circled"], 1);
        assert_eq!(dailies["card"]["cards"][0]["id"], "tk-1");
        assert!(
            dailies["card"]["cards"][0]["path"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{dailies}"
        );
        assert_no_lesson(&dailies);

        lot_core::stems_soundtrack(Some("bright organ, no lyrics"), None, false).unwrap();
        let stems = section(Some("stems"));
        assert_eq!(stems["card"]["brief"], "bright organ, no lyrics");
        assert!(
            stems["card"]["cue"].as_str().is_some_and(|s| !s.is_empty()),
            "{stems}"
        );
        assert!(stems["card"]["soundtrack"].is_null(), "{stems}");
        assert_no_lesson(&stems);

        let to_cut = handoff(true);
        assert_eq!(to_cut["ok"], true, "{to_cut}");
        assert_eq!(to_cut["phase"], "cut", "{to_cut}");
        assert_no_lesson(&to_cut);

        let cut = section(Some("cut"));
        assert_eq!(cut["card"]["circled"], 1);
        assert_eq!(cut["card"]["fcpxml"], false);
        assert!(cut["card"]["finish"].is_null(), "{cut}");
        assert_eq!(cut["handoff"]["missing"][0], "cut — no next");
        assert_no_lesson(&cut);

        let stuck = handoff(true);
        assert_eq!(stuck["ok"], false, "{stuck}");
        assert!(
            stuck["error"]
                .as_str()
                .unwrap_or("")
                .contains("cut — no next"),
            "{stuck}"
        );
        assert_no_lesson(&stuck);

        let st = status();
        assert_eq!(st["school"]["enabled"], false);
        assert_no_lesson(&st);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
