use clap::{Parser, Subcommand};
use lot_core::{create_show, open_show, Status};
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
    /// Native agent door (stdio MCP). Not wired yet — exits 2.
    Mcp,
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
        Cmd::Mcp => {
            eprintln!("lot mcp: stdio server not wired (kernel only).");
            ExitCode::from(2)
        }
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
