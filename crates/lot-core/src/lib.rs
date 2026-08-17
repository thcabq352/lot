use serde::Serialize;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "lot";

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
}

#[derive(Debug, Serialize)]
pub struct SchoolStatus {
    pub enabled: bool,
}

impl Status {
    pub fn bootstrap() -> Self {
        Self {
            ok: true,
            name: NAME,
            version: VERSION,
            door: "cli",
            school: SchoolStatus { enabled: false },
            renderer: "unavailable",
            show: None,
        }
    }
}
