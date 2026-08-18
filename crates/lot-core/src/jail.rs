//! Jail = this `show.lot` tree + declared media roots. No other-show paths.
//! Fountain / EXIF / web text are untrusted scene material (AC-013).

use crate::show::ShowError;
use crate::SHOW_FILE;
use std::path::{Path, PathBuf};

pub fn allow_source(path: &Path, show_dir: &Path) -> Result<PathBuf, ShowError> {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let show = canon(show_dir);
    if is_under(&abs, &show) {
        return Ok(abs);
    }
    let roots = media_roots();
    if !roots.is_empty() {
        if roots.iter().any(|r| is_under(&abs, r)) {
            if let Some(other) = enclosing_show(&abs) {
                if other != show {
                    return other_show(other);
                }
            }
            return Ok(abs);
        }
        return Err(ShowError::Msg(format!(
            "jailed — {} is outside the show and LOT_MEDIA_ROOTS",
            abs.display()
        )));
    }
    if let Some(other) = enclosing_show(&abs) {
        if other != show {
            return other_show(other);
        }
    }
    Ok(abs)
}

fn other_show(other: PathBuf) -> Result<PathBuf, ShowError> {
    Err(ShowError::Msg(format!(
        "jailed — other show: {}. Did not read it.",
        other.display()
    )))
}

fn media_roots() -> Vec<PathBuf> {
    let Ok(raw) = std::env::var("LOT_MEDIA_ROOTS") else {
        return Vec::new();
    };
    let sep = if cfg!(windows) { ';' } else { ':' };
    raw.split(sep)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn enclosing_show(path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_file() || path.extension().is_some() {
        path.parent().map(Path::to_path_buf)
    } else {
        Some(path.to_path_buf())
    };
    while let Some(d) = dir {
        if d.join(SHOW_FILE).is_file() {
            return Some(canon(&d));
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p)
        .unwrap_or_else(|_| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
}

fn is_under(child: &Path, root: &Path) -> bool {
    let c = canon(child);
    let r = canon(root);
    c.starts_with(&r)
}
