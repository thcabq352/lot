//! Writer / vision brains: Grok first when online, Ollama as the local LLM + VL.
//! LM Studio / other OpenAI-compat stay. Never invent a screenplay or a look.

use crate::packs::{self, lookup};
use crate::show::Show;
use crate::ShowError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const XAI_BASE: &str = "https://api.x.ai/v1";
const DEFAULT_GROK_MODEL: &str = "grok-4.6";
const DEFAULT_GROK_VISION_MODEL: &str = "grok-2-vision-1212";
const OLLAMA_DEFAULT_HOST: &str = "http://127.0.0.1:11434";
const DEFAULT_LOCAL_BASES: &[&str] = &[
    "http://127.0.0.1:11434/v1", // Ollama
    "http://127.0.0.1:1234/v1",  // LM Studio
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Provenance {
    pub backend: String,
    pub model: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub auth: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_cap: Option<String>,
}

impl Provenance {
    pub fn new(
        backend: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
        auth: impl Into<String>,
    ) -> Self {
        Self {
            backend: backend.into(),
            model: model.into(),
            base_url: base_url.into(),
            auth: auth.into(),
            seed: None,
            prompt_hash: None,
            duration_ms: None,
            vram_cap: vram_cap_from_env(),
        }
    }

    pub fn with_prompt(mut self, prompt: &str) -> Self {
        self.prompt_hash = Some(hash_prompt(prompt));
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn with_vram_cap(mut self, cap: impl Into<String>) -> Self {
        let s = cap.into();
        if !s.trim().is_empty() {
            self.vram_cap = Some(s);
        }
        self
    }
}

pub fn hash_prompt(prompt: &str) -> String {
    hex_sha256(prompt.as_bytes())
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn vram_cap_from_env() -> Option<String> {
    env::var("LOT_VRAM_CAP")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
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
        Some(f) if !f.is_empty() => {
            out.push_str(&format!("Format: {f}\n"));
            if let Some(notes) = lookup(packs::formats(), f)
                .and_then(|it| it.notes.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                out.push_str(&format!("Format notes: {notes}\n"));
            }
        }
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

/// Draft a Fountain screenplay from the full Writer contract. Grok first, then Ollama / local.
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
        let started = Instant::now();
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
                    provenance: Provenance::new(c.backend, c.model, c.base_url, c.auth_kind)
                        .with_prompt(user)
                        .with_duration_ms(elapsed_ms(started)),
                });
            }
            Err(e) => errors.push(format!("{} ({}): {e}", c.backend, c.model)),
        }
    }
    Err(ShowError::Msg(no_brain_message(&errors)))
}

/// Look at an image. Grok vision first when online, then Ollama VL. Never invent a look.
pub fn complete_vision(
    system: &str,
    user: &str,
    image: &[u8],
    mime: &str,
) -> Result<Completion, ShowError> {
    if image.is_empty() {
        return Err(ShowError::Msg("no vision — empty image".into()));
    }
    let candidates = resolve_vision_candidates()?;
    if candidates.is_empty() {
        return Err(ShowError::Msg(no_vision_message(&[])));
    }
    let mut errors: Vec<String> = Vec::new();
    for c in candidates {
        let started = Instant::now();
        match chat_completions_vision(&c, system, user, image, mime) {
            Ok(text) => {
                let text = strip_fences(&text);
                if text.trim().is_empty() {
                    errors.push(format!("{} ({}): empty completion", c.backend, c.model));
                    continue;
                }
                return Ok(Completion {
                    text,
                    provenance: Provenance::new(c.backend, c.model, c.base_url, c.auth_kind)
                        .with_prompt(user)
                        .with_duration_ms(elapsed_ms(started)),
                });
            }
            Err(e) => errors.push(format!("{} ({}): {e}", c.backend, c.model)),
        }
    }
    Err(ShowError::Msg(no_vision_message(&errors)))
}

