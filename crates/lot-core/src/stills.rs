//! Stills: Grok Imagine or local Comfy. Backend is required. No silent swap.
//! Board export packages stills + slate prompts. Never a fake PNG.

use crate::brain::{complete_vision, grok_auth};
use crate::model::{shot_nums_match, MediaItem};
use crate::show::{
    append_event, append_event_with, bump, require_write_current, write_show, Show, ShowError,
};
use crate::Provenance;
use base64::Engine;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const DESCRIBE_SYSTEM: &str = "You are Lot, looking at a production still or plate frame. \
Describe what is actually in the image: framing, light, wardrobe, set, faces. \
Do not invent a shot that is not in the picture. Do not wrap the answer in markdown fences.";

pub const BUNDLED_COMFY_STILL: &str = "comfy-flux-still.json";

fn comfy_workflow_disabled(raw: &str) -> bool {
    let s = raw.trim();
    s.is_empty() || s == "-" || s.eq_ignore_ascii_case("off")
}

/// Env path, or the Flux lock pack. `LOT_COMFY_WORKFLOW=off` skips the pack (tests).
pub fn resolve_comfy_workflow() -> Result<PathBuf, ShowError> {
    match std::env::var("LOT_COMFY_WORKFLOW") {
        Ok(s) if comfy_workflow_disabled(&s) => Err(ShowError::Msg(
            "no comfy stills — set LOT_COMFY_WORKFLOW to a JSON graph with {{prompt}}. Did not call Grok."
                .into(),
        )),
        Ok(s) => {
            let wf_path = s.trim().to_string();
            let p = PathBuf::from(&wf_path);
            if p.is_file() {
                Ok(p)
            } else {
                Err(ShowError::Msg(format!(
                    "no comfy stills — workflow not a file: {wf_path}. Did not call Grok."
                )))
            }
        }
        Err(_) => bundled_comfy_workflow().ok_or_else(|| {
            ShowError::Msg(
                "no comfy stills — set LOT_COMFY_WORKFLOW to a JSON graph with {{prompt}}. Did not call Grok."
                    .into(),
            )
        }),
    }
}

pub fn comfy_workflow_ready() -> bool {
    resolve_comfy_workflow().is_ok()
}

fn bundled_comfy_workflow() -> Option<PathBuf> {
    let mut cands = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("packs")
        .join(BUNDLED_COMFY_STILL)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("packs").join(BUNDLED_COMFY_STILL));
            cands.push(
                dir.join("../../crates/lot-core/packs")
                    .join(BUNDLED_COMFY_STILL),
            );
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut p = cwd;
        for _ in 0..6 {
            cands.push(p.join("crates/lot-core/packs").join(BUNDLED_COMFY_STILL));
            if !p.pop() {
                break;
            }
        }
    }
    cands.into_iter().find(|p| p.is_file())
}

pub fn resolve_stills_backend(raw: &str) -> Result<&'static str, ShowError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "grok" | "imagine" | "grok-imagine" => Ok("grok"),
        "comfy" | "comfyui" => Ok("comfy"),
        other => Err(ShowError::Msg(format!(
            "stills backend must be grok or comfy (got {other}) — no silent swap"
        ))),
    }
}

