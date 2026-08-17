//! Undo the last mutation from the event log. Does not need a prior snapshot.

use crate::show::{append_event_with, bump, write_show, Show, ShowError, SCREENPLAY_FILE};
use crate::SHOW_FILE;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub fn undo_show() -> Result<(PathBuf, Show, String), ShowError> {
    crate::caps::require_write()?;
    let (dir, current) = crate::show::require_write_current()?;
    let Some(ev) = last_undoable(&dir) else {
        return Err(ShowError::Msg("nothing to undo —".into()));
    };
    if ev.kind == "create" || ev.kind == "show.undo" || ev.rev <= 1 {
        return Err(ShowError::Msg("nothing to undo —".into()));
    }
    let prev = ev.rev.saturating_sub(1);
    let src = dir.join("journal").join(format!("rev-{prev}"));
    let snap_json = src.join(SHOW_FILE);
    if !snap_json.is_file() {
        return Err(ShowError::Msg(format!(
            "nothing to undo — no journal for rev {prev}"
        )));
    }
    let raw = fs::read_to_string(&snap_json)?;
    let mut snap: Show = serde_json::from_str(&raw)?;
    if snap.schema != crate::SHOW_SCHEMA {
        return Err(ShowError::Schema(snap.schema));
    }
    if snap.id != current.id {
        return Err(ShowError::Msg(
            "journal show id does not match the current show".into(),
        ));
    }
    let fountain_src = src.join(SCREENPLAY_FILE);
    let fountain_live = dir.join(SCREENPLAY_FILE);
    if fountain_src.is_file() {
        fs::copy(&fountain_src, &fountain_live)?;
        snap.writer.draft_path = Some(fountain_live.display().to_string());
    } else if fountain_live.is_file() {
        fs::remove_file(&fountain_live)?;
        snap.writer.draft_path = None;
    }
    snap.locked_by = current.locked_by.clone();
    snap.locked_at = current.locked_at.clone();
    snap.budget = current.budget.clone();
    snap.rev = current.rev;
    bump(&mut snap);
    write_show(&dir, &snap)?;
    append_event_with(
        &dir,
        "show.undo",
        &snap,
        Some(json!({
            "undid": ev.id,
            "undid_kind": ev.kind,
            "from_rev": ev.rev,
        })),
    )?;
    Ok((dir, snap, ev.id))
}

fn last_undoable(dir: &Path) -> Option<crate::EventMeta> {
    let evs = crate::audit::list_events(dir, None);
    evs.into_iter().rev().find(|e| e.kind != "show.snapshot")
}
