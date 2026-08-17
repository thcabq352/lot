use clap::{Parser, Subcommand};
use lot_core::{create_show, draft_screenplay, open_show, set_brief, Status};
use std::path::PathBuf;
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
    /// Writer: brief + real fountain draft (Grok OAuth → local → error).
    Writer {
        #[command(subcommand)]
        cmd: WriterCmd,
    },
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
    /// Write screenplay.fountain via Grok (xAI OAuth) or local OpenAI-compat.
    Draft,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Status => print_status(cli.json),
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
        Cmd::Writer { cmd } => match cmd {
            WriterCmd::Brief { text } => match set_brief(&text) {
                Ok((dir, show)) => {
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "ok": true,
                                "show": dir.display().to_string(),
                                "rev": show.rev,
                                "brief": show.writer.brief,
                            })
                        );
                    } else {
                        println!("brief set ({})", dir.display());
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(cli.json, &e.to_string()),
            },
            WriterCmd::Draft => match draft_screenplay() {
                Ok((dir, show)) => {
                    if cli.json {
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
                            .unwrap_or_else(|| dir.display().to_string());
                        if let Some(p) = show.writer.draft_provenance {
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
                Err(e) => fail(cli.json, &e.to_string()),
            },
        },
        Cmd::Mcp => match lot_mcp::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("lot mcp: {e}");
                ExitCode::from(1)
            }
        },
    }
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
