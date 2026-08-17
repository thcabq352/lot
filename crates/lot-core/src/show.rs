use crate::model::{Beat, MediaItem, Scene, Shot, Take};
use crate::packs::{self, IdKind};
use crate::{SchoolStatus, SHOW_FILE, SHOW_SCHEMA};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCREENPLAY_FILE: &str = "screenplay.fountain";

#[derive(Debug)]
pub enum ShowError {
    Io(io::Error),
    Json(serde_json::Error),
    Exists(PathBuf),
    NotAShow(PathBuf),
    Schema(u32),
    Msg(String),
}

impl fmt::Display for ShowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShowError::Io(e) => write!(f, "{e}"),
            ShowError::Json(e) => write!(f, "{e}"),
            ShowError::Exists(p) => write!(f, "show already exists: {}", p.display()),
            ShowError::NotAShow(p) => {
                write!(f, "not a show.lot (missing {SHOW_FILE}): {}", p.display())
            }
            ShowError::Schema(n) => write!(f, "unsupported show schema {n} (want {SHOW_SCHEMA})"),
            ShowError::Msg(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for ShowError {}

impl From<io::Error> for ShowError {
    fn from(e: io::Error) -> Self {
        ShowError::Io(e)
    }
}

impl From<serde_json::Error> for ShowError {
    fn from(e: serde_json::Error) -> Self {
        ShowError::Json(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub rev: u64,
    pub school: SchoolStatus,
    #[serde(default = "crate::model::default_phase_value")]
    pub phase: String,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub shots: Vec<Shot>,
    #[serde(default)]
    pub takes: Vec<Take>,
    #[serde(default)]
    pub media: Vec<MediaItem>,
    #[serde(default)]
    pub wall: Vec<Beat>,
    #[serde(default)]
    pub writer: Writer,
    #[serde(default)]
    pub stems: crate::stems::Stems,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Writer {
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub styles_living: Vec<String>,
    #[serde(default)]
    pub styles_canon: Vec<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub cast: Vec<CastMember>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub draft_path: Option<String>,
    /// Last successful draft/revise brain (backend/model/base_url/auth). Never a secret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_provenance: Option<crate::Provenance>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CastMember {
    pub name: String,
    #[serde(default)]
    pub function: String,
    #[serde(default)]
    pub look: String,
    #[serde(default, alias = "must-not")]
    pub must_not: String,
}

impl Show {
    fn new(name: String) -> Self {
        let now = now_rfc3339();
        Self {
            schema: SHOW_SCHEMA,
            id: new_id(),
            name,
            created_at: now.clone(),
            updated_at: now,
            rev: 1,
            school: SchoolStatus::default(),
            phase: "writer".into(),
            scenes: Vec::new(),
            shots: Vec::new(),
            takes: Vec::new(),
            media: Vec::new(),
            wall: Vec::new(),
            writer: Writer::default(),
            stems: crate::stems::Stems::default(),
        }
    }
}

pub fn create_show(dir: &Path, name: Option<&str>) -> Result<(PathBuf, Show), ShowError> {
    let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    let show_json = dir.join(SHOW_FILE);
    if show_json.exists() {
        return Err(ShowError::Exists(dir));
    }
    if dir.exists() && dir.read_dir()?.next().is_some() {
        return Err(ShowError::Msg(format!(
            "directory not empty: {}",
            dir.display()
        )));
    }
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join("media"))?;
    let title = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            dir.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string()
        });
    let show = Show::new(title);
    write_show(&dir, &show)?;
    append_event(&dir, "create", &show)?;
    set_current_show(&dir)?;
    Ok((dir, show))
}

pub fn open_show(dir: &Path) -> Result<(PathBuf, Show), ShowError> {
    let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    let show = read_show(&dir)?;
    set_current_show(&dir)?;
    Ok((dir, show))
}

pub fn read_show(dir: &Path) -> Result<Show, ShowError> {
    let path = dir.join(SHOW_FILE);
    if !path.is_file() {
        return Err(ShowError::NotAShow(dir.to_path_buf()));
    }
    let raw = fs::read_to_string(&path)?;
    let show: Show = serde_json::from_str(&raw)?;
    if show.schema != SHOW_SCHEMA {
        return Err(ShowError::Schema(show.schema));
    }
    Ok(show)
}

pub fn require_current() -> Result<(PathBuf, Show), ShowError> {
    let dir = current_show_path()?
        .ok_or_else(|| ShowError::Msg("no current show — lot create or lot open".into()))?;
    let show = read_show(&dir)?;
    Ok((dir, show))
}

fn require_unlocked() -> Result<(PathBuf, Show), ShowError> {
    let (dir, show) = require_current()?;
    if show.writer.locked {
        return Err(ShowError::Msg(
            "writer locked — unlock before changing the draft".into(),
        ));
    }
    Ok((dir, show))
}

pub(crate) fn bump(show: &mut Show) {
    show.rev += 1;
    show.updated_at = now_rfc3339();
}

pub fn set_brief(text: &str) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_unlocked()?;
    show.writer.brief = text.trim().to_string();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "writer.brief", &show)?;
    Ok((dir, show))
}

