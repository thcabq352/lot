//! Optional end-of-pipeline upscale + FPS pickup. Never a silent stub.

use crate::model::MediaItem;
use crate::show::{append_event_with, bump, write_show, Show, ShowError};
use crate::Provenance;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub fn finish_pickup(
    file: Option<&Path>,
    upscale: bool,
    fps: Option<&str>,
) -> Result<(PathBuf, Show, PathBuf), ShowError> {
    crate::cancel::check()?;
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
    }
    let (dir, mut show) = crate::show::require_write_current()?;
    if upscale {
        crate::budget::require_render(&show)?;
    }
    let started = Instant::now();
    let src = resolve_src(&show, file)?;
    let src = crate::jail::allow_source(&src, &dir)?;
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
            let status = run_cancellable(
                Command::new(&cmd).arg(&src).arg(&dest),
                &dest,
                "no finish — upscale engine",
            )?;
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
            return record(
                dir,
                &mut show,
                dest,
                upscale,
                fps.as_deref(),
                "lot_upscale_cmd",
                &cmd,
                started,
            );
        }
    }

    let Some(mut ffmpeg) = crate::doctor::bin_command("ffmpeg") else {
        return Err(ShowError::Msg(
            "no finish — ffmpeg not on PATH (or set LOT_UPSCALE_CMD). Did not write a stub.".into(),
        ));
    };

    let mut vf = Vec::new();
    if upscale {
        vf.push("scale=iw*2:ih*2:flags=lanczos".to_string());
    }
    if let Some(ref rate) = fps {
        vf.push(format!("fps={rate}"));
    }
    let status = run_cancellable(
        ffmpeg
            .args(["-y", "-i"])
            .arg(&src)
            .arg("-vf")
            .arg(vf.join(","))
            .arg("-an")
            .arg(&dest),
        &dest,
        "no finish — ffmpeg",
    )?;
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

fn run_cancellable(
    cmd: &mut Command,
    dest: &Path,
    ctx: &str,
) -> Result<std::process::ExitStatus, ShowError> {
    crate::cancel::check()?;
    let mut child = cmd
        .spawn()
        .map_err(|e| ShowError::Msg(format!("{ctx}: {e}")))?;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return Ok(st),
            Ok(None) => {
                if crate::cancel::is_cancelled() {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = fs::remove_file(dest);
                    return Err(crate::cancel::cancelled_err());
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(ShowError::Msg(format!("{ctx}: {e}"))),
        }
    }
}

fn fps_pass(file: &Path, rate: &str) -> Result<(), ShowError> {
    let Some(mut ffmpeg) = crate::doctor::bin_command("ffmpeg") else {
        return Err(ShowError::Msg(
            "no finish — ffmpeg not on PATH for --fps after LOT_UPSCALE_CMD".into(),
        ));
    };
    let tmp = file.with_extension("fps-tmp.mp4");
    let status = run_cancellable(
        ffmpeg
            .args(["-y", "-i"])
            .arg(file)
            .args(["-vf", &format!("fps={rate}"), "-an"])
            .arg(&tmp),
        &tmp,
        "no finish — fps",
    )?;
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
    if upscaled {
        crate::budget::record_render(show);
    }
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
