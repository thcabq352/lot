//! Dated Writer packs. IDs live in JSON, not hardcoded forever.

use crate::ShowError;
use serde::Deserialize;
use std::sync::OnceLock;

const GENRES_JSON: &str = include_str!("../packs/genres.json");
const LIVING_JSON: &str = include_str!("../packs/directors-living.json");
const CANON_JSON: &str = include_str!("../packs/directors-canon.json");
const FORMATS_JSON: &str = include_str!("../packs/formats.json");
const PROMPT_TARGETS_JSON: &str = include_str!("../packs/prompt-targets.json");

pub const FORMATS: &[&str] = &[
    "feature",
    "30min",
    "15s",
    "episodic",
    "advertisement",
    "music-video",
];

#[derive(Debug, Clone, Copy)]
pub enum IdKind {
    Genre,
    Living,
    Canon,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackFile {
    pub reviewed: String,
    pub disclaimer: String,
    pub items: Vec<PackItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackItem {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

impl PackItem {
    pub fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .or(self.label.as_deref())
            .unwrap_or(&self.id)
    }
}

impl IdKind {
    fn pack(self) -> &'static PackFile {
        match self {
            IdKind::Genre => genres(),
            IdKind::Living => living_directors(),
            IdKind::Canon => canon_directors(),
        }
    }

    fn unknown_msg(self, id: &str) -> String {
        match self {
            IdKind::Genre => format!("unknown genre: {id}"),
            IdKind::Living => format!("unknown living: {id}"),
            IdKind::Canon => format!("unknown canon: {id}"),
        }
    }
}

pub fn genres() -> &'static PackFile {
    static P: OnceLock<PackFile> = OnceLock::new();
    P.get_or_init(|| parse_pack(GENRES_JSON, "genres"))
}

pub fn living_directors() -> &'static PackFile {
    static P: OnceLock<PackFile> = OnceLock::new();
    P.get_or_init(|| parse_pack(LIVING_JSON, "directors-living"))
}

pub fn canon_directors() -> &'static PackFile {
    static P: OnceLock<PackFile> = OnceLock::new();
    P.get_or_init(|| parse_pack(CANON_JSON, "directors-canon"))
}

pub fn prompt_targets() -> &'static PackFile {
    static P: OnceLock<PackFile> = OnceLock::new();
    P.get_or_init(|| parse_pack(PROMPT_TARGETS_JSON, "prompt-targets"))
}

pub fn formats() -> &'static PackFile {
    static P: OnceLock<PackFile> = OnceLock::new();
    P.get_or_init(|| parse_pack(FORMATS_JSON, "formats"))
}

fn parse_pack(raw: &str, name: &str) -> PackFile {
    let pack: PackFile = serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name} pack: {e}"));
    if pack.reviewed.trim().is_empty() {
        panic!("{name} pack: missing reviewed date");
    }
    if pack.disclaimer.trim().is_empty() {
        panic!("{name} pack: missing disclaimer");
    }
    pack
}

pub fn resolve_ids(kind: IdKind, ids: &[String]) -> Result<Vec<String>, ShowError> {
    let pack = kind.pack();
    let mut out = Vec::new();
    for raw in ids.iter().flat_map(|s| s.split(',')) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        match pack.items.iter().find(|it| it.id.eq_ignore_ascii_case(raw)) {
            Some(it) => {
                if !out.iter().any(|id| id == &it.id) {
                    out.push(it.id.clone());
                }
            }
            None => return Err(ShowError::Msg(kind.unknown_msg(raw))),
        }
    }
    Ok(out)
}

pub fn resolve_format(raw: &str) -> Result<String, ShowError> {
    let raw = raw.trim();
    let key = raw.to_ascii_lowercase().replace('_', "-");
    let canonical = match key.as_str() {
        "ad" | "advert" | "advertisement" | "commercial" | "spot" => "advertisement",
        "mv" | "music-video" | "musicvideo" | "music video" => "music-video",
        other => other,
    };
    if formats()
        .items
        .iter()
        .any(|it| it.id.eq_ignore_ascii_case(canonical))
        || FORMATS.iter().any(|f| f.eq_ignore_ascii_case(canonical))
    {
        let id = formats()
            .items
            .iter()
            .find(|it| it.id.eq_ignore_ascii_case(canonical))
            .map(|it| it.id.clone())
            .unwrap_or_else(|| canonical.to_string());
        return Ok(id);
    }
    Err(ShowError::Msg(format!(
        "unknown format: {raw} (want feature | 30min | 15s | episodic | advertisement | music-video)"
    )))
}

