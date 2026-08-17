//! Agent caps: read | write | render | export | spend.
//! Unset / `all` = full (human CLI). Explicit `read` is the locked-down agent.

use crate::ShowError;
use std::cell::RefCell;
use std::fmt;

thread_local! {
    static OVERRIDE: RefCell<Option<Caps>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    Read,
    Write,
    Render,
    Export,
    Spend,
}

impl Cap {
    pub fn as_str(self) -> &'static str {
        match self {
            Cap::Read => "read",
            Cap::Write => "write",
            Cap::Render => "render",
            Cap::Export => "export",
            Cap::Spend => "spend",
        }
    }
}

impl fmt::Display for Cap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    read: bool,
    write: bool,
    render: bool,
    export: bool,
    spend: bool,
}

impl Caps {
    pub fn all() -> Self {
        Self {
            read: true,
            write: true,
            render: true,
            export: true,
            spend: true,
        }
    }

    pub fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            render: false,
            export: false,
            spend: false,
        }
    }

    pub fn allows(self, need: Cap) -> bool {
        match need {
            Cap::Read => self.read,
            Cap::Write => self.write,
            Cap::Render => self.render,
            Cap::Export => self.export,
            Cap::Spend => self.spend,
        }
    }

    pub fn names(self) -> Vec<String> {
        let mut out = Vec::new();
        if self.read {
            out.push("read".into());
        }
        if self.write {
            out.push("write".into());
        }
        if self.render {
            out.push("render".into());
        }
        if self.export {
            out.push("export".into());
        }
        if self.spend {
            out.push("spend".into());
        }
        out
    }

    fn label(self) -> String {
        if self == Self::all() {
            return "all".into();
        }
        let n = self.names();
        if n.is_empty() {
            "none".into()
        } else {
            n.join(",")
        }
    }
}

pub fn parse_caps(raw: &str) -> Result<Caps, ShowError> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("all") {
        return Ok(Caps::all());
    }
    let mut caps = Caps {
        read: false,
        write: false,
        render: false,
        export: false,
        spend: false,
    };
    for part in t.split([',', ' ', '+']) {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match p.to_ascii_lowercase().as_str() {
            "read" => caps.read = true,
            "write" => {
                caps.read = true;
                caps.write = true;
            }
            "render" => {
                caps.read = true;
                caps.write = true;
                caps.render = true;
            }
            "export" => {
                caps.read = true;
                caps.write = true;
                caps.export = true;
            }
            "spend" => {
                caps.read = true;
                caps.write = true;
                caps.spend = true;
            }
            "all" => return Ok(Caps::all()),
            other => {
                return Err(ShowError::Msg(format!(
                    "unknown cap: {other} — use read|write|render|export|spend|all"
                )))
            }
        }
    }
    if !caps.read {
        return Err(ShowError::Msg(
            "cap list is empty — use read|write|render|export|spend|all".into(),
        ));
    }
    Ok(caps)
}

pub fn active() -> Caps {
    if let Some(c) = OVERRIDE.with(|s| *s.borrow()) {
        return c;
    }
    match std::env::var("LOT_CAP") {
        Ok(s) if !s.trim().is_empty() => parse_caps(&s).unwrap_or_else(|_| Caps::all()),
        _ => Caps::all(),
    }
}

pub fn set_caps(caps: Caps) {
    OVERRIDE.with(|s| *s.borrow_mut() = Some(caps));
}

pub fn clear_caps() {
    OVERRIDE.with(|s| *s.borrow_mut() = None);
}

pub fn with_caps<T>(caps: Option<Caps>, f: impl FnOnce() -> T) -> T {
    match caps {
        None => f(),
        Some(c) => {
            let prev = OVERRIDE.with(|s| s.replace(Some(c)));
            let out = f();
            OVERRIDE.with(|s| *s.borrow_mut() = prev);
            out
        }
    }
}

pub fn require(need: Cap) -> Result<(), ShowError> {
    let have = active();
    if have.allows(need) {
        return Ok(());
    }
    let extra = match need {
        Cap::Spend => " stills generate --backend grok needs spend. Did not call Comfy.",
        Cap::Render => {
            " stills generate --backend comfy / finish --upscale needs render. Did not call Grok."
        }
        Cap::Write => " dailies circle and other writes need write.",
        Cap::Export => " export needs export.",
        Cap::Read => "",
    };
    Err(ShowError::Msg(format!(
        "need {need} — agent cap is {}.{}",
        have.label(),
        extra
    )))
}

pub fn allow_spend() -> bool {
    active().allows(Cap::Spend)
}

pub fn require_write() -> Result<(), ShowError> {
    require(Cap::Write)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_imply() {
        let r = parse_caps("read").unwrap();
        assert!(r.allows(Cap::Read));
        assert!(!r.allows(Cap::Write));
        assert!(!r.allows(Cap::Spend));
        let w = parse_caps("write").unwrap();
        assert!(w.allows(Cap::Read) && w.allows(Cap::Write));
        assert!(!w.allows(Cap::Spend) && !w.allows(Cap::Render));
        let s = parse_caps("spend").unwrap();
        assert!(s.allows(Cap::Write) && s.allows(Cap::Spend));
        assert!(!s.allows(Cap::Render));
        let both = parse_caps("write,spend").unwrap();
        assert!(both.allows(Cap::Write) && both.allows(Cap::Spend));
        assert!(!both.allows(Cap::Render));
        assert!(parse_caps("all").unwrap() == Caps::all());
        assert!(parse_caps("nope").is_err());
    }
}
