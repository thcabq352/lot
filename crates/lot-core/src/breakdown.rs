use crate::model::{shot_num_from_scene, Beat, Scene, Shot};
use crate::parse::{import_scriptbreak_json, parse_script};
use crate::show::{
    append_event_with, bump, require_write_current, write_show, Show, ShowError, SCREENPLAY_FILE,
};
use std::fs;
use std::path::Path;

pub fn breakdown_parse(file: Option<&Path>) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let (text, filename) = if let Some(p) = file {
        let p = crate::jail::allow_source(p, &dir)?;
        let raw = fs::read_to_string(&p)?;
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("script.txt")
            .to_string();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("scriptbreak") || e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
            && (raw.contains("\"scenes\"") || raw.contains("scriptbreak"))
        {
            let parsed = import_scriptbreak_json(&raw).map_err(|e| ShowError::Msg(e))?;
            apply_parsed(&dir, &mut show, parsed, Some(p.as_path()))?;
            return Ok((dir, show));
        }
        // Copy into the show; never delete the source.
        let dest = dir.join(SCREENPLAY_FILE);
        if p != dest.as_path() {
            fs::write(&dest, &raw)?;
            show.writer.draft_path = Some(dest.display().to_string());
        }
        (raw, name)
    } else {
        let path = dir.join(SCREENPLAY_FILE);
        if !path.is_file() {
            return Err(ShowError::Msg(
                "no screenplay — lot writer draft or lot breakdown import --file".into(),
            ));
        }
        (fs::read_to_string(&path)?, SCREENPLAY_FILE.to_string())
    };
    let parsed = parse_script(&text, Some(&filename));
    apply_parsed(&dir, &mut show, parsed, file)?;
    Ok((dir, show))
}

fn apply_parsed(
    dir: &Path,
    show: &mut Show,
    parsed: crate::parse::ParsedScript,
    source: Option<&Path>,
) -> Result<(), ShowError> {
    if parsed.scenes.is_empty() {
        return Err(ShowError::Msg(
            "breakdown found no scenes — need INT./EXT. sluglines".into(),
        ));
    }
    if show.name == "untitled" || show.name.is_empty() {
        if !parsed.title.is_empty() {
            show.name = parsed.title;
        }
    }
    show.scenes = parsed.scenes;
    show.shots = default_shots(&show.scenes);
    show.phase = "breakdown".into();
    bump(show);
    write_show(dir, show)?;
    append_event_with(
        dir,
        "breakdown.parse",
        show,
        Some(serde_json::json!({
            "scenes": show.scenes.len(),
            "shots": show.shots.len(),
            "source": source.map(|p| p.display().to_string()),
        })),
    )?;
    Ok(())
}

fn default_shots(scenes: &[Scene]) -> Vec<Shot> {
    scenes
        .iter()
        .enumerate()
        .map(|(i, sc)| {
            let num = shot_num_from_scene(&sc.num, i);
            Shot {
                id: format!("sh-{num}"),
                num: num.clone(),
                name: sc.slug.clone(),
                scene_id: sc.id.clone(),
                size: "WIDE".into(),
                desc: format!("Master — {}", sc.slug),
                ..Shot::default()
            }
        })
        .collect()
}

pub fn breakdown_summary(show: &Show) -> serde_json::Value {
    let mut chars = std::collections::BTreeSet::new();
    let mut locs = std::collections::BTreeSet::new();
    for sc in &show.scenes {
        for c in &sc.characters {
            chars.insert(c.clone());
        }
        if !sc.master.is_empty() {
            locs.insert(sc.master.clone());
        } else if !sc.location.is_empty() {
            locs.insert(sc.location.clone());
        }
    }
    serde_json::json!({
        "phase": show.phase,
        "scenes": show.scenes.len(),
        "shots": show.shots.len(),
        "characters": chars,
        "locations": locs,
        "scene_nums": show.scenes.iter().map(|s| &s.num).collect::<Vec<_>>(),
    })
}

pub fn wall_summary(show: &Show) -> serde_json::Value {
    serde_json::json!({
        "beats": show.wall.len(),
        "cards": show.wall,
    })
}

