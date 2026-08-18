//! Runtime probes. Never assume a drive letter, GPU, or Comfy path.
//! Optional apps (Ollama, Comfy, Resolve, Blockout, Motion Previs) are never required.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Doctor {
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub comfy: bool,
    pub grok_configured: bool,
    pub local_configured: bool,
    pub ollama: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_llm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_vision: Option<String>,
    pub vo_tts: bool,
    pub soundtrack_cmd: bool,
    pub stills_comfy_workflow: bool,
    pub prompt_server: bool,
    pub motion_previs: bool,
    pub motion_cmd: bool,
    pub blockout: bool,
    pub resolve: bool,
    pub upscale_cmd: bool,
    pub renderer: &'static str,
    /// Honest absences. Never fail `lot status` / `lot doctor`.
    pub notes: Vec<String>,
}

impl Doctor {
    pub fn probe() -> Self {
        let ffmpeg = bin_on_path("ffmpeg");
        let ffprobe = bin_on_path("ffprobe");
        let comfy = comfy_up();
        let grok_configured = grok_present();
        let ollama = crate::brain::probe_ollama();
        let local_configured = std::env::var("LOT_LOCAL_BASE_URL")
            .ok()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            || std::env::var("LOT_LOCAL_MODEL")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            || ollama.up;
        let motion_previs = crate::motion::motion_previs_control().is_some();
        let blockout = crate::stage::blockout_control().is_some();
        let resolve = resolve_studio();
        let mut notes = Vec::new();
        if !ffmpeg {
            notes.push("no ffmpeg —".into());
        }
        if !ffprobe {
            notes.push("no ffprobe —".into());
        }
        if !comfy {
            notes.push("no comfy —".into());
        }
        if !ollama.up {
            notes.push("no ollama —".into());
        }
        if !blockout {
            notes.push("no blockout —".into());
        }
        if !motion_previs {
            notes.push("no motion previs —".into());
        }
        if !resolve {
            notes.push("no resolve —".into());
        }
        Self {
            ffmpeg,
            ffprobe,
            comfy,
            grok_configured,
            local_configured,
            ollama: ollama.up,
            ollama_llm: ollama.llm,
            ollama_vision: ollama.vision,
            vo_tts: crate::stems::vo_backend_name().is_some(),
            soundtrack_cmd: std::env::var("LOT_SOUNDTRACK_CMD")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            stills_comfy_workflow: crate::stills::comfy_workflow_ready(),
            prompt_server: std::env::var("LOT_PROMPT_SERVER")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            motion_previs,
            motion_cmd: std::env::var("LOT_MOTION_CMD")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            blockout,
            resolve,
            upscale_cmd: std::env::var("LOT_UPSCALE_CMD")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            renderer: if comfy { "comfy" } else { "unavailable" },
            notes,
        }
    }
}

pub(crate) fn bin_on_path(name: &str) -> bool {
    bin_path(name).is_some()
}

pub(crate) fn bin_path(name: &str) -> Option<PathBuf> {
    let exe = win_exe(name);
    if let Ok(dir) = std::env::var("LOT_SIDECAR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            if let Some(p) = first_file(sidecar_candidates(Path::new(dir), name, &exe)) {
                return Some(p);
            }
        }
    }
    if let Ok(here) = std::env::current_exe() {
        if let Some(dir) = here.parent() {
            if let Some(p) = first_file(sidecar_candidates(dir, name, &exe)) {
                return Some(p);
            }
        }
    }
    if which_on_path(name) {
        return Some(PathBuf::from(name));
    }
    None
}

pub(crate) fn bin_command(name: &str) -> Option<Command> {
    bin_path(name).map(Command::new)
}

