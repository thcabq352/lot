//! Old-suite import. Does not delete the source. No invented glTF / OpenPose / PNG.

use crate::model::{
    filename_shot_prefix, shot_num_from_scene, shot_nums_match, Beat, Shot, SlateLora, StageMark,
    Take,
};
use crate::show::{
    append_event_with, bump, require_write_current, write_show, Show, ShowError,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub kind: String,
    pub source: String,
    pub kept: bool,
    pub added: Value,
}

pub fn import_file(file: &Path) -> Result<(PathBuf, Show, ImportReport), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let src = crate::jail::allow_source(file, &dir)?;
    if !src.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", src.display())));
    }
    let raw = fs::read_to_string(&src)?;
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("import.json")
        .to_string();
    let kind = detect(&name, &raw);
    if kind == "script" || kind == "scriptbreak" {
        crate::breakdown_parse(Some(&src))?;
        let show = crate::show::read_show(&dir)?;
        keep_copy(&dir, &src, &name)?;
        let report = ImportReport {
            kind,
            source: src.display().to_string(),
            kept: src.is_file(),
            added: json!({
                "scenes": show.scenes.len(),
                "shots": show.shots.len(),
            }),
        };
        return Ok((dir, show, report));
    }
    let v: Value = serde_json::from_str(&raw).map_err(|_| {
        ShowError::Msg(
            "unknown import — not scriptbreak, cork-board, canvas, blockout, sbref, slate, or ctake"
                .into(),
        )
    })?;
    let added = match kind.as_str() {
        "cork-board" => import_wall(&mut show, &v)?,
        "canvas" => import_canvas(&mut show, &v)?,
        "blockout" => import_blockout(&mut show, &v)?,
        "sbref" => import_sbref(&dir, &src, &mut show, &v)?,
        "slate" => import_slate(&mut show, &v)?,
        "ctake" => import_ctake(&src, &mut show, &v)?,
        _ => {
            return Err(ShowError::Msg(
                "unknown import — not scriptbreak, cork-board, canvas, blockout, sbref, slate, or ctake"
                    .into(),
            ))
        }
    };
    keep_copy(&dir, &src, &name)?;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "show.import",
        &show,
        Some(json!({
            "kind": kind,
            "source": src.display().to_string(),
            "added": added,
        })),
    )?;
    let report = ImportReport {
        kind,
        source: src.display().to_string(),
        kept: src.is_file(),
        added,
    };
    Ok((dir, show, report))
}

pub fn detect(filename: &str, raw: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".scriptbreak") {
        return "scriptbreak".into();
    }
    if lower.ends_with(".fountain") || lower.ends_with(".txt") {
        return "script".into();
    }
    if lower.ends_with(".cork-board.json") || lower.contains("cork-board") {
        return "cork-board".into();
    }
    if lower.ends_with(".blockout") {
        return "blockout".into();
    }
    if lower.ends_with(".sbref") {
        return "sbref".into();
    }
    if lower.ends_with(".ctake") {
        return "ctake".into();
    }
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        if let Some(app) = v.get("app").and_then(|a| a.as_str()) {
            return kind_from_app(app);
        }
        if v.get("state").and_then(|s| s.get("scenes")).is_some() || raw.contains("scriptbreak")
        {
            return "scriptbreak".into();
        }
        if v.get("cards").is_some() || v.get("beats").is_some() {
            return "cork-board".into();
        }
        if v.get("takes").is_some() || v.get("clips").is_some() {
            return "ctake".into();
        }
        if v.get("prompt_targets").is_some()
            || (lower == "project.json" && v.get("shots").is_some())
        {
            return "slate".into();
        }
        if v.get("marks").is_some() || v.get("blockout").is_some() {
            return "blockout".into();
        }
        if v.get("stills").is_some() || v.get("boards").is_some() {
            return "sbref".into();
        }
        if v.get("shots").is_some() {
            return "canvas".into();
        }
    }
    "unknown".into()
}

fn kind_from_app(app: &str) -> String {
    let a = app.trim().to_ascii_lowercase();
    match a.as_str() {
        "scriptbreak" | "script-break" => "scriptbreak".into(),
        "cork-board" | "corkboard" | "cork_board" => "cork-board".into(),
        "master-canvas" | "canvas" | "picture" => "canvas".into(),
        "blockout" | "block-out" => "blockout".into(),
        "sbref" | "storyboard-reference" | "board" => "sbref".into(),
        "slate" => "slate".into(),
        "circle-take" | "ctake" | "dailies" => "ctake".into(),
        other => other.to_string(),
    }
}