pub fn picture_summary(show: &Show) -> serde_json::Value {
    serde_json::json!({
        "shots": show.shots.len(),
        "locked": show.shots.iter().filter(|s| s.locked).count(),
        "refs": show.shots.iter().filter(|s| s.ref_path.is_some()).count(),
        "cards": show.shots.iter().map(|s| {
            serde_json::json!({
                "id": s.id,
                "num": s.num,
                "name": s.name,
                "locked": s.locked,
                "ref": s.ref_path,
                "size": s.size,
                "desc": s.desc,
            })
        }).collect::<Vec<_>>(),
    })
}

pub fn wall_add(act: Option<&str>, text: &str) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let text = text.trim();
    if text.is_empty() {
        return Err(ShowError::Msg("no beat —".into()));
    }
    let id = next_beat_id(&show.wall);
    show.wall.push(Beat {
        id: id.clone(),
        act: act.unwrap_or("").trim().to_string(),
        text: text.to_string(),
    });
    show.phase = "wall".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "wall.add",
        &show,
        Some(serde_json::json!({ "id": id })),
    )?;
    Ok((dir, show))
}

pub fn wall_update(
    id: &str,
    text: Option<&str>,
    act: Option<&str>,
) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let i = find_beat(&show.wall, id)?;
    let text = text.map(str::trim);
    if let Some(t) = text {
        if t.is_empty() {
            return Err(ShowError::Msg("no beat —".into()));
        }
    }
    let act = act.map(|a| a.trim().to_string());
    if text.is_none() && act.is_none() {
        return Err(ShowError::Msg("wall update needs --text or --act".into()));
    }
    let beat = &mut show.wall[i];
    if let Some(t) = text {
        beat.text = t.to_string();
    }
    if let Some(a) = act {
        beat.act = a;
    }
    let beat_id = beat.id.clone();
    show.phase = "wall".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "wall.update",
        &show,
        Some(serde_json::json!({ "id": beat_id })),
    )?;
    Ok((dir, show))
}

pub fn wall_remove(id: &str) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let i = find_beat(&show.wall, id)?;
    let beat = show.wall.remove(i);
    show.phase = "wall".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "wall.remove",
        &show,
        Some(serde_json::json!({ "id": beat.id })),
    )?;
    Ok((dir, show))
}

pub fn wall_reorder(
    id: &str,
    before: Option<&str>,
    after: Option<&str>,
    index: Option<usize>,
) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let from = find_beat(&show.wall, id)?;
    let named = [before.is_some(), after.is_some(), index.is_some()]
        .into_iter()
        .filter(|x| *x)
        .count();
    if named != 1 {
        return Err(ShowError::Msg(
            "wall reorder needs one of --before, --after, or --index".into(),
        ));
    }
    let dest = if let Some(before) = before {
        let t = find_beat(&show.wall, before)?;
        if from < t {
            t - 1
        } else {
            t
        }
    } else if let Some(after) = after {
        let t = find_beat(&show.wall, after)?;
        if from <= t {
            t
        } else {
            t + 1
        }
    } else {
        index.unwrap_or(0).min(show.wall.len().saturating_sub(1))
    };
    if from != dest {
        let beat = show.wall.remove(from);
        show.wall.insert(dest, beat);
    }
    let beat_id = show.wall[dest].id.clone();
    show.phase = "wall".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "wall.reorder",
        &show,
        Some(serde_json::json!({ "id": beat_id, "index": dest })),
    )?;
    Ok((dir, show))
}

pub fn picture_lock(shot_num: &str, locked: bool) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let name_before = show
        .shots
        .iter()
        .find(|s| crate::model::shot_nums_match(&s.num, shot_num))
        .map(|s| s.name.clone())
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| crate::model::shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    shot.locked = locked;
    debug_assert_eq!(shot.name, name_before);
    let num = shot.num.clone();
    if locked {
        show.phase = "picture".into();
    }
    bump(&mut show);
    write_show(&dir, &show)?;
    let kind = if locked {
        "picture.lock"
    } else {
        "picture.unlock"
    };
    append_event_with(&dir, kind, &show, Some(serde_json::json!({ "shot": num })))?;
    Ok((dir, show))
}