pub fn set_style(
    genres: Option<&[String]>,
    living: Option<&[String]>,
    canon: Option<&[String]>,
    format: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_unlocked()?;
    if genres.is_none() && living.is_none() && canon.is_none() && format.is_none() {
        return Err(ShowError::Msg(
            "style needs --genre, --living, --canon, or --format".into(),
        ));
    }
    let new_genres = genres
        .map(|g| packs::resolve_ids(IdKind::Genre, g))
        .transpose()?;
    let new_living = living
        .map(|g| packs::resolve_ids(IdKind::Living, g))
        .transpose()?;
    let new_canon = canon
        .map(|g| packs::resolve_ids(IdKind::Canon, g))
        .transpose()?;
    let new_format = format.map(packs::resolve_format).transpose()?;
    if let Some(g) = new_genres {
        show.writer.genres = g;
    }
    if let Some(g) = new_living {
        show.writer.styles_living = g;
    }
    if let Some(g) = new_canon {
        show.writer.styles_canon = g;
    }
    if let Some(f) = new_format {
        show.writer.format = Some(f);
    }
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "writer.style", &show)?;
    Ok((dir, show))
}

pub fn upsert_cast(
    name: &str,
    function: Option<&str>,
    look: Option<&str>,
    must_not: Option<&str>,
) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_unlocked()?;
    let name = name.trim();
    if name.is_empty() {
        return Err(ShowError::Msg("cast needs --name or --from-json".into()));
    }
    if let Some(existing) = show
        .writer
        .cast
        .iter_mut()
        .find(|c| c.name.eq_ignore_ascii_case(name))
    {
        existing.name = name.to_string();
        if let Some(f) = function {
            existing.function = f.trim().to_string();
        }
        if let Some(l) = look {
            existing.look = l.trim().to_string();
        }
        if let Some(m) = must_not {
            existing.must_not = m.trim().to_string();
        }
    } else {
        show.writer.cast.push(CastMember {
            name: name.to_string(),
            function: function.unwrap_or("").trim().to_string(),
            look: look.unwrap_or("").trim().to_string(),
            must_not: must_not.unwrap_or("").trim().to_string(),
        });
    }
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "writer.cast", &show)?;
    Ok((dir, show))
}

pub fn replace_cast(members: Vec<CastMember>) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_unlocked()?;
    for m in &members {
        if m.name.trim().is_empty() {
            return Err(ShowError::Msg("cast json: each member needs name".into()));
        }
    }
    show.writer.cast = members
        .into_iter()
        .map(|mut m| {
            m.name = m.name.trim().to_string();
            m.function = m.function.trim().to_string();
            m.look = m.look.trim().to_string();
            m.must_not = m.must_not.trim().to_string();
            m
        })
        .collect();
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "writer.cast", &show)?;
    Ok((dir, show))
}

pub fn replace_cast_json(raw: &str) -> Result<(PathBuf, Show), ShowError> {
    let members: Vec<CastMember> = serde_json::from_str(raw)?;
    replace_cast(members)
}

pub fn lock_writer() -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_current()?;
    if show.writer.locked {
        return Ok((dir, show));
    }
    show.writer.locked = true;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "writer.lock", &show)?;
    Ok((dir, show))
}

pub fn unlock_writer() -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_current()?;
    if !show.writer.locked {
        return Ok((dir, show));
    }
    show.writer.locked = false;
    bump(&mut show);
    write_show(&dir, &show)?;
    append_event(&dir, "writer.unlock", &show)?;
    Ok((dir, show))
}

