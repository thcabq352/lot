//! Kernel version and an honest upgrade check. Never downloads a binary.

use crate::show::ShowError;
use crate::{NAME, VERSION};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::time::Duration;

pub const UPGRADE_URL_ENV: &str = "LOT_UPGRADE_URL";
pub const NO_CHANNEL: &str = "no upgrade channel —";
pub const NO_UPGRADE: &str = "no upgrade — use --check";
pub const NO_MANIFEST: &str = "no upgrade —";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionInfo {
    pub ok: bool,
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpgradeReport {
    pub ok: bool,
    pub name: &'static str,
    pub version: &'static str,
    pub latest: String,
    pub update: bool,
    pub channel: String,
}

pub fn version_info() -> VersionInfo {
    VersionInfo {
        ok: true,
        name: NAME,
        version: VERSION,
    }
}

pub fn upgrade_check() -> Result<UpgradeReport, ShowError> {
    let channel = std::env::var(UPGRADE_URL_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ShowError::Msg(NO_CHANNEL.into()))?;
    let body = read_channel(&channel)?;
    let latest = parse_latest(&body)?;
    Ok(UpgradeReport {
        ok: true,
        name: NAME,
        version: VERSION,
        update: is_newer(&latest, VERSION),
        latest,
        channel,
    })
}

fn read_channel(channel: &str) -> Result<String, ShowError> {
    if let Some(rest) = channel.strip_prefix("file://") {
        let path = file_url_path(rest);
        return fs::read_to_string(path).map_err(|_| ShowError::Msg(NO_MANIFEST.into()));
    }
    if channel.starts_with("http://") || channel.starts_with("https://") {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(2))
            .timeout_read(Duration::from_secs(2))
            .build();
        let resp = agent
            .get(channel)
            .call()
            .map_err(|_| ShowError::Msg(NO_MANIFEST.into()))?;
        return resp
            .into_string()
            .map_err(|_| ShowError::Msg(NO_MANIFEST.into()));
    }
    fs::read_to_string(channel).map_err(|_| ShowError::Msg(NO_MANIFEST.into()))
}

fn file_url_path(rest: &str) -> String {
    let rest = rest.trim();
    if cfg!(windows) && rest.starts_with('/') && rest.get(2..3) == Some(":") {
        rest.trim_start_matches('/').to_string()
    } else {
        rest.to_string()
    }
}

fn parse_latest(body: &str) -> Result<String, ShowError> {
    let t = body.trim();
    if t.is_empty() {
        return Err(ShowError::Msg(NO_MANIFEST.into()));
    }
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        for key in ["version", "latest"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                let s = normalize_version(s);
                if !s.is_empty() {
                    return Ok(s);
                }
            }
        }
        return Err(ShowError::Msg(NO_MANIFEST.into()));
    }
    let line = t.lines().next().unwrap_or("").trim();
    let line = normalize_version(line);
    if line.is_empty() || line.len() > 64 || line.contains('<') {
        return Err(ShowError::Msg(NO_MANIFEST.into()));
    }
    Ok(line)
}

fn normalize_version(s: &str) -> String {
    s.trim().trim_start_matches('v').trim().to_string()
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (semver_parts(latest), semver_parts(current)) {
        (Some(a), Some(b)) => a > b,
        _ => latest != current,
    }
}

fn semver_parts(s: &str) -> Option<[u64; 3]> {
    let s = normalize_version(s);
    let mut it = s.split('.');
    let maj = it.next()?.parse().ok()?;
    let min = it.next().unwrap_or("0").parse().unwrap_or(0);
    let pat = it.next().unwrap_or("0").parse().unwrap_or(0);
    Some([maj, min, pat])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV: Mutex<()> = Mutex::new(());

    fn unique_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lot-upgrade-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn version_names_the_crate() {
        let v = version_info();
        assert!(v.ok);
        assert_eq!(v.name, "lot");
        assert!(!v.version.is_empty());
    }

    #[test]
    fn upgrade_check_without_channel_is_honest() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(UPGRADE_URL_ENV);
        let e = upgrade_check().unwrap_err().to_string();
        assert!(e.contains(NO_CHANNEL), "{e}");
    }

    #[test]
    fn upgrade_check_reads_file_channel() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir();
        let p = dir.join("latest.json");
        fs::write(&p, r#"{"version":"9.9.9"}"#).unwrap();
        std::env::set_var(UPGRADE_URL_ENV, p.display().to_string());
        let r = upgrade_check().unwrap();
        std::env::remove_var(UPGRADE_URL_ENV);
        assert_eq!(r.latest, "9.9.9");
        assert!(r.update);
        assert_eq!(r.version, VERSION);
    }

    #[test]
    fn upgrade_check_same_version_is_not_an_update() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir();
        let p = dir.join("same.json");
        fs::write(&p, format!(r#"{{"latest":"{VERSION}"}}"#)).unwrap();
        std::env::set_var(UPGRADE_URL_ENV, p.display().to_string());
        let r = upgrade_check().unwrap();
        std::env::remove_var(UPGRADE_URL_ENV);
        assert_eq!(r.latest, VERSION);
        assert!(!r.update);
    }

    #[test]
    fn upgrade_check_empty_manifest_does_not_invent() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = unique_dir();
        let p = dir.join("empty.txt");
        fs::write(&p, "").unwrap();
        std::env::set_var(UPGRADE_URL_ENV, p.display().to_string());
        let e = upgrade_check().unwrap_err().to_string();
        std::env::remove_var(UPGRADE_URL_ENV);
        assert!(e.contains(NO_MANIFEST), "{e}");
    }

    #[test]
    fn is_newer_compares_semver() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }
}
