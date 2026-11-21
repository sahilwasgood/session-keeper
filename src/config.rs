use std::path::PathBuf;

pub const DB_PATH: &str = "/media/mint/WDC_WD10EZRX_932G/opencode/data/opencode.db";
pub const DRIVE: &str = "/media/mint/WDC_WD10EZRX_932G";
pub const API: &str = "http://127.0.0.1:49374";
pub const SERVICE_JSON: &str = "/media/mint/WDC_WD10EZRX_932G/opencode/config/service.json";

pub struct Config;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/mint"))
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

pub fn format_log_time(epoch_secs: i64) -> String {
    let secs = epoch_secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

impl Config {
    pub fn db_path() -> PathBuf { env_path("SESSION_KEEPER_DB", PathBuf::from(DB_PATH)) }
    pub fn drive() -> PathBuf { env_path("SESSION_KEEPER_DRIVE", PathBuf::from(DRIVE)) }
    pub fn state_path() -> PathBuf {
        env_path("SESSION_KEEPER_STATE", home().join(".config/session-keeper/state.json"))
    }
    pub fn history_path() -> PathBuf {
        env_path("SESSION_KEEPER_HISTORY", home().join(".config/session-keeper/history.jsonl"))
    }
    pub fn log_path() -> PathBuf {
        env_path("SESSION_KEEPER_LOG", home().join("session-keeper.log"))
    }
    pub fn api_base() -> String {
        std::env::var("SESSION_KEEPER_API").unwrap_or_else(|_| API.to_string())
    }
    pub fn services_json() -> PathBuf {
        env_path("SESSION_KEEPER_SERVICE_JSON", PathBuf::from(SERVICE_JSON))
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
