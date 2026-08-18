//! Show lock. One writer at a time. Second agent gets `locked_by`, not a silent clobber.

use crate::show::{append_event, bump, now_rfc3339, require_current, write_show, Show, ShowError};

pub enum LockCheck {
    Ok,
    Claim(String),
}

pub fn check(show: &Show) -> Result<LockCheck, ShowError> {
    let me = crate::agent::current();
    match (
        show.locked_by.as_deref().and_then(crate::agent::normalize),
        me,
    ) {
        (Some(holder), Some(id)) if crate::agent::same(&holder, &id) => Ok(LockCheck::Ok),
        (Some(holder), _) => Err(ShowError::Msg(format!(
            "locked_by: {holder} — did not write"
        ))),
        (None, Some(id)) => Ok(LockCheck::Claim(id)),
        (None, None) => Ok(LockCheck::Ok),
    }
}

pub fn lock_show() -> Result<(std::path::PathBuf, Show), ShowError> {
    crate::caps::require_write()?;
    let (dir, mut show) = require_current()?;
    let id = crate::agent::current().unwrap_or_else(|| "human".into());
    if let Some(holder) = show.locked_by.as_deref() {
        if crate::agent::same(holder, &id) {
            return Ok((dir, show));
        }
        return Err(ShowError::Msg(format!(
            "locked_by: {holder} — did not write"
        )));
    }
    show.locked_by = Some(id);
    show.locked_at = Some(now_rfc3339());
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "show.lock", &show)?;
    Ok((dir, show))
}

pub fn unlock_show(force: bool) -> Result<(std::path::PathBuf, Show), ShowError> {
    crate::caps::require_write()?;
    let (dir, mut show) = require_current()?;
    let Some(holder) = show.locked_by.clone() else {
        return Ok((dir, show));
    };
    let me = crate::agent::current();
    let mine = match me.as_deref() {
        Some(id) => crate::agent::same(id, &holder),
        None => holder.eq_ignore_ascii_case("human"),
    };
    if !mine && !force {
        return Err(ShowError::Msg(format!(
            "locked_by: {holder} — unlock needs the holder or --force"
        )));
    }
    show.locked_by = None;
    show.locked_at = None;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "show.unlock", &show)?;
    Ok((dir, show))
}