pub fn picture_unlock(shot_num: &str) -> Result<(std::path::PathBuf, Show), ShowError> {
    picture_lock(shot_num, false)
}

pub fn picture_ref(
    shot_num: &str,
    file: &Path,
    note: Option<&str>,
    size: Option<&str>,
) -> Result<(std::path::PathBuf, Show), ShowError> {
    if file.as_os_str().is_empty() {
        return Err(ShowError::Msg("no ref —".into()));
    }
    if !file.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", file.display())));
    }
    let (dir, mut show) = require_write_current()?;
    let file = crate::jail::allow_source(file, &dir)?;
    if !file.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", file.display())));
    }
    let shot_i = show
        .shots
        .iter()
        .position(|s| crate::model::shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let num = show.shots[shot_i].num.clone();
    let name_before = show.shots[shot_i].name.clone();
    fs::create_dir_all(dir.join("picture"))?;
    let fname = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("ref.bin");
    let dest = dir.join("picture").join(format!("{num}-{fname}"));
    if dest != file {
        fs::copy(&file, &dest)?;
    }
    if !file.is_file() {
        return Err(ShowError::Msg(format!(
            "source vanished — did not invent a ref: {}",
            file.display()
        )));
    }
    let path_s = dest.display().to_string();
    {
        let shot = &mut show.shots[shot_i];
        shot.ref_path = Some(path_s.clone());
        if let Some(n) = note.map(str::trim).filter(|s| !s.is_empty()) {
            shot.desc = n.to_string();
        }
        if let Some(sz) = size.map(str::trim).filter(|s| !s.is_empty()) {
            shot.size = sz.to_string();
        }
        debug_assert_eq!(shot.name, name_before);
    }
    if !show.media.iter().any(|m| m.path == path_s) {
        show.media.push(crate::model::MediaItem {
            path: path_s.clone(),
            kind: "ref".into(),
            ..crate::model::MediaItem::default()
        });
    }
    show.phase = "picture".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "picture.ref",
        &show,
        Some(serde_json::json!({ "shot": num, "ref": path_s })),
    )?;
    Ok((dir, show))
}

fn next_beat_id(wall: &[Beat]) -> String {
    let mut max = 0u32;
    for b in wall {
        if let Some(n) = b.id.strip_prefix("beat-").and_then(|s| s.parse().ok()) {
            max = max.max(n);
        }
    }
    format!("beat-{}", max + 1)
}