pub fn stills_generate(
    shot_num: &str,
    backend: &str,
    prompt: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let backend = resolve_stills_backend(backend)?;
    match backend {
        "grok" => crate::caps::require(crate::caps::Cap::Spend)?,
        "comfy" => crate::caps::require(crate::caps::Cap::Render)?,
        _ => crate::caps::require_write()?,
    }
    let (dir, mut show) = require_write_current()?;
    match backend {
        "grok" => crate::budget::require_spend(&show)?,
        "comfy" => crate::budget::require_render(&show)?,
        _ => {}
    }
    let shot_i = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    let explicit = prompt.map(str::trim).filter(|s| !s.is_empty());
    if let Some(t) = explicit {
        show.shots[shot_i].prompt = t.to_string();
    }
    let text = explicit
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::slate::prompt_for_target(&show.shots[shot_i], backend));
    if text.is_empty() {
        return Err(ShowError::Msg(
            "stills needs a prompt (slate --prompt or stills --prompt)".into(),
        ));
    }
    let num = show.shots[shot_i].num.clone();
    fs::create_dir_all(dir.join("stills"))?;

    let (bytes, prov) = match backend {
        "grok" => generate_grok(&text)?,
        "comfy" => generate_comfy(&text)?,
        _ => unreachable!(),
    };
    match backend {
        "grok" => crate::budget::record_spend(&mut show),
        "comfy" => crate::budget::record_render(&mut show),
        _ => {}
    }
    let ext = ext_from_bytes(&bytes);
    let dest = dir.join("stills").join(format!("{num}.{ext}"));
    fs::write(&dest, &bytes)?;
    if dest.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = fs::remove_file(&dest);
        return Err(ShowError::Msg(
            "stills wrote no image — refusing an empty stub".into(),
        ));
    }

    let path_s = dest.display().to_string();
    show.shots[shot_i].still_path = Some(path_s.clone());
    show.shots[shot_i].still_backend = Some(backend.to_string());
    show.shots[shot_i].still_provenance = Some(prov.clone());
    show.stills_backend = Some(backend.to_string());
    show.media.push(MediaItem {
        path: path_s.clone(),
        kind: "still".into(),
        ..MediaItem::default()
    });
    show.phase = "board".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "stills.generate",
        &show,
        Some(json!({
            "shot": num,
            "backend": backend,
            "still": path_s,
            "provenance": prov,
        })),
    )?;
    Ok((dir, show))
}

/// Look at a still, plate frame, or --file. Grok vision #1 when online; Ollama VL locally.
/// Never invent a description if no vision brain answers.
pub fn stills_describe(shot_num: &str, file: Option<&Path>) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_write_current()?;
    let shot_i = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    if let Some(p) = file {
        crate::jail::allow_source(p, &dir)?;
    }
    let (bytes, mime, source) = load_look_image(&show.shots[shot_i], file)?;
    let shot = &show.shots[shot_i];
    let mut user = format!("Show: {}\nShot: {}", show.name, shot.num);
    if !shot.name.is_empty() {
        user.push_str(&format!(" ({})", shot.name));
    }
    user.push('\n');
    if !shot.prompt.is_empty() {
        user.push_str(&format!("Slate canon: {}\n", shot.prompt));
    }
    user.push_str("Describe this frame for the board.\n");
    let looked = complete_vision(DESCRIBE_SYSTEM, &user, &bytes, mime)?;
    let text = looked.text.trim().to_string();
    if text.is_empty() {
        return Err(ShowError::Msg(
            "no vision — empty look. Did not invent a description.".into(),
        ));
    }
    show.shots[shot_i].desc = text;
    let num = show.shots[shot_i].num.clone();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "stills.describe",
        &show,
        Some(json!({
            "shot": num,
            "source": source,
            "backend": looked.provenance.backend,
            "model": looked.provenance.model,
            "provenance": looked.provenance,
        })),
    )?;
    Ok((dir, show))
}

