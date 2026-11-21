use std::path::PathBuf;

pub const DB_PATH: &str = "/media/mint/WDC_WD10EZRX_932G/opencode/data/opencode.db";
pub const DRIVE: &str = "/media/mint/WDC_WD10EZRX_932G";
pub const API: &str = "http://127.0.0.1:49374";
pub const SERVICE_JSON: &str = "/media/mint/WDC_WD10EZRX_932G/opencode/config/service.json";

pub struct Config;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/home/mint"))
}

impl Config {
    pub fn db_path() -> PathBuf { PathBuf::from(DB_PATH) }
    pub fn state_path() -> PathBuf { home().join(".config/session-keeper/state.json") }
    pub fn history_path() -> PathBuf { home().join(".config/session-keeper/history.jsonl") }
    pub fn log_path() -> PathBuf { home().join("session-keeper.log") }
    pub fn api_base() -> &'static str { API }
    pub fn password() -> String {
        std::fs::read_to_string(SERVICE_JSON)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("password").and_then(|p| p.as_str()).map(str::to_string))
            .unwrap_or_default()
    }
}
