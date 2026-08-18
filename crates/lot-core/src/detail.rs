//! Lean mutation extras. Full shot/prompt cards only when detail=full.

use serde_json::{json, Value};
use std::cell::Cell;

thread_local! {
    static DETAIL_FULL: Cell<bool> = const { Cell::new(false) };
}

pub fn set_detail_full(full: bool) {
    DETAIL_FULL.with(|c| c.set(full));
}

pub fn clear_detail() {
    set_detail_full(false);
}

pub fn detail_full_active() -> bool {
    DETAIL_FULL.with(|c| c.get())
}

pub fn with_detail<T>(full: bool, f: impl FnOnce() -> T) -> T {
    let prev = DETAIL_FULL.with(|c| c.replace(full));
    let out = f();
    DETAIL_FULL.with(|c| c.set(prev));
    out
}

pub fn detail_full(raw: Option<&str>) -> bool {
    raw.map(|s| s.trim().eq_ignore_ascii_case("full"))
        .unwrap_or(false)
}

pub fn detail_full_value(v: Option<&Value>) -> bool {
    match v {
        Some(Value::String(s)) => detail_full(Some(s)),
        Some(Value::Bool(true)) => true,
        _ => false,
    }
}

/// Strip prompt dumps and full cards unless `full`.
pub fn lean_extra(extra: Value, full: bool) -> Value {
    if full {
        return extra;
    }
    let Value::Object(mut map) = extra else {
        return extra;
    };
    if let Some(shots) = map.remove("shots") {
        map.insert("shots".into(), lean_shots(&shots));
    }
    if let Some(takes) = map.remove("takes") {
        map.insert("takes".into(), lean_takes(&takes));
    }
    if let Some(cast) = map.remove("cast") {
        map.insert("cast".into(), lean_cast(&cast));
    }
    if let Some(stems) = map.remove("stems") {
        map.insert("stems".into(), lean_stems(&stems));
    }
    if let Some(slate) = map.remove("slate") {
        map.insert("slate".into(), lean_slate(&slate));
    }
    Value::Object(map)
}

fn lean_shots(v: &Value) -> Value {
    let Some(arr) = v.as_array() else {
        return v.clone();
    };
    Value::Array(
        arr.iter()
            .map(|s| {
                let marks = s
                    .get("stage_marks")
                    .and_then(|m| m.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                let mut card = json!({
                    "id": s.get("id").cloned().unwrap_or(Value::Null),
                    "num": s.get("num").cloned().unwrap_or(Value::Null),
                    "name": s.get("name").cloned().unwrap_or(Value::Null),
                    "size": s.get("size").cloned().unwrap_or(Value::Null),
                    "locked": s.get("locked").cloned().unwrap_or(json!(false)),
                    "marks": marks,
                });
                if let Some(obj) = card.as_object_mut() {
                    if let Some(media) = media_ref(s.get("ref_path"), None) {
                        obj.insert("ref".into(), media);
                    }
                    if let Some(media) = media_ref(s.get("still_path"), s.get("still_provenance")) {
                        obj.insert("still".into(), media);
                    }
                    if let Some(media) = media_ref(s.get("plate_path"), None) {
                        if let Some(d) = s.get("motion_duration").cloned() {
                            if let Some(m) = media.as_object() {
                                let mut m = m.clone();
                                m.insert("duration".into(), d);
                                obj.insert("plate".into(), Value::Object(m));
                            }
                        } else {
                            obj.insert("plate".into(), media);
                        }
                    }
                }
                card
            })
            .collect(),
    )
}

fn lean_takes(v: &Value) -> Value {
    let Some(arr) = v.as_array() else {
        return v.clone();
    };
    Value::Array(
        arr.iter()
            .map(|t| {
                json!({
                    "id": t.get("id").cloned().unwrap_or(Value::Null),
                    "shot_id": t.get("shot_id").cloned().unwrap_or(Value::Null),
                    "path": t.get("path").cloned().unwrap_or(Value::Null),
                    "sha256": t.get("sha256").cloned().unwrap_or(json!("")),
                    "duration_secs": t.get("duration_secs").cloned().unwrap_or(Value::Null),
                    "circled": t.get("circled").cloned().unwrap_or(json!(false)),
                })
            })
            .collect(),
    )
}

fn lean_cast(v: &Value) -> Value {
    let Some(arr) = v.as_array() else {
        return v.clone();
    };
    Value::Array(
        arr.iter()
            .map(|c| {
                json!({
                    "name": c.get("name").cloned().unwrap_or(Value::Null),
                    "function": c.get("function").cloned().unwrap_or(json!("")),
                })
            })
            .collect(),
    )
}

fn lean_stems(v: &Value) -> Value {
    json!({
        "soundtrack": media_ref(v.get("soundtrack_path"), v.get("soundtrack_provenance")),
        "vo": media_ref(v.get("vo_path"), v.get("vo_provenance")),
    })
}

fn lean_slate(v: &Value) -> Value {
    let ids: Vec<Value> = v
        .get("loras")
        .and_then(|l| l.as_array())
        .map(|a| a.iter().filter_map(|x| x.get("id").cloned()).collect())
        .unwrap_or_default();
    json!({
        "default_target": v.get("default_target").cloned().unwrap_or(Value::Null),
        "loras": ids,
    })
}

fn media_ref(path: Option<&Value>, provenance: Option<&Value>) -> Option<Value> {
    let path = path.and_then(|p| p.as_str()).filter(|s| !s.is_empty())?;
    let mut m = serde_json::Map::new();
    m.insert("path".into(), json!(path));
    if let Some(p) = provenance {
        if let Some(h) = p.get("prompt_hash").cloned() {
            if !h.is_null() {
                m.insert("sha256".into(), h);
            }
        }
        if let Some(d) = p.get("duration_ms").cloned() {
            if !d.is_null() {
                m.insert("duration_ms".into(), d);
            }
        }
    }
    Some(Value::Object(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lean_extra_strips_prompts_unless_full() {
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
        let lean = lean_extra(extra.clone(), false);
        assert_eq!(lean["shots"][0]["num"], "01");
        assert!(lean["shots"][0].get("prompt").is_none());
        assert!(lean["shots"][0].get("prompt_targets").is_none());
        assert_eq!(lean["shots"][0]["marks"], 1);
        let full = lean_extra(extra, true);
        assert_eq!(full["shots"][0]["prompt"], "wide tent, neon rain");
    }

    #[test]
    fn detail_full_reads_full_token() {
        assert!(detail_full(Some("full")));
        assert!(detail_full(Some("FULL")));
        assert!(!detail_full(Some("min")));
        assert!(!detail_full(None));
        assert!(detail_full_value(Some(&json!("full"))));
        assert!(detail_full_value(Some(&json!(true))));
        assert!(!detail_full_value(Some(&json!(false))));
    }
}
