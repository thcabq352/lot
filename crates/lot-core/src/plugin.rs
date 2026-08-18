//! Declared section adapters (sidecar stdio or WASM). No silent fork.

use crate::show::{append_event_with, bump, require_write_current, write_show, Show, ShowError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

pub const NO_PLUGIN: &str = "no plugin —";
pub const UNDECLARED: &str = "undeclared —";
pub const HASH_MISMATCH: &str = "plugin hash mismatch —";
pub const NO_WASM: &str = "no wasm runtime —";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub section: String,
    pub kind: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub wasm: String,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub id: String,
    pub section: String,
    pub kind: String,
    pub declared: bool,
}

struct Found {
    manifest: PluginManifest,
    dir: PathBuf,
}

pub fn plugin_list() -> Result<Vec<PluginInfo>, ShowError> {
    crate::caps::require(crate::caps::Cap::Read)?;
    Ok(discover()
        .into_iter()
        .map(|f| PluginInfo {
            id: f.manifest.id,
            section: f.manifest.section,
            kind: f.manifest.kind,
            declared: !f.manifest.sha256.trim().is_empty(),
        })
        .collect())
}

pub fn plugin_call(
    id: &str,
    verb: &str,
    args: Option<Value>,
) -> Result<(PathBuf, Show, Value), ShowError> {
    crate::caps::require(crate::caps::Cap::Write)?;
    let id = id.trim();
    if id.is_empty() {
        return Err(ShowError::Msg(NO_PLUGIN.into()));
    }
    let found = discover()
        .into_iter()
        .find(|f| f.manifest.id == id)
        .ok_or_else(|| ShowError::Msg(format!("{NO_PLUGIN} {id}")))?;
    if found.manifest.sha256.trim().is_empty() {
        return Err(ShowError::Msg(UNDECLARED.into()));
    }
    let adapter = adapter_path(&found)?;
    let digest = file_sha256(&adapter)?;
    if !digest.eq_ignore_ascii_case(found.manifest.sha256.trim()) {
        return Err(ShowError::Msg(HASH_MISMATCH.into()));
    }
    let kind = found.manifest.kind.to_ascii_lowercase();
    if kind == "wasm" {
        return Err(ShowError::Msg(NO_WASM.into()));
    }
    if kind != "stdio" {
        return Err(ShowError::Msg(format!("unknown plugin kind — {kind}")));
    }
    let (dir, mut show) = require_write_current()?;
    let req = json!({
        "verb": verb,
        "section": found.manifest.section,
        "show": dir.display().to_string(),
        "args": args.unwrap_or(json!({}))
    });
    let out = run_stdio(&adapter, &found.manifest.args, &found.dir, &req)?;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "plugin.call",
        &show,
        Some(json!({ "plugin": id, "verb": verb, "section": found.manifest.section })),
    )?;
    Ok((dir, show, out))
}

fn discover() -> Vec<Found> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for root in plugin_roots() {
        scan_root(&root, &mut out, &mut seen);
    }
    out
}

fn plugin_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(raw) = std::env::var("LOT_PLUGIN_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for part in raw.split(sep) {
            let p = PathBuf::from(part.trim());
            if !p.as_os_str().is_empty() {
                roots.push(p);
            }
        }
    }
    if let Ok(Some(show)) = crate::show::current_show_path() {
        roots.push(show.join("plugins"));
    }
    roots
}

fn scan_root(root: &Path, out: &mut Vec<Found>, seen: &mut Vec<String>) {
    let direct = root.join("plugin.json");
    if direct.is_file() {
        push_manifest(root, &direct, out, seen);
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for ent in entries.flatten() {
        let dir = ent.path();
        if dir.is_dir() {
            let man = dir.join("plugin.json");
            if man.is_file() {
                push_manifest(&dir, &man, out, seen);
            }
        }
    }
}

fn push_manifest(dir: &Path, file: &Path, out: &mut Vec<Found>, seen: &mut Vec<String>) {
    let Ok(raw) = fs::read_to_string(file) else {
        return;
    };
    let Ok(manifest) = serde_json::from_str::<PluginManifest>(&raw) else {
        return;
    };
    if manifest.id.trim().is_empty() || seen.iter().any(|id| id == &manifest.id) {
        return;
    }
    seen.push(manifest.id.clone());
    out.push(Found {
        manifest,
        dir: dir.to_path_buf(),
    });
}

fn adapter_path(found: &Found) -> Result<PathBuf, ShowError> {
    let name = if found.manifest.kind.to_ascii_lowercase() == "wasm" {
        found.manifest.wasm.trim()
    } else {
        found.manifest.command.trim()
    };
    if name.is_empty() {
        return Err(ShowError::Msg(UNDECLARED.into()));
    }
    let path = if Path::new(name).is_absolute() {
        PathBuf::from(name)
    } else {
        found.dir.join(name)
    };
    if !is_under(&path, &found.dir) {
        return Err(ShowError::Msg(format!(
            "jailed — plugin adapter outside {}",
            found.dir.display()
        )));
    }
    Ok(path)
}

fn is_under(child: &Path, root: &Path) -> bool {
    let c = std::fs::canonicalize(child)
        .unwrap_or_else(|_| std::path::absolute(child).unwrap_or_else(|_| child.to_path_buf()));
    let r = std::fs::canonicalize(root)
        .unwrap_or_else(|_| std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf()));
    c.starts_with(&r)
}

