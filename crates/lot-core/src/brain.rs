//! Writer brains: Grok (xAI OAuth / API key) first, local OpenAI-compat second.
//! Never invent a screenplay when no brain answers.

use crate::packs::{self, lookup};
use crate::show::Show;
use crate::ShowError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const XAI_BASE: &str = "https://api.x.ai/v1";
const DEFAULT_GROK_MODEL: &str = "grok-4.6";
const DEFAULT_LOCAL_BASES: &[&str] = &[
    "http://127.0.0.1:11434/v1", // Ollama
    "http://127.0.0.1:1234/v1",  // LM Studio
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    pub backend: String,
    pub model: String,
    pub base_url: String,
    pub auth: String,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone)]
struct Candidate {
    backend: &'static str,
    auth_kind: &'static str,
    base_url: String,
    token: String,
    model: String,
}

const DRAFT_SYSTEM: &str = "You are Lot Writer, a professional screenwriter. \
Write a complete screenplay in Fountain format only. \
Include a Title page (Title, Credit, Author, Draft date), then the script. \
Use proper Fountain: scene headings (INT./EXT.), action, CHARACTER cues, dialogue. \
Director names in the brief are coverage influence only — not endorsement, not impersonation. \
Do not wrap the script in markdown fences. Do not invent API keys or meta commentary. \
Author line: Lot Writer. Keep it produceable (locations and cast as specified).";

const REVISE_SYSTEM: &str = "You are Lot Writer, revising an existing Fountain screenplay. \
Apply the filmmaker notes. Output the complete revised Fountain only. \
Director names are coverage influence only — not endorsement, not impersonation. \
Do not wrap the script in markdown fences. Do not invent API keys or meta commentary. \
Keep Author: Lot Writer.";

/// Build the user prompt from brief + style + cast + format. No network.
pub fn draft_user_prompt(show: &Show) -> String {
    let mut out = String::new();
    out.push_str(&format!("Show title: {}\n\n", show.name));
    match show.writer.format.as_deref() {
        Some(f) if !f.is_empty() => out.push_str(&format!("Format: {f}\n")),
        _ => out.push_str("Format: unset\n"),
    }
    if show.writer.genres.is_empty() {
        out.push_str("Genres: unset\n");
    } else {
        let labels: Vec<String> = show
            .writer
            .genres
            .iter()
            .map(|id| match lookup(packs::genres(), id) {
                Some(it) => format!("{} ({id})", it.display_name()),
                None => id.clone(),
            })
            .collect();
        out.push_str(&format!("Genres: {}\n", labels.join(", ")));
    }
    out.push_str("\nLiving influence (coverage style, not endorsement):\n");
    append_style_lines(
        &mut out,
        packs::living_directors(),
        &show.writer.styles_living,
    );
    out.push_str("\nCanon influence (coverage style, not endorsement):\n");
    append_style_lines(
        &mut out,
        packs::canon_directors(),
        &show.writer.styles_canon,
    );
    out.push_str("\nCast:\n");
    if show.writer.cast.is_empty() {
        out.push_str("- (none set)\n");
    } else {
        for c in &show.writer.cast {
            out.push_str(&format!(
                "- {} — function: {}; look: {}; must-not: {}\n",
                c.name,
                empty_dash(&c.function),
                empty_dash(&c.look),
                empty_dash(&c.must_not)
            ));
        }
    }
    out.push_str("\nBrief:\n");
    out.push_str(show.writer.brief.trim());
    out.push_str("\n\nWrite the Fountain screenplay now.\n");
    out
}

fn append_style_lines(out: &mut String, pack: &packs::PackFile, ids: &[String]) {
    if ids.is_empty() {
        out.push_str("- (none set)\n");
        return;
    }
    for id in ids {
        match lookup(pack, id) {
            Some(it) => {
                let name = it.display_name();
                match it.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(notes) => out.push_str(&format!("- {name} ({id}) — {notes}\n")),
                    None => out.push_str(&format!("- {name} ({id})\n")),
                }
            }
            None => out.push_str(&format!("- {id}\n")),
        }
    }
}

fn empty_dash(s: &str) -> &str {
    let t = s.trim();
    if t.is_empty() {
        "-"
    } else {
        t
    }
}

/// Draft a Fountain screenplay from the full Writer contract. Grok first, then local.
pub fn draft_fountain(show: &Show) -> Result<Completion, ShowError> {
    complete_chat(DRAFT_SYSTEM, &draft_user_prompt(show))
}

pub fn revise_user_prompt(show: &Show, current: &str, notes: &str) -> String {
    format!(
        "{}\nFilmmaker revise notes:\n{}\n\nCurrent Fountain draft:\n{}\n\nRevise the Fountain screenplay now. Output the complete revised script only.\n",
        draft_user_prompt(show),
        notes.trim(),
        current
    )
}

