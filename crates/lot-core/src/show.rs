use crate::{SchoolStatus, SHOW_FILE, SHOW_SCHEMA};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    #[serde(default)]
    pub scenes: Vec<serde_json::Value>,
    #[serde(default)]
    pub shots: Vec<serde_json::Value>,
    #[serde(default)]
    pub takes: Vec<serde_json::Value>,
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
            scenes: Vec::new(),
            shots: Vec::new(),
            takes: Vec::new(),
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

fn write_show(dir: &Path, show: &Show) -> Result<(), ShowError> {
    let path = dir.join(SHOW_FILE);
    let tmp = dir.join(".show.json.tmp");
    let body = serde_json::to_string_pretty(show)?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn append_event(dir: &Path, kind: &str, show: &Show) -> Result<(), ShowError> {
    let path = dir.join("events.jsonl");
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let line = serde_json::json!({
        "at": now_rfc3339(),
        "kind": kind,
        "rev": show.rev,
        "show_id": show.id,
    });
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

    #[test]
    fn create_then_read() {
        let _g = ENV.lock().unwrap();
        let dir = tmp();
        let home = tmp().join("home");
        std::env::set_var("LOT_HOME", &home);
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
        let _g = ENV.lock().unwrap();
        let dir = tmp();
        std::env::set_var("LOT_HOME", tmp().join("home2"));
        create_show(&dir, Some("A")).unwrap();
        assert!(matches!(
            create_show(&dir, Some("B")),
            Err(ShowError::Exists(_))
        ));
    }
}
