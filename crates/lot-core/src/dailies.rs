use crate::model::{filename_shot_prefix, shot_nums_match, MediaItem, Take};
use crate::show::{
    append_event, append_event_with, bump, require_write_current, write_show, Show, ShowError,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestReport {
    pub ingested: u32,
    pub resumed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub file: PathBuf,
    pub edl: PathBuf,
    pub takes: usize,
    pub resumed: bool,
}

pub fn dailies_ingest(
    file: Option<&Path>,
    dir: Option<&Path>,
) -> Result<(PathBuf, Show, IngestReport), ShowError> {
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
    let mut report = IngestReport::default();
    for f in files {
        if ingest_one(&show_dir, &mut show, &f)? {
            report.ingested += 1;
        } else {
            report.resumed += 1;
        }
    }
    if report.ingested == 0 && show.takes.is_empty() {
        return Err(ShowError::Msg(
            "no clips bound — filename must start with a shot number (01-foo.mp4)".into(),
        ));
    }
    if report.ingested == 0 {
        return Ok((show_dir, show, report));
    }
    show.phase = "dailies".into();
    bump(&mut show);
    write_show(&show_dir, &show)?;
    append_event_with(
        &show_dir,
        "dailies.ingest",
        &show,
        Some(serde_json::json!({
            "ingested": report.ingested,
            "resumed": report.resumed,
            "takes": show.takes.len()
        })),
    )?;
    Ok((show_dir, show, report))
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
    let sha = file_sha256(file)?;
    if show
        .takes
        .iter()
        .any(|t| !t.sha256.is_empty() && t.sha256 == sha)
    {
        return Ok(false);
    }
    let dest = show_dir.join("media").join(&filename);
    own_copy(file, &dest, &sha)?;
    let path_s = std::path::absolute(&dest)
        .unwrap_or_else(|_| dest.to_path_buf())
        .display()
        .to_string();
    if show.takes.iter().any(|t| {
        t.filename == filename
            && (t.sha256.is_empty() || t.sha256 == sha || paths_match(&t.path, &path_s))
    }) {
        return Ok(false);
    }
    let duration = probe_duration(&dest).or_else(|| probe_duration(file));
    let id = format!("tk-{}", show.takes.len() + 1);
    show.takes.push(Take {
        id,
        shot_id: shot.id.clone(),
        path: path_s.clone(),
        filename,
        sha256: sha.clone(),
        duration_secs: duration,
        circled: false,
    });
    if !show.media.iter().any(|m| m.sha256 == sha) {
        show.media.push(MediaItem {
            path: path_s,
            sha256: sha,
            kind: "video".into(),
            duration_secs: duration,
        });
    }
    // AC-003: do not rename the shot to "01".
    if let Some(s) = show.shots.iter_mut().find(|s| s.id == shot.id) {
        debug_assert_eq!(s.name, shot_name_before);
    }
    Ok(true)
}

fn own_copy(src: &Path, dest: &Path, sha: &str) -> Result<(), ShowError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = dest.with_file_name(format!(
        "{}.part",
        dest.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("clip.part")
    ));
    if dest.is_file() && paths_match(&src.display().to_string(), &dest.display().to_string()) {
        let _ = fs::remove_file(&part);
        return Ok(());
    }
    if dest.is_file() {
        let dest_sha = file_sha256(dest)?;
        if dest_sha != sha {
            return Err(ShowError::Msg(format!(
                "media/{} already exists with a different hash",
                dest.file_name().and_then(|s| s.to_str()).unwrap_or("clip")
            )));
        }
        let _ = fs::remove_file(&part);
        return Ok(());
    }
    let _ = fs::remove_file(&part);
    if !paths_match(&src.display().to_string(), &dest.display().to_string()) {
        fs::copy(src, &part)?;
        fs::rename(&part, dest)?;
    }
    Ok(())
}

fn paths_match(a: &str, b: &str) -> bool {
    let pa = Path::new(a);
    let pb = Path::new(b);
    let ca = std::fs::canonicalize(pa)
        .unwrap_or_else(|_| std::path::absolute(pa).unwrap_or_else(|_| pa.to_path_buf()));
    let cb = std::fs::canonicalize(pb)
        .unwrap_or_else(|_| std::path::absolute(pb).unwrap_or_else(|_| pb.to_path_buf()));
    ca == cb
}

