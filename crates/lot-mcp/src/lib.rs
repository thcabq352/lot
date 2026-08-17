use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const PROTOCOL: &str = "2024-11-05";

pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle(&msg) {
            writeln!(stdout, "{}", resp)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

pub fn handle(msg: &Value) -> Option<Value> {
    let method = msg.get("method")?.as_str()?;
    let id = msg.get("id").cloned();
    match method {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "lot", "version": lot_core::VERSION }
            }),
        )),
        "notifications/initialized" => None,
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tools() }))),
        "tools/call" => Some(ok(id, call(msg.get("params")))),
        _ => {
            if id.is_some() {
                Some(err(id, -32601, &format!("Method not found: {method}")))
            } else {
                None
            }
        }
    }
}

fn path_prop() -> Value {
    json!({ "type": "string", "description": "Optional show.lot directory. Opens it, then runs. Omit to keep current." })
}

fn cap_prop() -> Value {
    json!({ "type": "string", "description": "Agent caps: read | write | render | export | spend | all. Unset = all. AC-012." })
}

fn tools() -> Value {
    json!([
        {
            "name": "lot_status",
            "description": "First call. Kernel + current show. No GUI.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop(), "cap": cap_prop() }
            }
        },
        {
            "name": "lot_create",
            "description": "Create a show.lot directory and make it current.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to create." },
                    "name": { "type": "string", "description": "Show title." }
                },
                "required": ["path"]
            }
        },
        {
            "name": "lot_open",
            "description": "Open an existing show.lot and make it current.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Show directory." }
                },
                "required": ["path"]
            }
        },
        {
            "name": "lot_writer_brief",
            "description": "Set the writer brief.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Brief / logline." },
                    "path": path_prop()
                },
                "required": ["text"]
            }
        },
        {
            "name": "lot_writer_style",
            "description": "Set genre, living/canon influence, and format. IDs from dated JSON packs. Influence / coverage style — not endorsement.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "genre": { "description": "Genre id(s)." },
                    "living": { "description": "Living director id(s)." },
                    "canon": { "description": "Canon director id(s)." },
                    "format": { "type": "string", "description": "feature | 30min | 15s | episodic | advertisement | music-video" },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_writer_cast",
            "description": "Add/update one character (--name) or replace the cast (json array).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "function": { "type": "string" },
                    "look": { "type": "string" },
                    "must_not": { "type": "string" },
                    "json": { "description": "Replace-all cast JSON array or string." },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_writer_draft",
            "description": "Write screenplay.fountain from brief + style + cast + format via Grok (xAI OAuth) or Ollama / local. Errors if no brain.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_writer_revise",
            "description": "Revise existing screenplay.fountain. Errors if no draft.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "notes": { "type": "string", "description": "Revise notes." },
                    "path": path_prop()
                },
                "required": ["notes"]
            }
        },
        {
            "name": "lot_writer_lock",
            "description": "Lock brief/style/cast/draft/revise.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_writer_unlock",
            "description": "Unlock the writer.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_breakdown_import",
            "description": "Import a .txt / .fountain / .scriptbreak and parse scenes. Does not delete the source.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Screenplay or .scriptbreak path." },
                    "path": path_prop()
                },
                "required": ["file"]
            }
        },
        {
            "name": "lot_breakdown_parse",
            "description": "Parse screenplay.fountain on the current show into scenes + default shots.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_wall_add",
            "description": "Add a Cork Board beat card.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "act": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["text"]
            }
        },
        {
            "name": "lot_stage_place",
            "description": "Place a 2D floor mark on a shot. 3D blocking stays in Blockout. Does not rename the shot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "who": { "type": "string" },
                    "mark": { "type": "string" },
                    "x": { "type": "string" },
                    "z": { "type": "string" },
                    "notes": { "type": "string" },
                    "kind": { "type": "string", "description": "actor | camera | prop" },
                    "path": path_prop()
                },
                "required": ["shot", "who"]
            }
        },
        {
            "name": "lot_stage_camera",
            "description": "Set the camera card on a shot (size, angle, lens, move).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "size": { "type": "string" },
                    "angle": { "type": "string" },
                    "lens": { "type": "string" },
                    "move": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["shot"]
            }
        },
        {
            "name": "lot_stage_export",
            "description": "Write stage/block.json from 2D marks. Never a fake glTF or depth pass.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_picture_lock",
            "description": "Lock or unlock a Picture shot card. Does not rename the shot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "locked": { "type": "boolean" },
                    "path": path_prop()
                },
                "required": ["shot"]
            }
        },
        {
            "name": "lot_stills_generate",
            "description": "Generate a still for a shot. backend must be grok or comfy. No silent swap. Never a fake PNG.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "backend": { "type": "string", "description": "grok | comfy" },
                    "prompt": { "type": "string" },
                    "path": path_prop(),
                    "cap": cap_prop()
                },
                "required": ["shot", "backend"]
            }
        },
        {
            "name": "lot_stills_describe",
            "description": "Look at a still, plate frame, or file. Grok vision #1 when online; Ollama VL locally. Never invent a look.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "file": { "type": "string", "description": "Optional image or plate path." },
                    "path": path_prop()
                },
                "required": ["shot"]
            }
        },
        {
            "name": "lot_board_export",
            "description": "Export board/board.json from shots, stills, and slate prompts. One tool toward Slate.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_slate_set",
            "description": "Set the Slate canon on a shot. With target, write a per-engine rewrite without replacing the canon (unless canon is empty).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "prompt": { "type": "string" },
                    "target": { "type": "string", "description": "ltx-2.3 | ltx-2.5 | grok | comfy | prompt-server | kling | veo | sora | seedance | hailuo | flux | midjourney | gpt-image | krea | wan | runway" },
                    "path": path_prop()
                },
                "required": ["shot", "prompt"]
            }
        },
        {
            "name": "lot_slate_compile",
            "description": "Compile the Slate canon into a target dialect. Brain or LOT_PROMPT_SERVER. Never invent a rewrite if they are down.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "target": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["shot"]
            }
        },
        {
            "name": "lot_slate_target",
            "description": "Set the show default Slate compile target.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["id"]
            }
        },
        {
            "name": "lot_slate_lora",
            "description": "Attach a LoRA to a shot or the show (id, weight, model family).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "id": { "type": "string" },
                    "weight": { "type": "string" },
                    "model": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["id"]
            }
        },
        {
            "name": "lot_motion_plate",
            "description": "Attach a Motion Previs plate to a shot. Does not rename the shot. Pose/depth stay in Motion Previs Studio.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "shot": { "type": "string" },
                    "mode": { "type": "string", "description": "camera_only | actor_motion | object_motion | full_scene" },
                    "path": path_prop()
                },
                "required": ["file", "shot"]
            }
        },
        {
            "name": "lot_motion_marks",
            "description": "Store camera / performance marks on a shot. No MediaPipe. No fake OpenPose.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "move": { "type": "string" },
                    "notes": { "type": "string" },
                    "mode": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["shot"]
            }
        },
        {
            "name": "lot_motion_export",
            "description": "Write motion/previs.json + prompt.md from plates and marks.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_motion_analyze",
            "description": "Probe a plate (ffprobe or LOT_MOTION_CMD). Does not invent pose/depth. Studio MCP stays the engine for OpenPose.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["shot"]
            }
        },
        {
            "name": "lot_finish",
            "description": "Optional end-of-pipeline upscale and/or FPS pickup via ffmpeg or LOT_UPSCALE_CMD. Never a stub.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "upscale": { "type": "boolean" },
                    "fps": { "type": "string" },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_dailies_ingest",
            "description": "Ingest a clip by filename prefix (01-foo.mp4 → shot 01). Does not rename the shot to 01.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "dir": { "type": "string" },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_dailies_circle",
            "description": "Circle a take by id. Requires take. No GUI.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "take": { "type": "string" },
                    "path": path_prop(),
                    "cap": cap_prop()
                },
                "required": ["take"]
            }
        },
        {
            "name": "lot_dailies_export",
            "description": "Export circled takes as FCPXML.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_cut_export",
            "description": "Cut interchange: same as dailies export (FCPXML). Resolve live is an adapter later.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
            }
        },
        {
            "name": "lot_stems_soundtrack",
            "description": "Write a soundtrack cue (Grok/local if up). Attach --file or LOT_SOUNDTRACK_CMD to generate audio. Never a fake track.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "brief": { "type": "string" },
                    "file": { "type": "string" },
                    "generate": { "type": "boolean" },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_stems_vo",
            "description": "Voiceover: set text, attach a file, or generate via local TTS (SAPI / piper / espeak / say).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "file": { "type": "string" },
                    "generate": { "type": "boolean" },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_snapshot",
            "description": "Freeze show.json + fountain at the current rev. list=true lists revs.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "list": { "type": "boolean" },
                    "path": path_prop()
                }
            }
        },
        {
            "name": "lot_restore",
            "description": "Restore a snapshot by rev. Later drafts do not eat earlier ones.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "rev": { "type": "integer" },
                    "path": path_prop()
                },
                "required": ["rev"]
            }
        },
        {
            "name": "lot_help",
            "description": "Machine-readable spec. The binary is the contract.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "lot_doctor",
            "description": "Probe ffmpeg, Comfy, Grok, Ollama (LLM + vision), VO TTS, soundtrack, prompt server, Motion Previs, Blockout, upscale.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn call(params: Option<&Value>) -> Value {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let args = params
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or(json!({}));
    let caps = match cap_from_args(&args) {
        Ok(c) => c,
        Err(e) => return tool_err(&e),
    };
    lot_core::with_caps(caps, || dispatch(name, &args))
}

