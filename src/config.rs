use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

pub const API: &str = "http://127.0.0.1:4096";

/// Display preferences for timestamps. `timezone` is either "auto" (the
/// device's own timezone) or a fixed UTC offset such as "+05:30".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimePrefs {
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub clock_24h: bool,
    #[serde(default = "default_date_style")]
    pub date_style: String,
}

fn default_timezone() -> String {
    "auto".to_string()
}
fn default_date_style() -> String {
    "day_month".to_string()
}

impl Default for TimePrefs {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            clock_24h: false,
            date_style: default_date_style(),
        }
    }
}

static TIME_PREFS: OnceLock<RwLock<TimePrefs>> = OnceLock::new();

fn prefs() -> &'static RwLock<TimePrefs> {
    TIME_PREFS.get_or_init(|| RwLock::new(TimePrefs::default()))
}

pub fn set_time_prefs(value: &TimePrefs) {
    if let Ok(mut guard) = prefs().write() {
        *guard = value.clone();
    }
}

pub fn get_time_prefs() -> TimePrefs {
    prefs().read().map(|g| g.clone()).unwrap_or_default()
}

pub struct Config;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Load persisted overrides from ~/.config/session-keeper/env before any
/// Config path is read. Real environment variables always win.
pub fn load_env_file() {
    let path = home().join(".config/session-keeper/env");
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return,
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
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
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

fn local_tm(epoch_secs: i64) -> Option<libc::tm> {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let secs = epoch_secs as libc::time_t;
    unsafe { libc::localtime_r(&secs, &mut tm).as_ref().copied() }
}

fn utc_tm(epoch_secs: i64) -> Option<libc::tm> {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let secs = epoch_secs as libc::time_t;
    unsafe { libc::gmtime_r(&secs, &mut tm).as_ref().copied() }
}

fn parse_offset(value: &str, now_secs: i64) -> i64 {
    let v = value.trim();
    if v.is_empty() || v == "auto" || v == "local" || v == "Z" || v == "UTC" {
        return local_tm(now_secs).map(|tm| tm.tm_gmtoff).unwrap_or(0);
    }
    let (sign, rest) = if let Some(r) = v.strip_prefix('-') {
        (-1i64, r)
    } else if let Some(r) = v.strip_prefix('+') {
        (1i64, r)
    } else {
        return 0;
    };
    let (h, m) = if let Some((a, b)) = rest.split_once(':') {
        (a, b)
    } else {
        (&rest[..rest.len().min(2)], &rest[rest.len().min(2)..])
    };
    let hours: i64 = h.parse().unwrap_or(0);
    let mins: i64 = m.parse().unwrap_or(0);
    sign * (hours * 3600 + mins * 60)
}

fn tm_for(epoch_secs: i64, offset_secs: i64) -> Option<libc::tm> {
    utc_tm(epoch_secs + offset_secs)
}

fn format_clock(tm: &libc::tm, use_24h: bool) -> String {
    if use_24h {
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    } else {
        let hour = match tm.tm_hour % 12 {
            0 => 12,
            h => h,
        };
        let suffix = if tm.tm_hour < 12 { "AM" } else { "PM" };
        format!("{:02}:{:02}:{:02} {}", hour, tm.tm_min, tm.tm_sec, suffix)
    }
}

fn format_date(tm: &libc::tm, style: &str) -> String {
    let d = tm.tm_mday;
    let m = tm.tm_mon + 1;
    let y = tm.tm_year + 1900;
    match style {
        "none" => String::new(),
        "day" => format!("{:02}", d),
        "month_day" => format!("{:02}/{:02}", m, d),
        "month_day_year" => format!("{:02}/{:02}/{:04}", m, d, y),
        "year_month_day" => format!("{:04}-{:02}-{:02}", y, m, d),
        "year" => format!("{:04}", y),
        _ => format!("{:02}/{:02}", d, m),
    }
}

/// Formats a timestamp with the active display preferences. Same-day entries
/// show only the clock; older ones get the configured date prefix.
pub fn format_log_time(epoch_secs: i64) -> String {
    let pref = get_time_prefs();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(epoch_secs);
    let offset = parse_offset(&pref.timezone, now);
    let tm = match tm_for(epoch_secs, offset) {
        Some(t) => t,
        None => return format_utc_time(epoch_secs),
    };
    let today = match tm_for(now, offset) {
        Some(t) => t,
        None => tm,
    };
    let same_day = tm.tm_year == today.tm_year && tm.tm_yday == today.tm_yday;
    let clock = format_clock(&tm, pref.clock_24h);
    if same_day || pref.date_style == "none" {
        clock
    } else {
        format!("{} {}", format_date(&tm, &pref.date_style), clock)
    }
}

/// Epoch milliseconds for local midnight, so "today" rolls over at the
/// operator's clock rather than at 00:00 UTC.
pub fn local_day_start_ms(now_ms: i64) -> i64 {
    let pref = get_time_prefs();
    let offset = parse_offset(&pref.timezone, now_ms / 1000);
    match tm_for(now_ms / 1000, offset) {
        Some(tm) => {
            let elapsed =
                (tm.tm_hour as i64 * 3600 + tm.tm_min as i64 * 60 + tm.tm_sec as i64) * 1000;
            now_ms - elapsed
        }
        None => now_ms - (now_ms.rem_euclid(86_400_000)),
    }
}

impl Config {
    pub fn db_path() -> PathBuf {
        env_path(
            "SESSION_KEEPER_DB",
            home().join(".local/share/opencode/opencode.db"),
        )
    }
    pub fn drive() -> PathBuf {
        env_path(
            "SESSION_KEEPER_DRIVE",
            Config::db_path()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(home),
        )
    }
    pub fn state_path() -> PathBuf {
        env_path(
            "SESSION_KEEPER_STATE",
            home().join(".config/session-keeper/state.json"),
        )
    }
    pub fn history_path() -> PathBuf {
        env_path(
            "SESSION_KEEPER_HISTORY",
            home().join(".config/session-keeper/history.jsonl"),
        )
    }
    pub fn lock_path() -> PathBuf {
        env_path(
            "SESSION_KEEPER_LOCK",
            home().join(".config/session-keeper/keeper.lock"),
        )
    }
    pub fn audit_path() -> PathBuf {
        env_path(
            "SESSION_KEEPER_AUDIT",
            home().join("session-keeper-audit.txt"),
        )
    }
    pub fn log_path() -> PathBuf {
        env_path("SESSION_KEEPER_LOG", home().join("session-keeper.log"))
    }
    pub fn api_base() -> String {
        std::env::var("SESSION_KEEPER_API").unwrap_or_else(|_| API.to_string())
    }
    pub fn services_json() -> PathBuf {
        env_path(
            "SESSION_KEEPER_SERVICE_JSON",
            home().join(".config/opencode/service.json"),
        )
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
            .and_then(|v| {
                v.get("password")
                    .and_then(|p| p.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default()
    }
}