pub fn board_export() -> Result<(PathBuf, Show, PathBuf), ShowError> {
    crate::caps::require(crate::caps::Cap::Export)?;
    let (dir, mut show) = require_write_current()?;
    if show.shots.is_empty() {
        return Err(ShowError::Msg(
            "board export needs shots (breakdown parse, then stills / slate)".into(),
        ));
    }
    fs::create_dir_all(dir.join("board"))?;
    let mut rows = Vec::new();
    for shot in &show.shots {
        if let Some(src) = shot.still_path.as_deref() {
            let src_p = Path::new(src);
            if src_p.is_file() {
                let name = src_p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("still.png");
                let dest = dir.join("board").join(name);
                fs::copy(src_p, &dest)?;
            }
        }
        rows.push(json!({
            "num": shot.num,
            "name": shot.name,
            "desc": shot.desc,
            "prompt": shot.prompt,
            "prompt_targets": shot.prompt_targets,
            "loras": shot.loras,
            "plate": shot.plate_path,
            "still": shot.still_path,
            "backend": shot.still_backend,
            "locked": shot.locked,
        }));
    }
    let pack = json!({
        "show": show.name,
        "rev": show.rev,
        "shots": rows,
    });
    let json_path = dir.join("board").join("board.json");
    fs::write(&json_path, serde_json::to_string_pretty(&pack)?)?;

    let mut md = format!("# Board — {}\n\n", show.name);
    for shot in &show.shots {
        md.push_str(&format!("## Shot {}\n\n", shot.num));
        if !shot.name.is_empty() {
            md.push_str(&format!("Name: {}\n\n", shot.name));
        }
        if !shot.desc.is_empty() {
            md.push_str(&format!("Look:\n\n{}\n\n", shot.desc));
        }
        if !shot.prompt.is_empty() {
            md.push_str(&format!("Prompt:\n\n{}\n\n", shot.prompt));
        }
        match shot.still_path.as_deref() {
            Some(p) => md.push_str(&format!("Still ({:?}): `{p}`\n\n", shot.still_backend)),
            None => md.push_str("Still: _(none)_\n\n"),
        }
    }
    fs::write(dir.join("board").join("board.md"), md)?;

    // Handoff: prompts already live on shots (Slate). Phase stays board.
    show.phase = "board".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "board.export", &show)?;
    Ok((dir, show, json_path))
}

fn generate_grok(prompt: &str) -> Result<(Vec<u8>, Provenance), ShowError> {
    let started = Instant::now();
    let (token, base, auth) = grok_auth().ok_or_else(|| {
        ShowError::Msg(
            "no grok stills — set HERMES xai-oauth, LOT_XAI_TOKEN, or XAI_API_KEY. Did not call Comfy."
                .into(),
        )
    })?;
    let model = std::env::var("LOT_STILLS_GROK_MODEL")
        .or_else(|_| std::env::var("LOT_GROK_IMAGE_MODEL"))
        .unwrap_or_else(|_| "grok-imagine-image-2.0".into());
    let seed = std::env::var("LOT_STILLS_SEED")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());
    let url = format!("{base}/images/generations");
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "response_format": "b64_json",
    });
    if let Some(s) = seed {
        body["seed"] = json!(s);
    }
    let timeout = Duration::from_secs(
        std::env::var("LOT_XAI_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(180),
    );
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(timeout)
        .build();
    let resp = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| ShowError::Msg(format!("no grok stills — http: {e}. Did not call Comfy.")))?;
    let status = resp.status();
    let v: Value = resp.into_json().map_err(|e| {
        ShowError::Msg(format!(
            "no grok stills — json status={status}: {e}. Did not call Comfy."
        ))
    })?;
    if status >= 400 {
        let msg = v
            .pointer("/error/message")
            .and_then(|m| m.as_str())
            .unwrap_or("request failed");
        return Err(ShowError::Msg(format!(
            "no grok stills — status={status}: {msg}. Did not call Comfy."
        )));
    }
    let bytes = image_from_generation(&v, &agent, &token)?;
    let mut prov = Provenance::new("grok", model, base, auth)
        .with_prompt(prompt)
        .with_duration_ms(crate::brain::elapsed_ms(started));
    if let Some(s) = seed {
        prov = prov.with_seed(s);
    }
    Ok((bytes, prov))
}