fn cap_from_args(args: &Value) -> Result<Option<lot_core::Caps>, String> {
    let raw = if let Some(s) = args.get("cap").and_then(|v| v.as_str()) {
        s.trim().to_string()
    } else if let Some(arr) = args.get("cap").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        String::new()
    };
    if raw.is_empty() {
        return Ok(None);
    }
    lot_core::parse_caps(&raw)
        .map(Some)
        .map_err(|e| e.to_string())
}

fn dispatch(name: &str, args: &Value) -> Value {
    match name {
        "lot_status" => with_path(&args, || {
            let mut st = lot_core::Status::bootstrap();
            st.door = "mcp";
            match serde_json::to_value(&st) {
                Ok(v) => tool_ok(&v),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_create" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return tool_err("path is required");
            }
            let name = args.get("name").and_then(|v| v.as_str());
            match lot_core::create_show(Path::new(path), name) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "id": show.id,
                    "name": show.name,
                    "rev": show.rev,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }
        "lot_open" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return tool_err("path is required");
            }
            match lot_core::open_show(Path::new(path)) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "id": show.id,
                    "name": show.name,
                    "rev": show.rev,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }
        "lot_writer_brief" => with_path(&args, || {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if text.is_empty() {
                return tool_err("text is required");
            }
            match lot_core::set_brief(text) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "brief": show.writer.brief,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_writer_style" => with_path(&args, || {
            let genre = str_list(&args, "genre");
            let living = str_list(&args, "living");
            let canon = str_list(&args, "canon");
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            match lot_core::set_style(
                genre.as_deref(),
                living.as_deref(),
                canon.as_deref(),
                format,
            ) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "genres": show.writer.genres,
                    "styles_living": show.writer.styles_living,
                    "styles_canon": show.writer.styles_canon,
                    "format": show.writer.format,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_writer_cast" => with_path(&args, || {
            if args.get("json").is_some() && args.get("name").and_then(|v| v.as_str()).is_some() {
                return tool_err("cast: use name or json, not both");
            }
            if let Some(v) = args.get("json") {
                let result = if v.is_array() {
                    match serde_json::from_value(v.clone()) {
                        Ok(members) => lot_core::replace_cast(members),
                        Err(e) => return tool_err(&e.to_string()),
                    }
                } else if let Some(s) = v.as_str() {
                    lot_core::replace_cast_json(s)
                } else {
                    return tool_err("json must be an array or a JSON string");
                };
                return match result {
                    Ok((dir, show)) => tool_ok(&json!({
                        "ok": true,
                        "show": dir.display().to_string(),
                        "rev": show.rev,
                        "cast": show.writer.cast,
                    })),
                    Err(e) => tool_err(&e.to_string()),
                };
            }
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return tool_err("cast needs name or json");
            }
            let function = args.get("function").and_then(|v| v.as_str());
            let look = args.get("look").and_then(|v| v.as_str());
            let must_not = args
                .get("must_not")
                .or_else(|| args.get("must-not"))
                .and_then(|v| v.as_str());
            match lot_core::upsert_cast(name, function, look, must_not) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "cast": show.writer.cast,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_writer_draft" => with_path(&args, || match lot_core::draft_screenplay() {
            Ok((dir, show)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "draft": show.writer.draft_path,
                "provenance": show.writer.draft_provenance,
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_writer_revise" => with_path(&args, || {
            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");
            if notes.is_empty() {
                return tool_err("notes is required");
            }
            match lot_core::revise_screenplay(notes) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "draft": show.writer.draft_path,
                    "provenance": show.writer.draft_provenance,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_writer_lock" => with_path(&args, || match lot_core::lock_writer() {
            Ok((dir, show)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "locked": show.writer.locked,
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_writer_unlock" => with_path(&args, || match lot_core::unlock_writer() {
            Ok((dir, show)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "locked": show.writer.locked,
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_breakdown_import" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            if file.is_empty() {
                return tool_err("file is required");
            }
            match lot_core::breakdown_parse(Some(Path::new(file))) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "summary": lot_core::breakdown_summary(&show),
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_breakdown_parse" => with_path(&args, || match lot_core::breakdown_parse(None) {
            Ok((dir, show)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "summary": lot_core::breakdown_summary(&show),
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_wall_add" => with_path(&args, || {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let act = args.get("act").and_then(|v| v.as_str());
            match lot_core::wall_add(act, text) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "wall": show.wall,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stage_place" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let who = args.get("who").and_then(|v| v.as_str()).unwrap_or("");
            let mark = args.get("mark").and_then(|v| v.as_str());
            let x = args.get("x").and_then(|v| v.as_str());
            let z = args.get("z").and_then(|v| v.as_str());
            let notes = args.get("notes").and_then(|v| v.as_str());
            let kind = args.get("kind").and_then(|v| v.as_str());
            match lot_core::stage_place(shot, who, mark, x, z, notes, kind) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stage_camera" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let size = args.get("size").and_then(|v| v.as_str());
            let angle = args.get("angle").and_then(|v| v.as_str());
            let lens = args.get("lens").and_then(|v| v.as_str());
            let mv = args
                .get("move")
                .or_else(|| args.get("move_kind"))
                .and_then(|v| v.as_str());
            match lot_core::stage_camera(shot, size, angle, lens, mv) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stage_export" => with_path(&args, || match lot_core::stage_export() {
            Ok((dir, show, file)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "export": file.display().to_string(),
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_picture_lock" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let locked = args.get("locked").and_then(|v| v.as_bool()).unwrap_or(true);
            match lot_core::picture_lock(shot, locked) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stills_generate" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let backend = args.get("backend").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = args.get("prompt").and_then(|v| v.as_str());
            match lot_core::stills_generate(shot, backend, prompt) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "stills_backend": show.stills_backend,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stills_describe" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            match lot_core::stills_describe(shot, file) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_board_export" => with_path(&args, || match lot_core::board_export() {
            Ok((dir, show, file)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "export": file.display().to_string(),
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_slate_set" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target").and_then(|v| v.as_str());
            match lot_core::slate_set(shot, prompt, target) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_slate_compile" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target").and_then(|v| v.as_str());
            match lot_core::slate_compile(shot, target) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                    "slate": show.slate,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_slate_target" => with_path(&args, || {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::slate_target(id) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "slate": show.slate,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_slate_lora" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str());
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let weight = args.get("weight").and_then(|v| v.as_str());
            let model = args.get("model").and_then(|v| v.as_str());
            match lot_core::slate_lora(shot, id, weight, model) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "slate": show.slate,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_motion_plate" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let mode = args.get("mode").and_then(|v| v.as_str());
            if file.is_empty() {
                return tool_err("file is required");
            }
            match lot_core::motion_plate(Path::new(file), shot, mode) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_motion_marks" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let mv = args
                .get("move")
                .or_else(|| args.get("move_kind"))
                .and_then(|v| v.as_str());
            let notes = args.get("notes").and_then(|v| v.as_str());
            let mode = args.get("mode").and_then(|v| v.as_str());
            match lot_core::motion_marks(shot, mv, notes, mode) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_motion_export" => with_path(&args, || match lot_core::motion_export() {
            Ok((dir, show, file)) => tool_ok(&json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "export": file.display().to_string(),
            })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_motion_analyze" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::motion_analyze(shot) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "shots": show.shots,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_finish" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            let upscale = args
                .get("upscale")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let fps = args.get("fps").and_then(|v| v.as_str());
            match lot_core::finish_pickup(file, upscale, fps) {
                Ok((dir, show, out)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "finish": show.finish,
                    "file": out.display().to_string(),
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_dailies_ingest" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            let dir = args.get("dir").and_then(|v| v.as_str()).map(Path::new);
            match lot_core::dailies_ingest(file, dir) {
                Ok((show_dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": show_dir.display().to_string(),
                    "rev": show.rev,
                    "takes": show.takes,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_dailies_circle" => with_path(&args, || {
            let take = args.get("take").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::dailies_circle(take) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "takes": show.takes,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_dailies_export" | "lot_cut_export" => {
            with_path(&args, || match lot_core::dailies_export() {
                Ok((dir, show, file)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "export": file.display().to_string(),
                })),
                Err(e) => tool_err(&e.to_string()),
            })
        }
        "lot_stems_soundtrack" => with_path(&args, || {
            let brief = args.get("brief").and_then(|v| v.as_str());
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            let generate = args
                .get("generate")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match lot_core::stems_soundtrack(brief, file, generate) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "stems": show.stems,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stems_vo" => with_path(&args, || {
            let text = args.get("text").and_then(|v| v.as_str());
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            let generate = args
                .get("generate")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match lot_core::stems_vo(text, file, generate) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "stems": show.stems,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_snapshot" => with_path(&args, || {
            let list = args.get("list").and_then(|v| v.as_bool()).unwrap_or(false);
            if list {
                return match lot_core::snapshot_list() {
                    Ok((dir, show, revs)) => tool_ok(&json!({
                        "ok": true,
                        "show": dir.display().to_string(),
                        "rev": show.rev,
                        "revs": revs,
                    })),
                    Err(e) => tool_err(&e.to_string()),
                };
            }
            match lot_core::snapshot_show() {
                Ok((dir, show, dest, rev)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "snapshot_rev": rev,
                    "snapshot": dest.display().to_string(),
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_restore" => with_path(&args, || {
            let rev = args.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
            if rev == 0 {
                return tool_err("rev is required");
            }
            match lot_core::restore_show(rev) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
                    "from_rev": rev,
                })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_help" => tool_ok(&lot_core::help_spec()),
        "lot_doctor" => {
            let d = lot_core::Doctor::probe();
            match serde_json::to_value(&d) {
                Ok(v) => tool_ok(&v),
                Err(e) => tool_err(&e.to_string()),
            }
        }
        "" => tool_err("tool name is required"),
        other => tool_err(&format!("Unknown tool: {other}")),
    }
}

fn with_path(args: &Value, f: impl FnOnce() -> Value) -> Value {
    if let Some(p) = args
        .get("path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Err(e) = lot_core::open_show(Path::new(p)) {
            return tool_err(&e.to_string());
        }
    }
    f()
}

fn str_list(args: &Value, key: &str) -> Option<Vec<String>> {
    let v = args.get(key)?;
    if v.is_null() {
        return None;
    }
    if let Some(s) = v.as_str() {
        let parts: Vec<String> = s
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_string)
            .collect();
        return Some(parts);
    }
    if let Some(arr) = v.as_array() {
        return Some(
            arr.iter()
                .filter_map(|x| x.as_str())
                .flat_map(|s| s.split(','))
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
                .collect(),
        );
    }
    None
}

fn tool_ok(v: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": v.to_string() }]
    })
}

fn tool_err(msg: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": msg }],
        "isError": true
    })
}

fn ok(id: Option<Value>, result: Value) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("jsonrpc".into(), json!("2.0"));
    if let Some(id) = id {
        o.insert("id".into(), id);
    }
    o.insert("result".into(), result);
    Value::Object(o)
}

