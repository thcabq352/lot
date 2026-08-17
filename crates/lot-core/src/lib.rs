use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "lot";
pub const SHOW_SCHEMA: u32 = 1;
pub const SHOW_FILE: &str = "show.json";

mod brain;
mod breakdown;
mod dailies;
mod doctor;
mod model;
mod packs;
mod parse;
mod show;

pub use brain::{
    complete_chat, draft_fountain, draft_user_prompt, revise_fountain, Completion, Provenance,
};
pub use breakdown::{breakdown_parse, breakdown_summary, picture_lock, slate_set, wall_add};
pub use dailies::{dailies_circle, dailies_export, dailies_ingest};
pub use doctor::Doctor;
pub use model::{Beat, MediaItem, Scene, Shot, Take};
pub use show::{
    create_show, current_show_path, draft_screenplay, lock_writer, open_show, read_show,
    replace_cast, replace_cast_json, require_current, revise_screenplay, set_brief,
    set_current_show, set_style, unlock_writer, upsert_cast, CastMember, Show, ShowError, Writer,
    SCREENPLAY_FILE,
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
    pub phase: Option<String>,
    pub scenes: Option<usize>,
    pub shots: Option<usize>,
    pub takes: Option<usize>,
    pub doctor: Doctor,
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
        let doctor = Doctor::probe();
        let renderer = doctor.renderer;
        match current_show_path() {
            Ok(Some(p)) => match read_show(&p) {
                Ok(show) => Self {
                    ok: true,
                    name: NAME,
                    version: VERSION,
                    door: "cli",
                    school: show.school.clone(),
                    renderer,
                    show: Some(p.display().to_string()),
                    show_name: Some(show.name),
                    rev: Some(show.rev),
                    phase: Some(show.phase.clone()),
                    scenes: Some(show.scenes.len()),
                    shots: Some(show.shots.len()),
                    takes: Some(show.takes.len()),
                    doctor,
                    error: None,
                },
                Err(e) => Self {
                    ok: false,
                    name: NAME,
                    version: VERSION,
                    door: "cli",
                    school: SchoolStatus::default(),
                    renderer,
                    show: Some(p.display().to_string()),
                    show_name: None,
                    rev: None,
                    phase: None,
                    scenes: None,
                    shots: None,
                    takes: None,
                    doctor,
                    error: Some(e.to_string()),
                },
            },
            Ok(None) => Self {
                ok: true,
                name: NAME,
                version: VERSION,
                door: "cli",
                school: SchoolStatus::default(),
                renderer,
                show: None,
                show_name: None,
                rev: None,
                phase: None,
                scenes: None,
                shots: None,
                takes: None,
                doctor,
                error: None,
            },
            Err(e) => Self {
                ok: false,
                name: NAME,
                version: VERSION,
                door: "cli",
                school: SchoolStatus::default(),
                renderer,
                show: None,
                show_name: None,
                rev: None,
                phase: None,
                scenes: None,
                shots: None,
                takes: None,
                doctor,
                error: Some(e.to_string()),
            },
        }
    }
}
