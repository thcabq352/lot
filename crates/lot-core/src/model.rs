use serde::{Deserialize, Serialize};

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
    /// Slate continuity prompt. Lives on the show.
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub locked: bool,
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
