#![recursion_limit = "512"]

use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::{self, BufRead, Write};
use std::path::Path;

thread_local! {
    static PROGRESS_TOKEN: RefCell<Option<Value>> = const { RefCell::new(None) };
    static PROGRESS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

/// Drain progress notifications emitted during the last `handle` call.
pub fn drain_progress() -> Vec<Value> {
    PROGRESS.with(|p| p.replace(Vec::new()))
}

fn set_progress_token(tok: Option<Value>) {
    PROGRESS_TOKEN.with(|t| *t.borrow_mut() = tok);
}

fn emit_progress(progress: f64, total: Option<f64>, message: &str) {
    let tok = PROGRESS_TOKEN.with(|t| t.borrow().clone());
    let Some(token) = tok else {
        return;
    };
    let mut params = serde_json::Map::new();
    params.insert("progressToken".into(), token);
    params.insert("progress".into(), json!(progress));
    if let Some(t) = total {
        params.insert("total".into(), json!(t));
    }
    if !message.is_empty() {
        params.insert("message".into(), json!(message));
    }
    PROGRESS.with(|p| {
        p.borrow_mut().push(json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": Value::Object(params)
        }));
    });
}

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
            for note in drain_progress() {
                writeln!(stdout, "{}", note)?;
            }
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
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": "lot", "version": lot_core::VERSION }
            }),
        )),
        "notifications/initialized" => None,
        "notifications/cancelled" => None,
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tools() }))),
        "tools/call" => Some(ok(id, call(msg.get("params")))),
        "resources/list" => Some(ok(id, resources_list())),
        "resources/read" => Some(ok(id, resources_read(msg.get("params")))),
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

fn agent_prop() -> Value {
    json!({ "type": "string", "description": "Who is writing (hermes, cursor, …). Unset = human (no auto-claim). Second writer gets locked_by." })
}

fn tools() -> Value {
    let mut list = json!([
        {
            "name": "lot_status",
            "description": "First call. Kernel + current show. No GUI.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop(), "cap": cap_prop(), "agent": agent_prop() }
            }
        },
        {
            "name": "lot_create",
            "description": "Create a show.lot directory and make it current.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to create." },
                    "name": { "type": "string", "description": "Show title." },
                    "agent": agent_prop()
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
            "name": "lot_lock",
            "description": "Claim the show. One writer at a time. Second agent gets locked_by, not a silent clobber.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path_prop(),
                    "cap": cap_prop(),
                    "agent": agent_prop()
                }
            }
        },
        {
            "name": "lot_unlock",
            "description": "Release the show lock. Holder or force.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "force": { "type": "boolean", "description": "Unlock even if another agent holds it." },
                    "path": path_prop(),
                    "cap": cap_prop(),
                    "agent": agent_prop()
                }
            }
        },
        {
            "name": "lot_budget",
            "description": "Set the show spend/render cap. Hit cap → stop. Unset = unlimited. Agent caps are separate.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "spend": { "type": "integer", "description": "Show spend cap (Grok stills). 0 blocks the next spend." },
                    "render": { "type": "integer", "description": "Show render cap (Comfy stills / finish --upscale)." },
                    "clear_spend": { "type": "boolean" },
                    "clear_render": { "type": "boolean" },
                    "path": path_prop(),
                    "cap": cap_prop(),
                    "agent": agent_prop()
                }
            }
        },
        {
            "name": "lot_log",
            "description": "Audit log: who/what/rev. export=true writes audit/export.jsonl with tokens redacted.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "n": { "type": "integer", "description": "Last N events. Default 20." },
                    "export": { "type": "boolean" },
                    "path": path_prop(),
                    "cap": cap_prop(),
                    "agent": agent_prop()
                }
            }
        },
        {
            "name": "lot_show",
            "description": "Read lot://show. Meta, phase, lock, last event. Not the fountain.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop(), "cap": cap_prop() }
            }
        },
        {
            "name": "lot_scene",
            "description": "Read lot://scenes/{id}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "path": path_prop(),
                    "cap": cap_prop()
                },
                "required": ["id"]
            }
        },
        {
            "name": "lot_shot",
            "description": "Read lot://shots/{id} (or num).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "num": { "type": "string" },
                    "path": path_prop(),
                    "cap": cap_prop()
                }
            }
        },
        {
            "name": "lot_take",
            "description": "Read lot://takes/{id}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "path": path_prop(),
                    "cap": cap_prop()
                },
                "required": ["id"]
            }
        },
        {
            "name": "lot_import",
            "description": "Import .scriptbreak / .cork-board.json / canvas / .blockout / .sbref / Slate project.json / .ctake. Does not delete the source. No invented glTF or still.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string" },
                    "path": path_prop(),
                    "cap": cap_prop(),
                    "agent": agent_prop()
                },
                "required": ["file"]
            }
        },
        {
            "name": "lot_handoff",
            "description": "Advance phase. Default is dry-run. commit=true writes only when the gate passes. cut — no next.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "commit": { "type": "boolean", "description": "Write the next phase. Default false (dry-run)." },
                    "path": path_prop(),
                    "cap": cap_prop(),
                    "agent": agent_prop()
                }
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
    ]);
    decorate_mutating_tools(&mut list);
    list
}

