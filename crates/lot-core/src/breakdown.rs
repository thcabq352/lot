use crate::model::{shot_num_from_scene, Beat, Scene, Shot};
use crate::parse::{import_scriptbreak_json, parse_script};
use crate::show::{
    append_event, append_event_with, bump, require_current, write_show, Show, ShowError,
    SCREENPLAY_FILE,
};
use std::fs;
use std::path::Path;

pub fn breakdown_parse(file: Option<&Path>) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_current()?;
    let (text, filename) = if let Some(p) = file {
        let raw = fs::read_to_string(p)?;
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
            apply_parsed(&dir, &mut show, parsed, Some(p))?;
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

pub fn wall_add(act: Option<&str>, text: &str) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_current()?;
    let text = text.trim();
    if text.is_empty() {
        return Err(ShowError::Msg("wall needs --text".into()));
    }
    let n = show.wall.len() + 1;
    show.wall.push(Beat {
        id: format!("beat-{n}"),
        act: act.unwrap_or("").trim().to_string(),
        text: text.to_string(),
    });
    show.phase = "wall".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "wall.add", &show)?;
    Ok((dir, show))
}

pub fn picture_lock(shot_num: &str, locked: bool) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_current()?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| crate::model::shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    shot.locked = locked;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "picture.lock", &show)?;
    Ok((dir, show))
}

pub fn slate_set(shot_num: &str, prompt: &str) -> Result<(std::path::PathBuf, Show), ShowError> {
    let (dir, mut show) = require_current()?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| crate::model::shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    shot.prompt = prompt.trim().to_string();
    show.phase = "slate".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "slate.set", &show)?;
    Ok((dir, show))
}
