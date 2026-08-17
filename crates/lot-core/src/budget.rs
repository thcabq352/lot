//! Per-show spend / render budget. Unset cap = unlimited. Hit cap → stop.

use crate::show::{
    append_event, bump, require_write_current, write_show, Show, ShowError,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_cap: Option<u32>,
    #[serde(default)]
    pub spend_used: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_cap: Option<u32>,
    #[serde(default)]
    pub render_used: u32,
}

pub fn require_spend(show: &Show) -> Result<(), ShowError> {
    if let Some(cap) = show.budget.spend_cap {
        if show.budget.spend_used >= cap {
            return Err(ShowError::Msg(format!(
                "show spend cap — {}/{cap}. Did not call Grok.",
                show.budget.spend_used
            )));
        }
    }
    Ok(())
}

pub fn require_render(show: &Show) -> Result<(), ShowError> {
    if let Some(cap) = show.budget.render_cap {
        if show.budget.render_used >= cap {
            return Err(ShowError::Msg(format!(
                "show render cap — {}/{cap}. Did not call Comfy.",
                show.budget.render_used
            )));
        }
    }
    Ok(())
}

pub fn record_spend(show: &mut Show) {
    show.budget.spend_used = show.budget.spend_used.saturating_add(1);
}

pub fn record_render(show: &mut Show) {
    show.budget.render_used = show.budget.render_used.saturating_add(1);
}

pub fn set_budget(
    spend: Option<u32>,
    render: Option<u32>,
    clear_spend: bool,
    clear_render: bool,
) -> Result<(std::path::PathBuf, Show), ShowError> {
    if spend.is_none() && render.is_none() && !clear_spend && !clear_render {
        return Err(ShowError::Msg(
            "budget needs --spend N, --render N, or --clear-spend / --clear-render".into(),
        ));
    }
    let (dir, mut show) = require_write_current()?;
    if clear_spend {
        show.budget.spend_cap = None;
    } else if let Some(n) = spend {
        show.budget.spend_cap = Some(n);
    }
    if clear_render {
        show.budget.render_cap = None;
    } else if let Some(n) = render {
        show.budget.render_cap = Some(n);
    }
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "show.budget", &show)?;
    Ok((dir, show))
}
