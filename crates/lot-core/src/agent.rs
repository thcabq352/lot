//! Who is writing. `LOT_AGENT` / `--agent` / MCP `agent`. Unset = human (no auto-claim).

use std::cell::RefCell;

thread_local! {
    static OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn normalize(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn current() -> Option<String> {
    if let Some(s) = OVERRIDE.with(|c| c.borrow().clone()) {
        return normalize(&s);
    }
    std::env::var("LOT_AGENT").ok().and_then(|s| normalize(&s))
}

pub fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

pub fn set_agent(id: Option<String>) {
    OVERRIDE.with(|c| *c.borrow_mut() = id.and_then(|s| normalize(&s)));
}

pub fn clear_agent() {
    OVERRIDE.with(|c| *c.borrow_mut() = None);
}

pub fn with_agent<T>(id: Option<String>, f: impl FnOnce() -> T) -> T {
    let prev = OVERRIDE.with(|c| c.replace(id.and_then(|s| normalize(&s))));
    let out = f();
    OVERRIDE.with(|c| *c.borrow_mut() = prev);
    out
}