fn decorate_mutating_tools(list: &mut Value) {
    let Some(arr) = list.as_array_mut() else {
        return;
    };
    let skip = ["lot_status", "lot_help", "lot_doctor"];
    let output_schema = json!({
        "type": "object",
        "properties": {
            "ok": { "type": "boolean" },
            "show": { "type": "string" },
            "show_id": { "type": "string" },
            "rev": { "type": "integer" },
            "event_id": { "type": ["string", "null"] },
            "who": { "type": "string" },
            "school": {
                "type": "object",
                "properties": { "enabled": { "type": "boolean" } }
            }
        },
        "required": ["ok", "show", "show_id", "rev", "who", "school"]
    });
    let detail_prop = json!({
        "type": "string",
        "description": "full dumps shot prompts and cards. Default is lean (ids, counts, media path+sha256+duration)."
    });
    for t in arr {
        let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if skip.contains(&name) {
            continue;
        }
        if let Some(obj) = t.as_object_mut() {
            obj.insert("outputSchema".into(), output_schema.clone());
            if let Some(props) = obj
                .get_mut("inputSchema")
                .and_then(|s| s.get_mut("properties"))
                .and_then(|p| p.as_object_mut())
            {
                props.insert("detail".into(), detail_prop.clone());
            }
        }
    }
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
    let agent = args
        .get("agent")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let full = lot_core::detail_full_value(args.get("detail"));
    let token = params
        .and_then(|p| p.get("_meta"))
        .and_then(|m| m.get("progressToken"))
        .cloned()
        .or_else(|| args.get("progressToken").cloned());
    set_progress_token(token);
    let out = lot_core::with_caps(caps, || {
        lot_core::with_agent(agent, || {
            lot_core::with_detail(full, || dispatch(name, &args))
        })
    });
    set_progress_token(None);
    out
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "id": show.id, "name": show.name })),
                Err(e) => tool_err(&e.to_string()),
            }
        }
        "lot_open" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return tool_err("path is required");
            }
            match lot_core::open_show(Path::new(path)) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "id": show.id, "name": show.name })),
                Err(e) => tool_err(&e.to_string()),
            }
        }
        "lot_writer_brief" => with_path(&args, || {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if text.is_empty() {
                return tool_err("text is required");
            }
            match lot_core::set_brief(text) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "brief": show.writer.brief })),
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
                Ok((dir, show)) => mut_ok(
                    &dir,
                    &show,
                    json!({
                        "genres": show.writer.genres,
                        "styles_living": show.writer.styles_living,
                        "styles_canon": show.writer.styles_canon,
                        "format": show.writer.format,
                    }),
                ),
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
                    Ok((dir, show)) => mut_ok(&dir, &show, json!({ "cast": show.writer.cast })),
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "cast": show.writer.cast })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_writer_draft" => with_path(&args, || match lot_core::draft_screenplay() {
            Ok((dir, show)) => mut_ok(
                &dir,
                &show,
                json!({
                    "draft": show.writer.draft_path,
                    "provenance": show.writer.draft_provenance,
                }),
            ),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_writer_revise" => with_path(&args, || {
            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");
            if notes.is_empty() {
                return tool_err("notes is required");
            }
            match lot_core::revise_screenplay(notes) {
                Ok((dir, show)) => mut_ok(
                    &dir,
                    &show,
                    json!({
                        "draft": show.writer.draft_path,
                        "provenance": show.writer.draft_provenance,
                    }),
                ),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_writer_lock" => with_path(&args, || match lot_core::lock_writer() {
            Ok((dir, show)) => mut_ok(&dir, &show, json!({ "locked": show.writer.locked })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_writer_unlock" => with_path(&args, || match lot_core::unlock_writer() {
            Ok((dir, show)) => mut_ok(&dir, &show, json!({ "locked": show.writer.locked })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_breakdown_import" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            if file.is_empty() {
                return tool_err("file is required");
            }
            match lot_core::breakdown_parse(Some(Path::new(file))) {
                Ok((dir, show)) => mut_ok(
                    &dir,
                    &show,
                    json!({ "summary": lot_core::breakdown_summary(&show) }),
                ),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_breakdown_parse" => with_path(&args, || match lot_core::breakdown_parse(None) {
            Ok((dir, show)) => mut_ok(
                &dir,
                &show,
                json!({ "summary": lot_core::breakdown_summary(&show) }),
            ),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_wall_add" => with_path(&args, || {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let act = args.get("act").and_then(|v| v.as_str());
            match lot_core::wall_add(act, text) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "wall": show.wall })),
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stage_export" => with_path(&args, || match lot_core::stage_export() {
            Ok((dir, show, file)) => {
                mut_ok(&dir, &show, json!({ "export": file.display().to_string() }))
            }
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_picture_lock" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let locked = args.get("locked").and_then(|v| v.as_bool()).unwrap_or(true);
            match lot_core::picture_lock(shot, locked) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stills_generate" => with_path(&args, || {
            emit_progress(0.0, Some(1.0), "stills generate");
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let backend = args.get("backend").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = args.get("prompt").and_then(|v| v.as_str());
            match lot_core::stills_generate(shot, backend, prompt) {
                Ok((dir, show)) => {
                    emit_progress(1.0, Some(1.0), "stills generate");
                    mut_ok(
                        &dir,
                        &show,
                        json!({
                            "stills_backend": show.stills_backend,
                            "shots": show.shots,
                        }),
                    )
                }
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_stills_describe" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            match lot_core::stills_describe(shot, file) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_board_export" => with_path(&args, || match lot_core::board_export() {
            Ok((dir, show, file)) => {
                mut_ok(&dir, &show, json!({ "export": file.display().to_string() }))
            }
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_slate_set" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target").and_then(|v| v.as_str());
            match lot_core::slate_set(shot, prompt, target) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_slate_compile" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let target = args.get("target").and_then(|v| v.as_str());
            match lot_core::slate_compile(shot, target) {
                Ok((dir, show)) => mut_ok(
                    &dir,
                    &show,
                    json!({
                        "shots": show.shots,
                        "slate": show.slate,
                    }),
                ),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_slate_target" => with_path(&args, || {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::slate_target(id) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "slate": show.slate })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_slate_lora" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str());
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let weight = args.get("weight").and_then(|v| v.as_str());
            let model = args.get("model").and_then(|v| v.as_str());
            match lot_core::slate_lora(shot, id, weight, model) {
                Ok((dir, show)) => mut_ok(
                    &dir,
                    &show,
                    json!({
                        "slate": show.slate,
                        "shots": show.shots,
                    }),
                ),
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_motion_export" => with_path(&args, || match lot_core::motion_export() {
            Ok((dir, show, file)) => {
                mut_ok(&dir, &show, json!({ "export": file.display().to_string() }))
            }
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_motion_analyze" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::motion_analyze(shot) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "shots": show.shots })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_finish" => with_path(&args, || {
            emit_progress(0.0, Some(1.0), "finish");
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            let upscale = args
                .get("upscale")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let fps = args.get("fps").and_then(|v| v.as_str());
            match lot_core::finish_pickup(file, upscale, fps) {
                Ok((dir, show, out)) => {
                    emit_progress(1.0, Some(1.0), "finish");
                    mut_ok(
                        &dir,
                        &show,
                        json!({
                            "finish": show.finish,
                            "file": out.display().to_string(),
                        }),
                    )
                }
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_dailies_ingest" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).map(Path::new);
            let dir = args.get("dir").and_then(|v| v.as_str()).map(Path::new);
            match lot_core::dailies_ingest(file, dir) {
                Ok((show_dir, show, report)) => mut_ok(
                    &show_dir,
                    &show,
                    json!({
                        "ingested": report.ingested,
                        "resumed": report.resumed,
                        "takes": show.takes
                    }),
                ),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_dailies_circle" => with_path(&args, || {
            let take = args.get("take").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::dailies_circle(take) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "takes": show.takes })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_dailies_export" | "lot_cut_export" => {
            with_path(&args, || match lot_core::dailies_export() {
                Ok((dir, show, file)) => {
                    mut_ok(&dir, &show, json!({ "export": file.display().to_string() }))
                }
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "stems": show.stems })),
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
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "stems": show.stems })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_snapshot" => with_path(&args, || {
            let list = args.get("list").and_then(|v| v.as_bool()).unwrap_or(false);
            if list {
                return match lot_core::snapshot_list() {
                    Ok((dir, show, revs)) => mut_ok(&dir, &show, json!({ "revs": revs })),
                    Err(e) => tool_err(&e.to_string()),
                };
            }
            match lot_core::snapshot_show() {
                Ok((dir, show, dest, rev)) => mut_ok(
                    &dir,
                    &show,
                    json!({
                        "snapshot_rev": rev,
                        "snapshot": dest.display().to_string(),
                    }),
                ),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_restore" => with_path(&args, || {
            let rev = args.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
            if rev == 0 {
                return tool_err("rev is required");
            }
            match lot_core::restore_show(rev) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "from_rev": rev })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_lock" => with_path(&args, || match lot_core::lock_show() {
            Ok((dir, show)) => mut_ok(&dir, &show, json!({ "locked_by": show.locked_by })),
            Err(e) => tool_err(&e.to_string()),
        }),
        "lot_unlock" => with_path(&args, || {
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            match lot_core::unlock_show(force) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "locked_by": show.locked_by })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_budget" => with_path(&args, || {
            let spend = args.get("spend").and_then(|v| v.as_u64()).map(|n| n as u32);
            let render = args
                .get("render")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            let clear_spend = args
                .get("clear_spend")
                .or_else(|| args.get("clear-spend"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let clear_render = args
                .get("clear_render")
                .or_else(|| args.get("clear-render"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match lot_core::set_budget(spend, render, clear_spend, clear_render) {
                Ok((dir, show)) => mut_ok(&dir, &show, json!({ "budget": show.budget })),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_log" => with_path(&args, || {
            let export = args
                .get("export")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if export {
                match lot_core::export_log() {
                    Ok((dir, show, dest, count)) => mut_ok(
                        &dir,
                        &show,
                        json!({
                            "export": dest.display().to_string(),
                            "events": count,
                        }),
                    ),
                    Err(e) => tool_err(&e.to_string()),
                }
            } else {
                let n = args.get("n").and_then(|v| v.as_u64()).unwrap_or(20) as u32;
                match lot_core::show_log(Some(n)) {
                    Ok((dir, show, events)) => {
                        mut_ok(&dir, &show, json!({ "events": events, "n": events.len() }))
                    }
                    Err(e) => tool_err(&e.to_string()),
                }
            }
        }),
        "lot_show" => with_path(&args, || card_tool("lot://show")),
        "lot_scene" => with_path(&args, || {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                return tool_err("id is required");
            }
            card_tool(&format!("lot://scenes/{id}"))
        }),
        "lot_shot" => with_path(&args, || {
            let key = args
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| args.get("num").and_then(|v| v.as_str()))
                .unwrap_or("");
            if key.is_empty() {
                return tool_err("shot needs id or num");
            }
            card_tool(&format!("lot://shots/{key}"))
        }),
        "lot_take" => with_path(&args, || {
            let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                return tool_err("id is required");
            }
            card_tool(&format!("lot://takes/{id}"))
        }),
        "lot_import" => with_path(&args, || {
            let file = args.get("file").and_then(|v| v.as_str()).unwrap_or("");
            if file.is_empty() {
                return tool_err("file is required");
            }
            match lot_core::import_file(Path::new(file)) {
                Ok((dir, show, report)) => mut_ok(
                    &dir,
                    &show,
                    json!({
                        "kind": report.kind,
                        "source": report.source,
                        "kept": report.kept,
                        "added": report.added,
                    }),
                ),
                Err(e) => tool_err(&e.to_string()),
            }
        }),
        "lot_handoff" => with_path(&args, || {
            let commit = args
                .get("commit")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            match lot_core::handoff(commit) {
                Ok((dir, show, report)) => {
                    let extra = serde_json::to_value(&report).unwrap_or(json!({}));
                    mut_ok(&dir, &show, extra)
                }
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

fn resources_list() -> Value {
    match lot_core::resource_list() {
        Ok((_, _, list)) => {
            let resources: Vec<Value> = list
                .iter()
                .map(|r| {
                    json!({
                        "uri": r.uri,
                        "name": r.name,
                        "mimeType": r.mime_type,
                        "description": r.description,
                    })
                })
                .collect();
            json!({ "resources": resources })
        }
        Err(e) => json!({ "resources": [], "error": e.to_string() }),
    }
}

fn resources_read(params: Option<&Value>) -> Value {
    let uri = params
        .and_then(|p| p.get("uri"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if uri.is_empty() {
        return json!({ "contents": [], "error": "uri is required" });
    }
    match lot_core::resource_read(uri) {
        Ok((_, _, card)) => json!({
            "contents": [{
                "uri": uri,
                "mimeType": "application/json",
                "text": card.to_string(),
            }]
        }),
        Err(e) => json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": e.to_string(),
            }],
            "isError": true
        }),
    }
}

fn card_tool(uri: &str) -> Value {
    match lot_core::resource_read(uri) {
        Ok((dir, show, card)) => mut_ok(&dir, &show, json!({ "uri": uri, "resource": card })),
        Err(e) => tool_err(&e.to_string()),
    }
}

fn mut_ok(dir: &Path, show: &lot_core::Show, extra: Value) -> Value {
    tool_ok(&lot_core::mutation_json(dir, show, extra))
}

fn tool_ok(v: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": v.to_string() }],
        "structuredContent": v
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
        assert!(r["result"]["capabilities"].get("resources").is_some());
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
            "lot_lock",
            "lot_unlock",
            "lot_budget",
            "lot_log",
            "lot_handoff",
            "lot_show",
            "lot_scene",
            "lot_shot",
            "lot_take",
            "lot_import",
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
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/cancelled"})).is_none());
    }

    fn call_body(name: &str, args: Value) -> Value {
        let r = handle(&json!({
            "jsonrpc":"2.0","id":9,"method":"tools/call",
            "params":{"name": name, "arguments": args}
        }))
        .unwrap();
        assert_ne!(
            r["result"]["isError"], true,
            "{}",
            r["result"]["content"][0]["text"]
        );
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    fn assert_mutation_envelope(body: &Value) {
        assert_eq!(body["ok"], true, "{body}");
        assert!(
            body["show"]
                .as_str()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "show {body}"
        );
        assert!(
            body["show_id"]
                .as_str()
                .map(|s| s.starts_with("show-"))
                .unwrap_or(false),
            "show_id {body}"
        );
        assert!(body["rev"].as_u64().is_some(), "rev {body}");
        assert!(body.get("event_id").is_some(), "event_id {body}");
        assert!(
            body["who"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "who {body}"
        );
        assert!(body.get("school").is_some(), "school {body}");
    }

    #[test]
    fn mutating_tools_return_cli_envelope() {
        let root = std::env::temp_dir().join(format!(
            "lot-mcp-envelope-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("LOT_HOME", root.join("home"));
        std::env::remove_var("LOT_SHOW");
        std::env::remove_var("LOT_CAP");
        std::env::remove_var("LOT_AGENT");
        lot_core::clear_caps();
        lot_core::clear_agent();

        let show_dir = root.join("show");
        let created = call_body(
            "lot_create",
            json!({ "path": show_dir.display().to_string(), "name": "Envelope" }),
        );
        assert_mutation_envelope(&created);
        assert_eq!(created["name"], "Envelope");

        let opened = call_body(
            "lot_open",
            json!({ "path": show_dir.display().to_string() }),
        );
        assert_mutation_envelope(&opened);
        assert_eq!(opened["id"], created["show_id"]);

        let brief = call_body(
            "lot_writer_brief",
            json!({
                "path": show_dir.display().to_string(),
                "text": "A woman waits by a neon tent."
            }),
        );
        assert_mutation_envelope(&brief);
        assert_eq!(brief["brief"], "A woman waits by a neon tent.");

        let style = call_body(
            "lot_writer_style",
            json!({
                "path": show_dir.display().to_string(),
                "genre": "drama",
                "format": "advertisement"
            }),
        );
        assert_mutation_envelope(&style);
        assert_eq!(style["format"], "advertisement");

        let snap = call_body(
            "lot_snapshot",
            json!({ "path": show_dir.display().to_string() }),
        );
        assert_mutation_envelope(&snap);
        assert!(snap.get("snapshot").is_some(), "{snap}");

        let listed = call_body(
            "lot_snapshot",
            json!({ "path": show_dir.display().to_string(), "list": true }),
        );
        assert_mutation_envelope(&listed);
        assert!(listed["revs"].as_array().is_some(), "{listed}");
    }

    #[test]
    fn tools_advertise_output_schema_and_detail() {
        let r = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        let tools = r["result"]["tools"].as_array().unwrap();
        let stage = tools
            .iter()
            .find(|t| t["name"] == "lot_stage_place")
            .unwrap();
        assert_eq!(
            stage["outputSchema"]["properties"]["show_id"]["type"],
            "string"
        );
        assert!(stage["inputSchema"]["properties"].get("detail").is_some());
        let status = tools.iter().find(|t| t["name"] == "lot_status").unwrap();
        assert!(status.get("outputSchema").is_none());
    }

    #[test]
    fn mutating_tools_are_lean_unless_detail_full() {
        let root = std::env::temp_dir().join(format!(
            "lot-mcp-lean-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("LOT_HOME", root.join("home"));
        std::env::remove_var("LOT_SHOW");
        lot_core::clear_caps();
        lot_core::clear_agent();

        let show_dir = root.join("show");
        call_body(
            "lot_create",
            json!({ "path": show_dir.display().to_string(), "name": "Lean" }),
        );
        let script = root.join("script.txt");
        std::fs::write(
            &script,
            "INT. TENT - NIGHT\n\nADA (quietly)\nDon't put it on.\n",
        )
        .unwrap();
        call_body(
            "lot_breakdown_import",
            json!({
                "path": show_dir.display().to_string(),
                "file": script.display().to_string()
            }),
        );
        call_body(
            "lot_slate_set",
            json!({
                "path": show_dir.display().to_string(),
                "shot": "01",
                "prompt": "wide tent, neon rain, keep this prompt off the default payload"
            }),
        );
        let placed = call_body(
            "lot_stage_place",
            json!({
                "path": show_dir.display().to_string(),
                "shot": "01",
                "who": "Ada",
                "mark": "by the trunk",
                "x": "2",
                "z": "4"
            }),
        );
        assert_mutation_envelope(&placed);
        let shot = &placed["shots"][0];
        assert_eq!(shot["num"], "01");
        assert!(shot.get("prompt").is_none(), "lean dump {placed}");
        assert!(shot.get("prompt_targets").is_none(), "lean dump {placed}");
        assert_eq!(shot["marks"], 1);

        let full = call_body(
            "lot_stage_place",
            json!({
                "path": show_dir.display().to_string(),
                "shot": "01",
                "who": "Ada",
                "mark": "by the trunk",
                "detail": "full"
            }),
        );
        assert_eq!(
            full["shots"][0]["prompt"],
            "wide tent, neon rain, keep this prompt off the default payload"
        );
        let raw = handle(&json!({
            "jsonrpc":"2.0","id":11,"method":"tools/call",
            "params":{"name":"lot_writer_brief","arguments":{
                "path": show_dir.display().to_string(),
                "text": "A woman waits."
            }}
        }))
        .unwrap();
        assert_eq!(raw["result"]["structuredContent"]["ok"], true);
    }

    #[test]
    fn dailies_ingest_is_idempotent() {
        let root = std::env::temp_dir().join(format!(
            "lot-mcp-ingest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("LOT_HOME", root.join("home"));
        std::env::remove_var("LOT_SHOW");
        lot_core::clear_caps();
        lot_core::clear_agent();

        let show_dir = root.join("show");
        call_body(
            "lot_create",
            json!({ "path": show_dir.display().to_string(), "name": "Ingest" }),
        );
        let script = root.join("script.txt");
        std::fs::write(
            &script,
            "INT. TENT - NIGHT\n\nADA (quietly)\nDon't put it on.\n",
        )
        .unwrap();
        call_body(
            "lot_breakdown_import",
            json!({
                "path": show_dir.display().to_string(),
                "file": script.display().to_string()
            }),
        );
        let clip = show_dir.join("media").join("01-foo.mp4");
        std::fs::create_dir_all(clip.parent().unwrap()).unwrap();
        std::fs::write(&clip, b"mcp-ingest-bytes").unwrap();
        let first = call_body(
            "lot_dailies_ingest",
            json!({
                "path": show_dir.display().to_string(),
                "file": clip.display().to_string()
            }),
        );
        assert_mutation_envelope(&first);
        assert_eq!(first["ingested"], 1);
        assert_eq!(first["takes"].as_array().unwrap().len(), 1);
        let take_id = first["takes"][0]["id"].clone();
        let rev = first["rev"].clone();
        let second = call_body(
            "lot_dailies_ingest",
            json!({
                "path": show_dir.display().to_string(),
                "file": clip.display().to_string()
            }),
        );
        assert_eq!(second["takes"].as_array().unwrap().len(), 1);
        assert_eq!(second["takes"][0]["id"], take_id);
        assert_eq!(second["rev"], rev);
        assert_eq!(second["resumed"], 1);
        assert_eq!(second["ingested"], 0);
    }

    #[test]
    fn stills_generate_emits_progress_for_token() {
        let root = std::env::temp_dir().join(format!(
            "lot-mcp-progress-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("LOT_HOME", root.join("home"));
        std::env::remove_var("LOT_SHOW");
        lot_core::clear_caps();
        lot_core::clear_agent();

        let show_dir = root.join("show");
        call_body(
            "lot_create",
            json!({ "path": show_dir.display().to_string(), "name": "Progress" }),
        );
        let script = root.join("script.txt");
        std::fs::write(
            &script,
            "INT. TENT - NIGHT\n\nADA (quietly)\nDon't put it on.\n",
        )
        .unwrap();
        call_body(
            "lot_breakdown_import",
            json!({
                "path": show_dir.display().to_string(),
                "file": script.display().to_string()
            }),
        );
        call_body(
            "lot_slate_set",
            json!({
                "path": show_dir.display().to_string(),
                "shot": "01",
                "prompt": "wide tent, neon rain"
            }),
        );
        let _ = drain_progress();
        let raw = handle(&json!({
            "jsonrpc":"2.0","id":12,"method":"tools/call",
            "params":{
                "name":"lot_stills_generate",
                "arguments":{
                    "path": show_dir.display().to_string(),
                    "shot":"01",
                    "backend":"grok"
                },
                "_meta": { "progressToken": "stills-1" }
            }
        }))
        .unwrap();
        let notes = drain_progress();
        assert!(
            !notes.is_empty(),
            "expected progress notifications, got {notes:?} result={raw}"
        );
        assert_eq!(notes[0]["method"], "notifications/progress");
        assert_eq!(notes[0]["params"]["progressToken"], "stills-1");
        assert!(notes[0]["params"]["progress"].as_f64().is_some());
    }
}