pub fn revise_fountain(show: &Show, current: &str, notes: &str) -> Result<Completion, ShowError> {
    complete_chat(REVISE_SYSTEM, &revise_user_prompt(show, current, notes))
}

pub fn complete_chat(system: &str, user: &str) -> Result<Completion, ShowError> {
    let candidates = resolve_candidates()?;
    if candidates.is_empty() {
        return Err(ShowError::Msg(no_brain_message(&[])));
    }
    let mut errors: Vec<String> = Vec::new();
    for c in candidates {
        match chat_completions(&c, system, user) {
            Ok(text) => {
                let text = strip_fences(&text);
                if text.trim().is_empty() {
                    errors.push(format!("{} ({}): empty completion", c.backend, c.model));
                    continue;
                }
                // Refuse hollow stub-shaped replies that pretend wiring failed.
                if text.contains("outline stub — Grok draft not wired") {
                    errors.push(format!(
                        "{} ({}): refused stub-shaped model output",
                        c.backend, c.model
                    ));
                    continue;
                }
                return Ok(Completion {
                    text,
                    provenance: Provenance {
                        backend: c.backend.to_string(),
                        model: c.model,
                        base_url: c.base_url,
                        auth: c.auth_kind.to_string(),
                    },
                });
            }
            Err(e) => errors.push(format!("{} ({}): {e}", c.backend, c.model)),
        }
    }
    Err(ShowError::Msg(no_brain_message(&errors)))
}

fn no_brain_message(errors: &[String]) -> String {
    let mut s = String::from(
        "no brain — Grok (xAI OAuth / XAI_API_KEY) and local OpenAI-compat both unavailable",
    );
    if !errors.is_empty() {
        s.push_str(". tried: ");
        s.push_str(&errors.join(" | "));
    }
    s.push_str(
        ". set HERMES auth xai-oauth, or XAI_API_KEY, or LOT_LOCAL_BASE_URL + LOT_LOCAL_MODEL (Ollama/LM Studio)",
    );
    s
}

fn resolve_candidates() -> Result<Vec<Candidate>, ShowError> {
    let mut out = Vec::new();
    let grok_model = env::var("LOT_WRITER_MODEL")
        .or_else(|_| env::var("LOT_GROK_MODEL"))
        .unwrap_or_else(|_| DEFAULT_GROK_MODEL.to_string());
    let xai_base = env::var("LOT_XAI_BASE_URL")
        .or_else(|_| env::var("XAI_BASE_URL"))
        .unwrap_or_else(|_| XAI_BASE.to_string());

    // 1) Explicit token env (tests / CI)
    if let Ok(tok) = env::var("LOT_XAI_TOKEN") {
        let t = tok.trim().to_string();
        if !t.is_empty() {
            out.push(Candidate {
                backend: "grok",
                auth_kind: "lot_xai_token",
                base_url: xai_base.clone(),
                token: t,
                model: grok_model.clone(),
            });
        }
    }

    // 2) Hermes xAI OAuth access_token
    if let Some((tok, base)) = read_hermes_xai_oauth() {
        out.push(Candidate {
            backend: "grok",
            auth_kind: "xai_oauth",
            base_url: base.unwrap_or_else(|| xai_base.clone()),
            token: tok,
            model: grok_model.clone(),
        });
    }

    // 3) XAI_API_KEY (console key)
    if let Ok(key) = env::var("XAI_API_KEY") {
        let k = key.trim().to_string();
        if !k.is_empty() {
            out.push(Candidate {
                backend: "grok",
                auth_kind: "xai_api_key",
                base_url: xai_base.clone(),
                token: k,
                model: grok_model.clone(),
            });
        }
    }

    // 4) Local OpenAI-compat
    let local_model = env::var("LOT_LOCAL_MODEL")
        .or_else(|_| env::var("OPENAI_MODEL"))
        .ok();
    let local_key = env::var("LOT_LOCAL_API_KEY")
        .or_else(|_| env::var("OPENAI_API_KEY"))
        .unwrap_or_else(|_| "ollama".to_string());

    let mut local_bases: Vec<String> = Vec::new();
    let mut explicit_local = false;
    if let Ok(b) = env::var("LOT_LOCAL_BASE_URL") {
        let b = b.trim().to_string();
        if !b.is_empty() {
            local_bases.push(b);
            explicit_local = true;
        }
    }
    if let Ok(b) = env::var("OPENAI_BASE_URL") {
        let b = b.trim().to_string();
        // Only treat as local if not pointing at xAI cloud.
        if !b.is_empty() && !b.contains("api.x.ai") {
            local_bases.push(b);
            explicit_local = true;
        }
    }
    // Explicit local URL replaces auto-probe defaults (tests + locked boxes).
    if !explicit_local {
        for b in DEFAULT_LOCAL_BASES {
            local_bases.push((*b).to_string());
        }
    }

    // de-dupe bases
    let mut seen = std::collections::HashSet::new();
    local_bases.retain(|b| seen.insert(normalize_base(b)));

    for base in local_bases {
        let model = match &local_model {
            Some(m) => m.clone(),
            None => match probe_first_model(&base, &local_key) {
                Some(m) => m,
                None => continue,
            },
        };
        out.push(Candidate {
            backend: "local",
            auth_kind: "openai_compat",
            base_url: base,
            token: local_key.clone(),
            model,
        });
    }

    // de-dupe identical backend+auth+base+model
    let mut seen_c = std::collections::HashSet::new();
    out.retain(|c| {
        seen_c.insert(format!(
            "{}|{}|{}|{}",
            c.backend,
            c.auth_kind,
            normalize_base(&c.base_url),
            c.model
        ))
    });

    Ok(out)
}

