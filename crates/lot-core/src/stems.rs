//! Stems: soundtrack + VO generation. Not an 11th movie stage.
//! Never write a fake song or a silent WAV and call it a score.

use crate::model::MediaItem;
use crate::show::{
    append_event_with, bump, require_current, write_show, Show, ShowError,
};
use crate::Provenance;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stems {
    #[serde(default)]
    pub soundtrack_brief: String,
    #[serde(default)]
    pub soundtrack_cue: Option<String>,
    #[serde(default)]
    pub soundtrack_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soundtrack_provenance: Option<Provenance>,
    #[serde(default)]
    pub vo_text: String,
    #[serde(default)]
    pub vo_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vo_provenance: Option<Provenance>,
}

pub fn stems_soundtrack(
    brief: Option<&str>,
    file: Option<&Path>,
    generate: bool,
) -> Result<(PathBuf, Show), ShowError> {
    crate::caps::require_write()?;
    let (dir, mut show) = require_current()?;
    if let Some(b) = brief {
        show.stems.soundtrack_brief = b.trim().to_string();
    }
    if show.stems.soundtrack_brief.is_empty() && file.is_none() {
        return Err(ShowError::Msg(
            "stems soundtrack needs --brief or --file".into(),
        ));
    }
    fs::create_dir_all(dir.join("stems"))?;

    if let Some(p) = file {
        if !p.is_file() {
            return Err(ShowError::Msg(format!("not a file: {}", p.display())));
        }
        let dest = dir.join("stems").join(
            p.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("soundtrack.wav"),
        );
        fs::copy(p, &dest)?;
        show.stems.soundtrack_path = Some(dest.display().to_string());
        show.media.push(MediaItem {
            path: dest.display().to_string(),
            kind: "audio".into(),
            ..MediaItem::default()
        });
        show.stems.soundtrack_provenance = Some(
            Provenance::new("file", "attach", "", "local")
                .with_prompt(&show.stems.soundtrack_brief),
        );
    }

    let cue = write_soundtrack_cue(&dir, &show)?;
    show.stems.soundtrack_cue = Some(cue.display().to_string());

    if generate && show.stems.soundtrack_path.is_none() {
        if let Some(cmd) = std::env::var("LOT_SOUNDTRACK_CMD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let out = dir.join("stems").join("soundtrack.wav");
            let started = Instant::now();
            let status = Command::new(&cmd)
                .arg(&show.stems.soundtrack_brief)
                .arg(&out)
                .status()
                .map_err(|e| ShowError::Msg(format!("soundtrack engine: {e}")))?;
            if !status.success() || !out.is_file() {
                return Err(ShowError::Msg(
                    "no soundtrack engine — LOT_SOUNDTRACK_CMD ran but wrote no audio".into(),
                ));
            }
            show.stems.soundtrack_path = Some(out.display().to_string());
            show.stems.soundtrack_provenance = Some(
                Provenance::new("local", cmd, "", "lot_soundtrack_cmd")
                    .with_prompt(&show.stems.soundtrack_brief)
                    .with_duration_ms(crate::brain::elapsed_ms(started)),
            );
        } else {
            return Err(ShowError::Msg(
                "no soundtrack engine — set LOT_SOUNDTRACK_CMD <cmd> <brief> <out.wav>, or attach --file. Cue is written; never a fake track.".into(),
            ));
        }
    }

    show.phase = "stems".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "stems.soundtrack",
        &show,
        Some(serde_json::json!({
            "cue": show.stems.soundtrack_cue,
            "audio": show.stems.soundtrack_path,
            "provenance": show.stems.soundtrack_provenance,
        })),
    )?;
    Ok((dir, show))
}

fn write_soundtrack_cue(dir: &Path, show: &Show) -> Result<PathBuf, ShowError> {
    let path = dir.join("stems").join("soundtrack-cue.md");
    let format = show.writer.format.clone().unwrap_or_else(|| "unset".into());
    let body = match crate::complete_chat(
        "You are Lot Stems, a film composer. Write a soundtrack cue sheet only: tempo, key/mood, instruments, structure (in/out times), what to avoid. No lyrics unless the format is music-video. Do not invent API keys.",
        &format!(
            "Show: {}\nFormat: {format}\nBrief:\n{}\n\nWrite the cue sheet now.",
            show.name, show.stems.soundtrack_brief
        ),
    ) {
        Ok(c) => format!(
            "# Soundtrack cue — {}\n\nFormat: {format}\nBackend: {} / {}\n\n{}\n",
            show.name, c.provenance.backend, c.provenance.model, c.text
        ),
        Err(_) => format!(
            "# Soundtrack cue — {}\n\nFormat: {format}\n\n{}\n\n_No language brain — cue is the filmmaker brief. Attach audio with --file or LOT_SOUNDTRACK_CMD._\n",
            show.name, show.stems.soundtrack_brief
        ),
    };
    fs::write(&path, body)?;
    Ok(path)
}