/// Live Ollama probe for doctor. Does not invent models.
#[derive(Debug, Clone, Serialize)]
pub struct OllamaProbe {
    pub up: bool,
    pub host: String,
    pub llm: Option<String>,
    pub vision: Option<String>,
}

pub fn probe_ollama() -> OllamaProbe {
    match ollama_host() {
        None => OllamaProbe {
            up: false,
            host: String::new(),
            llm: None,
            vision: None,
        },
        Some(host) => {
            let models = list_ollama_models(&host, true).unwrap_or_default();
            let up = !models.is_empty() || ollama_tags_ok(&host);
            let llm_explicit =
                env_nonempty("LOT_OLLAMA_MODEL").or_else(|| env_nonempty("LOT_LOCAL_MODEL"));
            let vis_explicit = env_nonempty("LOT_OLLAMA_VISION_MODEL")
                .or_else(|| env_nonempty("LOT_LOCAL_VISION_MODEL"));
            let llm = pick_llm(&models, llm_explicit.as_deref());
            let vision = pick_vision(&models, vis_explicit.as_deref());
            OllamaProbe {
                up,
                host,
                llm: if up { llm } else { None },
                vision: if up { vision } else { None },
            }
        }
    }
}

/// First configured Grok credential. Token is for HTTP only — never log it.
pub fn grok_auth() -> Option<(String, String, String)> {
    let cands = resolve_candidates().ok()?;
    cands.into_iter().find(|c| c.backend == "grok").map(|c| {
        (
            c.token,
            normalize_base(&c.base_url),
            c.auth_kind.to_string(),
        )
    })
}

fn no_brain_message(errors: &[String]) -> String {
    let mut s = String::from(
        "no brain — Grok (xAI OAuth / XAI_API_KEY) and Ollama / local OpenAI-compat both unavailable",
    );
    if !errors.is_empty() {
        s.push_str(". tried: ");
        s.push_str(&errors.join(" | "));
    }
    s.push_str(
        ". set HERMES auth xai-oauth, or XAI_API_KEY, or run Ollama (11434), or LOT_LOCAL_BASE_URL + LOT_LOCAL_MODEL",
    );
    s
}