fn image_from_generation(
    v: &Value,
    agent: &ureq::Agent,
    token: &str,
) -> Result<Vec<u8>, ShowError> {
    let item = v.pointer("/data/0").ok_or_else(|| {
        ShowError::Msg("no grok stills — missing data[0]. Did not call Comfy.".into())
    })?;
    if let Some(b64) = item.get("b64_json").and_then(|x| x.as_str()) {
        return decode_b64(b64);
    }
    if let Some(url) = item.get("url").and_then(|x| x.as_str()) {
        let resp = agent
            .get(url)
            .set("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(|e| {
                ShowError::Msg(format!("no grok stills — fetch: {e}. Did not call Comfy."))
            })?;
        let mut buf = Vec::new();
        resp.into_reader().read_to_end(&mut buf).map_err(|e| {
            ShowError::Msg(format!("no grok stills — read: {e}. Did not call Comfy."))
        })?;
        if buf.is_empty() {
            return Err(ShowError::Msg(
                "no grok stills — empty image. Did not call Comfy.".into(),
            ));
        }
        return Ok(buf);
    }
    Err(ShowError::Msg(
        "no grok stills — no b64_json or url. Did not call Comfy.".into(),
    ))
}

fn decode_b64(raw: &str) -> Result<Vec<u8>, ShowError> {
    let t = raw.trim().split(',').next_back().unwrap_or(raw).trim();
    base64::engine::general_purpose::STANDARD
        .decode(t)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(t))
        .map_err(|_| ShowError::Msg("no grok stills — bad b64. Did not call Comfy.".into()))
}

fn generate_comfy(prompt: &str) -> Result<(Vec<u8>, Provenance), ShowError> {
    let started = Instant::now();
    let wf_file = resolve_comfy_workflow()?;
    let wf_path = wf_file.display().to_string();
    let raw = fs::read_to_string(&wf_file)?;
    if !raw.contains("{{prompt}}") {
        return Err(ShowError::Msg(
            "no comfy stills — workflow needs {{prompt}}. Did not call Grok.".into(),
        ));
    }
    let mut wf: Value = serde_json::from_str(&raw).map_err(|e| {
        ShowError::Msg(format!(
            "no comfy stills — workflow json: {e}. Did not call Grok."
        ))
    })?;
    replace_prompt(&mut wf, prompt);
    let mut prompt_obj = wf.get("prompt").cloned().unwrap_or(wf);
    let seed = apply_comfy_seed(&mut prompt_obj);
    let base = comfy_base();
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_secs(30))
        .build();
    let submitted = agent
        .post(&format!("{base}/prompt"))
        .set("Content-Type", "application/json")
        .send_json(json!({ "prompt": prompt_obj }))
        .map_err(|e| ShowError::Msg(format!("no comfy stills — {e}. Did not call Grok.")))?;
    let sub: Value = submitted.into_json().map_err(|e| {
        ShowError::Msg(format!(
            "no comfy stills — submit json: {e}. Did not call Grok."
        ))
    })?;
    let prompt_id = sub
        .get("prompt_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ShowError::Msg("no comfy stills — no prompt_id. Did not call Grok.".into()))?
        .to_string();

    let deadline = Instant::now()
        + Duration::from_secs(
            std::env::var("LOT_COMFY_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
        );
    let image_meta = loop {
        if Instant::now() > deadline {
            return Err(ShowError::Msg(
                "no comfy stills — timed out. Did not call Grok.".into(),
            ));
        }
        thread::sleep(Duration::from_millis(400));
        let hist = match agent.get(&format!("{base}/history/{prompt_id}")).call() {
            Ok(r) => r,
            Err(_) => continue,
        };
        let h: Value = match hist.into_json() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(img) = first_comfy_image(&h, &prompt_id) {
            break img;
        }
    };

    let view = format!(
        "{base}/view?filename={}&subfolder={}&type={}",
        urlencode(&image_meta.0),
        urlencode(&image_meta.1),
        urlencode(&image_meta.2)
    );
    let resp = agent
        .get(&view)
        .call()
        .map_err(|e| ShowError::Msg(format!("no comfy stills — view: {e}. Did not call Grok.")))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| ShowError::Msg(format!("no comfy stills — read: {e}. Did not call Grok.")))?;
    if buf.is_empty() {
        return Err(ShowError::Msg(
            "no comfy stills — empty image. Did not call Grok.".into(),
        ));
    }
    let mut prov = Provenance::new("comfy", wf_path, &base, "lot_comfy_workflow")
        .with_prompt(prompt)
        .with_seed(seed)
        .with_duration_ms(crate::brain::elapsed_ms(started));
    if let Some(vram) = comfy_vram_cap(&base) {
        prov = prov.with_vram_cap(vram);
    }
    Ok((buf, prov))
}