pub fn draft_screenplay() -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_unlocked()?;
    if show.writer.brief.trim().is_empty() {
        return Err(ShowError::Msg(
            "no brief — lot writer brief --text \"...\"".into(),
        ));
    }
    // Real brain only. No fake INT. LOT outline stub.
    let completion = crate::brain::draft_fountain(&show)?;
    write_fountain(&dir, &mut show, completion, "writer.draft")?;
    Ok((dir, show))
}

pub fn revise_screenplay(notes: &str) -> Result<(PathBuf, Show), ShowError> {
    let (dir, mut show) = require_unlocked()?;
    let path = dir.join(SCREENPLAY_FILE);
    if !path.is_file() {
        return Err(ShowError::Msg(
            "no draft — lot writer draft first (missing screenplay.fountain)".into(),
        ));
    }
    let current = fs::read_to_string(&path)?;
    let completion = crate::brain::revise_fountain(&show, &current, notes)?;
    write_fountain(&dir, &mut show, completion, "writer.revise")?;
    Ok((dir, show))
}

fn write_fountain(
    dir: &Path,
    show: &mut Show,
    completion: crate::brain::Completion,
    kind: &str,
) -> Result<(), ShowError> {
    let mut fountain = completion.text;
    if !fountain.ends_with('\n') {
        fountain.push('\n');
    }
    let path = dir.join(SCREENPLAY_FILE);
    fs::write(&path, &fountain)?;
    show.writer.draft_path = Some(path.display().to_string());
    show.writer.draft_provenance = Some(completion.provenance);
    bump(show);
    write_show(dir, show)?;
    append_event_with(
        dir,
        kind,
        show,
        Some(serde_json::json!({
            "backend": show.writer.draft_provenance.as_ref().map(|p| &p.backend),
            "model": show.writer.draft_provenance.as_ref().map(|p| &p.model),
            "auth": show.writer.draft_provenance.as_ref().map(|p| &p.auth),
        })),
    )?;
    Ok(())
}

pub(crate) fn write_show(dir: &Path, show: &Show) -> Result<(), ShowError> {
    let path = dir.join(SHOW_FILE);
    let tmp = dir.join(".show.json.tmp");
    let body = serde_json::to_string_pretty(show)?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub(crate) fn append_event(dir: &Path, kind: &str, show: &Show) -> Result<(), ShowError> {
    append_event_with(dir, kind, show, None)
}

pub(crate) fn append_event_with(
    dir: &Path,
    kind: &str,
    show: &Show,
    extra: Option<serde_json::Value>,
) -> Result<(), ShowError> {
    let path = dir.join("events.jsonl");
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let mut line = serde_json::json!({
        "at": now_rfc3339(),
        "kind": kind,
        "rev": show.rev,
        "show_id": show.id,
    });
    if let Some(serde_json::Value::Object(map)) = extra {
        if let Some(obj) = line.as_object_mut() {
            for (k, v) in map {
                obj.insert(k, v);
            }
        }
    }
    writeln!(f, "{line}")?;
    Ok(())
}

pub fn set_current_show(dir: &Path) -> Result<(), ShowError> {
    let home = lot_home()?;
    fs::create_dir_all(&home)?;
    let abs = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    fs::write(home.join("current"), abs.to_string_lossy().as_bytes())?;
    Ok(())
}

pub fn current_show_path() -> Result<Option<PathBuf>, ShowError> {
    if let Ok(p) = std::env::var("LOT_SHOW") {
        let pb = PathBuf::from(p);
        if pb.as_os_str().is_empty() {
            return Ok(None);
        }
        return Ok(Some(pb));
    }
    let marker = lot_home()?.join("current");
    if !marker.is_file() {
        return Ok(None);
    }
    let s = fs::read_to_string(marker)?;
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(s)))
}