fn file_sha256(path: &Path) -> Result<String, ShowError> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65_536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn dailies_circle(take_id: &str) -> Result<(PathBuf, Show), ShowError> {
    let take_id = take_id.trim();
    if take_id.is_empty() {
        return Err(ShowError::Msg(
            "lot dailies circle needs --take (no GUI picker)".into(),
        ));
    }
    let (dir, mut show) = require_write_current()?;
    let already = show
        .takes
        .iter()
        .find(|t| t.id == take_id || t.filename == take_id)
        .ok_or_else(|| ShowError::Msg(format!("unknown take: {take_id}")))?
        .circled;
    if already {
        return Ok((dir, show));
    }
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

pub fn dailies_export() -> Result<(PathBuf, Show, ExportReport), ShowError> {
    crate::caps::require(crate::caps::Cap::Export)?;
    let (dir, show) = require_write_current()?;
    let circled: Vec<&Take> = show.takes.iter().filter(|t| t.circled).collect();
    if circled.is_empty() {
        return Err(ShowError::Msg(
            "no circled takes — lot dailies circle --take".into(),
        ));
    }
    let takes_n = circled.len();
    let xml = fcpxml(&show.name, &circled, &dir);
    let edl = cmx3600(&show.name, &circled);
    drop(circled);
    let xml_out = dir.join("export.fcpxml");
    let edl_out = dir.join("export.edl");
    if same_text(&xml_out, &xml) && same_text(&edl_out, &edl) {
        return Ok((
            dir,
            show,
            ExportReport {
                file: xml_out,
                edl: edl_out,
                takes: takes_n,
                resumed: true,
            },
        ));
    }
    fs::write(&xml_out, xml)?;
    fs::write(&edl_out, edl)?;
    append_event_with(
        &dir,
        "dailies.export",
        &show,
        Some(serde_json::json!({
            "file": xml_out.display().to_string(),
            "edl": edl_out.display().to_string(),
            "takes": takes_n
        })),
    )?;
    Ok((
        dir,
        show,
        ExportReport {
            file: xml_out,
            edl: edl_out,
            takes: takes_n,
            resumed: false,
        },
    ))
}

fn same_text(path: &Path, want: &str) -> bool {
    path.is_file() && fs::read_to_string(path).ok().as_deref() == Some(want)
}

const FCP_FPS: i64 = 24;

fn take_frames(t: &Take) -> i64 {
    let dur = t.duration_secs.filter(|d| *d > 0.0).unwrap_or(5.0);
    let frames = (dur * FCP_FPS as f64).round() as i64;
    frames.max(1)
}

fn cmx3600(title: &str, takes: &[&Take]) -> String {
    let mut out = String::from("TITLE: ");
    out.push_str(&edl_title(title));
    out.push_str("\nFCM: NON-DROP FRAME\n");
    let mut rec: i64 = 0;
    for (i, t) in takes.iter().enumerate() {
        let frames = take_frames(t);
        let src_out = timecode(frames);
        let rec_in = timecode(rec);
        let rec_out = timecode(rec + frames);
        let reel = reel_name(&t.filename);
        let ev = i + 1;
        out.push_str(&format!(
            "\n{ev:03}  {reel} V     C        00:00:00:00 {src_out} {rec_in} {rec_out}\n"
        ));
        let clip = if t.filename.is_empty() {
            Path::new(&t.path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("clip")
                .to_string()
        } else {
            t.filename.clone()
        };
        out.push_str(&format!("* FROM CLIP NAME: {clip}\n"));
        if !t.path.is_empty() {
            let src =
                std::path::absolute(Path::new(&t.path)).unwrap_or_else(|_| PathBuf::from(&t.path));
            out.push_str(&format!("* SOURCE FILE: {}\n", src.display()));
        }
        rec += frames;
    }
    out
}

fn edl_title(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(80)
        .collect()
}

fn reel_name(filename: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("AX");
    let mut s: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .take(8)
        .collect();
    if s.is_empty() {
        s.push_str("AX");
    }
    format!("{s:<8}")
}

fn timecode(frames: i64) -> String {
    let frames = frames.max(0);
    let ff = frames % FCP_FPS;
    let secs = frames / FCP_FPS;
    let ss = secs % 60;
    let mins = secs / 60;
    let mm = mins % 60;
    let hh = mins / 60;
    format!("{hh:02}:{mm:02}:{ss:02}:{ff:02}")
}

fn fcpxml(title: &str, takes: &[&Take], show_dir: &Path) -> String {
    let mut assets = String::from(
        "    <format id=\"r1\" name=\"FFVideoFormat1080p24\" frameDuration=\"1/24s\" width=\"1920\" height=\"1080\"/>\n",
    );
    let mut clips = String::new();
    let mut offset: i64 = 0;
    for (i, t) in takes.iter().enumerate() {
        let frames = take_frames(t);
        let asset_id = i + 2;
        let name = xml_esc(&t.filename);
        let src = xml_esc(&file_url(&t.path));
        let uid = if t.sha256.is_empty() {
            String::new()
        } else {
            format!(" uid=\"{}\"", xml_esc(&t.sha256))
        };
        assets.push_str(&format!(
            "    <asset id=\"r{asset_id}\" name=\"{name}\"{uid} start=\"0s\" duration=\"{frames}/24s\" hasVideo=\"1\" format=\"r1\">\n      <media-rep kind=\"original-media\" src=\"{src}\"/>\n    </asset>\n"
        ));
        clips.push_str(&format!(
            "        <asset-clip name=\"{name}\" ref=\"r{asset_id}\" offset=\"{offset}/24s\" duration=\"{frames}/24s\" format=\"r1\"/>\n"
        ));
        offset += frames;
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE fcpxml>
<fcpxml version="1.9">
  <resources>
{assets}  </resources>
  <library location="{lib}">
    <event name="{title}">
      <project name="{title}">
        <sequence format="r1" duration="{total}/24s" tcStart="0s" tcFormat="NDF">
          <spine>
{clips}          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>
"#,
        title = xml_esc(title),
        assets = assets,
        clips = clips,
        total = offset,
        lib = xml_esc(&file_url(&show_dir.display().to_string())),
    )
}

fn file_url(path: &str) -> String {
    let p = Path::new(path);
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut s = abs.to_string_lossy().replace('\\', "/");
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    format!("file://{}", percent_encode_path(&s))
}

fn percent_encode_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

    #[test]
    fn fcpxml_has_format_file_url_and_cumulative_offsets() {
        let t1 = Take {
            id: "tk-1".into(),
            filename: "01-foo.mp4".into(),
            path: "/tmp/show/media/01-foo.mp4".into(),
            duration_secs: Some(2.0),
            circled: true,
            ..Take::default()
        };
        let t2 = Take {
            id: "tk-2".into(),
            filename: "02-bar.mp4".into(),
            path: "/tmp/show/media/02 bar.mp4".into(),
            duration_secs: Some(3.0),
            circled: true,
            ..Take::default()
        };
        let xml = fcpxml("Carnival", &[&t1, &t2], Path::new("/tmp/show"));
        assert!(xml.contains(r#"<format id="r1""#));
        assert!(xml.contains(r#"<sequence format="r1""#));
        assert!(xml.contains(r#"<asset id="r2""#));
        assert!(xml.contains(r#"<asset id="r3""#));
        assert!(xml.contains(r#"ref="r2""#));
        assert!(xml.contains(r#"ref="r3""#));
        assert!(xml.contains(r#"offset="0/24s""#));
        assert!(xml.contains(r#"offset="48/24s""#));
        assert!(xml.contains(r#"duration="120/24s""#));
        assert!(xml.contains("file://"));
        assert!(xml.contains("media-rep"));
        assert!(
            xml.contains("%20"),
            "spaces in paths must be percent-encoded"
        );
        assert!(!xml.contains(r#"format="r0""#));
        assert!(!xml.contains("file://localhost/"));
        let edl = cmx3600("Carnival", &[&t1, &t2]);
        assert!(edl.starts_with("TITLE: Carnival\n"));
        assert!(edl.contains("FCM: NON-DROP FRAME"));
        assert!(edl.contains(
            "001  01FOO    V     C        00:00:00:00 00:00:02:00 00:00:00:00 00:00:02:00"
        ));
        assert!(edl.contains(
            "002  02BAR    V     C        00:00:00:00 00:00:03:00 00:00:02:00 00:00:05:00"
        ));
        assert!(edl.contains("* FROM CLIP NAME: 01-foo.mp4"));
        assert!(edl.contains("* FROM CLIP NAME: 02-bar.mp4"));
        assert!(edl.contains("* SOURCE FILE:"));
    }

    #[test]
    fn timecode_is_24fps_ndf() {
        assert_eq!(timecode(0), "00:00:00:00");
        assert_eq!(timecode(24), "00:00:01:00");
        assert_eq!(timecode(48), "00:00:02:00");
        assert_eq!(timecode(24 * 60), "00:01:00:00");
        assert_eq!(reel_name("01-foo.mp4"), "01FOO   ");
        assert_eq!(reel_name(""), "AX      ");
    }

    #[test]
    fn file_url_is_absolute_file_scheme() {
        let unix = file_url("/tmp/show/media/01-foo.mp4");
        assert!(unix.starts_with("file:///"), "{unix}");
        assert!(!unix.contains("localhost"));
        let win = file_url(r"C:\Users\thcab\lot\media\01-foo.mp4");
        assert!(win.starts_with("file:///"), "{win}");
        assert!(win.contains("C:"), "{win}");
    }
}
