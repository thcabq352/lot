//! Audit log: who / what / rev. Export redacts tokens.

use crate::show::{now_rfc3339, require_current, Show, ShowError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const EVENTS: &str = "events.jsonl";
const RESERVED: &[&str] = &["id", "at", "kind", "who", "rev", "show_id"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventMeta {
    pub id: String,
    pub at: String,
    pub kind: String,
    pub who: String,
    pub rev: u64,
}

pub fn stamp(kind: &str, show: &Show, extra: Option<Value>) -> (String, Value) {
    let id = new_event_id();
    let who = crate::agent::current().unwrap_or_else(|| "human".into());
    let mut line = json!({
        "id": id,
        "at": now_rfc3339(),
        "kind": kind,
        "who": who,
        "rev": show.rev,
        "show_id": show.id,
    });
    if let Some(Value::Object(map)) = extra {
        if let Some(obj) = line.as_object_mut() {
            for (k, v) in map {
                if RESERVED.contains(&k.as_str()) {
                    continue;
                }
                obj.insert(k, v);
            }
        }
    }
    (id, line)
}

pub fn last_event(dir: &Path) -> Option<EventMeta> {
    list_events(dir, Some(1)).into_iter().next()
}

pub fn list_events(dir: &Path, n: Option<usize>) -> Vec<EventMeta> {
    let raw = read_lines(dir);
    let mut out: Vec<EventMeta> = raw.iter().filter_map(meta_from).collect();
    if let Some(limit) = n {
        if out.len() > limit {
            out.drain(0..out.len() - limit);
        }
    }
    out
}

pub fn list_raw(dir: &Path, n: Option<usize>) -> Vec<Value> {
    let mut raw = read_lines(dir);
    if let Some(limit) = n {
        if raw.len() > limit {
            raw.drain(0..raw.len() - limit);
        }
    }
    raw
}

pub fn show_log(n: Option<u32>) -> Result<(PathBuf, Show, Vec<Value>), ShowError> {
    crate::caps::require(crate::caps::Cap::Read)?;
    let (dir, show) = require_current()?;
    let take = n.map(|x| x as usize);
    Ok((dir.clone(), show, list_raw(&dir, take)))
}

pub fn export_log() -> Result<(PathBuf, Show, PathBuf, usize), ShowError> {
    crate::caps::require(crate::caps::Cap::Export)?;
    let (dir, show) = require_current()?;
    let dest_dir = dir.join("audit");
    fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join("export.jsonl");
    let lines = read_lines(&dir);
    let mut f = fs::File::create(&dest)?;
    let mut n = 0usize;
    for v in &lines {
        let redacted = redact_value(v);
        writeln!(f, "{redacted}")?;
        n += 1;
    }
    Ok((dir, show, dest, n))
}

pub fn mutation_json(dir: &Path, show: &Show, extra: Value) -> Value {
    let extra = crate::lean_extra(extra, crate::detail_full_active());
    let ev = last_event(dir);
    let mut v = json!({
        "ok": true,
        "show": dir.display().to_string(),
        "show_id": show.id,
        "rev": show.rev,
        "event_id": ev.as_ref().map(|e| e.id.clone()),
        "who": ev.as_ref().map(|e| e.who.clone()).unwrap_or_else(|| {
            crate::agent::current().unwrap_or_else(|| "human".into())
        }),
        "school": show.school,
    });
    if let (Some(obj), Value::Object(map)) = (v.as_object_mut(), extra) {
        for (k, val) in map {
            if matches!(
                k.as_str(),
                "ok" | "show" | "show_id" | "rev" | "event_id" | "who" | "school"
            ) {
                continue;
            }
            obj.insert(k, val);
        }
    }
    v
}

pub fn redact_value(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                if sensitive_key(k) {
                    out.insert(k.clone(), json!("[redacted]"));
                } else {
                    out.insert(k.clone(), redact_value(val));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        Value::String(s) if looks_like_secret(s) => json!("[redacted]"),
        other => other.clone(),
    }
}

fn sensitive_key(k: &str) -> bool {
    let n = k.to_ascii_lowercase().replace('-', "_");
    matches!(
        n.as_str(),
        "token"
            | "secret"
            | "password"
            | "authorization"
            | "api_key"
            | "access_token"
            | "refresh_token"
            | "xai_token"
            | "bearer"
            | "client_secret"
    ) || n.ends_with("_token")
        || n.ends_with("_secret")
        || n.ends_with("_api_key")
}

fn looks_like_secret(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("sk-")
        || t.starts_with("xai-")
        || t.starts_with("gsk_")
        || t.starts_with("xox")
        || t.starts_with("Bearer ")
        || (t.starts_with("eyJ") && t.len() > 40)
}

fn read_lines(dir: &Path) -> Vec<Value> {
    let path = dir.join(EVENTS);
    let Ok(f) = fs::File::open(path) else {
        return Vec::new();
    };
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

fn meta_from(v: &Value) -> Option<EventMeta> {
    Some(EventMeta {
        id: v.get("id")?.as_str()?.to_string(),
        at: v.get("at")?.as_str()?.to_string(),
        kind: v.get("kind")?.as_str()?.to_string(),
        who: v.get("who")?.as_str()?.to_string(),
        rev: v.get("rev")?.as_u64()?,
    })
}

fn new_event_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("ev-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_drops_token_fields_and_key_prefixes() {
        let v = json!({
            "kind": "writer.brief",
            "token": "sk-secret-abc",
            "note": "Ada will not put it on",
            "nested": { "api_key": "xai-nope", "ok": true },
            "prompt": "wide tent"
        });
        let r = redact_value(&v);
        assert_eq!(r["token"], "[redacted]");
        assert_eq!(r["nested"]["api_key"], "[redacted]");
        assert_eq!(r["nested"]["ok"], true);
        assert_eq!(r["note"], "Ada will not put it on");
        assert_eq!(r["prompt"], "wide tent");
        assert_eq!(redact_value(&json!("sk-live-xyz")), "[redacted]");
        assert_eq!(redact_value(&json!("wide tent")), "wide tent");
    }

    #[test]
    fn mutation_json_has_cli_envelope() {
        let stamp = std::process::id();
        let root = std::env::temp_dir().join(format!("lot-audit-envelope-{stamp}"));
        let _ = fs::remove_dir_all(&root);
        std::env::set_var("LOT_HOME", root.join("home"));
        let (dir, show) = crate::create_show(&root.join("show"), Some("Envelope")).unwrap();
        let v = mutation_json(&dir, &show, json!({ "brief": "neon" }));
        assert_eq!(v["ok"], true);
        assert_eq!(v["show"], dir.display().to_string());
        assert_eq!(v["show_id"], show.id);
        assert_eq!(v["rev"], show.rev);
        assert!(v["event_id"].as_str().is_some());
        assert_eq!(v["who"], "human");
        assert_eq!(v["school"]["enabled"], false);
        assert_eq!(v["brief"], "neon");
    }

    #[test]
    fn mutation_json_leans_shots_unless_detail_full() {
        let stamp = std::process::id();
        let root = std::env::temp_dir().join(format!("lot-audit-lean-{stamp}"));
        let _ = fs::remove_dir_all(&root);
        std::env::set_var("LOT_HOME", root.join("home"));
        let (dir, show) = crate::create_show(&root.join("show"), Some("Lean")).unwrap();
        let extra = json!({
            "shots": [{
                "id": "sh-01",
                "num": "01",
                "name": "INT. TENT - NIGHT",
                "size": "WIDE",
                "locked": false,
                "prompt": "wide tent, neon rain",
                "prompt_targets": { "kling": "do not dump" },
                "stage_marks": [{ "who": "Ada" }]
            }]
        });
        crate::clear_detail();
        let lean = mutation_json(&dir, &show, extra.clone());
        assert_eq!(lean["shots"][0]["num"], "01");
        assert!(lean["shots"][0].get("prompt").is_none(), "{lean}");
        assert_eq!(lean["shots"][0]["marks"], 1);
        crate::with_detail(true, || {
            let full = mutation_json(&dir, &show, extra);
            assert_eq!(full["shots"][0]["prompt"], "wide tent, neon rain");
        });
    }
}
