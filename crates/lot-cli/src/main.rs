use clap::{Parser, Subcommand};
use lot_core::{
    board_export, breakdown_parse, breakdown_summary, create_show, dailies_circle, dailies_export,
    dailies_ingest, draft_screenplay, export_log, finish_pickup, handoff, help_plain, help_spec,
    import_file, lock_show, lock_writer, motion_analyze, motion_export, motion_marks, motion_plate,
    mutation_json, open_show, picture_lock, replace_cast_json, resource_read, restore_show,
    revise_screenplay, set_brief, set_budget, set_style, show_log, slate_compile, slate_lora,
    slate_set, slate_target, snapshot_list, snapshot_show, stage_camera, stage_export, stage_place,
    stems_soundtrack, stems_vo, stills_describe, stills_generate, undo_show, unlock_show,
    unlock_writer, upsert_cast, wall_add, Doctor, Status,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "lot",
    version,
    about = "Lot — agent-first film tools. Stdio first, GUI last.",
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(long, global = true, default_value_t = false)]
    json: bool,
    /// Open this show.lot, then run. Omit to keep the current pointer.
    #[arg(long, global = true)]
    show: Option<PathBuf>,
    /// Agent caps: read | write | render | export | spend | all. Repeat or comma-separate. Unset = all.
    #[arg(long = "cap", global = true)]
    cap: Vec<String>,
    /// Who is writing. Unset = human (no auto-claim). Second agent gets locked_by.
    #[arg(long, global = true)]
    agent: Option<String>,
    /// Dump full shot/prompt cards on --json. Default is lean. Use `--detail full`.
    #[arg(long, global = true)]
    detail: Option<String>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Kernel probe. First call for any agent.
    Status,
    /// Create a show.lot directory and make it current.
    Create {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Open an existing show.lot and make it current.
    Open { path: PathBuf },
    /// Read lot://show (meta, phase, lock). Not the fountain.
    Show,
    /// Read lot://scenes/{id}.
    Scene {
        #[arg(long)]
        id: String,
    },
    /// Read lot://shots/{id} (or --num).
    Shot {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        num: Option<String>,
    },
    /// Read lot://takes/{id}.
    Take {
        #[arg(long)]
        id: String,
    },
    /// Import an old-suite file. Does not delete the source.
    Import {
        #[arg(long)]
        file: PathBuf,
    },
    /// Writer: brief, style, cast, draft, revise, lock.
    Writer {
        #[command(subcommand)]
        cmd: WriterCmd,
    },
    /// Breakdown (ScriptBreak logic): import + parse. Agents can write.
    Breakdown {
        #[command(subcommand)]
        cmd: BreakdownCmd,
    },
    /// Wall (Cork Board): beat cards.
    Wall {
        #[command(subcommand)]
        cmd: WallCmd,
    },
    /// Picture (Master Canvas): lock a shot card.
    Picture {
        #[command(subcommand)]
        cmd: PictureCmd,
    },
    /// Stage: 2D floor marks + camera card. 3D stays in Blockout.
    Stage {
        #[command(subcommand)]
        cmd: StageCmd,
    },
    /// Stills: Grok Imagine or local Comfy. --backend required. No silent swap.
    Stills {
        #[command(subcommand)]
        cmd: StillsCmd,
    },
    /// Board: export stills + slate prompts (one tool toward Slate).
    Board {
        #[command(subcommand)]
        cmd: BoardCmd,
    },
    /// Slate: continuity-locked prompts on shots.
    Slate {
        #[command(subcommand)]
        cmd: SlateCmd,
    },
    /// Motion Previs: plates → marks. Pose/depth stay in Motion Previs Studio.
    Motion {
        #[command(subcommand)]
        cmd: MotionCmd,
    },
    /// Optional end-of-pipeline upscale + FPS pickup. Never a stub.
    Finish {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        upscale: bool,
        #[arg(long)]
        fps: Option<String>,
    },
    /// Dailies (Circle Take): ingest, circle, FCPXML.
    Dailies {
        #[command(subcommand)]
        cmd: DailiesCmd,
    },
    /// Stems: soundtrack + VO (generate or attach). Not a new movie stage.
    Stems {
        #[command(subcommand)]
        cmd: StemsCmd,
    },
    /// Cut: interchange export (FCPXML). Resolve live is an adapter later.
    Cut {
        #[command(subcommand)]
        cmd: CutCmd,
    },
    /// Freeze show.json + fountain at the current rev.
    Snapshot {
        #[arg(long, default_value_t = false)]
        list: bool,
    },
    /// Restore a snapshot. Later drafts do not eat earlier ones.
    Restore {
        #[arg(long)]
        rev: u64,
    },
    /// Undo the last mutation from the event log. No prior snapshot required.
    Undo,
    /// Claim the show. Second agent gets locked_by, not a silent clobber.
    Lock,
    /// Release the show lock. Holder or --force.
    Unlock {
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Per-show spend / render budget. Hit cap → stop. Unset = unlimited.
    Budget {
        #[arg(long)]
        spend: Option<u32>,
        #[arg(long)]
        render: Option<u32>,
        #[arg(long = "clear-spend", default_value_t = false)]
        clear_spend: bool,
        #[arg(long = "clear-render", default_value_t = false)]
        clear_render: bool,
    },
    /// Audit log: who / what / rev. --export redacts tokens.
    Log {
        #[arg(long, default_value_t = 20)]
        n: u32,
        #[arg(long, default_value_t = false)]
        export: bool,
    },
    /// Advance phase. Default is dry-run. --commit writes only when the gate passes.
    Handoff {
        #[arg(long, default_value_t = false)]
        commit: bool,
    },
    /// Machine-readable spec. lot help --json is the contract.
    Help,
    /// Runtime probes (ffmpeg / Comfy / brains). No GUI.
    Doctor,
    /// Native agent door (stdio MCP).
    Mcp,
}

#[derive(Subcommand)]
enum WriterCmd {
    /// Set the brief on the current show.
    Brief {
        #[arg(long)]
        text: String,
    },
    /// Set genre / living / canon influence / format. IDs from dated JSON packs.
    Style {
        #[arg(long = "genre")]
        genre: Vec<String>,
        #[arg(long = "living")]
        living: Vec<String>,
        #[arg(long = "canon")]
        canon: Vec<String>,
        #[arg(long)]
        format: Option<String>,
    },
    /// Add/update one character, or replace the whole cast with --from-json.
    Cast {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        function: Option<String>,
        #[arg(long)]
        look: Option<String>,
        #[arg(long = "must-not")]
        must_not: Option<String>,
        /// Replace-all cast JSON array. Not --json (that flag is global output).
        #[arg(long = "from-json")]
        from_json: Option<String>,
    },
    /// Write screenplay.fountain via Grok (xAI OAuth) or Ollama / local.
    Draft,
    /// Revise the existing screenplay.fountain. Fails if no draft.
    Revise {
        #[arg(long)]
        notes: String,
    },
    /// Lock brief/style/cast/draft/revise.
    Lock,
    /// Unlock the writer.
    Unlock,
}

#[derive(Subcommand)]
enum BreakdownCmd {
    /// Parse screenplay.fountain on the current show.
    Parse,
    /// Import a .txt / .fountain / .scriptbreak (does not delete the source).
    Import {
        #[arg(long)]
        file: PathBuf,
    },
    /// Scene / character / location counts.
    Status,
}

#[derive(Subcommand)]
enum WallCmd {
    Add {
        #[arg(long)]
        text: String,
        #[arg(long)]
        act: Option<String>,
    },
}

#[derive(Subcommand)]
enum PictureCmd {
    Lock {
        #[arg(long)]
        shot: String,
    },
    Unlock {
        #[arg(long)]
        shot: String,
    },
}

#[derive(Subcommand)]
enum StageCmd {
    /// Place a 2D floor mark. Does not rename the shot.
    Place {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        who: String,
        #[arg(long)]
        mark: Option<String>,
        #[arg(long)]
        x: Option<String>,
        #[arg(long)]
        z: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Set the camera card on a shot.
    Camera {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        size: Option<String>,
        #[arg(long)]
        angle: Option<String>,
        #[arg(long)]
        lens: Option<String>,
        #[arg(long = "move")]
        move_kind: Option<String>,
    },
    /// Write stage/block.json + prompt.md. Never a fake glTF.
    Export,
}

#[derive(Subcommand)]
enum StillsCmd {
    /// Generate one still. Backend is grok or comfy — never swapped.
    Generate {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        backend: String,
        #[arg(long)]
        prompt: Option<String>,
    },
    /// Look at a still / plate / --file. Grok vision or Ollama VL. No invented look.
    Describe {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum BoardCmd {
    /// Write board/board.json + board.md from shots / stills / prompts.
    Export,
}

#[derive(Subcommand)]
enum SlateCmd {
    /// Set the Slate canon (or a per-target rewrite with --target).
    Set {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        target: Option<String>,
    },
    /// Compile the canon into a target dialect (kling, veo, ltx-2.5, prompt-server, …).
    Compile {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        target: Option<String>,
    },
    /// Attach a LoRA to a shot or the show.
    Lora {
        #[arg(long)]
        shot: Option<String>,
        #[arg(long)]
        id: String,
        #[arg(long)]
        weight: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    /// Default compile target for the show.
    Target {
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
enum MotionCmd {
    /// Attach a reference plate to a shot. Does not rename the shot.
    Plate {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        shot: String,
        #[arg(long)]
        mode: Option<String>,
    },
    /// Store camera / performance marks (no MediaPipe).
    Marks {
        #[arg(long)]
        shot: String,
        #[arg(long = "move")]
        move_kind: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        mode: Option<String>,
    },
    /// Write motion/previs.json + prompt.md. Never a fake OpenPose bundle.
    Export,
    /// Probe the plate (ffprobe / LOT_MOTION_CMD). Studio pose stays in Motion Previs.
    Analyze {
        #[arg(long)]
        shot: String,
    },
}

#[derive(Subcommand)]
enum DailiesCmd {
    Ingest {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    Circle {
        #[arg(long)]
        take: String,
    },
    Export,
}

#[derive(Subcommand)]
enum CutCmd {
    Export,
}

#[derive(Subcommand)]
enum StemsCmd {
    /// Write a soundtrack cue (and optional audio). Never a fake track.
    Soundtrack {
        #[arg(long)]
        brief: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        generate: bool,
    },
    /// Voiceover: set text, attach a file, or --generate via local TTS.
    Vo {
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        generate: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if !cli.cap.is_empty() {
        match lot_core::parse_caps(&cli.cap.join(",")) {
            Ok(c) => lot_core::set_caps(c),
            Err(e) => return fail(cli.json, &e.to_string()),
        }
    }
    if let Some(id) = cli.agent {
        lot_core::set_agent(Some(id));
    }
    lot_core::set_detail_full(lot_core::detail_full(cli.detail.as_deref()));
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Status => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            print_status(cli.json)
        }
        Cmd::Create { path, name } => match create_show(&path, name.as_deref()) {
            Ok((dir, show)) => ok_writer(
                cli.json,
                &dir,
                &show,
                serde_json::json!({ "id": show.id, "name": show.name }),
                &format!("created {} ({})", dir.display(), show.name),
            ),
            Err(e) => fail(cli.json, &e.to_string()),
        },
        Cmd::Open { path } => match open_show(&path) {
            Ok((dir, show)) => ok_writer(
                cli.json,
                &dir,
                &show,
                serde_json::json!({ "id": show.id, "name": show.name }),
                &format!("opened {} ({})", dir.display(), show.name),
            ),
            Err(e) => fail(cli.json, &e.to_string()),
        },
        Cmd::Show => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            card_cmd("lot://show", cli.json)
        }
        Cmd::Scene { id } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            card_cmd(&format!("lot://scenes/{id}"), cli.json)
        }
        Cmd::Shot { id, num } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            let key = id.or(num).unwrap_or_default();
            if key.is_empty() {
                return fail(cli.json, "shot needs --id or --num");
            }
            card_cmd(&format!("lot://shots/{key}"), cli.json)
        }
        Cmd::Take { id } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            card_cmd(&format!("lot://takes/{id}"), cli.json)
        }
        Cmd::Import { file } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match import_file(&file) {
                Ok((dir, show, report)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({
                        "kind": report.kind,
                        "source": report.source,
                        "kept": report.kept,
                        "added": report.added,
                    }),
                    &format!("imported {} ({})", report.kind, report.source),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Writer { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            writer_cmd(cmd, cli.json)
        }
        Cmd::Breakdown { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            breakdown_cmd(cmd, cli.json)
        }
        Cmd::Wall { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                WallCmd::Add { text, act } => match wall_add(act.as_deref(), &text) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "wall": show.wall }),
                        &format!("wall beat added ({})", dir.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Stills { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                StillsCmd::Generate {
                    shot,
                    backend,
                    prompt,
                } => match stills_generate(&shot, &backend, prompt.as_deref()) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({
                            "stills_backend": show.stills_backend,
                            "shots": show.shots.iter().map(|s| {
                                serde_json::json!({
                                    "num": s.num,
                                    "name": s.name,
                                    "prompt": s.prompt,
                                    "still": s.still_path,
                                    "backend": s.still_backend,
                                    "provenance": s.still_provenance,
                                })
                            }).collect::<Vec<_>>()
                        }),
                        "still generated",
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                StillsCmd::Describe { shot, file } => {
                    match stills_describe(&shot, file.as_deref()) {
                        Ok((dir, show)) => ok_writer(
                            cli.json,
                            &dir,
                            &show,
                            serde_json::json!({
                                "shots": show.shots.iter().map(|s| {
                                    serde_json::json!({
                                        "num": s.num,
                                        "name": s.name,
                                        "desc": s.desc,
                                        "still": s.still_path,
                                    })
                                }).collect::<Vec<_>>()
                            }),
                            "still described",
                        ),
                        Err(e) => fail(cli.json, &e.to_string()),
                    }
                }
            }
        }
        Cmd::Board { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                BoardCmd::Export => match board_export() {
                    Ok((dir, show, file)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "export": file.display().to_string() }),
                        &format!("board export {}", file.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Picture { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            let (shot, locked) = match cmd {
                PictureCmd::Lock { shot } => (shot, true),
                PictureCmd::Unlock { shot } => (shot, false),
            };
            match picture_lock(&shot, locked) {
                Ok((dir, show)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({ "shots": show.shots }),
                    &format!("picture shot {shot} locked={locked}"),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Stage { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                StageCmd::Place {
                    shot,
                    who,
                    mark,
                    x,
                    z,
                    notes,
                    kind,
                } => match stage_place(
                    &shot,
                    &who,
                    mark.as_deref(),
                    x.as_deref(),
                    z.as_deref(),
                    notes.as_deref(),
                    kind.as_deref(),
                ) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots }),
                        &format!("stage place {who} on shot {shot}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                StageCmd::Camera {
                    shot,
                    size,
                    angle,
                    lens,
                    move_kind,
                } => match stage_camera(
                    &shot,
                    size.as_deref(),
                    angle.as_deref(),
                    lens.as_deref(),
                    move_kind.as_deref(),
                ) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots }),
                        &format!("stage camera on shot {shot}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                StageCmd::Export => match stage_export() {
                    Ok((dir, show, file)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "export": file.display().to_string() }),
                        &format!("stage export {}", file.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Slate { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                SlateCmd::Set {
                    shot,
                    prompt,
                    target,
                } => match slate_set(&shot, &prompt, target.as_deref()) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots, "slate": show.slate }),
                        &format!("slate set on shot {shot}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                SlateCmd::Compile { shot, target } => {
                    match slate_compile(&shot, target.as_deref()) {
                        Ok((dir, show)) => ok_writer(
                            cli.json,
                            &dir,
                            &show,
                            serde_json::json!({ "shots": show.shots, "slate": show.slate }),
                            &format!("slate compiled shot {shot}"),
                        ),
                        Err(e) => fail(cli.json, &e.to_string()),
                    }
                }
                SlateCmd::Lora {
                    shot,
                    id,
                    weight,
                    model,
                } => match slate_lora(shot.as_deref(), &id, weight.as_deref(), model.as_deref()) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots, "slate": show.slate }),
                        "slate lora set",
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                SlateCmd::Target { id } => match slate_target(&id) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "slate": show.slate }),
                        &format!("slate default target {id}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Motion { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                MotionCmd::Plate { file, shot, mode } => {
                    match motion_plate(&file, &shot, mode.as_deref()) {
                        Ok((dir, show)) => ok_writer(
                            cli.json,
                            &dir,
                            &show,
                            serde_json::json!({ "shots": show.shots }),
                            &format!("motion plate on shot {shot}"),
                        ),
                        Err(e) => fail(cli.json, &e.to_string()),
                    }
                }
                MotionCmd::Marks {
                    shot,
                    move_kind,
                    notes,
                    mode,
                } => match motion_marks(
                    &shot,
                    move_kind.as_deref(),
                    notes.as_deref(),
                    mode.as_deref(),
                ) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots }),
                        &format!("motion marks on shot {shot}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                MotionCmd::Export => match motion_export() {
                    Ok((dir, show, file)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "export": file.display().to_string() }),
                        &format!("motion export {}", file.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
                MotionCmd::Analyze { shot } => match motion_analyze(&shot) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots }),
                        &format!("motion analyze shot {shot}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Finish { file, upscale, fps } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match finish_pickup(file.as_deref(), upscale, fps.as_deref()) {
                Ok((dir, show, out)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({
                        "finish": show.finish,
                        "file": out.display().to_string(),
                    }),
                    &format!("finish {}", out.display()),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Dailies { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            dailies_cmd(cmd, cli.json)
        }
        Cmd::Stems { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            stems_cmd(cmd, cli.json)
        }
        Cmd::Cut { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                CutCmd::Export => match dailies_export() {
                    Ok((dir, show, report)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({
                            "export": report.file.display().to_string(),
                            "edl": report.edl.display().to_string(),
                            "takes": report.takes,
                            "resumed": report.resumed
                        }),
                        &format!("cut export {}", report.file.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Snapshot { list } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            if list {
                match snapshot_list() {
                    Ok((dir, show, revs)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "revs": revs }),
                        &format!(
                            "snapshots: {}",
                            if revs.is_empty() {
                                "none".into()
                            } else {
                                revs.iter()
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                }
            } else {
                match snapshot_show() {
                    Ok((dir, show, dest, rev)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({
                            "rev": rev,
                            "snapshot": dest.display().to_string(),
                        }),
                        &format!("snapshot rev-{rev} {}", dest.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                }
            }
        }
        Cmd::Undo => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match undo_show() {
                Ok((dir, show, undid)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({
                        "undid": undid,
                        "brief": show.writer.brief,
                    }),
                    &format!("undid {undid} → now rev {}", show.rev),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Restore { rev } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match restore_show(rev) {
                Ok((dir, show)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({
                        "rev": show.rev,
                        "from_rev": rev,
                        "brief": show.writer.brief,
                    }),
                    &format!("restored rev-{rev} → now rev {}", show.rev),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Lock => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match lock_show() {
                Ok((dir, show)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({ "locked_by": show.locked_by }),
                    &format!("locked by {}", show.locked_by.as_deref().unwrap_or("human")),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Unlock { force } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match unlock_show(force) {
                Ok((dir, show)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({ "locked_by": show.locked_by }),
                    "unlocked",
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Budget {
            spend,
            render,
            clear_spend,
            clear_render,
        } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match set_budget(spend, render, clear_spend, clear_render) {
                Ok((dir, show)) => ok_writer(
                    cli.json,
                    &dir,
                    &show,
                    serde_json::json!({ "budget": show.budget }),
                    &format!(
                        "budget spend={}/{} render={}/{}",
                        show.budget.spend_used,
                        show.budget
                            .spend_cap
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "∞".into()),
                        show.budget.render_used,
                        show.budget
                            .render_cap
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "∞".into()),
                    ),
                ),
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Log { n, export } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            if export {
                match export_log() {
                    Ok((dir, show, dest, count)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({
                            "export": dest.display().to_string(),
                            "events": count,
                        }),
                        &format!("audit export {} ({count} events)", dest.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                }
            } else {
                match show_log(Some(n)) {
                    Ok((dir, show, events)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "events": events, "n": events.len() }),
                        &format!("log {} events ({})", events.len(), dir.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                }
            }
        }
        Cmd::Handoff { commit } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match handoff(commit) {
                Ok((dir, show, report)) => {
                    let extra = serde_json::to_value(&report).unwrap_or(serde_json::json!({}));
                    let plain = if report.committed {
                        format!("handoff {} → {}", report.from, report.phase)
                    } else if report.ready {
                        format!(
                            "handoff ready {} → {}",
                            report.from,
                            report.next.as_deref().unwrap_or("-")
                        )
                    } else {
                        format!(
                            "handoff blocked {} — {}",
                            report.from,
                            report.missing.join("; ")
                        )
                    };
                    ok_writer(cli.json, &dir, &show, extra, &plain)
                }
                Err(e) => fail(cli.json, &e.to_string()),
            }
        }
        Cmd::Help => {
            if cli.json {
                println!("{}", help_spec());
            } else {
                print!("{}", help_plain());
            }
            ExitCode::SUCCESS
        }
        Cmd::Doctor => {
            let d = Doctor::probe();
            if cli.json {
                println!("{}", serde_json::to_string(&d).expect("doctor json"));
            } else {
                println!(
                    "ffmpeg={} ffprobe={} comfy={} grok={} local={} ollama={} ollama_llm={} ollama_vision={} vo_tts={} soundtrack_cmd={} prompt_server={} motion_previs={} renderer={}",
                    d.ffmpeg,
                    d.ffprobe,
                    d.comfy,
                    d.grok_configured,
                    d.local_configured,
                    d.ollama,
                    d.ollama_llm.as_deref().unwrap_or("-"),
                    d.ollama_vision.as_deref().unwrap_or("-"),
                    d.vo_tts,
                    d.soundtrack_cmd,
                    d.prompt_server,
                    d.motion_previs,
                    d.renderer
                );
            }
            ExitCode::SUCCESS
        }
        Cmd::Mcp => match lot_mcp::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("lot mcp: {e}");
                ExitCode::from(1)
            }
        },
    }
}

fn breakdown_cmd(cmd: BreakdownCmd, json: bool) -> ExitCode {
    match cmd {
        BreakdownCmd::Parse => match breakdown_parse(None) {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                breakdown_summary(&show),
                &format!("breakdown {} scenes ({})", show.scenes.len(), dir.display()),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        BreakdownCmd::Import { file } => match breakdown_parse(Some(&file)) {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                breakdown_summary(&show),
                &format!("imported {} scenes ({})", show.scenes.len(), dir.display()),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        BreakdownCmd::Status => match lot_core::require_current() {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                breakdown_summary(&show),
                &format!(
                    "breakdown scenes={} shots={} ({})",
                    show.scenes.len(),
                    show.shots.len(),
                    dir.display()
                ),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
    }
}

fn dailies_cmd(cmd: DailiesCmd, json: bool) -> ExitCode {
    match cmd {
        DailiesCmd::Ingest { file, dir } => match dailies_ingest(file.as_deref(), dir.as_deref()) {
            Ok((show_dir, show, report)) => ok_writer(
                json,
                &show_dir,
                &show,
                serde_json::json!({
                    "ingested": report.ingested,
                    "resumed": report.resumed,
                    "takes": show.takes,
                    "shots": show.shots.iter().map(|s| {
                        serde_json::json!({ "num": s.num, "name": s.name })
                    }).collect::<Vec<_>>()
                }),
                &format!(
                    "ingested {} takes (resumed {})",
                    report.ingested, report.resumed
                ),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        DailiesCmd::Circle { take } => match dailies_circle(&take) {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({ "takes": show.takes }),
                &format!("circled {take}"),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        DailiesCmd::Export => match dailies_export() {
            Ok((dir, show, report)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({
                    "export": report.file.display().to_string(),
                    "edl": report.edl.display().to_string(),
                    "takes": report.takes,
                    "resumed": report.resumed
                }),
                &format!("exported {}", report.file.display()),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
    }
}

fn stems_cmd(cmd: StemsCmd, json: bool) -> ExitCode {
    match cmd {
        StemsCmd::Soundtrack {
            brief,
            file,
            generate,
        } => match stems_soundtrack(brief.as_deref(), file.as_deref(), generate) {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({
                    "stems": show.stems,
                }),
                "soundtrack cue written",
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        StemsCmd::Vo {
            text,
            file,
            generate,
        } => match stems_vo(text.as_deref(), file.as_deref(), generate) {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({ "stems": show.stems }),
                "vo set",
            ),
            Err(e) => fail(json, &e.to_string()),
        },
    }
}

fn apply_show(show: Option<&Path>, json: bool) -> Option<ExitCode> {
    if let Some(p) = show {
        if let Err(e) = open_show(p) {
            return Some(fail(json, &e.to_string()));
        }
    }
    None
}

fn writer_cmd(cmd: WriterCmd, json: bool) -> ExitCode {
    match cmd {
        WriterCmd::Brief { text } => match set_brief(&text) {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({ "brief": show.writer.brief }),
                &format!("brief set ({})", dir.display()),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        WriterCmd::Style {
            genre,
            living,
            canon,
            format,
        } => {
            let g = if genre.is_empty() { None } else { Some(genre) };
            let l = if living.is_empty() {
                None
            } else {
                Some(living)
            };
            let c = if canon.is_empty() { None } else { Some(canon) };
            match set_style(g.as_deref(), l.as_deref(), c.as_deref(), format.as_deref()) {
                Ok((dir, show)) => ok_writer(
                    json,
                    &dir,
                    &show,
                    serde_json::json!({
                        "genres": show.writer.genres,
                        "styles_living": show.writer.styles_living,
                        "styles_canon": show.writer.styles_canon,
                        "format": show.writer.format,
                    }),
                    &format!("style set ({})", dir.display()),
                ),
                Err(e) => fail(json, &e.to_string()),
            }
        }
        WriterCmd::Cast {
            name,
            function,
            look,
            must_not,
            from_json,
        } => {
            if from_json.is_some() && name.is_some() {
                return fail(json, "cast: use --name or --from-json, not both");
            }
            let result = if let Some(raw) = from_json {
                replace_cast_json(&raw)
            } else if let Some(n) = name {
                upsert_cast(
                    &n,
                    function.as_deref(),
                    look.as_deref(),
                    must_not.as_deref(),
                )
            } else {
                return fail(json, "cast needs --name or --from-json");
            };
            match result {
                Ok((dir, show)) => ok_writer(
                    json,
                    &dir,
                    &show,
                    serde_json::json!({ "cast": show.writer.cast }),
                    &format!("cast set ({})", dir.display()),
                ),
                Err(e) => fail(json, &e.to_string()),
            }
        }
        WriterCmd::Draft => match draft_screenplay() {
            Ok((dir, show)) => draft_ok(json, &dir, &show),
            Err(e) => fail(json, &e.to_string()),
        },
        WriterCmd::Revise { notes } => match revise_screenplay(&notes) {
            Ok((dir, show)) => draft_ok(json, &dir, &show),
            Err(e) => fail(json, &e.to_string()),
        },
        WriterCmd::Lock => match lock_writer() {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({ "locked": show.writer.locked }),
                &format!("writer locked ({})", dir.display()),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
        WriterCmd::Unlock => match unlock_writer() {
            Ok((dir, show)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({ "locked": show.writer.locked }),
                &format!("writer unlocked ({})", dir.display()),
            ),
            Err(e) => fail(json, &e.to_string()),
        },
    }
}

fn ok_writer(
    json: bool,
    dir: &Path,
    show: &lot_core::Show,
    extra: serde_json::Value,
    plain: &str,
) -> ExitCode {
    if json {
        println!("{}", mutation_json(dir, show, extra));
    } else {
        println!("{plain}");
    }
    ExitCode::SUCCESS
}

fn draft_ok(json: bool, dir: &Path, show: &lot_core::Show) -> ExitCode {
    if json {
        println!(
            "{}",
            mutation_json(
                dir,
                show,
                serde_json::json!({
                    "draft": show.writer.draft_path,
                    "provenance": show.writer.draft_provenance,
                }),
            )
        );
    } else {
        let path = show
            .writer
            .draft_path
            .clone()
            .unwrap_or_else(|| dir.display().to_string());
        if let Some(p) = &show.writer.draft_provenance {
            println!(
                "draft {}  backend={} model={} auth={}",
                path, p.backend, p.model, p.auth
            );
        } else {
            println!("draft {path}");
        }
    }
    ExitCode::SUCCESS
}

fn card_cmd(uri: &str, json: bool) -> ExitCode {
    match resource_read(uri) {
        Ok((dir, show, card)) => ok_writer(
            json,
            &dir,
            &show,
            serde_json::json!({ "uri": uri, "resource": card }),
            &format!("{uri} ({})", dir.display()),
        ),
        Err(e) => fail(json, &e.to_string()),
    }
}

fn print_status(json: bool) -> ExitCode {
    let mut st = Status::bootstrap();
    st.door = "cli";
    if json {
        println!("{}", serde_json::to_string(&st).expect("status json"));
    } else {
        println!(
            "lot {v}  school={s}  renderer={r}  cap={cap}  agent={agent}  locked_by={lock}  show={show}",
            v = st.version,
            s = if st.school.enabled { "on" } else { "off" },
            r = st.renderer,
            cap = if st.cap.is_empty() {
                "-".into()
            } else {
                st.cap.join(",")
            },
            agent = st.agent.as_deref().unwrap_or("-"),
            lock = st.locked_by.as_deref().unwrap_or("-"),
            show = st.show.as_deref().unwrap_or("-"),
        );
    }
    if st.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn fail(json: bool, msg: &str) -> ExitCode {
    if json {
        println!("{}", serde_json::json!({"ok": false, "error": msg}));
    } else {
        eprintln!("lot: {msg}");
    }
    ExitCode::from(2)
}
