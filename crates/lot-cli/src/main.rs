use clap::{Parser, Subcommand};
use lot_core::{
    breakdown_parse, breakdown_summary, create_show, dailies_circle, dailies_export,
    dailies_ingest, draft_screenplay, lock_writer, open_show, picture_lock, replace_cast_json,
    revise_screenplay, set_brief, set_style, slate_set, unlock_writer, upsert_cast, wall_add,
    Doctor, Status,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "lot",
    version,
    about = "Lot — agent-first film tools. Stdio first, GUI last."
)]
struct Cli {
    #[arg(long, global = true, default_value_t = false)]
    json: bool,
    /// Open this show.lot, then run. Omit to keep the current pointer.
    #[arg(long, global = true)]
    show: Option<PathBuf>,
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
    /// Slate: continuity-locked prompts on shots.
    Slate {
        #[command(subcommand)]
        cmd: SlateCmd,
    },
    /// Dailies (Circle Take): ingest, circle, FCPXML.
    Dailies {
        #[command(subcommand)]
        cmd: DailiesCmd,
    },
    /// Cut: interchange export (FCPXML). Resolve live is an adapter later.
    Cut {
        #[command(subcommand)]
        cmd: CutCmd,
    },
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
    /// Write screenplay.fountain via Grok (xAI OAuth) or local OpenAI-compat.
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
enum SlateCmd {
    Set {
        #[arg(long)]
        shot: String,
        #[arg(long)]
        prompt: String,
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Status => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            print_status(cli.json)
        }
        Cmd::Create { path, name } => match create_show(&path, name.as_deref()) {
            Ok((dir, show)) => {
                if cli.json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "show": dir.display().to_string(),
                            "id": show.id,
                            "name": show.name,
                            "rev": show.rev,
                        })
                    );
                } else {
                    println!("created {} ({})", dir.display(), show.name);
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(cli.json, &e.to_string()),
        },
        Cmd::Open { path } => match open_show(&path) {
            Ok((dir, show)) => {
                if cli.json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "show": dir.display().to_string(),
                            "id": show.id,
                            "name": show.name,
                            "rev": show.rev,
                        })
                    );
                } else {
                    println!("opened {} ({})", dir.display(), show.name);
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(cli.json, &e.to_string()),
        },
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
        Cmd::Slate { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                SlateCmd::Set { shot, prompt } => match slate_set(&shot, &prompt) {
                    Ok((dir, show)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "shots": show.shots }),
                        &format!("slate set on shot {shot}"),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Dailies { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            dailies_cmd(cmd, cli.json)
        }
        Cmd::Cut { cmd } => {
            if let Some(code) = apply_show(cli.show.as_deref(), cli.json) {
                return code;
            }
            match cmd {
                CutCmd::Export => match dailies_export() {
                    Ok((dir, show, file)) => ok_writer(
                        cli.json,
                        &dir,
                        &show,
                        serde_json::json!({ "export": file.display().to_string() }),
                        &format!("cut export {}", file.display()),
                    ),
                    Err(e) => fail(cli.json, &e.to_string()),
                },
            }
        }
        Cmd::Doctor => {
            let d = Doctor::probe();
            if cli.json {
                println!("{}", serde_json::to_string(&d).expect("doctor json"));
            } else {
                println!(
                    "ffmpeg={} ffprobe={} comfy={} grok={} local={} renderer={}",
                    d.ffmpeg, d.ffprobe, d.comfy, d.grok_configured, d.local_configured, d.renderer
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
            Ok((show_dir, show)) => ok_writer(
                json,
                &show_dir,
                &show,
                serde_json::json!({
                    "takes": show.takes,
                    "shots": show.shots.iter().map(|s| {
                        serde_json::json!({ "num": s.num, "name": s.name })
                    }).collect::<Vec<_>>()
                }),
                &format!("ingested {} takes", show.takes.len()),
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
            Ok((dir, show, file)) => ok_writer(
                json,
                &dir,
                &show,
                serde_json::json!({ "export": file.display().to_string() }),
                &format!("exported {}", file.display()),
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
        let mut v = serde_json::json!({
            "ok": true,
            "show": dir.display().to_string(),
            "rev": show.rev,
        });
        if let (Some(obj), Some(extra_obj)) = (v.as_object_mut(), extra.as_object()) {
            for (k, val) in extra_obj {
                obj.insert(k.clone(), val.clone());
            }
        }
        println!("{v}");
    } else {
        println!("{plain}");
    }
    ExitCode::SUCCESS
}

fn draft_ok(json: bool, dir: &Path, show: &lot_core::Show) -> ExitCode {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "show": dir.display().to_string(),
                "rev": show.rev,
                "draft": show.writer.draft_path,
                "provenance": show.writer.draft_provenance,
            })
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

fn print_status(json: bool) -> ExitCode {
    let mut st = Status::bootstrap();
    st.door = "cli";
    if json {
        println!("{}", serde_json::to_string(&st).expect("status json"));
    } else {
        println!(
            "lot {v}  school={s}  renderer={r}  show={show}",
            v = st.version,
            s = if st.school.enabled { "on" } else { "off" },
            r = st.renderer,
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
