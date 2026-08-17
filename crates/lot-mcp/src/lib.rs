use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

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

fn tools() -> Value {
    json!([
        {
            "name": "lot_status",
            "description": "First call. Kernel + current show. No GUI.",
            "inputSchema": { "type": "object", "properties": {} }
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
        "lot_status" => {
            let mut st = lot_core::Status::bootstrap();
            st.door = "mcp";
            match serde_json::to_value(&st) {
                Ok(v) => tool_ok(&v),
                Err(e) => tool_err(&e.to_string()),
            }
        }
        "lot_create" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return tool_err("path is required");
            }
            let name = args.get("name").and_then(|v| v.as_str());
            match lot_core::create_show(std::path::Path::new(path), name) {
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
            match lot_core::open_show(std::path::Path::new(path)) {
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
        "" => tool_err("tool name is required"),
        other => tool_err(&format!("Unknown tool: {other}")),
    }
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
    fn tools_list_has_status() {
        let r = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"lot_status"));
        assert!(names.contains(&"lot_create"));
        assert!(names.contains(&"lot_open"));
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
