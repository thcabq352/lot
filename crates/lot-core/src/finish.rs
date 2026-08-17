//! Optional end-of-pipeline upscale + FPS pickup. Never a silent stub.

use crate::model::MediaItem;
use crate::show::{append_event_with, bump, require_current, write_show, Show, ShowError};
use crate::Provenance;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub fn finish_pickup(
    file: Option<&Path>,
    upscale: bool,
    fps: Option<&str>,
) -> Result<(PathBuf, Show, PathBuf), ShowError> {
    if !upscale && fps.map(str::trim).filter(|s| !s.is_empty()).is_none() {
        return Err(ShowError::Msg(
            "finish needs --upscale and/or --fps (optional end of pipeline)".into(),
        ));
    }
    let fps = fps
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let n: f64 = s
                .parse()
                .map_err(|_| ShowError::Msg(format!("finish --fps must be a number (got {s})")))?;
            if n <= 0.0 {
                return Err(ShowError::Msg("finish --fps must be > 0".into()));
            }
            Ok(s.to_string())
        })
        .transpose()?;

    if upscale {
        crate::caps::require(crate::caps::Cap::Render)?;
    } else {
        crate::caps::require_write()?;
    }
    let (dir, mut show) = require_current()?;
    let started = Instant::now();
    let src = resolve_src(&show, file)?;
    if !src.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", src.display())));
    }

    fs::create_dir_all(dir.join("finish"))?;
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("take");
    let dest = dir.join("finish").join(format!("{stem}-finish.mp4"));

    if upscale {
        if let Some(cmd) = std::env::var("LOT_UPSCALE_CMD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            let status = Command::new(&cmd)
                .arg(&src)
                .arg(&dest)
                .status()
                .map_err(|e| ShowError::Msg(format!("no finish — upscale engine: {e}")))?;
            if !status.success()
                || !dest.is_file()
                || dest.metadata().map(|m| m.len()).unwrap_or(0) == 0
            {
                let _ = fs::remove_file(&dest);
                return Err(ShowError::Msg(
                    "no finish — LOT_UPSCALE_CMD wrote no video. Did not write a stub.".into(),
                ));
            }
            if let Some(ref rate) = fps {
                fps_pass(&dest, rate)?;
            }
            return record(dir, &mut show, dest, upscale, fps.as_deref(), "lot_upscale_cmd", &cmd, started);
        }
    }

    if !crate::doctor::bin_on_path("ffmpeg") {
        return Err(ShowError::Msg(
            "no finish — ffmpeg not on PATH (or set LOT_UPSCALE_CMD). Did not write a stub.".into(),
        ));
    }

    let mut vf = Vec::new();
    if upscale {
        vf.push("scale=iw*2:ih*2:flags=lanczos".to_string());
    }
    if let Some(ref rate) = fps {
        vf.push(format!("fps={rate}"));
    }
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(&src)
        .arg("-vf")
        .arg(vf.join(","))
        .arg("-an")
        .arg(&dest)
        .status()
        .map_err(|e| ShowError::Msg(format!("no finish — ffmpeg: {e}")))?;
    if !status.success() || !dest.is_file() || dest.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = fs::remove_file(&dest);
        return Err(ShowError::Msg(
            "no finish — ffmpeg wrote no video. Did not write a stub.".into(),
        ));
    }
    record(
        dir,
        &mut show,
        dest,
        upscale,
        fps.as_deref(),
        "ffmpeg",
        "ffmpeg",
        started,
    )
}

fn fps_pass(file: &Path, rate: &str) -> Result<(), ShowError> {
    if !crate::doctor::bin_on_path("ffmpeg") {
        return Err(ShowError::Msg(
            "no finish — ffmpeg not on PATH for --fps after LOT_UPSCALE_CMD".into(),
        ));
    }
    let tmp = file.with_extension("fps-tmp.mp4");
    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(file)
        .args(["-vf", &format!("fps={rate}"), "-an"])
        .arg(&tmp)
        .status()
        .map_err(|e| ShowError::Msg(format!("no finish — fps: {e}")))?;
    if !status.success() || !tmp.is_file() {
        let _ = fs::remove_file(&tmp);
        return Err(ShowError::Msg(
            "no finish — fps pass wrote no video. Did not write a stub.".into(),
        ));
    }
    fs::rename(&tmp, file)?;
    Ok(())
}

fn resolve_src(show: &Show, file: Option<&Path>) -> Result<PathBuf, ShowError> {
    if let Some(p) = file {
        return Ok(p.to_path_buf());
    }
    if let Some(t) = show.takes.iter().rev().find(|t| t.circled) {
        return Ok(PathBuf::from(&t.path));
    }
    if let Some(t) = show.takes.last() {
        return Ok(PathBuf::from(&t.path));
    }
    Err(ShowError::Msg(
        "finish needs --file or a take (lot dailies ingest / circle)".into(),
    ))
}

fn record(
    dir: PathBuf,
    show: &mut Show,
    dest: PathBuf,
    upscaled: bool,
    fps: Option<&str>,
    engine: &str,
    model: &str,
    started: Instant,
) -> Result<(PathBuf, Show, PathBuf), ShowError> {
    let path_s = dest.display().to_string();
    let filter = match (upscaled, fps) {
        (true, Some(r)) => format!("scale*2,fps={r}"),
        (true, None) => "scale*2".into(),
        (false, Some(r)) => format!("fps={r}"),
        (false, None) => String::new(),
    };
    show.finish.path = Some(path_s.clone());
    show.finish.upscaled = upscaled;
    show.finish.fps = fps.map(str::to_string);
    show.finish.provenance = Some(
        Provenance::new("finish", model, "", engine)
            .with_prompt(&filter)
            .with_duration_ms(crate::brain::elapsed_ms(started)),
    );
    show.media.push(MediaItem {
        path: path_s.clone(),
        kind: "finish".into(),
        ..MediaItem::default()
    });
    show.phase = "cut".into();
    bump(show);
    write_show(&dir, show)?;
    append_event_with(
        &dir,
        "finish.pickup",
        show,
        Some(json!({
            "file": path_s,
            "upscaled": upscaled,
            "fps": fps,
            "provenance": show.finish.provenance,
        })),
    )?;
    Ok((dir, show.clone(), dest))
}
