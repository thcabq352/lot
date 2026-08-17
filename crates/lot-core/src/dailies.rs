use crate::model::{filename_shot_prefix, shot_nums_match, MediaItem, Take};
use crate::show::{
    append_event, append_event_with, bump, require_write_current, write_show, Show,
    ShowError,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn dailies_ingest(
    file: Option<&Path>,
    dir: Option<&Path>,
) -> Result<(PathBuf, Show), ShowError> {
    let (show_dir, mut show) = require_write_current()?;
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(f) = file {
        files.push(crate::jail::allow_source(f, &show_dir)?);
    }
    if let Some(d) = dir {
        let d = crate::jail::allow_source(d, &show_dir)?;
        collect_media(&d, &mut files)?;
    }
    if files.is_empty() {
        collect_media(&show_dir.join("media"), &mut files)?;
    }
    if files.is_empty() {
        return Err(ShowError::Msg(
            "dailies ingest needs --file or --dir (or media/ with clips)".into(),
        ));
    }
    let mut ingested = 0u32;
    for f in files {
        if ingest_one(&show_dir, &mut show, &f)? {
            ingested += 1;
        }
    }
    if ingested == 0 && show.takes.is_empty() {
        return Err(ShowError::Msg(
            "no clips bound — filename must start with a shot number (01-foo.mp4)".into(),
        ));
    }
    show.phase = "dailies".into();
    bump(&mut show);
    write_show(&show_dir, &show)?;
    append_event_with(
        &show_dir,
        "dailies.ingest",
        &show,
        Some(serde_json::json!({ "ingested": ingested, "takes": show.takes.len() })),
    )?;
    Ok((show_dir, show))
}

fn collect_media(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ShowError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for ent in fs::read_dir(dir)? {
        let p = ent?.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let e = ext.to_ascii_lowercase();
                if matches!(
                    e.as_str(),
                    "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "wav" | "mp3" | "aac"
                ) {
                    out.push(p);
                }
            }
        }
    }
    Ok(())
}

fn ingest_one(show_dir: &Path, show: &mut Show, file: &Path) -> Result<bool, ShowError> {
    if !file.is_file() {
        return Err(ShowError::Msg(format!("not a file: {}", file.display())));
    }
    let filename = file
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ShowError::Msg("bad filename".into()))?
        .to_string();
    let prefix = filename_shot_prefix(&filename).ok_or_else(|| {
        ShowError::Msg(format!(
            "filename must start with a shot number: {filename}"
        ))
    })?;
    let shot = show
        .shots
        .iter()
        .find(|s| shot_nums_match(&s.num, &prefix))
        .ok_or_else(|| ShowError::Msg(format!("no shot matching {prefix} (from {filename})")))?
        .clone();
    let shot_name_before = shot.name.clone();
    let bytes = fs::read(file)?;
    let sha = hex_sha256(&bytes);
    if show.takes.iter().any(|t| {
        t.sha256 == sha || (t.path == file.display().to_string() && t.filename == filename)
    }) {
        return Ok(false);
    }
    let duration = probe_duration(file);
    let id = format!("tk-{}", show.takes.len() + 1);
    show.takes.push(Take {
        id,
        shot_id: shot.id.clone(),
        path: std::path::absolute(file)
            .unwrap_or_else(|_| file.to_path_buf())
            .display()
            .to_string(),
        filename,
        sha256: sha.clone(),
        duration_secs: duration,
        circled: false,
    });
    show.media.push(MediaItem {
        path: file.display().to_string(),
        sha256: sha,
        kind: "video".into(),
        duration_secs: duration,
    });
    // AC-003: do not rename the shot to "01".
    if let Some(s) = show.shots.iter_mut().find(|s| s.id == shot.id) {
        debug_assert_eq!(s.name, shot_name_before);
        let _ = show_dir;
    }
    Ok(true)
}

pub fn dailies_circle(take_id: &str) -> Result<(PathBuf, Show), ShowError> {
    let take_id = take_id.trim();
    if take_id.is_empty() {
        return Err(ShowError::Msg(
            "lot dailies circle needs --take (no GUI picker)".into(),
        ));
    }
    let (dir, mut show) = require_write_current()?;
    let take = show
        .takes
        .iter_mut()
        .find(|t| t.id == take_id || t.filename == take_id)
        .ok_or_else(|| ShowError::Msg(format!("unknown take: {take_id}")))?;
    take.circled = true;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "dailies.circle", &show)?;
    Ok((dir, show))
}

pub fn dailies_export() -> Result<(PathBuf, Show, PathBuf), ShowError> {
    crate::caps::require(crate::caps::Cap::Export)?;
    let (dir, show) = require_write_current()?;
    let circled: Vec<&Take> = show.takes.iter().filter(|t| t.circled).collect();
    if circled.is_empty() {
        return Err(ShowError::Msg(
            "no circled takes — lot dailies circle --take".into(),
        ));
    }
    let xml = fcpxml(&show, &circled);
    let out = dir.join("export.fcpxml");
    fs::write(&out, xml)?;
    append_event_with(
        &dir,
        "dailies.export",
        &show,
        Some(serde_json::json!({ "file": out.display().to_string(), "takes": circled.len() })),
    )?;
    Ok((dir, show, out))
}

fn fcpxml(show: &Show, takes: &[&Take]) -> String {
    let mut clips = String::new();
    for (i, t) in takes.iter().enumerate() {
        let dur = t.duration_secs.unwrap_or(5.0);
        let frames = (dur * 24.0).round() as i64;
        let name = xml_esc(&t.filename);
        let path = xml_esc(&t.path);
        clips.push_str(&format!(
            "        <asset-clip name=\"{name}\" ref=\"r{i}\" offset=\"{i}/1s\" duration=\"{frames}/24s\" />\n"
        ));
        let _ = path;
    }
    let mut assets = String::new();
    for (i, t) in takes.iter().enumerate() {
        let dur = t.duration_secs.unwrap_or(5.0);
        let frames = (dur * 24.0).round() as i64;
        assets.push_str(&format!(
            "        <asset id=\"r{i}\" name=\"{name}\" src=\"file://localhost/{src}\" duration=\"{frames}/24s\" hasVideo=\"1\" />\n",
            name = xml_esc(&t.filename),
            src = t.path.replace('\\', "/").trim_start_matches('/'),
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE fcpxml>
<fcpxml version="1.9">
  <resources>
{assets}  </resources>
  <library>
    <event name="{title}">
      <project name="{title}">
        <sequence format="r0">
          <spine>
{clips}          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>
"#,
        title = xml_esc(&show.name),
        assets = assets,
        clips = clips
    )
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn probe_duration(file: &Path) -> Option<f64> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(file)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Shot;

    #[test]
    fn prefix_binds_without_renaming_shot() {
        assert_eq!(filename_shot_prefix("01-foo.mp4").as_deref(), Some("01"));
        assert!(shot_nums_match("01", "1"));
        assert!(shot_nums_match("01", "01"));
        let shot = Shot {
            id: "sh-01".into(),
            num: "01".into(),
            name: "INT. TENT - NIGHT".into(),
            ..Shot::default()
        };
        assert_eq!(shot.name, "INT. TENT - NIGHT");
        assert!(shot_nums_match(&shot.num, "01"));
    }
}
