use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "lot";
pub const SHOW_SCHEMA: u32 = 1;
pub const SHOW_FILE: &str = "show.json";

mod agent;
mod audit;
mod brain;
mod breakdown;
mod budget;
mod cancel;
mod caps;
mod dailies;
mod detail;
mod doctor;
mod finish;
mod handoff;
mod help;
mod import;
mod jail;
mod lock;
mod model;
mod motion;
mod packs;
mod parse;
mod resource;
mod show;
mod slate;
mod snapshot;
mod stage;
mod stems;
mod stills;
mod undo;

pub use agent::{clear_agent, current as current_agent, set_agent, with_agent};
pub use audit::{export_log, last_event, mutation_json, show_log, EventMeta};
pub use brain::{
    complete_chat, complete_vision, draft_fountain, draft_user_prompt, hash_prompt, probe_ollama,
    revise_fountain, Completion, OllamaProbe, Provenance,
};
pub use breakdown::{breakdown_parse, breakdown_summary, picture_lock, wall_add};
pub use budget::{set_budget, Budget};
pub use cancel::{
    begin_request, check as check_cancel, clear as clear_cancel, end_request, from_notification,
    is_cancelled, request_cancel, run_interruptible, CANCELLED_MSG,
};
pub use caps::{active as active_caps, clear_caps, parse_caps, set_caps, with_caps, Cap, Caps};
pub use dailies::{dailies_circle, dailies_export, dailies_ingest, IngestReport};
pub use detail::{
    clear_detail, detail_full, detail_full_active, detail_full_value, lean_extra, set_detail_full,
    with_detail,
};
pub use doctor::Doctor;
pub use finish::finish_pickup;
pub use handoff::{
    dirty_sections, handoff, inspect as inspect_handoff, missing_media, phase_missing, Handoff,
    PHASES as HANDOFF_PHASES,
};
pub use help::{help_plain, help_spec};
pub use import::{import_file, ImportReport};
pub use lock::{lock_show, unlock_show};
pub use model::{
    Beat, FinishState, MediaItem, Scene, Shot, SlateLora, SlateState, StageMark, Take,
};
pub use motion::{motion_analyze, motion_export, motion_marks, motion_plate};
pub use resource::{resource_list, resource_read, ResourceRef};
pub use show::{
    create_show, current_show_path, draft_screenplay, lock_writer, open_show, read_show,
    replace_cast, replace_cast_json, require_current, revise_screenplay, set_brief,
    set_current_show, set_style, unlock_writer, upsert_cast, CastMember, Show, ShowError, Writer,
    SCREENPLAY_FILE,
};
pub use slate::{slate_compile, slate_lora, slate_set, slate_target};
pub use snapshot::{restore_show, snapshot_list, snapshot_show};
pub use stage::{stage_camera, stage_export, stage_place};
pub use stems::{stems_soundtrack, stems_vo, Stems};
pub use stills::{
    board_export, comfy_workflow_ready, resolve_comfy_workflow, stills_describe, stills_generate,
};
pub use undo::undo_show;

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
    pub cap: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<EventMeta>,
    /// Sections that already have work (so the first call does not need `lot handoff`).
    pub dirty: Vec<String>,
    /// Current-phase handoff blockers (same strings as `lot handoff`).
    pub missing: Vec<String>,
    /// Referenced stills / plates / takes / stems / finish that are not on disk.
    pub missing_media: Vec<MediaGap>,
    pub doctor: Doctor,
    pub error: Option<String>,
}

/// A show path that `show.json` names but the file is gone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaGap {
    pub kind: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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
        let cap = crate::caps::active().names();
        match current_show_path() {
            Ok(Some(p)) => match read_show(&p) {
                Ok(show) => {
                    let dirty = crate::handoff::dirty_sections(&p, &show);
                    let missing = crate::handoff::phase_missing(&p, &show);
                    let missing_media = crate::handoff::missing_media(&p, &show);
                    Self {
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
                        cap: cap.clone(),
                        locked_by: show.locked_by.clone(),
                        agent: crate::agent::current(),
                        budget: Some(show.budget.clone()),
                        last_event: crate::audit::last_event(&p),
                        dirty,
                        missing,
                        missing_media,
                        doctor,
                        error: None,
                    }
                }
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
                    cap: cap.clone(),
                    locked_by: None,
                    agent: crate::agent::current(),
                    budget: None,
                    last_event: None,
                    dirty: Vec::new(),
                    missing: Vec::new(),
                    missing_media: Vec::new(),
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
                cap,
                locked_by: None,
                agent: crate::agent::current(),
                budget: None,
                last_event: None,
                dirty: Vec::new(),
                missing: Vec::new(),
                missing_media: Vec::new(),
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
                cap,
                locked_by: None,
                agent: crate::agent::current(),
                budget: None,
                last_event: None,
                dirty: Vec::new(),
                missing: Vec::new(),
                missing_media: Vec::new(),
                doctor,
                error: Some(e.to_string()),
            },
        }
    }
}
