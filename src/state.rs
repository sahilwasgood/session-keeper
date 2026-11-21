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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
            Err(_) => {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = std::path::PathBuf::from(format!("{}.corrupt-{}", path.display(), stamp));
                let _ = std::fs::copy(path, backup);
                State::default()
            }
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

#[derive(Debug, Clone)]
pub struct TickFields {
    pub id: String,
    pub handled: i64,
    pub prompted: i64,
    pub last_nudge: i64,
    pub nudges: i64,
    pub idle_streak: i32,
    pub last_hash: Option<String>,
    pub last_action: String,
    pub last_status: String,
    pub last_alert_ms: i64,
    pub question_alert_ms: i64,
    pub stalled_alert_ms: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct GlobalTickFields {
    pub last_tick_ms: i64,
    pub cap_alert_ms: i64,
    pub missing_alert_ms: i64,
}

pub fn apply_tick_fields(state: &mut State, updates: &[TickFields], globals: GlobalTickFields) {
    for u in updates {
        if let Some(s) = state.sessions.iter_mut().find(|s| s.id == u.id) {
            s.handled = u.handled;
            s.prompted = u.prompted;
            s.last_nudge = u.last_nudge;
            s.nudges = u.nudges;
            s.idle_streak = u.idle_streak;
            s.last_hash = u.last_hash.clone();
            s.last_action = u.last_action.clone();
            s.last_status = u.last_status.clone();
            s.last_alert_ms = u.last_alert_ms;
            s.question_alert_ms = u.question_alert_ms;
            s.stalled_alert_ms = u.stalled_alert_ms;
        }
    }
    state.last_tick_ms = globals.last_tick_ms;
    state.cap_alert_ms = globals.cap_alert_ms;
    state.missing_alert_ms = globals.missing_alert_ms;
}