fn no_vision_message(errors: &[String]) -> String {
    let mut s = String::from("no vision — Grok vision and Ollama VL both unavailable");
    if !errors.is_empty() {
        s.push_str(". tried: ");
        s.push_str(&errors.join(" | "));
    }
    s.push_str(". pull a VL model (llava, qwen2.5vl, qwen3.5) or set LOT_OLLAMA_VISION_MODEL");
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

    // Cloud Grok is spend. Write-only agents stay on Ollama / local.
    let allow_grok = crate::caps::allow_spend();

    // 1) Explicit token env (tests / CI)
    if allow_grok {
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
    }

    // 4) Ollama (named local) + LM Studio / other OpenAI-compat
    let local_model = env_nonempty("LOT_LOCAL_MODEL").or_else(|| env_nonempty("OPENAI_MODEL"));
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
    // LOT_OLLAMA_HOST pins Ollama even when another local URL is set.
    if let Some(host) = env_nonempty("LOT_OLLAMA_HOST") {
        local_bases.push(ollama_v1(&host));
    }

    // de-dupe bases
    let mut seen = std::collections::HashSet::new();
    local_bases.retain(|b| seen.insert(normalize_base(b)));

    for base in local_bases {
        let backend = backend_label(&base);
        let model = if backend == "ollama" {
            env_nonempty("LOT_OLLAMA_MODEL")
                .or_else(|| local_model.clone())
                .or_else(|| ollama_llm_model(&base))
                .or_else(|| probe_first_model(&base, &local_key))
        } else {
            local_model
                .clone()
                .or_else(|| probe_first_model(&base, &local_key))
        };
        let Some(model) = model else { continue };
        out.push(Candidate {
            backend,
            auth_kind: if backend == "ollama" {
                "ollama"
            } else {
                "openai_compat"
            },
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

fn resolve_vision_candidates() -> Result<Vec<Candidate>, ShowError> {
    let mut out = Vec::new();
    let grok_vision = env_nonempty("LOT_GROK_VISION_MODEL")
        .unwrap_or_else(|| DEFAULT_GROK_VISION_MODEL.to_string());
    for c in resolve_candidates()? {
        if c.backend == "grok" {
            let mut v = c;
            v.model = grok_vision.clone();
            out.push(v);
            break;
        }
    }
    if let Some(host) = ollama_host() {
        let models = list_ollama_models(&host, false).unwrap_or_default();
        let vis = env_nonempty("LOT_OLLAMA_VISION_MODEL")
            .or_else(|| env_nonempty("LOT_LOCAL_VISION_MODEL"))
            .or_else(|| pick_vision(&models, None));
        if let Some(model) = vis {
            let key = env::var("LOT_LOCAL_API_KEY")
                .or_else(|_| env::var("OPENAI_API_KEY"))
                .unwrap_or_else(|_| "ollama".to_string());
            out.push(Candidate {
                backend: "ollama",
                auth_kind: "ollama",
                base_url: ollama_v1(&host),
                token: key,
                model,
            });
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| {
        seen.insert(format!(
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

fn env_nonempty(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn backend_label(base: &str) -> &'static str {
    let n = normalize_base(base);
    if n.contains(":11434") || n.contains("ollama") {
        return "ollama";
    }
    if let Some(host) = env_nonempty("LOT_OLLAMA_HOST") {
        let root = normalize_base(&ollama_root(&host));
        if !root.is_empty()
            && (n == root || n == format!("{root}/v1") || n.starts_with(&format!("{root}/")))
        {
            return "ollama";
        }
    }
    if n.contains(":1234") || n.contains("lmstudio") || n.contains("lm-studio") {
        "lmstudio"
    } else {
        "local"
    }
}

fn ollama_root(base: &str) -> String {
    normalize_base(base)
        .trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string()
}

fn ollama_v1(host: &str) -> String {
    let root = ollama_root(host);
    if root.ends_with("/v1") {
        root
    } else {
        format!("{root}/v1")
    }
}

/// None when an explicit non-Ollama local URL replaced the default probe.
fn ollama_host() -> Option<String> {
    if let Some(h) = env_nonempty("LOT_OLLAMA_HOST") {
        return Some(ollama_root(&h));
    }
    if let Some(b) = env_nonempty("LOT_LOCAL_BASE_URL") {
        if backend_label(&b) == "ollama" {
            return Some(ollama_root(&b));
        }
        return None;
    }
    if let Some(b) = env_nonempty("OPENAI_BASE_URL") {
        if !b.contains("api.x.ai") && backend_label(&b) == "ollama" {
            return Some(ollama_root(&b));
        }
        if !b.contains("api.x.ai") {
            return None;
        }
    }
    Some(OLLAMA_DEFAULT_HOST.to_string())
}

fn ollama_llm_model(base: &str) -> Option<String> {
    let host = ollama_root(base);
    let models = list_ollama_models(&host, false)?;
    pick_llm(&models, None)
}

fn is_embed(id: &str) -> bool {
    id.to_lowercase().contains("embed")
}

pub(crate) fn looks_vision(id: &str) -> bool {
    let l = id.to_lowercase();
    if is_embed(&l) {
        return false;
    }
    l.contains("llava")
        || l.contains("bakllava")
        || l.contains("moondream")
        || l.contains("minicpm-v")
        || l.contains("pixtral")
        || l.contains("internvl")
        || l.contains("vision")
        || l.contains("qwen2-vl")
        || l.contains("qwen2.5-vl")
        || l.contains("qwen2.5vl")
        || l.contains("qwen3-vl")
        || l.contains("-vl")
        || l.contains("vl:")
        || l.contains("gemma3")
        || l.contains("qwen3.5")
}

fn pick_llm(models: &[String], explicit: Option<&str>) -> Option<String> {
    if let Some(m) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(m.to_string());
    }
    models.iter().find(|m| !is_embed(m)).cloned()
}

fn pick_vision(models: &[String], explicit: Option<&str>) -> Option<String> {
    if let Some(m) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(m.to_string());
    }
    models.iter().find(|m| looks_vision(m)).cloned()
}

fn ollama_tags_ok(host: &str) -> bool {
    list_ollama_raw(host, true).is_some()
}

fn list_ollama_models(host: &str, short: bool) -> Option<Vec<String>> {
    let v = list_ollama_raw(host, short)?;
    let arr = v.get("models")?.as_array()?;
    let mut out = Vec::new();
    for m in arr {
        if let Some(name) = m
            .get("name")
            .or_else(|| m.get("model"))
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            out.push(name.to_string());
        }
    }
    Some(out)
}

fn list_ollama_raw(host: &str, short: bool) -> Option<Value> {
    let url = format!("{}/api/tags", ollama_root(host));
    let (connect, read) = if short {
        (Duration::from_millis(250), Duration::from_millis(250))
    } else {
        (Duration::from_secs(2), Duration::from_secs(3))
    };
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(connect)
        .timeout_read(read)
        .build();
    let resp = agent.get(&url).call().ok()?;
    resp.into_json().ok()
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
    chat_completions_body(c, system, json!(user))
}

fn chat_completions_vision(
    c: &Candidate,
    system: &str,
    user: &str,
    image: &[u8],
    mime: &str,
) -> Result<String, String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(image);
    let mime = if mime.trim().is_empty() {
        "image/png"
    } else {
        mime
    };
    let user_content = json!([
        { "type": "text", "text": user },
        { "type": "image_url", "image_url": { "url": format!("data:{mime};base64,{b64}") } }
    ]);
    match chat_completions_body(c, system, user_content) {
        Ok(t) => Ok(t),
        Err(e) if c.backend == "ollama" => chat_ollama_native(c, system, user, &b64)
            .map_err(|n| format!("{e} | ollama native: {n}")),
        Err(e) => Err(e),
    }
}

fn chat_timeout(c: &Candidate) -> Duration {
    if c.backend == "grok" {
        Duration::from_secs(
            env::var("LOT_XAI_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(180),
        )
    } else {
        Duration::from_secs(
            env::var("LOT_LOCAL_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
        )
    }
}

fn chat_completions_body(c: &Candidate, system: &str, user: Value) -> Result<String, String> {
    let url = format!("{}/chat/completions", normalize_base(&c.base_url));
    let body = json!({
        "model": c.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": 0.7,
    });
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(chat_timeout(c))
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
        .ok_or_else(|| "missing choices[0].message.content".to_string())?;
    match content {
        Value::String(s) => Ok(s.clone()),
        Value::Array(parts) => {
            let mut text = String::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                }
            }
            if text.trim().is_empty() {
                Err("empty vision content".into())
            } else {
                Ok(text)
            }
        }
        _ => Err("missing choices[0].message.content".into()),
    }
}

fn chat_ollama_native(
    c: &Candidate,
    system: &str,
    user: &str,
    b64: &str,
) -> Result<String, String> {
    let url = format!("{}/api/chat", ollama_root(&c.base_url));
    let body = json!({
        "model": c.model,
        "stream": false,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user, "images": [b64] }
        ]
    });
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(chat_timeout(c))
        .build();
    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("http: {e}"))?;
    let status = resp.status();
    let v: Value = resp
        .into_json()
        .map_err(|e| format!("json status={status}: {e}"))?;
    if status >= 400 {
        return Err(format!("status={status}"));
    }
    v.pointer("/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "missing message.content".to_string())
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
        env::remove_var("LOT_OLLAMA_MODEL");
        env::remove_var("LOT_OLLAMA_VISION_MODEL");
        // Point default local probes at closed ports via LOT_LOCAL only — still may probe 11434.
        // Force empty by setting LOT_LOCAL_BASE_URL to dead + model so we don't skip probe path.
        env::set_var("LOT_LOCAL_BASE_URL", "http://127.0.0.1:9/v1");
        env::set_var("LOT_LOCAL_MODEL", "nope");
        env::set_var("LOT_OLLAMA_HOST", "http://127.0.0.1:9");
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
        env::remove_var("LOT_OLLAMA_HOST");
    }

    #[test]
    fn ollama_is_named_local_backend() {
        let _g = ENV.lock().unwrap();
        assert_eq!(backend_label("http://127.0.0.1:11434/v1"), "ollama");
        assert_eq!(backend_label("http://127.0.0.1:1234/v1"), "lmstudio");
        assert_eq!(backend_label("http://127.0.0.1:9/v1"), "local");
        assert!(looks_vision("qwen3.5:9b"));
        assert!(looks_vision("llava:latest"));
        assert!(looks_vision("qwen2.5vl:7b"));
        assert!(!looks_vision("llama3.2:latest"));
        assert!(!looks_vision("nomic-embed-text"));
        let models = vec![
            "nomic-embed-text".into(),
            "llama3.2:latest".into(),
            "llava:latest".into(),
        ];
        assert_eq!(pick_llm(&models, None).as_deref(), Some("llama3.2:latest"));
        assert_eq!(pick_vision(&models, None).as_deref(), Some("llava:latest"));
        assert_eq!(
            pick_vision(&models, Some("qwen3.5:9b")).as_deref(),
            Some("qwen3.5:9b")
        );
    }

    #[test]
    fn no_vision_when_nothing_configured() {
        let _g = ENV.lock().unwrap();
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
        env::set_var("LOT_LOCAL_BASE_URL", "http://127.0.0.1:9/v1");
        env::set_var("LOT_LOCAL_MODEL", "nope");
        env::set_var("LOT_OLLAMA_HOST", "http://127.0.0.1:9");
        env::remove_var("LOT_OLLAMA_VISION_MODEL");
        env::remove_var("LOT_LOCAL_VISION_MODEL");
        env::remove_var("LOT_GROK_VISION_MODEL");
        let err = complete_vision("sys", "user", b"not-empty", "image/png")
            .unwrap_err()
            .to_string();
        assert!(err.contains("no vision"), "expected no vision, got: {err}");
        env::remove_var("HERMES_HOME");
        env::remove_var("LOT_HERMES_HOME");
        env::remove_var("LOT_LOCAL_BASE_URL");
        env::remove_var("LOT_LOCAL_MODEL");
        env::remove_var("LOT_OLLAMA_HOST");
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
            stems: crate::stems::Stems::default(),
            stills_backend: None,
            slate: crate::model::SlateState::default(),
            finish: crate::model::FinishState::default(),
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
        let mut ad = show.clone();
        ad.writer.format = Some("advertisement".into());
        let ad_p = draft_user_prompt(&ad);
        assert!(ad_p.contains("advertisement"), "{ad_p}");
        assert!(
            ad_p.to_lowercase().contains("spot") || ad_p.to_lowercase().contains("commercial"),
            "{ad_p}"
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
        env::set_var("LOT_OLLAMA_HOST", "http://127.0.0.1:9");
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
        env::remove_var("LOT_OLLAMA_HOST");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn prompt_hash_is_stable_sha256() {
        assert_eq!(
            hash_prompt("wide tent"),
            "cfe7703a03190f7b3ad0f115475c18e804974a1244db979387f3c2232fceac3a"
        );
        let old = r#"{"backend":"grok","model":"grok-4.6","base_url":"https://api.x.ai/v1","auth":"xai-oauth"}"#;
        let p: Provenance = serde_json::from_str(old).unwrap();
        assert_eq!(p.backend, "grok");
        assert_eq!(p.seed, None);
        assert_eq!(p.prompt_hash, None);
        assert_eq!(p.duration_ms, None);
        assert_eq!(p.vram_cap, None);
    }
}
