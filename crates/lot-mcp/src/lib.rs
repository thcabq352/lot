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

fn tools() -> Value {
    json!([
        {
            "name": "lot_status",
            "description": "First call. Kernel + current show. No GUI.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path_prop() }
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
            "description": "Write screenplay.fountain from brief + style + cast + format via Grok (xAI OAuth) or local OpenAI-compat. Errors if no brain.",
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
            "name": "lot_slate_set",
            "description": "Set a continuity-locked prompt on a shot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "shot": { "type": "string" },
                    "prompt": { "type": "string" },
                    "path": path_prop()
                },
                "required": ["shot", "prompt"]
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
                    "path": path_prop()
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
            "name": "lot_doctor",
            "description": "Probe ffmpeg, Comfy :8188, Grok/local, VO TTS, and LOT_SOUNDTRACK_CMD.",
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
        "lot_slate_set" => with_path(&args, || {
            let shot = args.get("shot").and_then(|v| v.as_str()).unwrap_or("");
            let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
            match lot_core::slate_set(shot, prompt) {
                Ok((dir, show)) => tool_ok(&json!({
                    "ok": true,
                    "show": dir.display().to_string(),
                    "rev": show.rev,
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