fn keep_copy(dir: &Path, src: &Path, name: &str) -> Result<(), ShowError> {
    let dest_dir = dir.join("import");
    fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(name);
    if dest != src {
        let _ = fs::copy(src, dest);
    }
    Ok(())
}

fn import_wall(show: &mut Show, v: &Value) -> Result<Value, ShowError> {
    let arr = first_array(v, &["cards", "beats", "items"]).ok_or_else(|| {
        ShowError::Msg("cork-board has no cards — did not invent beats".into())
    })?;
    let mut n = 0u32;
    for card in arr {
        let text = first_str(card, &["text", "title", "body", "note", "beat"]);
        if text.is_empty() {
            continue;
        }
        if show.wall.iter().any(|b| b.text == text) {
            continue;
        }
        let i = show.wall.len() + 1;
        show.wall.push(Beat {
            id: format!("beat-{i}"),
            act: first_str(card, &["act", "column", "lane"]),
            text,
        });
        n += 1;
    }
    if n == 0 && show.wall.is_empty() {
        return Err(ShowError::Msg(
            "cork-board has no cards — did not invent beats".into(),
        ));
    }
    show.phase = "wall".into();
    Ok(json!({ "beats": n }))
}

fn import_canvas(show: &mut Show, v: &Value) -> Result<Value, ShowError> {
    let arr = first_array(v, &["shots", "cards", "items"]).ok_or_else(|| {
        ShowError::Msg("canvas has no shots — did not invent cards".into())
    })?;
    let mut n = 0u32;
    for row in arr {
        let num = shot_num_of(row, show.shots.len());
        if num.is_empty() {
            continue;
        }
        let name = first_str(row, &["name", "title", "slug", "label"]);
        let locked = row
            .get("locked")
            .or_else(|| row.get("lock"))
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let shot = upsert_shot(show, &num);
        if !name.is_empty() && name != shot.num {
            shot.name = name;
        }
        if locked {
            shot.locked = true;
        }
        apply_camera(shot, row);
        n += 1;
    }
    if n == 0 {
        return Err(ShowError::Msg(
            "canvas has no shots — did not invent cards".into(),
        ));
    }
    show.phase = "picture".into();
    Ok(json!({ "shots": n }))
}

fn import_blockout(show: &mut Show, v: &Value) -> Result<Value, ShowError> {
    if looks_3d_only(v) && first_array(v, &["shots", "marks", "cameras"]).is_none() {
        return Err(ShowError::Msg(
            "no 2D marks — Blockout 3D stays in Blockout. Did not invent glTF.".into(),
        ));
    }
    let mut marks = 0u32;
    if let Some(arr) = first_array(v, &["shots"]) {
        for row in arr {
            let num = shot_num_of(row, show.shots.len());
            if num.is_empty() {
                continue;
            }
            let shot = upsert_shot(show, &num);
            apply_camera(shot, row);
            if let Some(cam) = row.get("camera") {
                apply_camera(shot, cam);
            }
            marks += push_marks(shot, row);
        }
    }
    if let Some(arr) = first_array(v, &["marks"]) {
        let num = first_str(v, &["shot", "num"]).if_empty("01");
        let shot = upsert_shot(show, &num);
        for m in arr {
            if push_one_mark(shot, m) {
                marks += 1;
            }
        }
    }
    if marks == 0 && !show.shots.iter().any(|s| !s.stage_marks.is_empty() || !s.size.is_empty())
    {
        return Err(ShowError::Msg(
            "no 2D marks — Blockout 3D stays in Blockout. Did not invent glTF.".into(),
        ));
    }
    show.phase = "stage".into();
    Ok(json!({ "marks": marks }))
}

fn import_sbref(
    show_dir: &Path,
    src: &Path,
    show: &mut Show,
    v: &Value,
) -> Result<Value, ShowError> {
    let arr = first_array(v, &["shots", "boards", "stills", "items"]).ok_or_else(|| {
        ShowError::Msg("sbref has no boards — did not invent a still".into())
    })?;
    let mut n = 0u32;
    for row in arr {
        let num = shot_num_of(row, show.shots.len());
        if num.is_empty() {
            continue;
        }
        let shot = upsert_shot(show, &num);
        let prompt = first_str(row, &["prompt", "canon", "text"]);
        if !prompt.is_empty() {
            shot.prompt = prompt;
        }
        if let Some(p) = resolve_media(src, row, &["still", "image", "path", "file"]) {
            if let Ok(ok) = crate::jail::allow_source(&p, show_dir) {
                if ok.is_file() {
                    shot.still_path = Some(ok.display().to_string());
                }
            }
        }
        n += 1;
    }
    if n == 0 {
        return Err(ShowError::Msg(
            "sbref has no boards — did not invent a still".into(),
        ));
    }
    show.phase = "board".into();
    Ok(json!({ "shots": n }))
}

