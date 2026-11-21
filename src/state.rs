use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

fn default_enabled() -> bool { true }
fn default_prompt() -> String { "continue, dont stop".to_string() }
fn default_min_gap() -> i64 { 10 }
fn default_max_nudges() -> i64 { 30 }
fn default_global_cap() -> i64 { 60 }
fn default_interval() -> i64 { 30 }
fn default_true() -> bool { true }
fn default_cooldown() -> i64 { 300 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub handled: i64,
    #[serde(default)]
    pub prompted: i64,
    #[serde(default)]
    pub last_nudge: i64,
    #[serde(default)]
    pub nudges: i64,
    #[serde(default)]
    pub last_action: String,
    #[serde(default)]
    pub idle_streak: i32,
    #[serde(default)]
    pub last_hash: Option<String>,
    #[serde(default)]
    pub last_status: String,
    #[serde(default)]
    pub last_alert_ms: i64,
    #[serde(default)]
    pub question_alert_ms: i64,
    #[serde(default)]
    pub stalled_alert_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_prompt")]
    pub default_prompt: String,
    #[serde(default = "default_min_gap")]
    pub min_gap: i64,
    #[serde(default = "default_max_nudges")]
    pub max_nudges_per_hour: i64,
    #[serde(default)]
    pub away_mode: bool,
    #[serde(default = "default_global_cap")]
    pub global_nudge_cap: i64,
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub last_tick_ms: i64,
    #[serde(default = "default_interval")]
    pub tick_interval_s: i64,
    #[serde(default)]
    pub persist_dry_run: bool,
    #[serde(default = "default_true")]
    pub persist_notifications: bool,
    #[serde(default)]
    pub cap_alert_ms: i64,
    #[serde(default)]
    pub missing_alert_ms: i64,
    #[serde(default)]
    pub sort_by_name: bool,
    #[serde(default = "default_cooldown")]
    pub alert_cooldown_s: i64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: true,
            default_prompt: default_prompt(),
            min_gap: 10,
            max_nudges_per_hour: 30,
            away_mode: false,
            global_nudge_cap: 60,
            sessions: Vec::new(),
            last_tick_ms: 0,
            tick_interval_s: 30,
            persist_dry_run: false,
            persist_notifications: true,
            cap_alert_ms: 0,
            missing_alert_ms: 0,
            sort_by_name: false,
            alert_cooldown_s: 300,
        }
    }
}

impl State {
    pub fn load() -> State { Self::load_from_path(&Config::state_path()) }

    pub fn load_from_path(path: &Path) -> State {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return State::default(),
        };
        match serde_json::from_str::<State>(&text) {
            Ok(mut s) => {
                s.sessions.retain(|x| !x.id.is_empty());
                s
            }
            Err(_) => State::default(),
        }
    }

    pub fn save(&self) -> io::Result<()> { self.save_to_path(&Config::state_path()) }

    pub fn save_to_path(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default())?;
        std::fs::rename(&tmp, path)
    }
}

pub fn states_differ(a: &State, b: &State) -> bool {
    serde_json::to_string(a).unwrap_or_default() != serde_json::to_string(b).unwrap_or_default()
}