fn apply_comfy_seed(wf: &mut Value) -> u64 {
    let chosen = std::env::var("LOT_COMFY_SEED")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .or_else(|| first_graph_seed(wf))
        .unwrap_or_else(fresh_seed);
    set_graph_seeds(wf, chosen);
    chosen
}

fn first_graph_seed(wf: &Value) -> Option<u64> {
    let obj = wf.as_object()?;
    for node in obj.values() {
        if let Some(n) = node_seed(node) {
            return Some(n);
        }
    }
    None
}

fn node_seed(node: &Value) -> Option<u64> {
    let seed = node.get("inputs")?.get("seed")?;
    seed.as_u64()
        .or_else(|| seed.as_i64().and_then(|i| u64::try_from(i).ok()))
}

fn set_graph_seeds(wf: &mut Value, seed: u64) {
    let Some(obj) = wf.as_object_mut() else {
        return;
    };
    for node in obj.values_mut() {
        if let Some(inputs) = node.get_mut("inputs").and_then(|i| i.as_object_mut()) {
            if inputs.contains_key("seed") {
                inputs.insert("seed".into(), json!(seed));
            }
        }
    }
}

fn fresh_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

fn comfy_vram_cap(base: &str) -> Option<String> {
    if let Some(c) = crate::brain::vram_cap_from_env() {
        return Some(c);
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(250))
        .timeout_read(Duration::from_millis(400))
        .build();
    let v: Value = agent
        .get(&format!("{base}/system_stats"))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let bytes = v
        .pointer("/devices/0/vram_total")
        .and_then(|x| x.as_u64())?;
    Some(format!("{}mb", bytes / 1024 / 1024))
}

fn first_comfy_image(hist: &Value, prompt_id: &str) -> Option<(String, String, String)> {
    let outputs = hist
        .get(prompt_id)
        .or_else(|| hist.as_object().and_then(|m| m.values().next()))
        .and_then(|e| e.get("outputs"))?;
    for node in outputs.as_object()?.values() {
        if let Some(images) = node.get("images").and_then(|i| i.as_array()) {
            if let Some(img) = images.first() {
                let filename = img.get("filename")?.as_str()?.to_string();
                let sub = img
                    .get("subfolder")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let kind = img
                    .get("type")
                    .and_then(|s| s.as_str())
                    .unwrap_or("output")
                    .to_string();
                return Some((filename, sub, kind));
            }
        }
    }
    None
}

fn replace_prompt(v: &mut Value, prompt: &str) {
    match v {
        Value::String(s) if s.contains("{{prompt}}") => {
            *s = s.replace("{{prompt}}", prompt);
        }
        Value::Object(map) => {
            for val in map.values_mut() {
                replace_prompt(val, prompt);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                replace_prompt(val, prompt);
            }
        }
        _ => {}
    }
}

fn comfy_base() -> String {
    let raw = std::env::var("LOT_COMFY_URL").unwrap_or_else(|_| "http://127.0.0.1:8188".into());
    raw.trim()
        .trim_end_matches('/')
        .trim_end_matches("/system_stats")
        .to_string()
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn ext_from_bytes(b: &[u8]) -> &'static str {
    if b.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if b.len() > 2 && b[0] == 0xFF && b[1] == 0xD8 {
        "jpg"
    } else if b.starts_with(b"RIFF") {
        "webp"
    } else {
        "png"
    }
}

