//! Motion Previs kernel: plates → marks → export.
//! Pose / depth / OpenPose stay in Motion Previs Studio. Never a fake bundle.

use crate::model::{shot_nums_match, MediaItem};
use crate::show::{append_event, append_event_with, bump, write_show, Show, ShowError};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn resolve_motion_mode(raw: Option<&str>) -> Result<Option<String>, ShowError> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let key = raw.to_ascii_lowercase().replace('-', "_");
    let id = match key.as_str() {
        "camera_only" | "camera" | "cam" => "camera_only",
        "actor_motion" | "actor" | "performance" | "pose" => "actor_motion",
        "object_motion" | "object" | "vehicle" => "object_motion",
        "full_scene" | "full" | "scene" => "full_scene",
        _ => {
            return Err(ShowError::Msg(format!(
                "unknown motion mode: {raw} (want camera_only | actor_motion | object_motion | full_scene)"
            )))
        }
    };
    Ok(Some(id.into()))
}

pub fn motion_plate(
    file: &Path,
    shot_num: &str,
    mode: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    if !file.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", file.display())));
    }
    let mode = resolve_motion_mode(mode)?;
    let (dir, mut show) = crate::show::require_write_current()?;
    let file = crate::jail::allow_source(file, &dir)?;
    let shot_i = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let num = show.shots[shot_i].num.clone();
    let name_before = show.shots[shot_i].name.clone();
    fs::create_dir_all(dir.join("motion"))?;
    let fname = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("plate.mp4");
    let dest = dir.join("motion").join(format!("{num}-{fname}"));
    fs::copy(file, &dest)?;
    let probe = probe_plate(&dest);
    let path_s = dest.display().to_string();
    {
        let shot = &mut show.shots[shot_i];
        shot.plate_path = Some(path_s.clone());
        if let Some(m) = mode {
            shot.motion_mode = Some(m);
        }
        shot.motion_duration = probe.0.clone();
        shot.motion_fps = probe.1.clone();
        debug_assert_eq!(shot.name, name_before);
    }
    show.media.push(MediaItem {
        path: path_s.clone(),
        kind: "plate".into(),
        duration_secs: probe.0.as_deref().and_then(|s| s.parse().ok()),
        ..MediaItem::default()
    });
    show.phase = "motion".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "motion.plate",
        &show,
        Some(json!({ "shot": num, "plate": path_s })),
    )?;
    Ok((dir, show))
}

pub fn motion_marks(
    shot_num: &str,
    move_kind: Option<&str>,
    notes: Option<&str>,
    mode: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let mode = resolve_motion_mode(mode)?;
    let move_kind = move_kind.map(str::trim).filter(|s| !s.is_empty());
    let notes = notes.map(str::trim).filter(|s| !s.is_empty());
    if mode.is_none() && move_kind.is_none() && notes.is_none() {
        return Err(ShowError::Msg(
            "motion marks need --move, --notes, or --mode".into(),
        ));
    }
    let (dir, mut show) = crate::show::require_write_current()?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    if let Some(m) = mode {
        shot.motion_mode = Some(m);
    }
    if let Some(mv) = move_kind {
        shot.motion_move = mv.to_string();
        shot.move_kind = mv.to_string();
    }
    if let Some(n) = notes {
        shot.motion_notes = n.to_string();
    }
    show.phase = "motion".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "motion.marks", &show)?;
    Ok((dir, show))
}

