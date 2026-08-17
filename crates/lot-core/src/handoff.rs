//! Advance the show phase. Dry-run first. Commit only when the gate passes.

use crate::show::{
    append_event_with, bump, require_current, require_write_current, write_show, Show, ShowError,
    SCREENPLAY_FILE,
};
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};

pub const PHASES: &[&str] = &[
    "writer",
    "breakdown",
    "wall",
    "picture",
    "stage",
    "motion",
    "board",
    "slate",
    "dailies",
    "stems",
    "cut",
];

#[derive(Debug, Clone, Serialize)]
pub struct Handoff {
    pub phase: String,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    pub ready: bool,
    pub missing: Vec<String>,
    pub dry_run: bool,
    pub committed: bool,
}

pub fn inspect(dir: &Path, show: &Show) -> Handoff {
    let phase = normalize_phase(&show.phase);
    let next = next_phase(&phase);
    let missing = missing_for(&phase, dir, show);
    let ready = missing.is_empty() && next.is_some();
    Handoff {
        phase: phase.clone(),
        from: phase,
        next,
        ready,
        missing,
        dry_run: true,
        committed: false,
    }
}

pub fn handoff(commit: bool) -> Result<(PathBuf, Show, Handoff), ShowError> {
    if commit {
        crate::caps::require_write()?;
        let (dir, mut show) = require_write_current()?;
        let mut report = inspect(&dir, &show);
        if let Some(msg) = blocked_msg(&report) {
            return Err(ShowError::Msg(msg));
        }
        let to = report.next.clone().expect("ready implies next");
        show.phase = to.clone();
        bump(&mut show);
        write_show(&dir, &show)?;
        append_event_with(
            &dir,
            "show.handoff",
            &show,
            Some(json!({ "from": report.phase, "to": to })),
        )?;
        report.dry_run = false;
        report.committed = true;
        report.from = report.phase.clone();
        report.phase = show.phase.clone();
        report.next = next_phase(&show.phase);
        report.missing = missing_for(&show.phase, &dir, &show);
        report.ready = report.missing.is_empty() && report.next.is_some();
        Ok((dir, show, report))
    } else {
        crate::caps::require(crate::caps::Cap::Read)?;
        let (dir, show) = require_current()?;
        Ok((dir.clone(), show.clone(), inspect(&dir, &show)))
    }
}

fn blocked_msg(report: &Handoff) -> Option<String> {
    if report.next.is_none() {
        return Some("cut — no next".into());
    }
    if report.ready {
        return None;
    }
    Some(format!("handoff blocked — {}", report.missing.join("; ")))
}

fn normalize_phase(raw: &str) -> String {
    let t = raw.trim().to_ascii_lowercase();
    if PHASES.contains(&t.as_str()) {
        t
    } else if t.is_empty() {
        "writer".into()
    } else {
        t
    }
}

fn next_phase(phase: &str) -> Option<String> {
    let i = PHASES.iter().position(|p| *p == phase)?;
    PHASES.get(i + 1).map(|s| (*s).to_string())
}

/// Current-phase blockers. Same strings as `lot handoff` dry-run.
pub fn phase_missing(dir: &Path, show: &Show) -> Vec<String> {
    missing_for(&normalize_phase(&show.phase), dir, show)
}

/// Sections that already have work on disk or in `show.json`.
pub fn dirty_sections(dir: &Path, show: &Show) -> Vec<String> {
    PHASES
        .iter()
        .filter(|p| section_touched(p, dir, show))
        .map(|s| (*s).to_string())
        .collect()
}

fn section_touched(phase: &str, dir: &Path, show: &Show) -> bool {
    match phase {
        "writer" => {
            !show.writer.brief.trim().is_empty()
                || has_draft(dir, show)
                || !show.writer.cast.is_empty()
                || show.writer.locked
        }
        "breakdown" => !show.scenes.is_empty() || !show.shots.is_empty(),
        "wall" => !show.wall.is_empty(),
        "picture" => show.shots.iter().any(|s| s.locked),
        "stage" => {
            show.shots.iter().any(|s| {
                !s.stage_marks.is_empty()
                    || !s.size.is_empty()
                    || !s.angle.is_empty()
                    || !s.lens.is_empty()
                    || !s.move_kind.is_empty()
            }) || dir.join("stage").join("block.json").is_file()
        }
        "motion" => {
            show.shots.iter().any(|s| {
                s.plate_path.is_some() || !s.motion_move.is_empty() || !s.motion_notes.is_empty()
            }) || dir.join("motion").join("previs.json").is_file()
        }
        "board" => {
            show.shots.iter().any(|s| s.still_path.is_some())
                || dir.join("board").join("board.json").is_file()
        }
        "slate" => show.shots.iter().any(|s| !s.prompt.trim().is_empty()),
        "dailies" => !show.takes.is_empty(),
        "stems" => {
            let s = &show.stems;
            !s.soundtrack_brief.trim().is_empty()
                || s.soundtrack_cue.is_some()
                || s.soundtrack_path.is_some()
                || !s.vo_text.trim().is_empty()
                || s.vo_path.is_some()
        }
        "cut" => show.finish.path.is_some() || dir.join("export.fcpxml").is_file(),
        _ => false,
    }
}

