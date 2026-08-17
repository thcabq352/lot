//! Cooperative cancel for long MCP tools (stills, finish, draft).
//! `notifications/cancelled` sets a flag; tools return `cancelled —` and write no media.

use crate::show::ShowError;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const CANCELLED_MSG: &str = "cancelled —";

struct InFlight {
    id: Option<Value>,
    token: Arc<AtomicBool>,
}

thread_local! {
    static TOKEN: std::cell::RefCell<Arc<AtomicBool>> =
        std::cell::RefCell::new(Arc::new(AtomicBool::new(false)));
}

static IN_FLIGHT: Mutex<Option<InFlight>> = Mutex::new(None);

pub fn cancelled_err() -> ShowError {
    ShowError::Msg(CANCELLED_MSG.into())
}

pub fn is_cancelled() -> bool {
    TOKEN.with(|t| t.borrow().load(Ordering::SeqCst))
}

pub fn check() -> Result<(), ShowError> {
    if is_cancelled() {
        Err(cancelled_err())
    } else {
        Ok(())
    }
}

/// Start a request. Inherits a same-thread pre-set cancel so tests can arm first.
pub fn begin_request(id: Option<&Value>) {
    let already = is_cancelled();
    let token = Arc::new(AtomicBool::new(already));
    TOKEN.with(|t| *t.borrow_mut() = token.clone());
    if let Ok(mut g) = IN_FLIGHT.lock() {
        *g = Some(InFlight {
            id: id.cloned(),
            token,
        });
    }
}

pub fn end_request() {
    let fresh = Arc::new(AtomicBool::new(false));
    TOKEN.with(|t| *t.borrow_mut() = fresh);
    if let Ok(mut g) = IN_FLIGHT.lock() {
        *g = None;
    }
}

pub fn clear() {
    end_request();
}

/// Arm cancel. `None` cancels this thread (and any in-flight token).
/// A request id only matches the in-flight MCP request.
pub fn request_cancel(request_id: Option<&Value>) {
    match request_id {
        None => {
            TOKEN.with(|t| t.borrow().store(true, Ordering::SeqCst));
            if let Ok(g) = IN_FLIGHT.lock() {
                if let Some(f) = g.as_ref() {
                    f.token.store(true, Ordering::SeqCst);
                }
            }
        }
        Some(id) => {
            if let Ok(g) = IN_FLIGHT.lock() {
                if let Some(f) = g.as_ref() {
                    if f.id.as_ref().is_some_and(|cur| ids_match(cur, id)) {
                        f.token.store(true, Ordering::SeqCst);
                    }
                }
            }
        }
    }
}

pub fn from_notification(msg: &Value) {
    let id = msg
        .get("params")
        .and_then(|p| p.get("requestId"))
        .or_else(|| msg.get("requestId"));
    request_cancel(id);
}

fn ids_match(a: &Value, b: &Value) -> bool {
    if a == b {
        return true;
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_i64() == y.as_i64() && x.as_i64().is_some(),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Number(x), Value::String(y)) => x.to_string() == *y,
        (Value::String(x), Value::Number(y)) => *x == y.to_string(),
        _ => false,
    }
}

/// Run `work` on a helper thread; return `cancelled —` if the flag flips first.
pub fn run_interruptible<T, F>(work: F) -> Result<T, ShowError>
where
    F: FnOnce() -> Result<T, ShowError> + Send + 'static,
    T: Send + 'static,
{
    check()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(r) => {
                check()?;
                return r;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => check()?,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(ShowError::Msg("cancelled — worker dropped".into()));
            }
        }
    }
}

#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn check_errors_after_request_cancel() {
        let _g = test_lock();
        clear();
        assert!(check().is_ok());
        request_cancel(None);
        let err = check().unwrap_err().to_string();
        assert!(err.starts_with(CANCELLED_MSG), "{err}");
        clear();
        assert!(check().is_ok());
    }

    #[test]
    fn other_request_id_does_not_cancel() {
        let _g = test_lock();
        clear();
        begin_request(Some(&json!(12)));
        request_cancel(Some(&json!(99)));
        assert!(!is_cancelled());
        request_cancel(Some(&json!(12)));
        assert!(is_cancelled());
        clear();
    }

    #[test]
    fn notification_cancels_matching_in_flight() {
        let _g = test_lock();
        clear();
        begin_request(Some(&json!("stills-1")));
        from_notification(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": "stills-1", "reason": "user" }
        }));
        assert!(is_cancelled());
        clear();
    }

    #[test]
    fn interruptible_stops_when_other_thread_cancels() {
        let _g = test_lock();
        clear();
        begin_request(Some(&json!(1)));
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(40));
            request_cancel(Some(&json!(1)));
        });
        let err = run_interruptible(|| {
            thread::sleep(Duration::from_millis(800));
            Ok(())
        })
        .unwrap_err()
        .to_string();
        assert!(err.starts_with(CANCELLED_MSG), "{err}");
        clear();
    }
}
