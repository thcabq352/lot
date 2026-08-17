//! Stills: Grok Imagine or local Comfy. Backend is required. No silent swap.
//! Board export packages stills + slate prompts. Never a fake PNG.

use crate::brain::grok_auth;
use crate::model::{shot_nums_match, MediaItem};
use crate::show::{
    append_event, append_event_with, bump, require_current, write_show, Show, ShowError,
};
use crate::Provenance;
use base64::Engine;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

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
    let (dir, mut show) = require_current()?;
    let shot_i = show
        .shots
        .iter()
        .position(|s| shot_nums_match(&s.num, shot_num))
        .ok_or_else(|| ShowError::Msg(format!("unknown shot: {shot_num}")))?;
    if let Some(p) = prompt {
        let t = p.trim();
        if !t.is_empty() {
            show.shots[shot_i].prompt = t.to_string();
        }
    }
    let text = show.shots[shot_i].prompt.trim().to_string();
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
        })),
    )?;
    Ok((dir, show))
}

pub fn board_export() -> Result<(PathBuf, Show, PathBuf), ShowError> {
    let (dir, mut show) = require_current()?;
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
            "prompt": shot.prompt,
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
    let (token, base, auth) = grok_auth().ok_or_else(|| {
        ShowError::Msg(
            "no grok stills — set HERMES xai-oauth, LOT_XAI_TOKEN, or XAI_API_KEY. Did not call Comfy."
                .into(),
        )
    })?;
    let model = std::env::var("LOT_STILLS_GROK_MODEL")
        .or_else(|_| std::env::var("LOT_GROK_IMAGE_MODEL"))
        .unwrap_or_else(|_| "grok-imagine-image-2.0".into());
    let url = format!("{base}/images/generations");
    let body = json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "response_format": "b64_json",
    });
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
    Ok((
        bytes,
        Provenance {
            backend: "grok".into(),
            model,
            base_url: base,
            auth,
        },
    ))
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
    let wf_path = std::env::var("LOT_COMFY_WORKFLOW")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ShowError::Msg(
                "no comfy stills — set LOT_COMFY_WORKFLOW to a JSON graph with {{prompt}}. Did not call Grok."
                    .into(),
            )
        })?;
    let wf_file = PathBuf::from(&wf_path);
    if !wf_file.is_file() {
        return Err(ShowError::Msg(format!(
            "no comfy stills — workflow not a file: {wf_path}. Did not call Grok."
        )));
    }
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
    let prompt_obj = wf.get("prompt").cloned().unwrap_or(wf);
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
                .unwrap_or(120),
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
    Ok((
        buf,
        Provenance {
            backend: "comfy".into(),
            model: wf_path,
            base_url: base,
            auth: "lot_comfy_workflow".into(),
        },
    ))
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