fn find_beat(wall: &[Beat], id: &str) -> Result<usize, ShowError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(ShowError::Msg("unknown beat: ".into()));
    }
    wall.iter()
        .position(|b| b.id == id || b.id == format!("beat-{id}"))
        .ok_or_else(|| ShowError::Msg(format!("unknown beat: {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::show::create_show;
    use std::path::{Path, PathBuf};

    fn isolate() {
        std::env::remove_var("LOT_SHOW");
        std::env::remove_var("LOT_CAP");
        std::env::remove_var("LOT_AGENT");
        std::env::remove_var("LOT_MEDIA_ROOTS");
        crate::clear_caps();
        crate::clear_agent();
        let tmp = std::env::temp_dir().join(format!(
            "lot-wall-{}-{}",
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

    fn show_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lot-wall-show-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn open_show(tag: &str) -> PathBuf {
        isolate();
        let dir = show_dir(tag);
        let _ = std::fs::remove_dir_all(&dir);
        create_show(&dir, Some("Carnival")).unwrap();
        dir
    }

    fn with_shots(dir: &Path) {
        fs::write(
            dir.join(SCREENPLAY_FILE),
            "INT. TENT - NIGHT\n\nADA\nDon't put it on.\n",
        )
        .unwrap();
        breakdown_parse(None).unwrap();
    }

    fn last_kind(dir: &Path) -> String {
        crate::audit::last_event(dir)
            .map(|e| e.kind)
            .unwrap_or_default()
    }

    #[test]
    fn wall_empty_text_is_honest() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = open_show("empty");
        let err = wall_add(None, "   ").unwrap_err().to_string();
        assert!(err.contains("no beat —"), "{err}");
        let show = crate::show::read_show(&dir).unwrap();
        assert!(show.wall.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wall_add_update_remove_reorder() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = open_show("beats");
        wall_add(Some("i"), "Ada waits by the trunk.").unwrap();
        wall_add(Some("ii"), "She does not put it on.").unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert_eq!(show.wall.len(), 2);
        assert_eq!(show.wall[0].id, "beat-1");
        assert_eq!(show.wall[1].id, "beat-2");
        assert_eq!(last_kind(&dir), "wall.add");

        wall_update("beat-1", Some("Ada waits in the rain."), None).unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert_eq!(show.wall[0].text, "Ada waits in the rain.");
        assert_eq!(show.wall[0].act, "i");
        assert_eq!(last_kind(&dir), "wall.update");

        let empty = wall_update("beat-1", Some("  "), None)
            .unwrap_err()
            .to_string();
        assert!(empty.contains("no beat —"), "{empty}");

        wall_reorder("beat-2", Some("beat-1"), None, None).unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert_eq!(show.wall[0].id, "beat-2");
        assert_eq!(show.wall[1].id, "beat-1");
        assert_eq!(last_kind(&dir), "wall.reorder");

        wall_remove("1").unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert_eq!(show.wall.len(), 1);
        assert_eq!(show.wall[0].id, "beat-2");
        assert_eq!(last_kind(&dir), "wall.remove");

        wall_add(Some("i"), "A later draft keeps the first beat.").unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert_eq!(show.wall[1].id, "beat-3");

        let unknown = wall_remove("beat-9").unwrap_err().to_string();
        assert!(unknown.contains("unknown beat"), "{unknown}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn picture_lock_unlock_ref_and_jail() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = open_show("picture");
        with_shots(&dir);
        let name_before = crate::show::read_show(&dir).unwrap().shots[0].name.clone();

        picture_lock("01", true).unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert!(show.shots[0].locked);
        assert_eq!(show.shots[0].name, name_before);
        assert_eq!(last_kind(&dir), "picture.lock");

        picture_unlock("01").unwrap();
        let show = crate::show::read_show(&dir).unwrap();
        assert!(!show.shots[0].locked);
        assert_eq!(show.shots[0].name, name_before);
        assert_eq!(last_kind(&dir), "picture.unlock");

        let missing = picture_ref("01", Path::new("no-such-ref.png"), None, None)
            .unwrap_err()
            .to_string();
        assert!(
            missing.contains("not a file") || missing.contains("no ref"),
            "{missing}"
        );

        let src = dir.join("media").join("tent.png");
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        fs::write(&src, b"png-bytes").unwrap();
        picture_ref("01", &src, Some("wide tent"), Some("WIDE")).unwrap();
        assert!(src.is_file(), "source must stay");
        let show = crate::show::read_show(&dir).unwrap();
        let dest = show.shots[0].ref_path.clone().expect("ref");
        assert!(Path::new(&dest).is_file(), "{dest}");
        assert!(dest.contains("picture"), "{dest}");
        assert_eq!(show.shots[0].desc, "wide tent");
        assert_eq!(show.shots[0].size, "WIDE");
        assert_eq!(show.shots[0].name, name_before);
        assert_eq!(last_kind(&dir), "picture.ref");

        let other = show_dir("other");
        let _ = std::fs::remove_dir_all(&other);
        create_show(&other, Some("Other")).unwrap();
        let stolen = other.join("secret.png");
        fs::write(&stolen, b"nope").unwrap();
        crate::show::open_show(&dir).unwrap();
        let jailed = picture_ref("01", &stolen, None, None)
            .unwrap_err()
            .to_string();
        assert!(jailed.contains("jailed"), "{jailed}");
        assert!(stolen.is_file(), "other-show source must stay");

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }
}