fn import_slate(show: &mut Show, v: &Value) -> Result<Value, ShowError> {
    if let Some(t) = v
        .get("target")
        .or_else(|| v.get("default_target"))
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        show.slate.default_target = Some(t.to_string());
    }
    let arr = first_array(v, &["shots", "items"]).ok_or_else(|| {
        ShowError::Msg("slate has no shots — did not invent a prompt".into())
    })?;
    let mut n = 0u32;
    for row in arr {
        let num = shot_num_of(row, show.shots.len());
        if num.is_empty() {
            continue;
        }
        let shot = upsert_shot(show, &num);
        let prompt = first_str(row, &["prompt", "canon", "text"]);
        if !prompt.is_empty() {
            shot.prompt = prompt;
        }
        if let Some(map) = row
            .get("targets")
            .or_else(|| row.get("prompt_targets"))
            .and_then(|x| x.as_object())
        {
            for (k, val) in map {
                if let Some(s) = val.as_str() {
                    if !s.trim().is_empty() {
                        shot.prompt_targets.insert(k.clone(), s.to_string());
                    }
                }
            }
        }
        if let Some(loras) = row.get("loras").and_then(|x| x.as_array()) {
            for l in loras {
                let id = first_str(l, &["id", "name"]);
                if id.is_empty() {
                    continue;
                }
                if !shot.loras.iter().any(|x| x.id == id) {
                    shot.loras.push(SlateLora {
                        id,
                        weight: first_str(l, &["weight"]).if_empty("1.0"),
                        model: first_str(l, &["model"]),
                    });
                }
            }
        }
        n += 1;
    }
    if n == 0 {
        return Err(ShowError::Msg(
            "slate has no shots — did not invent a prompt".into(),
        ));
    }
    show.phase = "slate".into();
    Ok(json!({ "shots": n }))
}

fn import_ctake(src: &Path, show: &mut Show, v: &Value) -> Result<Value, ShowError> {
    let arr = first_array(v, &["takes", "clips", "items"]).ok_or_else(|| {
        ShowError::Msg("ctake has no takes — did not invent a clip".into())
    })?;
    let mut n = 0u32;
    for row in arr {
        let filename = {
            let f = first_str(row, &["filename", "name"]);
            if !f.is_empty() {
                f
            } else {
                first_str(row, &["path", "file"])
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("")
                    .to_string()
            }
        };
        if filename.is_empty() {
            continue;
        }
        let prefix = filename_shot_prefix(&filename).ok_or_else(|| {
            ShowError::Msg(format!(
                "filename must start with a shot number: {filename}"
            ))
        })?;
        if !show.shots.iter().any(|s| shot_nums_match(&s.num, &prefix)) {
            let i = show.shots.len();
            show.shots.push(Shot {
                id: format!("sh-{}", i + 1),
                num: shot_num_from_scene(&prefix, i),
                name: String::new(),
                ..Shot::default()
            });
        }
        let shot = show
            .shots
            .iter()
            .find(|s| shot_nums_match(&s.num, &prefix))
            .cloned()
            .unwrap();
        let path = resolve_media(src, row, &["path", "file"])
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| first_str(row, &["path", "file"]));
        let sha = first_str(row, &["sha256", "sha"]);
        if show.takes.iter().any(|t| {
            (!sha.is_empty() && t.sha256 == sha)
                || (t.filename == filename && t.path == path)
        }) {
            continue;
        }
        let id = first_str(row, &["id"]);
        let id = if id.is_empty() {
            format!("tk-{}", show.takes.len() + 1)
        } else {
            id
        };
        show.takes.push(Take {
            id,
            shot_id: shot.id,
            path,
            filename,
            sha256: sha,
            duration_secs: row.get("duration_secs").and_then(|x| x.as_f64()),
            circled: row
                .get("circled")
                .or_else(|| row.get("circle"))
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
        });
        n += 1;
    }
    if n == 0 && show.takes.is_empty() {
        return Err(ShowError::Msg(
            "ctake has no takes — did not invent a clip".into(),
        ));
    }
    show.phase = "dailies".into();
    Ok(json!({ "takes": n }))
}