fn file_sha256(path: &Path) -> Result<String, ShowError> {
    let bytes = fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

fn run_stdio(cmd: &Path, args: &[String], cwd: &Path, req: &Value) -> Result<Value, ShowError> {
    let mut child = if cfg!(windows) {
        let ext = cmd
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "cmd" || ext == "bat" {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(cmd);
            c
        } else {
            Command::new(cmd)
        }
    } else {
        Command::new(cmd)
    };
    for a in args {
        child.arg(a);
    }
    let mut child = child
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ShowError::Msg(format!("no plugin — {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(req.to_string().as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let output = wait_output(child, Duration::from_secs(8))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(ShowError::Msg(format!(
            "plugin failed — {}",
            err.trim().chars().take(120).collect::<String>()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("{}");
    serde_json::from_str(line).map_err(|e| ShowError::Msg(format!("plugin json — {e}")))
}

fn wait_output(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, ShowError> {
    use std::io::Read;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stdout.take() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_h = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stderr.take() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ShowError::Msg("plugin timeout —".into()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(ShowError::Msg(e.to_string())),
        }
    };
    Ok(std::process::Output {
        status,
        stdout: out_h.join().unwrap_or_default(),
        stderr: err_h.join().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::Write;
    fn isolate() -> std::path::PathBuf {
        std::env::remove_var("LOT_SHOW");
        std::env::remove_var("LOT_CAP");
        std::env::remove_var("LOT_AGENT");
        std::env::remove_var("LOT_PLUGIN_PATH");
        crate::clear_caps();
        crate::clear_agent();
        let tmp = std::env::temp_dir().join(format!(
            "lot-plugin-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("LOT_HOME", tmp.join("home"));
        tmp
    }

    fn sha256_file(path: &std::path::Path) -> String {
        let bytes = fs::read(path).unwrap();
        let mut h = Sha256::new();
        h.update(&bytes);
        format!("{:x}", h.finalize())
    }

    fn write_color_adapter(root: &std::path::Path, sha: Option<&str>) -> std::path::PathBuf {
        let dir = root.join("color");
        fs::create_dir_all(&dir).unwrap();
        let cmd = if cfg!(windows) {
            let p = dir.join("color.cmd");
            fs::write(
                &p,
                "@echo off\r\necho {\"ok\":true,\"section\":\"color\",\"verb\":\"grade\",\"look\":\"teal-orange\"}\r\n",
            )
            .unwrap();
            p
        } else {
            let p = dir.join("color.sh");
            let mut f = fs::File::create(&p).unwrap();
            writeln!(f, "#!/bin/sh").unwrap();
            writeln!(
                f,
                "echo '{{\"ok\":true,\"section\":\"color\",\"verb\":\"grade\",\"look\":\"teal-orange\"}}'"
            )
            .unwrap();
            p
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&cmd).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&cmd, perms).unwrap();
        }
        let digest = sha.unwrap_or(&sha256_file(&cmd)).to_string();
        let man = serde_json::json!({
            "id": "color",
            "section": "color",
            "kind": "stdio",
            "command": cmd.file_name().unwrap().to_string_lossy(),
            "sha256": digest
        });
        fs::write(dir.join("plugin.json"), man.to_string()).unwrap();
        dir
    }

    #[test]
    fn undeclared_and_hash_mismatch_refuse() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = isolate();
        let show = tmp.join("show");
        crate::create_show(&show, Some("Plug")).unwrap();
        let plug_root = tmp.join("plugins");
        fs::create_dir_all(plug_root.join("bare")).unwrap();
        fs::write(
            plug_root.join("bare").join("plugin.json"),
            r#"{"id":"bare","section":"color","kind":"stdio","command":"bare.cmd"}"#,
        )
        .unwrap();
        std::env::set_var("LOT_PLUGIN_PATH", plug_root.display().to_string());
        let err = crate::plugin_call("bare", "grade", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("undeclared —"), "{err}");

        write_color_adapter(&plug_root, Some("deadbeef"));
        let err = crate::plugin_call("color", "grade", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("plugin hash mismatch —"), "{err}");
    }

    #[test]
    fn wasm_is_honest_no_runtime() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = isolate();
        let show = tmp.join("show");
        crate::create_show(&show, Some("Wasm")).unwrap();
        let dir = tmp.join("plugins").join("grade");
        fs::create_dir_all(&dir).unwrap();
        let wasm = dir.join("grade.wasm");
        fs::write(&wasm, b"\0asm").unwrap();
        let digest = sha256_file(&wasm);
        fs::write(
            dir.join("plugin.json"),
            serde_json::json!({
                "id": "grade",
                "section": "color",
                "kind": "wasm",
                "wasm": "grade.wasm",
                "sha256": digest
            })
            .to_string(),
        )
        .unwrap();
        std::env::set_var("LOT_PLUGIN_PATH", tmp.join("plugins").display().to_string());
        let err = crate::plugin_call("grade", "run", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no wasm runtime —"), "{err}");
    }

    #[test]
    fn stdio_color_plugin_grades_without_inventing_lut() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = isolate();
        let show = tmp.join("show");
        crate::create_show(&show, Some("Color")).unwrap();
        let plug_root = tmp.join("plugins");
        write_color_adapter(&plug_root, None);
        std::env::set_var("LOT_PLUGIN_PATH", plug_root.display().to_string());
        let listed = crate::plugin_list().unwrap();
        assert!(
            listed
                .iter()
                .any(|p| p.id == "color" && p.section == "color"),
            "{listed:?}"
        );
        let (_, _, out) = crate::plugin_call("color", "grade", None).unwrap();
        assert_eq!(out["ok"], true);
        assert_eq!(out["section"], "color");
        assert_eq!(out["look"], "teal-orange");
        assert!(!show.join("color.cube").exists());
        assert!(!show.join("lut.cube").exists());
    }

    #[test]
    fn unknown_plugin_is_honest() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate();
        std::env::remove_var("LOT_PLUGIN_PATH");
        let err = crate::plugin_call("color", "grade", None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no plugin —"), "{err}");
    }
}