fn normalize_base(b: &str) -> String {
    b.trim().trim_end_matches('/').to_lowercase()
}

fn hermes_home() -> PathBuf {
    if let Ok(p) = env::var("HERMES_HOME") {
        return PathBuf::from(p);
    }
    if let Ok(p) = env::var("LOT_HERMES_HOME") {
        return PathBuf::from(p);
    }
    // Prefer profile home only if auth lives there; default Hermes root.
    let user = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(user).join(".hermes")
}

fn read_hermes_xai_oauth() -> Option<(String, Option<String>)> {
    let path = hermes_home().join("auth.json");
    let raw = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;

    // providers.xai-oauth.tokens.access_token
    if let Some(tok) = v
        .pointer("/providers/xai-oauth/tokens/access_token")
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let base = v
            .pointer("/credential_pool/xai-oauth")
            .and_then(|p| p.as_array())
            .and_then(|arr| {
                arr.iter().find_map(|e| {
                    e.get("base_url")
                        .and_then(|b| b.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                })
            });
        return Some((tok.to_string(), base));
    }

    // credential_pool.xai-oauth[0].access_token
    if let Some(arr) = v
        .pointer("/credential_pool/xai-oauth")
        .and_then(|p| p.as_array())
    {
        for e in arr {
            if let Some(tok) = e
                .get("access_token")
                .and_then(|t| t.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let base = e
                    .get("base_url")
                    .and_then(|b| b.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                return Some((tok.to_string(), base));
            }
        }
    }
    None
}

fn probe_first_model(base: &str, token: &str) -> Option<String> {
    let url = format!("{}/models", normalize_base(base));
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout_read(Duration::from_secs(3))
        .build();
    let resp = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .ok()?;
    let v: Value = resp.into_json().ok()?;
    let data = v.get("data")?.as_array()?;
    for m in data {
        if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
            // Prefer chat models over embed
            let lower = id.to_lowercase();
            if lower.contains("embed") {
                continue;
            }
            return Some(id.to_string());
        }
    }
    None
}