pub fn motion_export() -> Result<(PathBuf, Show, PathBuf), ShowError> {
    crate::caps::require(crate::caps::Cap::Export)?;
    let (dir, mut show) = crate::show::require_write_current()?;
    if show.shots.is_empty() {
        return Err(ShowError::Msg(
            "motion export needs shots (breakdown parse, then motion plate / marks)".into(),
        ));
    }
    fs::create_dir_all(dir.join("motion"))?;
    let studio = motion_previs_control();
    let mut rows = Vec::new();
    for shot in &show.shots {
        rows.push(json!({
            "num": shot.num,
            "name": shot.name,
            "plate": shot.plate_path,
            "mode": shot.motion_mode,
            "move": shot.motion_move,
            "notes": shot.motion_notes,
            "duration": shot.motion_duration,
            "fps": shot.motion_fps,
            "prompt": shot.prompt,
            "prompt_targets": shot.prompt_targets,
        }));
    }
    let pack = json!({
        "show": show.name,
        "rev": show.rev,
        "engine": "lot-marks",
        "notice": "Plates and marks only. Pose / depth / OpenPose stay in Motion Previs Studio (Sam Wasserman). Lot does not invent a control bundle.",
        "studio": studio.as_ref().map(|p| json!({
            "ready": true,
            "control": p.display().to_string(),
        })),
        "shots": rows,
    });
    let json_path = dir.join("motion").join("previs.json");
    fs::write(&json_path, serde_json::to_string_pretty(&pack)?)?;

    let mut md = format!("# Motion Previs — {}\n\n", show.name);
    md.push_str("Marks for generators. Not a MediaPipe / OpenPose export.\n\n");
    for shot in &show.shots {
        md.push_str(&format!("## Shot {}\n\n", shot.num));
        if !shot.name.is_empty() {
            md.push_str(&format!("Name: {}\n\n", shot.name));
        }
        if let Some(m) = shot.motion_mode.as_deref() {
            md.push_str(&format!("Mode: `{m}`\n\n"));
        }
        if !shot.motion_move.is_empty() {
            md.push_str(&format!("Move: {}\n\n", shot.motion_move));
        }
        if !shot.motion_notes.is_empty() {
            md.push_str(&format!("Notes: {}\n\n", shot.motion_notes));
        }
        match shot.plate_path.as_deref() {
            Some(p) => md.push_str(&format!("Plate: `{p}`\n\n")),
            None => md.push_str("Plate: _(none)_\n\n"),
        }
        if !shot.prompt.is_empty() {
            md.push_str(&format!("Slate canon:\n\n{}\n\n", shot.prompt));
        }
    }
    fs::write(dir.join("motion").join("prompt.md"), md)?;
    show.phase = "motion".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "motion.export", &show)?;
    Ok((dir, show, json_path))
}

pub fn motion_analyze(shot_num: &str) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = crate::show::require_write_current()?;
    let shot_i = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let plate = show.shots[shot_i].plate_path.clone().ok_or_else(|| {
        ShowError::Msg(format!(
            "motion analyze needs a plate — lot motion plate --file --shot {shot_num}"
        ))
    })?;
    let plate_p = PathBuf::from(&plate);
    if !plate_p.is_file() {
        return Err(ShowError::Msg(format!("plate missing: {plate}")));
    }

    if let Some(cmd) = std::env::var("LOT_MOTION_CMD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        crate::caps::require(crate::caps::Cap::Render)?;
        let out = dir
            .join("motion")
            .join(format!("{}-engine", show.shots[shot_i].num));
        fs::create_dir_all(&out)?;
        let status = Command::new(&cmd)
            .arg(&plate_p)
            .arg(&out)
            .status()
            .map_err(|e| ShowError::Msg(format!("motion engine: {e}")))?;
        let wrote = out
            .read_dir()
            .map(|it| it.filter_map(|e| e.ok()).any(|e| e.path().is_file()))
            .unwrap_or(false);
        if !status.success() || !wrote {
            return Err(ShowError::Msg(
                "no motion engine — LOT_MOTION_CMD ran but wrote no bundle. Did not invent pose/depth."
                    .into(),
            ));
        }
        show.phase = "motion".into();
        bump(&mut show);
        write_show(&dir, &show)?;
        append_event_with(
            &dir,
            "motion.analyze",
            &show,
            Some(json!({
                "shot": show.shots[shot_i].num,
                "engine": "lot_motion_cmd",
                "out": out.display().to_string(),
            })),
        )?;
        return Ok((dir, show));
    }

    let probe = probe_plate(&plate_p);
    show.shots[shot_i].motion_duration = probe.0;
    show.shots[shot_i].motion_fps = probe.1;
    let studio = motion_previs_control();
    show.phase = "motion".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "motion.analyze",
        &show,
        Some(json!({
            "shot": show.shots[shot_i].num,
            "engine": "lot-marks",
            "studio": studio.as_ref().map(|p| p.display().to_string()),
            "note": if studio.is_some() {
                "Motion Previs Studio is running. Use its MCP for pose/depth. Lot stored plate + marks only."
            } else {
                "No LOT_MOTION_CMD and no Motion Previs Studio control.json. Marks / ffprobe only — no fake OpenPose."
            },
        })),
    )?;
    let _ = motion_export();
    Ok((dir, show))
}

pub fn motion_previs_control() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LOT_MOTION_CONTROL") {
        let pb = PathBuf::from(p.trim());
        if pb.is_file() {
            return Some(pb);
        }
    }
    let mut cands = Vec::new();
    if let Ok(app) = std::env::var("APPDATA") {
        cands.push(
            PathBuf::from(app)
                .join("Motion Previs Studio")
                .join("v4")
                .join("control.json"),
        );
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let h = PathBuf::from(home);
        cands.push(h.join(".config").join("motion-previs").join("control.json"));
    }
    cands.into_iter().find(|p| p.is_file())
}

fn probe_plate(file: &Path) -> (Option<String>, Option<String>) {
    let dur = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(file)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });
    let fps = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(file)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        });
    (dur, fps)
}
