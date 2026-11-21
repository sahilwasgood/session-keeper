use std::path::PathBuf;

pub const API: &str = "http://127.0.0.1:4096";

pub struct Config;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Load persisted overrides from ~/.config/session-keeper/env before any
/// Config path is read. Real environment variables always win.
pub fn load_env_file() {
    let path = home().join(".config/session-keeper/env");
    let text = match std::fs::read_to_string(path) { Ok(t) => t, Err(_) => return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty() && std::env::var_os(key).is_none() {
                std::env::set_var(key, value.trim());
            }
        }
    }
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

/// UTC clock time, kept as a timezone-independent reference for tests.
pub fn format_utc_time(epoch_secs: i64) -> String {
    let secs = epoch_secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

/// Wall-clock time in the machine's local timezone (localtime_r is thread-safe).
pub fn format_log_time(epoch_secs: i64) -> String {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut secs = epoch_secs as libc::time_t;
    unsafe { libc::localtime_r(&mut secs, &mut tm); }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

impl Config {
    pub fn db_path() -> PathBuf {
        env_path("SESSION_KEEPER_DB", home().join(".local/share/opencode/opencode.db"))
    }
    pub fn drive() -> PathBuf {
        env_path(
            "SESSION_KEEPER_DRIVE",
            Config::db_path().parent().map(PathBuf::from).unwrap_or_else(home),
        )
    }
    pub fn state_path() -> PathBuf {
        env_path("SESSION_KEEPER_STATE", home().join(".config/session-keeper/state.json"))
    }
    pub fn history_path() -> PathBuf {
        env_path("SESSION_KEEPER_HISTORY", home().join(".config/session-keeper/history.jsonl"))
    }
    pub fn lock_path() -> PathBuf {
        env_path("SESSION_KEEPER_LOCK", home().join(".config/session-keeper/keeper.lock"))
    }
    pub fn audit_path() -> PathBuf {
        env_path("SESSION_KEEPER_AUDIT", home().join("session-keeper-audit.txt"))
    }
    pub fn log_path() -> PathBuf {
        env_path("SESSION_KEEPER_LOG", home().join("session-keeper.log"))
    }
    pub fn api_base() -> String {
        std::env::var("SESSION_KEEPER_API").unwrap_or_else(|_| API.to_string())
    }
    pub fn services_json() -> PathBuf {
        env_path("SESSION_KEEPER_SERVICE_JSON", home().join(".config/opencode/service.json"))
    }
    pub fn dry_run() -> bool {
        std::env::var("SESSION_KEEPER_DRY_RUN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }
    pub fn notifications() -> bool {
        std::env::var("SESSION_KEEPER_NOTIFY")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    }
    pub fn password() -> String {
        std::fs::read_to_string(Self::services_json())
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("password").and_then(|p| p.as_str()).map(str::to_string))
            .unwrap_or_default()
    }
}