fn upsert_shot<'a>(show: &'a mut Show, num: &str) -> &'a mut Shot {
    if let Some(i) = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, num))
    {
        return &mut show.shots[i];
    }
    let i = show.shots.len();
    show.shots.push(Shot {
        id: format!("sh-{}", i + 1),
        num: shot_num_from_scene(num, i),
        name: String::new(),
        ..Shot::default()
    });
    let last = show.shots.len() - 1;
    &mut show.shots[last]
}

fn apply_camera(shot: &mut Shot, row: &Value) {
    let size = first_str(row, &["size", "shot_size"]);
    if !size.is_empty() {
        shot.size = size;
    }
    let angle = first_str(row, &["angle"]);
    if !angle.is_empty() {
        shot.angle = angle;
    }
    let lens = first_str(row, &["lens"]);
    if !lens.is_empty() {
        shot.lens = lens;
    }
    let mv = first_str(row, &["move", "move_kind"]);
    if !mv.is_empty() {
        shot.move_kind = mv;
    }
}

fn push_marks(shot: &mut Shot, row: &Value) -> u32 {
    let Some(arr) = first_array(row, &["marks", "stage_marks"]) else {
        return 0;
    };
    let mut n = 0;
    for m in arr {
        if push_one_mark(shot, m) {
            n += 1;
        }
    }
    n
}

fn push_one_mark(shot: &mut Shot, m: &Value) -> bool {
    let who = first_str(m, &["who", "name"]);
    if who.is_empty() {
        return false;
    }
    let mark = first_str(m, &["mark", "label"]);
    let x = first_str(m, &["x"]);
    let z = first_str(m, &["z"]);
    if shot.stage_marks.iter().any(|s| s.who == who && s.mark == mark && s.x == x && s.z == z)
    {
        return false;
    }
    let i = shot.stage_marks.len() + 1;
    shot.stage_marks.push(StageMark {
        id: format!("mk-{i}"),
        who,
        kind: first_str(m, &["kind"]).if_empty("actor"),
        mark,
        x,
        z,
        notes: first_str(m, &["notes"]),
    });
    true
}

fn looks_3d_only(v: &Value) -> bool {
    v.get("gltf").is_some()
        || v.get("meshes").is_some()
        || v.get("glb").is_some()
        || v.get("depth").is_some()
}

fn shot_num_of(row: &Value, fallback_index: usize) -> String {
    let raw = first_str(row, &["num", "shot", "shot_num", "id"]);
    if raw.is_empty() {
        return String::new();
    }
    if raw.chars().any(|c| c.is_ascii_digit()) {
        shot_num_from_scene(&raw, fallback_index)
    } else {
        raw
    }
}

fn first_array<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    for k in keys {
        if let Some(a) = v.get(*k).and_then(|x| x.as_array()) {
            return Some(a);
        }
    }
    v.get("state").and_then(|s| {
        for k in keys {
            if let Some(a) = s.get(*k).and_then(|x| x.as_array()) {
                return Some(a);
            }
        }
        None
    })
}

fn first_str(v: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()).map(str::trim) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
        if let Some(n) = v.get(*k).and_then(|x| x.as_u64()) {
            return n.to_string();
        }
    }
    String::new()
}

fn resolve_media(src: &Path, row: &Value, keys: &[&str]) -> Option<PathBuf> {
    let p = first_str(row, keys);
    if p.is_empty() {
        return None;
    }
    let pb = PathBuf::from(&p);
    if pb.is_file() {
        return Some(pb);
    }
    if let Some(parent) = src.parent() {
        let joined = parent.join(&p);
        if joined.is_file() {
            return Some(joined);
        }
    }
    Some(pb)
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_kinds() {
        assert_eq!(detect("x.scriptbreak", "{}"), "scriptbreak");
        assert_eq!(
            detect("board.cork-board.json", r#"{"cards":[]}"#),
            "cork-board"
        );
        assert_eq!(detect("set.blockout", r#"{"marks":[]}"#), "blockout");
        assert_eq!(detect("ref.sbref", r#"{"boards":[]}"#), "sbref");
        assert_eq!(detect("day.ctake", r#"{"takes":[]}"#), "ctake");
        assert_eq!(
            detect("project.json", r#"{"app":"slate","shots":[]}"#),
            "slate"
        );
        assert_eq!(
            detect("wall.json", r#"{"app":"cork-board","cards":[{"text":"a"}]}"#),
            "cork-board"
        );
    }
}
