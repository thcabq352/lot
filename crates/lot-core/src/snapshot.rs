//! Show snapshots. A later draft must not eat an earlier one.

use crate::show::{
    append_event_with, bump, require_current, write_show, Show, ShowError, SCREENPLAY_FILE,
};
use crate::SHOW_FILE;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

pub fn snapshot_show() -> Result<(PathBuf, Show, PathBuf, u64), ShowError> {
    let (dir, show) = crate::show::require_write_current()?;
    let dest = dir.join("snapshots").join(format!("rev-{}", show.rev));
    fs::create_dir_all(&dest)?;
    fs::copy(dir.join(SHOW_FILE), dest.join(SHOW_FILE))?;
    let fountain = dir.join(SCREENPLAY_FILE);
    if fountain.is_file() {
        fs::copy(&fountain, dest.join(SCREENPLAY_FILE))?;
    }
    let manifest = json!({
        "rev": show.rev,
        "show_id": show.id,
        "name": show.name,
        "phase": show.phase,
        "at": show.updated_at,
    });
    fs::write(
        dest.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    append_event_with(
        &dir,
        "show.snapshot",
        &show,
        Some(json!({ "rev": show.rev, "path": dest.display().to_string() })),
    )?;
    let rev = show.rev;
    Ok((dir, show, dest, rev))
}

pub fn snapshot_list() -> Result<(PathBuf, Show, Vec<u64>), ShowError> {
    let (dir, show) = require_current()?;
    let revs = list_revs(&dir);
    Ok((dir, show, revs))
}

pub fn restore_show(rev: u64) -> Result<(PathBuf, Show), ShowError> {
    let (dir, current) = crate::show::require_write_current()?;
    let src = dir.join("snapshots").join(format!("rev-{rev}"));
    let snap_json = src.join(SHOW_FILE);
    if !snap_json.is_file() {
        let have = list_revs(&dir);
        let listed = if have.is_empty() {
            "none".into()
        } else {
            have.iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Err(ShowError::Msg(format!(
            "no snapshot for rev {rev} (have: {listed})"
        )));
    }
    let raw = fs::read_to_string(&snap_json)?;
    let mut snap: Show = serde_json::from_str(&raw)?;
    if snap.schema != crate::SHOW_SCHEMA {
        return Err(ShowError::Schema(snap.schema));
    }
    if snap.id != current.id {
        return Err(ShowError::Msg(
            "snapshot show id does not match the current show".into(),
        ));
    }
    let fountain_src = src.join(SCREENPLAY_FILE);
    if fountain_src.is_file() {
        fs::copy(&fountain_src, dir.join(SCREENPLAY_FILE))?;
        snap.writer.draft_path = Some(dir.join(SCREENPLAY_FILE).display().to_string());
    }
    snap.locked_by = current.locked_by.clone();
    snap.locked_at = current.locked_at.clone();
    snap.budget = current.budget.clone();
    snap.rev = current.rev;
    bump(&mut snap);
    write_show(&dir, &snap)?;
    append_event_with(
        &dir,
        "show.restore",
        &snap,
        Some(json!({ "from_rev": rev, "now_rev": snap.rev })),
    )?;
    Ok((dir, snap))
}

fn list_revs(dir: &Path) -> Vec<u64> {
    let root = dir.join("snapshots");
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(&root) else {
        return out;
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let s = name.to_string_lossy();
        if let Some(n) = s.strip_prefix("rev-") {
            if let Ok(v) = n.parse::<u64>() {
                if ent.path().join(SHOW_FILE).is_file() {
                    out.push(v);
                }
            }
        }
    }
    out.sort_unstable();
    out
}
