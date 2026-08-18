//! Slate: continuity canon + per-target compile + LoRAs.
//! Targets do not replace the canon. No invented rewrite if the brain / prompt server is down.

use crate::brain::complete_chat;
use crate::model::{shot_nums_match, SlateLora};
use crate::packs::{self, lookup};
use crate::show::{append_event, append_event_with, bump, write_show, Show, ShowError};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

const COMPILE_SYSTEM: &str = "You are Lot Slate. Rewrite the CANON prompt for one TARGET only. \
Keep continuity facts. Do not invent a new scene, location, or cast. \
No markdown fences. No [Shot N] wrappers unless the target notes ask. \
Director names are coverage influence only — not endorsement. \
Output the rewritten prompt only (or JSON {\"prompt\":\"...\"} if you must).";

pub fn slate_set(
    shot_num: &str,
    prompt: &str,
    target: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let text = prompt.trim();
    if text.is_empty() {
        return Err(ShowError::Msg("slate needs --prompt".into()));
    }
    let target = match target.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => Some(packs::resolve_prompt_target(raw)?),
        None => None,
    };
    let (dir, mut show) = crate::show::require_write_current()?;
    let shot = show
        .shots
        .iter_mut()
        .find(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    match target {
        Some(id) => {
            if shot.prompt.trim().is_empty() {
                shot.prompt = text.to_string();
            }
            shot.prompt_targets.insert(id, text.to_string());
        }
        None => {
            shot.prompt = text.to_string();
        }
    }
    show.phase = "slate".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "slate.set", &show)?;
    Ok((dir, show))
}

pub fn slate_target(id: &str) -> Result<(PathBuf, Show), ShowError> {
    let id = packs::resolve_prompt_target(id)?;
    let (dir, mut show) = crate::show::require_write_current()?;
    show.slate.default_target = Some(id);
    show.phase = "slate".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "slate.target", &show)?;
    Ok((dir, show))
}

pub fn slate_lora(
    shot_num: Option<&str>,
    id: &str,
    weight: Option<&str>,
    model: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(ShowError::Msg("slate lora needs --id".into()));
    }
    let lora = SlateLora {
        id: id.to_string(),
        weight: weight
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("1.0")
            .to_string(),
        model: model.unwrap_or("").trim().to_string(),
    };
    let (dir, mut show) = crate::show::require_write_current()?;
    if let Some(num) = shot_num.map(str::trim).filter(|s| !s.is_empty()) {
        let shot = show
            .shots
            .iter_mut()
            .find(|s| shot_nums_match(&s.num, num))
            .ok_or_else(|| ShowError::Msg(format!("unknown shot: {num}")))?;
        upsert_lora(&mut shot.loras, lora);
    } else {
        upsert_lora(&mut show.slate.loras, lora);
    }
    show.phase = "slate".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "slate.lora", &show)?;
    Ok((dir, show))
}

fn upsert_lora(list: &mut Vec<SlateLora>, lora: SlateLora) {
    if let Some(existing) = list.iter_mut().find(|l| l.id == lora.id) {
        *existing = lora;
    } else {
        list.push(lora);
    }
}

pub fn slate_compile(shot_num: &str, target: Option<&str>) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = crate::show::require_write_current()?;
    let target = match target.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => packs::resolve_prompt_target(raw)?,
        None => show.slate.default_target.clone().ok_or_else(|| {
            ShowError::Msg("slate compile needs --target (or lot slate target --id first)".into())
        })?,
    };
    let shot_i = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let canon = show.shots[shot_i].prompt.trim().to_string();
    if canon.is_empty() {
        return Err(ShowError::Msg(
            "slate compile needs a canon prompt — lot slate set --shot --prompt".into(),
        ));
    }
    let item = lookup(packs::prompt_targets(), &target)
        .ok_or_else(|| ShowError::Msg(format!("unknown prompt target: {target}")))?;
    let notes = item.notes.clone().unwrap_or_default();
    let kind = item.kind.clone().unwrap_or_default();
    let label = item.display_name().to_string();
    let loras = merge_loras(&show.slate.loras, &show.shots[shot_i].loras);
    let motion = motion_line(&show.shots[shot_i]);

    let (rewrite, mut prov) = if target == "prompt-server" || kind == "http" {
        rewrite_via_server(&canon, &target, &show.shots[shot_i].num, &loras)?
    } else {
        rewrite_via_brain(&show, &canon, &target, &label, &notes, &loras, &motion)?
    };
    prov = prov.with_prompt(&canon);

    show.shots[shot_i]
        .prompt_targets
        .insert(target.clone(), rewrite);
    show.slate.default_target = Some(target.clone());
    show.phase = "slate".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "slate.compile",
        &show,
        Some(json!({
            "shot": show.shots[shot_i].num,
            "target": target,
            "provenance": prov,
        })),
    )?;
    Ok((dir, show))
}

