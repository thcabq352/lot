use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_phase() -> String {
    "writer".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scene {
    pub id: String,
    pub num: String,
    pub slug: String,
    #[serde(default)]
    pub int_ext: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub master: String,
    #[serde(default)]
    pub sub: String,
    #[serde(default)]
    pub area: String,
    #[serde(default)]
    pub tod: String,
    #[serde(default)]
    pub tod_bucket: String,
    #[serde(default)]
    pub eighths: u32,
    #[serde(default)]
    pub synopsis: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub characters: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Shot {
    pub id: String,
    /// Ingest key, e.g. "01". Never overwritten by dailies filename.
    pub num: String,
    /// Human label (slug / coverage). Ingest must not rename this to "01".
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub scene_id: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub move_kind: String,
    #[serde(default)]
    pub lens: String,
    #[serde(default)]
    pub desc: String,
    /// Slate continuity prompt. Lives on the show. Canon — targets do not replace this.
    #[serde(default)]
    pub prompt: String,
    /// Per-engine rewrites (ltx-2.3, ltx-2.5, grok, comfy, prompt-server).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prompt_targets: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loras: Vec<SlateLora>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still_path: Option<String>,
    /// `grok` or `comfy`. Never inferred from the other engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still_backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still_provenance: Option<crate::Provenance>,
    /// Motion Previs plate (owned copy under motion/). Not a pose/depth bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate_path: Option<String>,
    /// camera_only | actor_motion | object_motion | full_scene
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_mode: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub motion_move: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub motion_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_duration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_fps: Option<String>,
    /// 2D floor marks. 3D blocking stays in Blockout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stage_marks: Vec<StageMark>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Take {
    pub id: String,
    pub shot_id: String,
    pub path: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub circled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MediaItem {
    pub path: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Beat {
    pub id: String,
    #[serde(default)]
    pub act: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlateLora {
    pub id: String,
    #[serde(default = "default_lora_weight")]
    pub weight: String,
    /// Optional family: ltx-2.3, flux, wan, …
    #[serde(default)]
    pub model: String,
}

fn default_lora_weight() -> String {
    "1.0".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlateState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_target: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loras: Vec<SlateLora>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageMark {
    pub id: String,
    pub who: String,
    /// actor | camera | prop
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mark: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub x: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub z: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FinishState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<String>,
    #[serde(default)]
    pub upscaled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<crate::Provenance>,
}

pub fn default_phase_value() -> String {
    default_phase()
}

pub fn shot_num_from_scene(num: &str, index: usize) -> String {
    let digits: String = num.chars().take_while(|c| c.is_ascii_digit()).collect();
    if let Ok(n) = digits.parse::<u32>() {
        format!("{n:02}")
    } else {
        format!("{:02}", index + 1)
    }
}

/// Leading digits of a filename stem (`01-foo.mp4` → `01`).
pub fn filename_shot_prefix(filename: &str) -> Option<String> {
    let stem = filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(filename);
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

pub fn shot_nums_match(shot_num: &str, prefix: &str) -> bool {
    normalize_shot_key(shot_num) == normalize_shot_key(prefix)
}

fn normalize_shot_key(s: &str) -> String {
    let t = s.trim();
    let stripped = t.trim_start_matches('0');
    if stripped.is_empty() {
        "0".into()
    } else {
        stripped.to_string()
    }
}
