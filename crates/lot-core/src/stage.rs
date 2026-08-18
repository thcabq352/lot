//! Stage: 2D floor marks + camera card. 3D blocking stays in Blockout.

use crate::model::{shot_nums_match, StageMark};
use crate::show::{append_event, append_event_with, bump, write_show, Show, ShowError};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub fn resolve_mark_kind(who: &str, raw: Option<&str>) -> String {
    if let Some(k) = raw.map(str::trim).filter(|s| !s.is_empty()) {
        let key = k.to_ascii_lowercase();
        return match key.as_str() {
            "actor" | "cast" | "talent" => "actor".into(),
            "camera" | "cam" => "camera".into(),
            "prop" | "object" | "vehicle" => "prop".into(),
            other => other.to_string(),
        };
    }
    let w = who.trim().to_ascii_lowercase();
    if matches!(w.as_str(), "camera" | "cam" | "lens") {
        "camera".into()
    } else {
        "actor".into()
    }
}

pub fn stage_place(
    shot_num: &str,
    who: &str,
    mark: Option<&str>,
    x: Option<&str>,
    z: Option<&str>,
    notes: Option<&str>,
    kind: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let who = who.trim();
    if who.is_empty() {
        return Err(ShowError::Msg("stage place needs --who".into()));
    }
    let mark = mark.unwrap_or("").trim();
    let x = x.unwrap_or("").trim();
    let z = z.unwrap_or("").trim();
    let notes = notes.unwrap_or("").trim();
    if mark.is_empty() && x.is_empty() && z.is_empty() && notes.is_empty() {
        return Err(ShowError::Msg(
            "stage place needs --mark, --x/--z, or --notes".into(),
        ));
    }
    let kind = resolve_mark_kind(who, kind);
    let (dir, mut show) = crate::show::require_write_current()?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let name_before = shot.name.clone();
    if let Some(existing) = shot
        .stage_marks
        .iter_mut()
        .find(|m| m.who.eq_ignore_ascii_case(who))
    {
        existing.kind = kind;
        if !mark.is_empty() {
            existing.mark = mark.to_string();
        }
        if !x.is_empty() {
            existing.x = x.to_string();
        }
        if !z.is_empty() {
            existing.z = z.to_string();
        }
        if !notes.is_empty() {
            existing.notes = notes.to_string();
        }
    } else {
        let n = shot.stage_marks.len() + 1;
        shot.stage_marks.push(StageMark {
            id: format!("mk-{n}"),
            who: who.to_string(),
            kind,
            mark: mark.to_string(),
            x: x.to_string(),
            z: z.to_string(),
            notes: notes.to_string(),
        });
    }
    debug_assert_eq!(shot.name, name_before);
    show.phase = "stage".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "stage.place", &show)?;
    Ok((dir, show))
}

pub fn stage_camera(
    shot_num: &str,
    size: Option<&str>,
    angle: Option<&str>,
    lens: Option<&str>,
    move_kind: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let size = size.map(str::trim).filter(|s| !s.is_empty());
    let angle = angle.map(str::trim).filter(|s| !s.is_empty());
    let lens = lens.map(str::trim).filter(|s| !s.is_empty());
    let move_kind = move_kind.map(str::trim).filter(|s| !s.is_empty());
    if size.is_none() && angle.is_none() && lens.is_none() && move_kind.is_none() {
        return Err(ShowError::Msg(
            "stage camera needs --size, --angle, --lens, or --move".into(),
        ));
    }
    let (dir, mut show) = crate::show::require_write_current()?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    if let Some(v) = size {
        shot.size = v.to_string();
    }
    if let Some(v) = angle {
        shot.angle = v.to_string();
    }
    if let Some(v) = lens {
        shot.lens = v.to_string();
    }
    if let Some(v) = move_kind {
        shot.move_kind = v.to_string();
    }
    show.phase = "stage".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "stage.camera", &show)?;
    Ok((dir, show))
}

pub fn stage_export() -> Result<(PathBuf, Show, PathBuf), ShowError> {
    crate::caps::require(crate::caps::Cap::Export)?;
    let (dir, mut show) = crate::show::require_write_current()?;
    if show.shots.is_empty() {
        return Err(ShowError::Msg(
            "stage export needs shots (breakdown parse, then stage place / camera)".into(),
        ));
    }
    fs::create_dir_all(dir.join("stage"))?;
    let studio = blockout_control();
    let mut rows = Vec::new();
    for shot in &show.shots {
        rows.push(json!({
            "num": shot.num,
            "name": shot.name,
            "size": shot.size,
            "angle": shot.angle,
            "lens": shot.lens,
            "move": shot.move_kind,
            "marks": shot.stage_marks,
            "prompt": shot.prompt,
        }));
    }
    let pack = json!({
        "show": show.name,
        "rev": show.rev,
        "engine": "lot-marks",
        "notice": "2D floor marks only. 3D grey-box blocking stays in Blockout (Sam Wasserman). Lot does not invent a glTF or depth pass.",
        "blockout": studio.as_ref().map(|p| json!({
            "ready": true,
            "control": p.display().to_string(),
        })),
        "shots": rows,
    });
    let json_path = dir.join("stage").join("block.json");
    fs::write(&json_path, serde_json::to_string_pretty(&pack)?)?;

    let mut md = format!("# Stage — {}\n\n", show.name);
    md.push_str("2D marks for generators. Not a Blockout 3D export.\n\n");
    for shot in &show.shots {
        md.push_str(&format!("## Shot {}\n\n", shot.num));
        if !shot.name.is_empty() {
            md.push_str(&format!("Name: {}\n\n", shot.name));
        }
        let cam = [
            (!shot.size.is_empty()).then(|| shot.size.as_str()),
            (!shot.angle.is_empty()).then(|| shot.angle.as_str()),
            (!shot.lens.is_empty()).then(|| shot.lens.as_str()),
            (!shot.move_kind.is_empty()).then(|| shot.move_kind.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        if !cam.is_empty() {
            md.push_str(&format!("Camera: {cam}\n\n"));
        }
        if shot.stage_marks.is_empty() {
            md.push_str("Marks: _(none)_\n\n");
        } else {
            md.push_str("Marks:\n\n");
            for m in &shot.stage_marks {
                md.push_str(&format!(
                    "- {} ({}) {} {},{} {}\n",
                    m.who, m.kind, m.mark, m.x, m.z, m.notes
                ));
            }
            md.push('\n');
        }
    }
    fs::write(dir.join("stage").join("prompt.md"), md)?;
    show.phase = "stage".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "stage.export",
        &show,
        Some(json!({ "export": json_path.display().to_string() })),
    )?;
    Ok((dir, show, json_path))
}

pub fn blockout_control() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LOT_BLOCKOUT_CONTROL") {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Some(pb);
        }
    }
    let mut cands = Vec::new();
    if let Ok(app) = std::env::var("APPDATA") {
        cands.push(PathBuf::from(app).join("blockout").join("control.json"));
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let h = PathBuf::from(home);
        cands.push(h.join(".config").join("blockout").join("control.json"));
    }
    cands.into_iter().find(|p| p.is_file())
}