fn merge_loras(show: &[SlateLora], shot: &[SlateLora]) -> Vec<SlateLora> {
    let mut out = show.to_vec();
    for l in shot {
        upsert_lora(&mut out, l.clone());
    }
    out
}

fn motion_line(shot: &crate::model::Shot) -> String {
    let mut bits = Vec::new();
    if let Some(m) = shot.motion_mode.as_deref() {
        bits.push(format!("mode={m}"));
    }
    if !shot.motion_move.is_empty() {
        bits.push(format!("move={}", shot.motion_move));
    }
    if !shot.motion_notes.is_empty() {
        bits.push(shot.motion_notes.clone());
    }
    bits.join("; ")
}

fn rewrite_via_brain(
    show: &Show,
    canon: &str,
    target: &str,
    label: &str,
    notes: &str,
    loras: &[SlateLora],
    motion: &str,
) -> Result<(String, crate::Provenance), ShowError> {
    let mut user = format!(
        "Show: {}\nTARGET: {label} ({target})\nDIALECT:\n{notes}\n\nCANON PROMPT:\n{canon}\n",
        show.name
    );
    if !motion.is_empty() {
        user.push_str(&format!(
            "\nMOTION PREVIS MARKS (honor; do not invent pose/depth):\n{motion}\n"
        ));
    }
    if !loras.is_empty() {
        user.push_str("\nLoRAs (metadata for Comfy/local; do not invent weights):\n");
        for l in loras {
            user.push_str(&format!(
                "- {} weight={} model={}\n",
                l.id, l.weight, l.model
            ));
        }
    }
    user.push_str("\nRewrite the canon for this target now.\n");
    let completion = complete_chat(COMPILE_SYSTEM, &user)?;
    Ok((extract_prompt(&completion.text), completion.provenance))
}

fn extract_prompt(raw: &str) -> String {
    let t = raw.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if let Some(p) = v.get("prompt").and_then(|x| x.as_str()) {
            return p.trim().to_string();
        }
    }
    t.to_string()
}

fn rewrite_via_server(
    canon: &str,
    target: &str,
    shot: &str,
    loras: &[SlateLora],
) -> Result<(String, crate::Provenance), ShowError> {
    let started = std::time::Instant::now();
    let base = std::env::var("LOT_PROMPT_SERVER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ShowError::Msg(
                "no prompt server — set LOT_PROMPT_SERVER (POST /rewrite). Did not invent a rewrite."
                    .into(),
            )
        })?;
    let url = prompt_server_rewrite_url(&base);
    let body = json!({
        "prompt": canon,
        "target": target,
        "shot": shot,
        "loras": loras,
    });
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(60))
        .build();
    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| {
            ShowError::Msg(format!("no prompt server — {e}. Did not invent a rewrite."))
        })?;
    let status = resp.status();
    let v: Value = resp.into_json().map_err(|e| {
        ShowError::Msg(format!(
            "no prompt server — json status={status}: {e}. Did not invent a rewrite."
        ))
    })?;
    if status >= 400 {
        return Err(ShowError::Msg(format!(
            "no prompt server — status={status}. Did not invent a rewrite."
        )));
    }
    let prompt = v
        .get("prompt")
        .and_then(|p| p.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ShowError::Msg(
                "no prompt server — response missing prompt. Did not invent a rewrite.".into(),
            )
        })?;
    Ok((
        prompt.to_string(),
        crate::Provenance::new("prompt-server", target, &base, "lot_prompt_server")
            .with_prompt(canon)
            .with_duration_ms(crate::brain::elapsed_ms(started)),
    ))
}

pub(crate) fn prompt_server_rewrite_url(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    if b.ends_with("/rewrite") {
        b.to_string()
    } else {
        format!("{b}/rewrite")
    }
}

pub fn prompt_for_target(shot: &crate::model::Shot, target: &str) -> String {
    shot.prompt_targets
        .get(target)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| shot.prompt.trim().to_string())
}