fn err(id: Option<Value>, code: i64, message: &str) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("jsonrpc".into(), json!("2.0"));
    if let Some(id) = id {
        o.insert("id".into(), id);
    }
    o.insert("error".into(), json!({ "code": code, "message": message }));
    Value::Object(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize() {
        let r = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})).unwrap();
        assert_eq!(r["result"]["serverInfo"]["name"], "lot");
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL);
    }

    #[test]
    fn tools_list_has_writer_contract() {
        let r = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for n in [
            "lot_status",
            "lot_create",
            "lot_open",
            "lot_writer_brief",
            "lot_writer_style",
            "lot_writer_cast",
            "lot_writer_draft",
            "lot_writer_revise",
            "lot_writer_lock",
            "lot_writer_unlock",
            "lot_breakdown_import",
            "lot_breakdown_parse",
            "lot_stage_place",
            "lot_stage_camera",
            "lot_stage_export",
            "lot_snapshot",
            "lot_restore",
            "lot_help",
            "lot_stills_generate",
            "lot_stills_describe",
            "lot_board_export",
            "lot_slate_set",
            "lot_slate_compile",
            "lot_slate_target",
            "lot_slate_lora",
            "lot_motion_plate",
            "lot_motion_marks",
            "lot_motion_export",
            "lot_motion_analyze",
            "lot_finish",
            "lot_dailies_ingest",
            "lot_dailies_circle",
            "lot_dailies_export",
            "lot_stems_soundtrack",
            "lot_stems_vo",
            "lot_doctor",
        ] {
            assert!(names.contains(&n), "missing {n} in {names:?}");
        }
    }

    #[test]
    fn call_status() {
        let r = handle(&json!({
            "jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"lot_status","arguments":{}}
        }))
        .unwrap();
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        let body: Value = serde_json::from_str(text).unwrap();
        assert_eq!(body["name"], "lot");
        assert_eq!(body["door"], "mcp");
    }

    #[test]
    fn notify_no_reply() {
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
    }
}