fn win_exe(name: &str) -> String {
    if cfg!(windows) && !name.ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn sidecar_candidates(root: &Path, name: &str, exe: &str) -> Vec<PathBuf> {
    vec![
        root.join(exe),
        root.join(name),
        root.join("sidecar").join(exe),
        root.join("sidecar").join(name),
        root.join("sidecar").join("ffmpeg").join(exe),
        root.join("sidecar").join("ffmpeg").join(name),
    ]
}

fn first_file(cands: Vec<PathBuf>) -> Option<PathBuf> {
    cands.into_iter().find(|p| p.is_file())
}

fn which_on_path(name: &str) -> bool {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("where");
        c.arg(name);
        c
    } else {
        let mut c = Command::new("which");
        c.arg(name);
        c
    };
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

fn comfy_up() -> bool {
    let url = std::env::var("LOT_COMFY_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8188/system_stats".into());
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(250))
        .timeout_read(Duration::from_millis(250))
        .build();
    agent
        .get(&url)
        .call()
        .map(|r| r.status() < 500)
        .unwrap_or(false)
}

fn grok_present() -> bool {
    if std::env::var("LOT_XAI_TOKEN")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    if std::env::var("XAI_API_KEY")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    let home = std::env::var("HERMES_HOME")
        .or_else(|_| std::env::var("LOT_HERMES_HOME"))
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()
                .map(|h| PathBuf::from(h).join(".hermes"))
        });
    if let Some(h) = home {
        if let Ok(raw) = std::fs::read_to_string(h.join("auth.json")) {
            if raw.contains("xai-oauth") && raw.contains("access_token") {
                return true;
            }
        }
    }
    false
}

/// Probe only. Resolve is never bundled and never required.
fn resolve_studio() -> bool {
    if which_on_path("Resolve") || which_on_path("resolve") {
        return true;
    }
    let mut cands = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        cands.push(
            PathBuf::from(pf)
                .join("Blackmagic Design")
                .join("DaVinci Resolve")
                .join("Resolve.exe"),
        );
    }
    if let Ok(pf) = std::env::var("ProgramFiles(x86)") {
        cands.push(
            PathBuf::from(pf)
                .join("Blackmagic Design")
                .join("DaVinci Resolve")
                .join("Resolve.exe"),
        );
    }
    cands.push(PathBuf::from(
        "/Applications/DaVinci Resolve/DaVinci Resolve.app",
    ));
    cands.push(PathBuf::from("/opt/resolve/bin/resolve"));
    cands.iter().any(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_succeeds_without_optional_studios() {
        let d = Doctor::probe();
        if !d.blockout {
            assert!(
                d.notes.iter().any(|n| n == "no blockout —"),
                "missing blockout must note honestly {d:?}"
            );
        }
        if !d.resolve {
            assert!(
                d.notes.iter().any(|n| n == "no resolve —"),
                "missing resolve must note honestly {d:?}"
            );
        }
        if !d.comfy {
            assert!(d.notes.iter().any(|n| n == "no comfy —"), "{d:?}");
        }
        if !d.ollama {
            assert!(d.notes.iter().any(|n| n == "no ollama —"), "{d:?}");
        }
        if !d.ffmpeg {
            assert!(d.notes.iter().any(|n| n == "no ffmpeg —"), "{d:?}");
        }
        assert!(
            d.renderer == "comfy" || d.renderer == "unavailable",
            "renderer {0}",
            d.renderer
        );
    }

    #[test]
    fn sidecar_finds_pack_ffmpeg() {
        let _g = crate::TEST_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!(
            "lot-sidecar-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ff_dir = tmp.join("sidecar").join("ffmpeg");
        std::fs::create_dir_all(&ff_dir).unwrap();
        let name = if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };
        let fake = ff_dir.join(name);
        std::fs::write(&fake, []).unwrap();
        let prev = std::env::var_os("LOT_SIDECAR");
        std::env::set_var("LOT_SIDECAR", &tmp);
        let found = bin_path("ffmpeg");
        match prev {
            Some(v) => std::env::set_var("LOT_SIDECAR", v),
            None => std::env::remove_var("LOT_SIDECAR"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(found.as_deref(), Some(fake.as_path()));
    }
}