pub fn stems_vo(
    text: Option<&str>,
    file: Option<&Path>,
    generate: bool,
) -> Result<(PathBuf, Show), ShowError> {
    crate::caps::require_write()?;
    let (dir, mut show) = require_current()?;
    if let Some(t) = text {
        show.stems.vo_text = t.trim().to_string();
    }
    if show.stems.vo_text.is_empty() && file.is_none() {
        return Err(ShowError::Msg("stems vo needs --text or --file".into()));
    }
    fs::create_dir_all(dir.join("stems"))?;

    if let Some(p) = file {
        if !p.is_file() {
            return Err(ShowError::Msg(format!("not a file: {}", p.display())));
        }
        let dest = dir
            .join("stems")
            .join(p.file_name().and_then(|s| s.to_str()).unwrap_or("vo.wav"));
        fs::copy(p, &dest)?;
        show.stems.vo_path = Some(dest.display().to_string());
        show.stems.vo_provenance = Some(
            Provenance::new("file", "attach", "", "local").with_prompt(&show.stems.vo_text),
        );
    }

    if generate {
        if show.stems.vo_text.is_empty() {
            return Err(ShowError::Msg("stems vo --generate needs --text".into()));
        }
        let out = dir.join("stems").join("vo.wav");
        let prov = generate_vo(&show.stems.vo_text, &out)?;
        show.stems.vo_path = Some(out.display().to_string());
        show.stems.vo_provenance = Some(prov);
        show.media.push(MediaItem {
            path: out.display().to_string(),
            kind: "audio".into(),
            ..MediaItem::default()
        });
    }

    show.phase = "stems".into();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event_with(
        &dir,
        "stems.vo",
        &show,
        Some(serde_json::json!({
            "audio": show.stems.vo_path,
            "provenance": show.stems.vo_provenance,
        })),
    )?;
    Ok((dir, show))
}

pub fn vo_backend_name() -> Option<&'static str> {
    if std::env::var("LOT_TTS_CMD")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return Some("lot_tts_cmd");
    }
    if crate::doctor::bin_on_path("piper") {
        return Some("piper");
    }
    if crate::doctor::bin_on_path("espeak-ng") || crate::doctor::bin_on_path("espeak") {
        return Some("espeak");
    }
    if cfg!(windows) {
        return Some("sapi");
    }
    if crate::doctor::bin_on_path("say") {
        return Some("say");
    }
    None
}

fn generate_vo(text: &str, out: &Path) -> Result<Provenance, ShowError> {
    let started = Instant::now();
    let timed = |p: Provenance| p.with_duration_ms(crate::brain::elapsed_ms(started));
    if let Ok(cmd) = std::env::var("LOT_TTS_CMD") {
        let cmd = cmd.trim();
        if !cmd.is_empty() {
            let status = Command::new(cmd)
                .arg(text)
                .arg(out)
                .status()
                .map_err(|e| ShowError::Msg(format!("vo engine: {e}")))?;
            if status.success() && out.is_file() {
                return Ok(timed(
                    Provenance::new("local", cmd, "", "lot_tts_cmd").with_prompt(text),
                ));
            }
        }
    }
    if crate::doctor::bin_on_path("piper") {
        let status = Command::new("piper")
            .args(["--output_file"])
            .arg(out)
            .arg("--text")
            .arg(text)
            .status();
        if status.map(|s| s.success()).unwrap_or(false) && out.is_file() {
            return Ok(timed(
                Provenance::new("local", "piper", "", "piper").with_prompt(text),
            ));
        }
    }
    let espeak = if crate::doctor::bin_on_path("espeak-ng") {
        "espeak-ng"
    } else if crate::doctor::bin_on_path("espeak") {
        "espeak"
    } else {
        ""
    };
    if !espeak.is_empty() {
        let status = Command::new(espeak)
            .args(["-w"])
            .arg(out)
            .arg(text)
            .status();
        if status.map(|s| s.success()).unwrap_or(false) && out.is_file() {
            return Ok(timed(
                Provenance::new("local", espeak, "", "espeak").with_prompt(text),
            ));
        }
    }
    if cfg!(windows) {
        sapi_wav(text, out)?;
        return Ok(timed(
            Provenance::new("local", "sapi", "", "windows_sapi").with_prompt(text),
        ));
    }
    if crate::doctor::bin_on_path("say") {
        let status = Command::new("say").args(["-o"]).arg(out).arg(text).status();
        if status.map(|s| s.success()).unwrap_or(false) && out.is_file() {
            return Ok(timed(
                Provenance::new("local", "say", "", "say").with_prompt(text),
            ));
        }
    }
    Err(ShowError::Msg(
        "no vo brain — set LOT_TTS_CMD, or install piper/espeak, or use Windows SAPI / macOS say. Never a silent stub.".into(),
    ))
}

fn sapi_wav(text: &str, out: &Path) -> Result<(), ShowError> {
    let dest = out.display().to_string().replace('\'', "''");
    let spoken = text.replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.SetOutputToWaveFile('{dest}'); $s.Speak('{spoken}'); $s.Dispose()"
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|e| ShowError::Msg(format!("sapi: {e}")))?;
    if !status.success() || !out.is_file() {
        return Err(ShowError::Msg(
            "no vo brain — Windows SAPI did not write a wav".into(),
        ));
    }
    Ok(())
}