/// Referenced media that is not a file. Does not invent a still or take.
pub fn missing_media(dir: &Path, show: &Show) -> Vec<crate::MediaGap> {
    let mut out = Vec::new();
    if let Some(p) = &show.writer.draft_path {
        push_gap(&mut out, "draft", p, None, None);
    }
    for shot in &show.shots {
        if let Some(p) = &shot.still_path {
            push_gap(&mut out, "still", p, Some(&shot.num), Some(&shot.id));
        }
        if let Some(p) = &shot.plate_path {
            push_gap(&mut out, "plate", p, Some(&shot.num), Some(&shot.id));
        }
    }
    for take in &show.takes {
        push_gap(&mut out, "take", &take.path, None, Some(&take.id));
    }
    if let Some(p) = &show.stems.soundtrack_path {
        push_gap(&mut out, "soundtrack", p, None, None);
    }
    if let Some(p) = &show.stems.vo_path {
        push_gap(&mut out, "vo", p, None, None);
    }
    if let Some(p) = &show.finish.path {
        push_gap(&mut out, "finish", p, None, None);
    }
    for item in &show.media {
        if out.iter().any(|g| g.path == item.path) {
            continue;
        }
        push_gap(&mut out, &item.kind, &item.path, None, None);
    }
    let _ = dir;
    out
}

fn push_gap(
    out: &mut Vec<crate::MediaGap>,
    kind: &str,
    path: &str,
    shot: Option<&str>,
    id: Option<&str>,
) {
    if path.trim().is_empty() || Path::new(path).is_file() {
        return;
    }
    out.push(crate::MediaGap {
        kind: kind.into(),
        path: path.to_string(),
        shot: shot.filter(|s| !s.is_empty()).map(|s| s.to_string()),
        id: id.filter(|s| !s.is_empty()).map(|s| s.to_string()),
    });
}

fn missing_for(phase: &str, dir: &Path, show: &Show) -> Vec<String> {
    match phase {
        "writer" => writer_missing(dir, show),
        "breakdown" => breakdown_missing(show),
        "wall" => wall_missing(show),
        "picture" => picture_missing(show),
        "stage" => stage_missing(dir, show),
        "motion" => motion_missing(dir, show),
        "board" => board_missing(dir, show),
        "slate" => slate_missing(show),
        "dailies" => dailies_missing(show),
        "stems" => stems_missing(show),
        "cut" => vec!["cut — no next".into()],
        other => vec![format!("unknown phase — {other}")],
    }
}

fn writer_missing(dir: &Path, show: &Show) -> Vec<String> {
    let mut m = Vec::new();
    if show.writer.brief.trim().is_empty() {
        m.push("no brief".into());
    }
    if !has_draft(dir, show) {
        m.push("no draft — lot writer draft".into());
    }
    m
}

fn has_draft(dir: &Path, show: &Show) -> bool {
    if let Some(p) = &show.writer.draft_path {
        if Path::new(p).is_file() {
            return true;
        }
    }
    dir.join(SCREENPLAY_FILE).is_file()
}

fn breakdown_missing(show: &Show) -> Vec<String> {
    let mut m = Vec::new();
    if show.scenes.is_empty() {
        m.push("no scenes — lot breakdown parse".into());
    }
    if show.shots.is_empty() {
        m.push("no shots — lot breakdown parse".into());
    }
    m
}

fn wall_missing(show: &Show) -> Vec<String> {
    if show.wall.is_empty() {
        vec!["no beat — lot wall add --text".into()]
    } else {
        Vec::new()
    }
}

fn picture_missing(show: &Show) -> Vec<String> {
    if show.shots.iter().any(|s| s.locked) {
        Vec::new()
    } else {
        vec!["no shot locked — lot picture lock --shot".into()]
    }
}

fn stage_missing(dir: &Path, show: &Show) -> Vec<String> {
    let marked = show.shots.iter().any(|s| {
        !s.stage_marks.is_empty()
            || !s.size.is_empty()
            || !s.angle.is_empty()
            || !s.lens.is_empty()
            || !s.move_kind.is_empty()
    });
    if marked || dir.join("stage").join("block.json").is_file() {
        Vec::new()
    } else {
        vec!["no stage mark or camera — lot stage place / lot stage camera".into()]
    }
}

fn motion_missing(dir: &Path, show: &Show) -> Vec<String> {
    let marked = show
        .shots
        .iter()
        .any(|s| s.plate_path.is_some() || !s.motion_move.is_empty() || !s.motion_notes.is_empty());
    if marked || dir.join("motion").join("previs.json").is_file() {
        Vec::new()
    } else {
        vec!["no plate or marks — lot motion plate / lot motion marks".into()]
    }
}

fn board_missing(dir: &Path, show: &Show) -> Vec<String> {
    if show.shots.iter().any(|s| s.still_path.is_some())
        || dir.join("board").join("board.json").is_file()
    {
        Vec::new()
    } else {
        vec!["no still — lot stills generate --shot --backend".into()]
    }
}

fn slate_missing(show: &Show) -> Vec<String> {
    if show.shots.iter().any(|s| !s.prompt.trim().is_empty()) {
        Vec::new()
    } else {
        vec!["no slate canon — lot slate set --shot --prompt".into()]
    }
}

fn dailies_missing(show: &Show) -> Vec<String> {
    if show.takes.iter().any(|t| t.circled) {
        Vec::new()
    } else {
        vec!["no circled take — lot dailies circle --take".into()]
    }
}

fn stems_missing(show: &Show) -> Vec<String> {
    let s = &show.stems;
    if !s.soundtrack_brief.trim().is_empty()
        || s.soundtrack_cue.is_some()
        || s.soundtrack_path.is_some()
        || !s.vo_text.trim().is_empty()
        || s.vo_path.is_some()
    {
        Vec::new()
    } else {
        vec!["no soundtrack or vo — lot stems soundtrack / lot stems vo".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_order() {
        assert_eq!(next_phase("writer").as_deref(), Some("breakdown"));
        assert_eq!(next_phase("stems").as_deref(), Some("cut"));
        assert_eq!(next_phase("cut"), None);
        assert_eq!(next_phase("nope"), None);
    }
}