pub fn resolve_prompt_target(raw: &str) -> Result<String, ShowError> {
    let raw = raw.trim();
    let key = raw.to_ascii_lowercase().replace('_', "-");
    let canonical = match key.as_str() {
        "ltx-2.3" | "ltx23" | "ltx2.3" | "ltx-23" => "ltx-2.3",
        "ltx-2.5" | "ltx25" | "ltx2.5" | "ltx-25" => "ltx-2.5",
        "grok" | "imagine" | "grok-imagine" => "grok",
        "comfy" | "comfyui" => "comfy",
        "prompt-server" | "server" | "ps" => "prompt-server",
        "kling" | "kling-2" | "kling-3" | "kling2" | "kling3" => "kling",
        "veo" | "veo-3" | "veo3" | "veo-2" | "veo2" => "veo",
        "sora" | "sora-2" | "sora2" => "sora",
        "seedance" | "seedance-2" | "seedance2" => "seedance",
        "hailuo" | "minimax" | "minimax-h3" | "h3" => "hailuo",
        "flux" | "flux-3" | "flux3" => "flux",
        "midjourney" | "mj" => "midjourney",
        "gpt-image" | "gpt-image-2" | "dalle" | "gptimage" => "gpt-image",
        "krea" => "krea",
        "wan" | "wan-2.2" | "wan22" => "wan",
        "runway" | "gen-4" | "gen4" => "runway",
        other => other,
    };
    if let Some(it) = prompt_targets()
        .items
        .iter()
        .find(|it| it.id.eq_ignore_ascii_case(canonical))
    {
        return Ok(it.id.clone());
    }
    let want = prompt_targets()
        .items
        .iter()
        .map(|it| it.id.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    Err(ShowError::Msg(format!(
        "unknown prompt target: {raw} (want {want})"
    )))
}

pub fn lookup<'a>(pack: &'a PackFile, id: &str) -> Option<&'a PackItem> {
    pack.items.iter().find(|it| it.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn unique_ids(pack: &PackFile, name: &str) {
        let mut seen = HashSet::new();
        for it in &pack.items {
            assert!(!it.id.is_empty(), "{name}: empty id");
            assert!(
                seen.insert(it.id.as_str()),
                "{name}: duplicate id {}",
                it.id
            );
        }
    }

    #[test]
    fn packs_parse_dated_unique() {
        for (name, pack) in [
            ("genres", genres()),
            ("living", living_directors()),
            ("canon", canon_directors()),
            ("formats", formats()),
            ("prompt-targets", prompt_targets()),
        ] {
            assert!(!pack.reviewed.is_empty(), "{name} missing reviewed");
            assert!(
                pack.disclaimer.to_lowercase().contains("not")
                    || pack.disclaimer.to_lowercase().contains("coverage"),
                "{name} disclaimer should mark influence, not endorsement"
            );
            unique_ids(pack, name);
            assert!(!pack.items.is_empty(), "{name} empty");
        }
    }

    #[test]
    fn unknown_ids_name_the_kind() {
        let err = resolve_ids(IdKind::Genre, &["not-a-genre".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown genre"), "{err}");
        let err = resolve_ids(IdKind::Living, &["not-a-director".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown living"), "{err}");
        let err = resolve_ids(IdKind::Canon, &["not-a-director".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown canon"), "{err}");
    }

    #[test]
    fn format_ids() {
        assert_eq!(resolve_format("30Min").unwrap(), "30min");
        assert_eq!(resolve_format("ad").unwrap(), "advertisement");
        assert_eq!(resolve_format("commercial").unwrap(), "advertisement");
        assert_eq!(resolve_format("spot").unwrap(), "advertisement");
        assert_eq!(resolve_format("music-video").unwrap(), "music-video");
        assert_eq!(resolve_format("mv").unwrap(), "music-video");
        assert_eq!(resolve_format("musicvideo").unwrap(), "music-video");
        assert_eq!(resolve_prompt_target("ltx23").unwrap(), "ltx-2.3");
        assert_eq!(resolve_prompt_target("ltx-2.5").unwrap(), "ltx-2.5");
        assert_eq!(resolve_prompt_target("imagine").unwrap(), "grok");
        assert_eq!(resolve_prompt_target("ps").unwrap(), "prompt-server");
        assert_eq!(resolve_prompt_target("kling-3").unwrap(), "kling");
        assert_eq!(resolve_prompt_target("veo3").unwrap(), "veo");
        assert_eq!(resolve_prompt_target("mj").unwrap(), "midjourney");
        assert_eq!(resolve_prompt_target("minimax").unwrap(), "hailuo");
        assert_eq!(resolve_prompt_target("seedance-2").unwrap(), "seedance");
        assert!(resolve_prompt_target("not-a-model")
            .unwrap_err()
            .to_string()
            .contains("unknown prompt target"));
        assert!(resolve_format("webisode")
            .unwrap_err()
            .to_string()
            .contains("unknown format"));
    }
}
