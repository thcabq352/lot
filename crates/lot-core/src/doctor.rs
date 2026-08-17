//! Runtime probes. Never assume a drive letter, GPU, or Comfy path.

use serde::Serialize;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct Doctor {
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub comfy: bool,
    pub grok_configured: bool,
    pub local_configured: bool,
    pub renderer: &'static str,
}

impl Doctor {
    pub fn probe() -> Self {
        let ffmpeg = on_path("ffmpeg");
        let ffprobe = on_path("ffprobe");
        let comfy = comfy_up();
        let grok_configured = grok_present();
        let local_configured = std::env::var("LOT_LOCAL_BASE_URL")
            .ok()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            || std::env::var("LOT_LOCAL_MODEL")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
        Self {
            ffmpeg,
            ffprobe,
            comfy,
            grok_configured,
            local_configured,
            renderer: if comfy { "comfy" } else { "unavailable" },
        }
    }
}

fn on_path(name: &str) -> bool {
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
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()
                .map(|h| std::path::PathBuf::from(h).join(".hermes"))
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