fn lot_home() -> Result<PathBuf, ShowError> {
    if let Ok(p) = std::env::var("LOT_HOME") {
        return Ok(PathBuf::from(p));
    }
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| ShowError::Msg("no HOME/USERPROFILE".into()))?;
    Ok(PathBuf::from(base).join(".lot"))
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("show-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    static N: AtomicU64 = AtomicU64::new(0);
    static ENV: Mutex<()> = Mutex::new(());

    fn tmp() -> PathBuf {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("lot-test-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    fn isolate_home() {
        std::env::remove_var("LOT_SHOW");
        std::env::set_var("LOT_HOME", tmp().join("home"));
    }

    fn setup_show(name: &str) -> (PathBuf, Show) {
        isolate_home();
        create_show(&tmp(), Some(name)).unwrap()
    }

    fn isolate_brain() {
        let dead = tmp().join("no-hermes");
        let _ = fs::create_dir_all(&dead);
        std::env::set_var("HERMES_HOME", &dead);
        std::env::set_var("LOT_HERMES_HOME", &dead);
        std::env::remove_var("LOT_XAI_TOKEN");
        std::env::remove_var("XAI_API_KEY");
        std::env::set_var("LOT_LOCAL_BASE_URL", "http://127.0.0.1:9/v1");
        std::env::set_var("LOT_LOCAL_MODEL", "nope");
    }

    #[test]
    fn create_then_read() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate_home();
        let dir = tmp();
        let (path, show) = create_show(&dir, Some("Demo")).unwrap();
        assert_eq!(show.name, "Demo");
        assert_eq!(show.schema, 1);
        assert!(path.join(SHOW_FILE).is_file());
        assert!(path.join("media").is_dir());
        let again = read_show(&path).unwrap();
        assert_eq!(again.id, show.id);
        let cur = current_show_path().unwrap().unwrap();
        assert_eq!(cur, path);
    }

    #[test]
    fn refuse_second_create() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        isolate_home();
        let dir = tmp();
        create_show(&dir, Some("A")).unwrap();
        assert!(matches!(
            create_show(&dir, Some("B")),
            Err(ShowError::Exists(_))
        ));
    }

    #[test]
    fn unknown_genre_errors() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        setup_show("G");
        let err = set_style(Some(&["not-a-genre".into()]), None, None, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown genre"), "{err}");
    }

    #[test]
    fn style_and_cast_persist() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("Style");
        set_style(
            Some(&["drama".into(), "thriller".into()]),
            Some(&["greta-gerwig".into()]),
            Some(&["akira-kurosawa".into()]),
            Some("30min"),
        )
        .unwrap();
        upsert_cast(
            "Ada",
            Some("lead"),
            Some("bare face"),
            Some("franchise cameo"),
        )
        .unwrap();
        let show = read_show(&dir).unwrap();
        assert_eq!(show.writer.genres, vec!["drama", "thriller"]);
        assert_eq!(show.writer.styles_living, vec!["greta-gerwig"]);
        assert_eq!(show.writer.styles_canon, vec!["akira-kurosawa"]);
        assert_eq!(show.writer.format.as_deref(), Some("30min"));
        assert_eq!(show.writer.cast.len(), 1);
        assert_eq!(show.writer.cast[0].name, "Ada");
        assert_eq!(show.writer.cast[0].function, "lead");
        assert_eq!(show.writer.cast[0].look, "bare face");
        assert_eq!(show.writer.cast[0].must_not, "franchise cameo");
    }

    #[test]
    fn lock_blocks_then_unlock_restores() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        setup_show("Lock");
        set_brief("a clown at the gate").unwrap();
        lock_writer().unwrap();
        let brief_err = set_brief("nope").unwrap_err().to_string();
        assert!(brief_err.contains("locked"), "{brief_err}");
        let style_err = set_style(Some(&["drama".into()]), None, None, None)
            .unwrap_err()
            .to_string();
        assert!(style_err.contains("locked"), "{style_err}");
        let cast_err = upsert_cast("Ada", None, None, None)
            .unwrap_err()
            .to_string();
        assert!(cast_err.contains("locked"), "{cast_err}");
        let draft_err = draft_screenplay().unwrap_err().to_string();
        assert!(draft_err.contains("locked"), "{draft_err}");
        let revise_err = revise_screenplay("tighten").unwrap_err().to_string();
        assert!(revise_err.contains("locked"), "{revise_err}");
        unlock_writer().unwrap();
        set_brief("unlocked brief").unwrap();
        let show = require_current().unwrap().1;
        assert_eq!(show.writer.brief, "unlocked brief");
        assert!(!show.writer.locked);
    }

    #[test]
    fn revise_without_draft_errors() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        setup_show("Revise");
        set_brief("something happens").unwrap();
        let err = revise_screenplay("make it shorter")
            .unwrap_err()
            .to_string();
        assert!(err.contains("no draft"), "{err}");
    }

    #[test]
    fn empty_brief_refuses_draft() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        setup_show("Empty");
        let err = draft_screenplay().unwrap_err().to_string();
        assert!(err.contains("no brief"), "{err}");
    }

    #[test]
    fn no_brain_does_not_write_stub() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("Brain");
        isolate_brain();
        set_brief("a carnival drama").unwrap();
        let err = draft_screenplay().unwrap_err().to_string();
        assert!(err.contains("no brain —"), "{err}");
        assert!(!dir.join(SCREENPLAY_FILE).exists());
        std::env::remove_var("HERMES_HOME");
        std::env::remove_var("LOT_HERMES_HOME");
        std::env::remove_var("LOT_LOCAL_BASE_URL");
        std::env::remove_var("LOT_LOCAL_MODEL");
    }

    #[test]
    fn replace_cast_json_all() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("JsonCast");
        upsert_cast("Temp", None, None, None).unwrap();
        replace_cast_json(
            r#"[{"name":"Ada","function":"lead"},{"name":"Bo","function":"foil","look":"red coat","must_not":"gun"}]"#,
        )
        .unwrap();
        let show = read_show(&dir).unwrap();
        assert_eq!(show.writer.cast.len(), 2);
        assert_eq!(show.writer.cast[0].name, "Ada");
        assert_eq!(show.writer.cast[1].must_not, "gun");
    }

    #[test]
    fn breakdown_import_matches_golden_fixture() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("Carnival");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/carnival.txt");
        crate::breakdown_parse(Some(&fixture)).unwrap();
        let show = read_show(&dir).unwrap();
        assert_eq!(show.scenes.len(), 3, "AC-002 scene count");
        assert_eq!(show.shots.len(), 3);
        assert_eq!(show.shots[0].num, "01");
        assert_eq!(show.shots[0].name, show.scenes[0].slug);
        assert!(show.scenes[0].characters.iter().any(|c| c == "ADA"));
        assert_eq!(show.phase, "breakdown");
    }

    #[test]
    fn dailies_ingest_binds_prefix_keeps_shot_name() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("Ingest");
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/carnival.txt");
        crate::breakdown_parse(Some(&fixture)).unwrap();
        let before = read_show(&dir).unwrap().shots[0].name.clone();
        let clip = dir.join("media").join("01-foo.mp4");
        fs::write(&clip, b"fake-mp4-bytes").unwrap();
        crate::dailies_ingest(Some(&clip), None).unwrap();
        let show = read_show(&dir).unwrap();
        assert_eq!(show.takes.len(), 1);
        assert_eq!(show.shots[0].name, before);
        assert_ne!(show.shots[0].name, "01");
        assert_eq!(show.takes[0].shot_id, show.shots[0].id);
        crate::dailies_circle(&show.takes[0].id).unwrap();
        let (_, _, xml) = crate::dailies_export().unwrap();
        assert!(xml.is_file());
        let body = fs::read_to_string(&xml).unwrap();
        assert!(body.contains("fcpxml"));
    }

    #[test]
    fn advertisement_and_music_video_formats() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("Ad");
        set_style(None, None, None, Some("ad")).unwrap();
        assert_eq!(
            read_show(&dir).unwrap().writer.format.as_deref(),
            Some("advertisement")
        );
        set_style(None, None, None, Some("mv")).unwrap();
        assert_eq!(
            read_show(&dir).unwrap().writer.format.as_deref(),
            Some("music-video")
        );
    }

    #[test]
    fn soundtrack_cue_no_fake_audio() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, _) = setup_show("Score");
        isolate_brain();
        std::env::remove_var("LOT_SOUNDTRACK_CMD");
        crate::stems_soundtrack(Some("bright carnival organ, no lyrics"), None, false).unwrap();
        let show = read_show(&dir).unwrap();
        assert!(!show.stems.soundtrack_brief.is_empty());
        assert!(show.stems.soundtrack_path.is_none());
        let cue = dir.join("stems").join("soundtrack-cue.md");
        assert!(cue.is_file());
        let err = crate::stems_soundtrack(None, None, true)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no soundtrack engine"), "{err}");
        assert!(!dir.join("stems").join("soundtrack.wav").exists());
        std::env::remove_var("HERMES_HOME");
        std::env::remove_var("LOT_HERMES_HOME");
        std::env::remove_var("LOT_LOCAL_BASE_URL");
        std::env::remove_var("LOT_LOCAL_MODEL");
    }

    #[test]
    fn vo_needs_text() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        setup_show("VO");
        let err = crate::stems_vo(None, None, false).unwrap_err().to_string();
        assert!(err.contains("vo"), "{err}");
    }

    #[test]
    fn circle_without_take_errors() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        setup_show("NoTake");
        let err = crate::dailies_circle("").unwrap_err().to_string();
        assert!(err.contains("take") || err.contains("circle"), "{err}");
    }
}
