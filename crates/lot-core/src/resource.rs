//! `lot://` cards. Agents read one resource without dumping show.json.

use crate::model::shot_nums_match;
use crate::show::{require_current, Show, ShowError};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ResourceRef {
    pub uri: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: &'static str,
    pub description: String,
}

pub fn resource_list() -> Result<(PathBuf, Show, Vec<ResourceRef>), ShowError> {
    crate::caps::require(crate::caps::Cap::Read)?;
    let (dir, show) = require_current()?;
    Ok((dir.clone(), show.clone(), list_for(&dir, &show)))
}

pub fn resource_read(uri: &str) -> Result<(PathBuf, Show, Value), ShowError> {
    crate::caps::require(crate::caps::Cap::Read)?;
    let (dir, show) = require_current()?;
    let card = read_for(&dir, &show, uri)?;
    Ok((dir, show, card))
}

pub fn list_for(dir: &std::path::Path, show: &Show) -> Vec<ResourceRef> {
    let _ = dir;
    let mut out = vec![ResourceRef {
        uri: "lot://show".into(),
        name: show.name.clone(),
        mime_type: "application/json",
        description: "Show meta, phase, school, lock, last event. Not the fountain.".into(),
    }];
    for sc in &show.scenes {
        out.push(ResourceRef {
            uri: format!("lot://scenes/{}", sc.id),
            name: if sc.slug.is_empty() {
                sc.num.clone()
            } else {
                format!("{} {}", sc.num, sc.slug)
            },
            mime_type: "application/json",
            description: "One scene card.".into(),
        });
    }
    for sh in &show.shots {
        out.push(ResourceRef {
            uri: format!("lot://shots/{}", sh.id),
            name: if sh.name.is_empty() {
                sh.num.clone()
            } else {
                format!("{} {}", sh.num, sh.name)
            },
            mime_type: "application/json",
            description: "One shot card.".into(),
        });
    }
    for tk in &show.takes {
        out.push(ResourceRef {
            uri: format!("lot://takes/{}", tk.id),
            name: if tk.filename.is_empty() {
                tk.id.clone()
            } else {
                tk.filename.clone()
            },
            mime_type: "application/json",
            description: "One take card.".into(),
        });
    }
    out
}

pub fn read_for(dir: &std::path::Path, show: &Show, uri: &str) -> Result<Value, ShowError> {
    let uri = uri.trim();
    let rest = uri
        .strip_prefix("lot://")
        .ok_or_else(|| ShowError::Msg(format!("unknown resource — {uri}")))?;
    if rest == "show" {
        return Ok(show_card(dir, show));
    }
    if let Some(id) = rest.strip_prefix("scenes/") {
        return scene_card(show, id);
    }
    if let Some(id) = rest.strip_prefix("shots/") {
        return shot_card(show, id);
    }
    if let Some(id) = rest.strip_prefix("takes/") {
        return take_card(show, id);
    }
    if let Some(id) = rest.strip_prefix("school/rubric/") {
        if !show.school.enabled {
            return Err(ShowError::Msg("school off — no rubric".into()));
        }
        return crate::school::rubric(id);
    }
    Err(ShowError::Msg(format!("unknown resource — {uri}")))
}

fn show_card(dir: &std::path::Path, show: &Show) -> Value {
    json!({
        "uri": "lot://show",
        "id": show.id,
        "name": show.name,
        "phase": show.phase,
        "rev": show.rev,
        "school": show.school,
        "locked_by": show.locked_by,
        "last_event": crate::audit::last_event(dir),
        "brief": show.writer.brief,
        "scenes": show.scenes.len(),
        "shots": show.shots.len(),
        "takes": show.takes.len(),
        "budget": show.budget,
    })
}

fn scene_card(show: &Show, key: &str) -> Result<Value, ShowError> {
    let sc = show
        .scenes
        .iter()
        .find(|s| s.id == key || s.num == key)
        .ok_or_else(|| ShowError::Msg(format!("unknown scene: {key}")))?;
    let mut v = serde_json::to_value(sc)?;
    if let Some(obj) = v.as_object_mut() {
        obj.insert("uri".into(), json!(format!("lot://scenes/{}", sc.id)));
    }
    Ok(v)
}

fn shot_card(show: &Show, key: &str) -> Result<Value, ShowError> {
    let sh = show
        .shots
        .iter()
        .find(|s| s.id == key || shot_nums_match(&s.num, key))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {key}")))?;
    let mut v = serde_json::to_value(sh)?;
    if let Some(obj) = v.as_object_mut() {
        obj.insert("uri".into(), json!(format!("lot://shots/{}", sh.id)));
    }
    Ok(v)
}

fn take_card(show: &Show, key: &str) -> Result<Value, ShowError> {
    let tk = show
        .takes
        .iter()
        .find(|t| t.id == key || t.filename == key)
        .ok_or_else(|| ShowError::Msg(format!("unknown take: {key}")))?;
    let mut v = serde_json::to_value(tk)?;
    if let Some(obj) = v.as_object_mut() {
        obj.insert("uri".into(), json!(format!("lot://takes/{}", tk.id)));
    }
    Ok(v)
}