fn mime_from_bytes(b: &[u8]) -> &'static str {
    match ext_from_bytes(b) {
        "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn is_video_path(p: &Path) -> bool {
    match p
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4" | "mov" | "mkv" | "webm" | "m4v" | "avi") => true,
        _ => false,
    }
}

fn load_look_image(
    shot: &crate::model::Shot,
    file: Option<&Path>,
) -> Result<(Vec<u8>, &'static str, String), ShowError> {
    if let Some(p) = file {
        return read_image_or_frame(p);
    }
    if let Some(p) = shot.still_path.as_deref() {
        let p = Path::new(p);
        if p.is_file() {
            return read_image_or_frame(p);
        }
    }
    if let Some(p) = shot.plate_path.as_deref() {
        let p = Path::new(p);
        if p.is_file() {
            return read_image_or_frame(p);
        }
    }
    Err(ShowError::Msg(
        "no still — lot stills generate, attach a plate, or stills describe --file".into(),
    ))
}

fn read_image_or_frame(p: &Path) -> Result<(Vec<u8>, &'static str, String), ShowError> {
    if !p.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", p.display())));
    }
    if is_video_path(p) {
        return extract_frame(p);
    }
    let bytes = fs::read(p)?;
    if bytes.is_empty() {
        return Err(ShowError::Msg(format!(
            "no still — empty file: {}",
            p.display()
        )));
    }
    let mime = mime_from_bytes(&bytes);
    Ok((bytes, mime, p.display().to_string()))
}

fn extract_frame(p: &Path) -> Result<(Vec<u8>, &'static str, String), ShowError> {
    if !crate::doctor::bin_on_path("ffmpeg") {
        return Err(ShowError::Msg(
            "no vision frame — ffmpeg needed to look at a plate. Did not invent a description."
                .into(),
        ));
    }
    let dest = std::env::temp_dir().join(format!(
        "lot-look-{}-{}.jpg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(p)
        .args(["-ss", "0.3", "-frames:v", "1"])
        .arg(&dest)
        .status()
        .map_err(|e| {
            ShowError::Msg(format!(
                "no vision frame — ffmpeg: {e}. Did not invent a description."
            ))
        })?;
    let bytes = if status.success() && dest.is_file() {
        fs::read(&dest).unwrap_or_default()
    } else {
        Vec::new()
    };
    let _ = fs::remove_file(&dest);
    if bytes.is_empty() {
        return Err(ShowError::Msg(
            "no vision frame — ffmpeg wrote no still. Did not invent a description.".into(),
        ));
    }
    Ok((bytes, "image/jpeg", format!("{}#frame", p.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_flux_still_has_prompt_slot() {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("packs")
            .join(BUNDLED_COMFY_STILL);
        let raw = fs::read_to_string(&p).expect("comfy-flux-still.json");
        assert!(raw.contains("{{prompt}}"));
        assert!(raw.contains("flux1-dev-fp8.safetensors"));
        assert!(
            !raw.contains("CheckpointLoaderSimple"),
            "lock pack is Flux UNET, not a dummy SDXL checkpoint"
        );
    }

    #[test]
    fn comfy_seed_from_graph_is_recorded() {
        let mut wf: Value = serde_json::from_str(
            r#"{"3":{"class_type":"KSampler","inputs":{"seed":42,"steps":8}}}"#,
        )
        .unwrap();
        assert_eq!(apply_comfy_seed(&mut wf), 42);
        assert_eq!(wf["3"]["inputs"]["seed"], 42);
    }

    #[test]
    fn comfy_seed_env_overrides_graph() {
        std::env::set_var("LOT_COMFY_SEED", "99");
        let mut wf: Value = serde_json::from_str(
            r#"{"3":{"class_type":"KSampler","inputs":{"seed":42,"steps":8}}}"#,
        )
        .unwrap();
        let n = apply_comfy_seed(&mut wf);
        std::env::remove_var("LOT_COMFY_SEED");
        assert_eq!(n, 99);
        assert_eq!(wf["3"]["inputs"]["seed"], 99);
    }
}
