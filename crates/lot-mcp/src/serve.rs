//! Optional HTTP/OpenAPI twin. Same tool names as `lot mcp`.
//! Default bind is loopback. Agents should still use stdio MCP.

use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};

pub const DEFAULT_BIND: &str = "127.0.0.1:8787";

pub fn default_bind() -> &'static str {
    DEFAULT_BIND
}

pub fn resolve_bind(bind: Option<&str>) -> String {
    bind.map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_BIND)
        .to_string()
}

#[derive(Debug, Clone)]
pub struct HttpOut {
    pub status: u16,
    pub body: Value,
}

pub fn openapi_spec() -> Value {
    let tools = super::tools();
    let mut paths = serde_json::Map::new();
    paths.insert(
        "/openapi.json".into(),
        json!({
            "get": {
                "operationId": "openapi",
                "summary": "OpenAPI document. Same tool names as lot mcp.",
                "responses": { "200": { "description": "OpenAPI 3.0.3" } }
            }
        }),
    );
    paths.insert(
        "/".into(),
        json!({
            "get": {
                "operationId": "lot_serve_root",
                "summary": "HTTP door card.",
                "responses": { "200": { "description": "Lot HTTP twin" } }
            }
        }),
    );
    if let Some(arr) = tools.as_array() {
        for t in arr {
            let name = t["name"].as_str().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let mut post = serde_json::Map::new();
            post.insert("operationId".into(), json!(name));
            if let Some(d) = t.get("description") {
                post.insert("summary".into(), d.clone());
            }
            if let Some(schema) = t.get("inputSchema") {
                post.insert(
                    "requestBody".into(),
                    json!({
                        "required": true,
                        "content": { "application/json": { "schema": schema } }
                    }),
                );
            }
            let response_schema = t.get("outputSchema").cloned().unwrap_or(json!({
                "type": "object"
            }));
            post.insert(
                "responses".into(),
                json!({
                    "200": {
                        "description": "Same payload as MCP tools/call.",
                        "content": { "application/json": { "schema": response_schema } }
                    }
                }),
            );
            paths.insert(format!("/{name}"), json!({ "post": Value::Object(post) }));
        }
    }
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "lot",
            "version": lot_core::VERSION,
            "description": "Optional HTTP/OpenAPI twin. Same tool names as lot mcp. Default bind 127.0.0.1:8787. Agents should use lot mcp."
        },
        "paths": paths
    })
}

pub fn handle_http(method: &str, path: &str, body: &[u8]) -> HttpOut {
    let method = method.to_ascii_uppercase();
    let path = path.split('?').next().unwrap_or(path);
    let path = if path.is_empty() { "/" } else { path };
    match (method.as_str(), path) {
        ("GET", "/openapi.json") => HttpOut {
            status: 200,
            body: openapi_spec(),
        },
        ("GET", "/") => HttpOut {
            status: 200,
            body: json!({
                "ok": true,
                "name": "lot",
                "version": lot_core::VERSION,
                "door": "http",
                "openapi": "/openapi.json"
            }),
        },
        ("POST", p) => match tool_name_from_path(p) {
            Some(name) if known_tool(name) => {
                let args = if body.is_empty() {
                    json!({})
                } else {
                    match serde_json::from_slice::<Value>(body) {
                        Ok(v) => v,
                        Err(e) => {
                            return HttpOut {
                                status: 400,
                                body: json!({ "ok": false, "error": format!("bad json — {e}") }),
                            };
                        }
                    }
                };
                let out = invoke_http(name, args);
                let status = if out.get("isError") == Some(&json!(true)) {
                    400
                } else {
                    200
                };
                HttpOut { status, body: out }
            }
            Some(name) => HttpOut {
                status: 404,
                body: json!({ "ok": false, "error": format!("unknown tool — {name}") }),
            },
            None => HttpOut {
                status: 404,
                body: json!({ "ok": false, "error": format!("unknown tool — {p}") }),
            },
        },
        _ => HttpOut {
            status: 405,
            body: json!({ "ok": false, "error": "method not allowed —" }),
        },
    }
}

fn tool_name_from_path(path: &str) -> Option<&str> {
    let p = path.trim_start_matches('/');
    let p = p.strip_prefix("tools/").unwrap_or(p);
    if p.is_empty() || p.contains('/') || p.contains("..") {
        return None;
    }
    if p.starts_with("lot_") {
        Some(p)
    } else {
        None
    }
}

fn known_tool(name: &str) -> bool {
    super::tools()
        .as_array()
        .map(|arr| arr.iter().any(|t| t["name"] == name))
        .unwrap_or(false)
}

fn invoke_http(name: &str, args: Value) -> Value {
    let mut out = super::call(Some(&json!({
        "name": name,
        "arguments": args
    })));
    if name == "lot_status" {
        patch_door(&mut out);
    }
    out
}

fn patch_door(out: &mut Value) {
    if let Some(obj) = out
        .get_mut("structuredContent")
        .and_then(|v| v.as_object_mut())
    {
        obj.insert("door".into(), json!("http"));
    }
    let text = out
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());
    if let Some(text) = text {
        if let Ok(mut v) = serde_json::from_str::<Value>(&text) {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("door".into(), json!("http"));
            }
            if let Some(c) = out
                .get_mut("content")
                .and_then(|c| c.get_mut(0))
                .and_then(|c| c.as_object_mut())
            {
                c.insert("text".into(), json!(v.to_string()));
            }
        }
    }
}

pub fn serve_start(bind: &str) -> io::Result<(TcpListener, String)> {
    let listener = TcpListener::bind(bind)?;
    let actual = listener.local_addr()?.to_string();
    Ok((listener, actual))
}

pub fn run_listener(listener: TcpListener) -> io::Result<()> {
    for incoming in listener.incoming() {
        let mut stream = incoming?;
        let _ = handle_connection(&mut stream);
    }
    Ok(())
}

pub fn run_http(bind: &str) -> io::Result<()> {
    let (listener, _) = serve_start(bind)?;
    run_listener(listener)
}

pub fn handle_connection(stream: &mut TcpStream) -> io::Result<()> {
    let req = read_http(stream)?;
    let out = handle_http(&req.method, &req.path, &req.body);
    write_http(stream, out.status, &out.body)
}

struct HttpReq {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_http(stream: &mut TcpStream) -> io::Result<HttpReq> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_headers_end(&buf) {
            let header = String::from_utf8_lossy(&buf[..pos]);
            let rest = buf[pos + 4..].to_vec();
            let mut lines = header.split("\r\n");
            let req_line = lines.next().unwrap_or("");
            let mut parts = req_line.split_whitespace();
            let method = parts.next().unwrap_or("GET").to_string();
            let path = parts.next().unwrap_or("/").to_string();
            let mut content_len = 0usize;
            for line in lines {
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_len = v.trim().parse().unwrap_or(0);
                }
            }
            content_len = content_len.min(1_000_000);
            let mut body = rest;
            while body.len() < content_len {
                let n = stream.read(&mut tmp)?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
            body.truncate(content_len);
            return Ok(HttpReq { method, path, body });
        }
        if buf.len() > 64_000 {
            break;
        }
    }
    Ok(HttpReq {
        method: "GET".into(),
        path: "/".into(),
        body: Vec::new(),
    })
}

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn write_http(stream: &mut TcpStream, status: u16, body: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()
}
