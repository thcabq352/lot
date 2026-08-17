use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "lot";
pub const SHOW_SCHEMA: u32 = 1;
pub const SHOW_FILE: &str = "show.json";

mod show;

pub use show::{
    create_show, current_show_path, open_show, read_show, set_current_show, Show, ShowError,
};

/// Kernel status — CLI `--json` and MCP `lot_status` share this shape.
#[derive(Debug, Serialize)]
pub struct Status {
    pub ok: bool,
    pub name: &'static str,
    pub version: &'static str,
    pub door: &'static str,
    pub school: SchoolStatus,
    pub renderer: &'static str,
    pub show: Option<String>,
    pub show_name: Option<String>,
    pub rev: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchoolStatus {
    pub enabled: bool,
}

impl Default for SchoolStatus {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl Status {
    pub fn bootstrap() -> Self {
        match current_show_path() {
            Ok(Some(p)) => match read_show(&p) {
                Ok(show) => Self {
                    ok: true,
                    name: NAME,
                    version: VERSION,
                    door: "cli",
                    school: show.school.clone(),
                    renderer: "unavailable",
                    show: Some(p.display().to_string()),
                    show_name: Some(show.name),
                    rev: Some(show.rev),
                    error: None,
                },
                Err(e) => Self {
                    ok: false,
                    name: NAME,
                    version: VERSION,
                    door: "cli",
                    school: SchoolStatus::default(),
                    renderer: "unavailable",
                    show: Some(p.display().to_string()),
                    show_name: None,
                    rev: None,
                    error: Some(e.to_string()),
                },
            },
            Ok(None) => Self {
                ok: true,
                name: NAME,
                version: VERSION,
                door: "cli",
                school: SchoolStatus::default(),
                renderer: "unavailable",
                show: None,
                show_name: None,
                rev: None,
                error: None,
            },
            Err(e) => Self {
                ok: false,
                name: NAME,
                version: VERSION,
                door: "cli",
                school: SchoolStatus::default(),
                renderer: "unavailable",
                show: None,
                show_name: None,
                rev: None,
                error: Some(e.to_string()),
            },
        }
    }
}
