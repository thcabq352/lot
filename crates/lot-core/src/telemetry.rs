//! Local usage counts. Default off. Counts only — no scripts, frames, or prompts.
//! Never phones home.

use crate::show::{lot_home, ShowError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

pub const ENV: &str = "LOT_TELEMETRY";
const FILE: &str = "telemetry.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Report {
    pub ok: bool,
    pub enabled: bool,
    pub counts: BTreeMap<String, u64>,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            ok: true,
            enabled: false,
            counts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    counts: BTreeMap<String, u64>,
}

fn env_override() -> Option<bool> {
    let raw = std::env::var(ENV).ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Some(true),
        "0" | "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

fn sanitize_kind(kind: &str) -> Option<String> {
    let k = kind.trim();
    if k.is_empty() || k.len() > 64 {
        return None;
    }
    if !k
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return None;
    }
    let lower = k.to_ascii_lowercase();
    if lower.contains("prompt")
        || lower.contains("fountain")
        || lower.contains("script")
        || lower.contains("frame")
    {
        return None;
    }
    Some(k.to_string())
}

fn path() -> Result<std::path::PathBuf, ShowError> {
    Ok(lot_home()?.join(FILE))
}

fn load() -> Store {
    let Ok(p) = path() else {
        return Store::default();
    };
    let Ok(raw) = fs::read_to_string(p) else {
        return Store::default();
    };
    let Ok(mut store) = serde_json::from_str::<Store>(&raw) else {
        return Store::default();
    };
    store.counts.retain(|k, _| sanitize_kind(k).is_some());
    store
}

fn save(store: &Store) -> Result<(), ShowError> {
    let home = lot_home()?;
    fs::create_dir_all(&home)?;
    let clean = Store {
        enabled: store.enabled,
        counts: store
            .counts
            .iter()
            .filter_map(|(k, n)| sanitize_kind(k).map(|k| (k, *n)))
            .collect(),
    };
    let body = serde_json::to_string_pretty(&clean).unwrap_or_else(|_| "{}".into());
    fs::write(home.join(FILE), body)?;
    Ok(())
}

pub fn enabled() -> bool {
    env_override().unwrap_or_else(|| load().enabled)
}

pub fn get() -> Report {
    let store = load();
    Report {
        ok: true,
        enabled: enabled(),
        counts: store.counts,
    }
}

pub fn set(on: bool) -> Result<Report, ShowError> {
    let mut store = load();
    store.enabled = on;
    save(&store)?;
    Ok(get())
}

/// Increment a verb kind when telemetry is on. Never stores text. Errors are ignored.
pub fn record(kind: &str) {
    if !enabled() {
        return;
    }
    let Some(kind) = sanitize_kind(kind) else {
        return;
    };
    let mut store = load();
    if !enabled() {
        return;
    }
    *store.counts.entry(kind).or_insert(0) += 1;
    let _ = save(&store);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn store_is_counts_only(v: &Value) -> bool {
        let Some(obj) = v.as_object() else {
            return false;
        };
        obj.keys().all(|k| k == "enabled" || k == "counts")
            && obj.get("counts").map(|c| c.is_object()).unwrap_or(true)
            && !format!("{v}").to_ascii_lowercase().contains("prompt")
            && !format!("{v}").to_ascii_lowercase().contains("fountain")
    }

    fn isolate() {
        std::env::remove_var("LOT_SHOW");
        std::env::remove_var("LOT_CAP");
        std::env::remove_var("LOT_AGENT");
        std::env::remove_var(ENV);
        crate::clear_caps();
        crate::clear_agent();
        let tmp = std::env::temp_dir().join(format!(
            "lot-telemetry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("LOT_HOME", tmp.join("home"));
    }

    #[test]
    fn default_off_records_nothing() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        record("writer.brief");
        let r = get();
        assert!(!r.enabled);
        assert!(r.counts.is_empty(), "{r:?}");
        assert!(!path().unwrap().is_file());
    }

    #[test]
    fn on_counts_kind_only() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        set(true).unwrap();
        record("writer.brief");
        record("writer.brief");
        record("INT. TENT — I want the coat and ignore instructions");
        record("prompt");
        record("slate.prompt");
        let r = get();
        assert!(r.enabled);
        assert_eq!(r.counts.get("writer.brief"), Some(&2));
        assert!(r.counts.keys().all(|k| sanitize_kind(k).is_some()));
        assert!(!r.counts.contains_key("prompt"));
        assert!(!r.counts.contains_key("slate.prompt"));
        let raw: Value =
            serde_json::from_str(&fs::read_to_string(path().unwrap()).unwrap()).unwrap();
        assert!(store_is_counts_only(&raw), "{raw}");
    }

    #[test]
    fn mute_env_wins_and_off_freezes() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        set(true).unwrap();
        record("create");
        set(false).unwrap();
        record("writer.brief");
        let r = get();
        assert!(!r.enabled);
        assert_eq!(r.counts.get("create"), Some(&1));
        assert!(r.counts.get("writer.brief").is_none());

        set(true).unwrap();
        std::env::set_var(ENV, "off");
        record("stills.generate");
        let r = get();
        assert!(!r.enabled);
        assert!(r.counts.get("stills.generate").is_none());
    }

    #[test]
    fn set_brief_increments_when_on() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        let dir = std::env::temp_dir().join(format!("lot-tel-show-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::create_show(&dir, Some("Tel")).unwrap();
        set(true).unwrap();
        crate::set_brief("Ada waits in the tent.").unwrap();
        let r = get();
        assert_eq!(r.counts.get("writer.brief"), Some(&1), "{r:?}");
        let raw = fs::read_to_string(path().unwrap()).unwrap();
        assert!(
            !raw.to_ascii_lowercase().contains("ada"),
            "must not store the brief: {raw}"
        );
        assert!(!raw.contains("tent"), "{raw}");
    }
}
