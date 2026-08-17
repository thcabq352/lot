use clap::{Parser, Subcommand};
use lot_core::Status;

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
    /// Native agent door (stdio MCP). Not wired yet — exits 2.
    Mcp,
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Status => {
            let mut st = Status::bootstrap();
            st.door = "cli";
            if cli.json {
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
        }
        Cmd::Mcp => {
            eprintln!("lot mcp: stdio server not wired (kernel only).");
            std::process::exit(2);
        }
    }
}