fn chat_completions(c: &Candidate, system: &str, user: &str) -> Result<String, String> {
    let url = format!("{}/chat/completions", normalize_base(&c.base_url));
    let body = json!({
        "model": c.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": 0.7,
    });
    let timeout = if c.backend == "local" {
        Duration::from_secs(
            env::var("LOT_LOCAL_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
        )
    } else {
        Duration::from_secs(
            env::var("LOT_XAI_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(180),
        )
    };
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(timeout)
        .build();
    let resp = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {}", c.token))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("http: {e}"))?;
    let status = resp.status();
    let v: Value = resp
        .into_json()
        .map_err(|e| format!("json status={status}: {e}"))?;
    if status >= 400 {
        let msg = v
            .pointer("/error/message")
            .and_then(|m| m.as_str())
            .or_else(|| v.get("error").and_then(|e| e.as_str()))
            .unwrap_or("request failed");
        return Err(format!("status={status}: {msg}"));
    }
    let content = v
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| "missing choices[0].message.content".to_string())?;
    Ok(content.to_string())
}

fn strip_fences(s: &str) -> String {
    let t = s.trim();
    if !t.starts_with("```") {
        return t.to_string();
    }
    let mut lines: Vec<&str> = t.lines().collect();
    if lines.first().is_some_and(|l| l.starts_with("```")) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|l| l.trim() == "```") {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV: Mutex<()> = Mutex::new(());

    #[test]
    fn strip_fences_plain() {
        assert_eq!(strip_fences("FADE IN:"), "FADE IN:");
    }

    #[test]
    fn strip_fences_md() {
        let s = "```fountain\nFADE IN:\n\nINT. ROOM - DAY\n```";
        assert!(strip_fences(s).starts_with("FADE IN:"));
    }

    #[test]
    fn no_brain_when_nothing_configured() {
        let _g = ENV.lock().unwrap();
        // Isolate from real machine brains.
        env::set_var(
            "HERMES_HOME",
            std::env::temp_dir().join("lot-no-hermes-xyz"),
        );
        env::set_var(
            "LOT_HERMES_HOME",
            std::env::temp_dir().join("lot-no-hermes-xyz"),
        );
        env::remove_var("LOT_XAI_TOKEN");
        env::remove_var("XAI_API_KEY");
        env::remove_var("LOT_LOCAL_BASE_URL");
        env::remove_var("OPENAI_BASE_URL");
        env::remove_var("LOT_LOCAL_MODEL");
        env::remove_var("OPENAI_MODEL");
        // Point default local probes at closed ports via LOT_LOCAL only — still may probe 11434.
        // Force empty by setting LOT_LOCAL_BASE_URL to dead + model so we don't skip probe path.
        env::set_var("LOT_LOCAL_BASE_URL", "http://127.0.0.1:9/v1");
        env::set_var("LOT_LOCAL_MODEL", "nope");
        // Block default bases by... we can't. So call resolve and complete with dead local only by
        // temporarily relying on complete_chat error path.
        let err = complete_chat("sys", "user").unwrap_err().to_string();
        assert!(
            err.contains("no brain"),
            "expected no brain message, got: {err}"
        );
        env::remove_var("HERMES_HOME");
        env::remove_var("LOT_HERMES_HOME");
        env::remove_var("LOT_LOCAL_BASE_URL");
        env::remove_var("LOT_LOCAL_MODEL");
    }

    #[test]
    fn draft_prompt_includes_style_cast_format() {
        use crate::show::{CastMember, Writer};
        use crate::SchoolStatus;
        let show = Show {
            schema: 1,
            id: "t".into(),
            name: "Carnival".into(),
            created_at: "0".into(),
            updated_at: "0".into(),
            rev: 1,
            school: SchoolStatus::default(),
            phase: "writer".into(),
            scenes: vec![],
            shots: vec![],
            takes: vec![],
            media: vec![],
            wall: vec![],
            writer: Writer {
                brief: "A clown loses the mask.".into(),
                genres: vec!["drama".into()],
                styles_living: vec!["greta-gerwig".into()],
                styles_canon: vec!["akira-kurosawa".into()],
                format: Some("30min".into()),
                cast: vec![CastMember {
                    name: "Ada".into(),
                    function: "lead".into(),
                    look: "bare face".into(),
                    must_not: "franchise cameo".into(),
                }],
                locked: false,
                draft_path: None,
                draft_provenance: None,
            },
        };
        let p = draft_user_prompt(&show);
        assert!(p.contains("A clown loses the mask."), "{p}");
        assert!(p.contains("drama"), "{p}");
        assert!(p.contains("30min"), "{p}");
        assert!(p.contains("Ada"), "{p}");
        assert!(p.contains("bare face"), "{p}");
        assert!(p.contains("franchise cameo"), "{p}");
        assert!(
            p.contains("Greta Gerwig") && p.contains("greta-gerwig"),
            "{p}"
        );
        assert!(
            p.contains("Kurosawa") && p.contains("akira-kurosawa"),
            "{p}"
        );
        assert!(
            p.to_lowercase().contains("coverage") || p.to_lowercase().contains("influence"),
            "{p}"
        );
    }

    #[test]
    fn reads_hermes_oauth_from_auth_json() {
        let _g = ENV.lock().unwrap();
        let home = std::env::temp_dir().join(format!("lot-hermes-auth-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let auth = json!({
            "providers": {
                "xai-oauth": {
                    "tokens": {
                        "access_token": "test-oauth-token-abc",
                        "token_type": "Bearer"
                    }
                }
            },
            "credential_pool": {
                "xai-oauth": [{ "base_url": "https://api.x.ai/v1", "access_token": "test-oauth-token-abc" }]
            }
        });
        fs::write(home.join("auth.json"), auth.to_string()).unwrap();
        env::set_var("HERMES_HOME", &home);
        env::remove_var("LOT_XAI_TOKEN");
        env::remove_var("XAI_API_KEY");
        env::set_var("LOT_LOCAL_BASE_URL", "http://127.0.0.1:9/v1");
        env::set_var("LOT_LOCAL_MODEL", "nope");
        let c = resolve_candidates().unwrap();
        assert!(
            c.iter()
                .any(|x| x.auth_kind == "xai_oauth" && x.token == "test-oauth-token-abc"),
            "candidates: {:?}",
            c.iter().map(|x| x.auth_kind).collect::<Vec<_>>()
        );
        env::remove_var("HERMES_HOME");
        env::remove_var("LOT_LOCAL_BASE_URL");
        env::remove_var("LOT_LOCAL_MODEL");
        let _ = fs::remove_dir_all(&home);
    }
}
