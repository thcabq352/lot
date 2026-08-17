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
    pub upscale_cmd: bool,
    pub renderer: &'static str,
}

impl Doctor {
    pub fn probe() -> Self {
        let ffmpeg = on_path("ffmpeg");
        let ffprobe = on_path("ffprobe");
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
            motion_previs: crate::motion::motion_previs_control().is_some(),
            motion_cmd: std::env::var("LOT_MOTION_CMD")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            blockout: crate::stage::blockout_control().is_some(),
            upscale_cmd: std::env::var("LOT_UPSCALE_CMD")
                .ok()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            renderer: if comfy { "comfy" } else { "unavailable" },
        }
    }
}

pub(crate) fn bin_on_path(name: &str) -> bool {
    on_path(name)
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
