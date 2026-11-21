use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use std::sync::OnceLock;

static CORRUPT_LOAD: OnceLock<()> = OnceLock::new();
static SAVE_FAILED: OnceLock<()> = OnceLock::new();

/// True when a corrupt state file was backed up and reset this process.
pub fn state_was_corrupt() -> bool {
    CORRUPT_LOAD.get().is_some()
}

/// Copy the current state file to a timestamped backup before an import
/// overwrites it. Returns the backup path when one was created.
pub fn backup_state() -> Option<std::path::PathBuf> {
    let path = Config::state_path();
    if !path.exists() {
        return None;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let backup = path.with_extension(format!("bak-{}", stamp));
    std::fs::copy(&path, &backup).ok().map(|_| backup)
}

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
    #[serde(default)]
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
    pub prev_hash: Option<String>,
    #[serde(default)]
    pub last_status: String,
    #[serde(default)]
    pub last_alert_ms: i64,
    #[serde(default)]
    pub question_alert_ms: i64,
    #[serde(default)]
    pub stalled_alert_ms: i64,
    #[serde(default)]
    pub missing_alert_ms: i64,
    #[serde(default)]
    pub failure_alert_ms: i64,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            prompt: None,
            handled: 0,
            prompted: 0,
            last_nudge: 0,
            nudges: 0,
            last_action: String::new(),
            idle_streak: 0,
            last_hash: None,
            prev_hash: None,
            last_status: String::new(),
            last_alert_ms: 0,
            question_alert_ms: 0,
            stalled_alert_ms: 0,
            missing_alert_ms: 0,
            failure_alert_ms: 0,
        }
    }
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
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    eprintln!("session-keeper: cannot read state {}: {}", path.display(), e);
                }
                return State::default();
            }
        };
        match State::from_json(&text) {
            Ok(s) => s,
            Err(_) => {
                // Only the first load backs up the file; later loads in the same
                // process would otherwise pile up identical corrupt backups.
                if CORRUPT_LOAD.set(()).is_ok() {
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    let backup = std::path::PathBuf::from(format!("{}.corrupt-{}", path.display(), stamp));
                    let _ = std::fs::copy(path, backup);
                }
                State::default()
            }
        }
    }

    // Single parse + sanitize entry point shared by load and import.
    pub fn from_json(text: &str) -> Result<State, serde_json::Error> {
        serde_json::from_str::<State>(text).map(State::normalize)
    }

    fn normalize(mut s: State) -> State {
        let mut seen = std::collections::HashSet::new();
        s.sessions.retain(|x| is_valid_session_id(&x.id) && seen.insert(x.id.clone()));
        s.min_gap = s.min_gap.clamp(0, 3600);
        s.max_nudges_per_hour = s.max_nudges_per_hour.max(0);
        s.global_nudge_cap = s.global_nudge_cap.max(0);
        s.tick_interval_s = s.tick_interval_s.clamp(5, 300);
        s.alert_cooldown_s = s.alert_cooldown_s.max(0);
        s.last_tick_ms = s.last_tick_ms.max(0);
        s.cap_alert_ms = s.cap_alert_ms.max(0);
        s.missing_alert_ms = s.missing_alert_ms.max(0);
        for x in &mut s.sessions {
            x.handled = x.handled.max(0);
            x.prompted = x.prompted.max(0);
            x.last_nudge = x.last_nudge.max(0);
            x.nudges = x.nudges.max(0);
            x.idle_streak = x.idle_streak.max(0);
            x.last_alert_ms = x.last_alert_ms.max(0);
            x.question_alert_ms = x.question_alert_ms.max(0);
            x.stalled_alert_ms = x.stalled_alert_ms.max(0);
            x.missing_alert_ms = x.missing_alert_ms.max(0);
        }
        s
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to_path(&Config::state_path()).map_err(|e| {
            // Callers ignore the Result; surface the first failure once so a
            // read-only or full disk cannot silently lose settings and the watch list.
            if SAVE_FAILED.set(()).is_ok() {
                crate::nudge::log_line(&format!("State save failed: {} (later failures this process are not logged)", e));
            }
            e
        })
    }

    pub fn save_to_path(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp, json)?;
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
    pub prev_hash: Option<String>,
    pub last_action: String,
    pub last_status: String,
    pub last_alert_ms: i64,
    pub question_alert_ms: i64,
    pub stalled_alert_ms: i64,
    pub missing_alert_ms: i64,
    pub failure_alert_ms: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct GlobalTickFields {
    pub last_tick_ms: i64,
    pub cap_alert_ms: i64,
    pub missing_alert_ms: i64,
    /// Remaining global nudge budget this hour, or -1 when unknown/unlimited.
    pub cap_remaining: i64,
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
            s.prev_hash = u.prev_hash.clone();
            s.last_action = u.last_action.clone();
            s.last_status = u.last_status.clone();
            s.last_alert_ms = u.last_alert_ms;
            s.question_alert_ms = u.question_alert_ms;
            s.stalled_alert_ms = u.stalled_alert_ms;
            s.missing_alert_ms = u.missing_alert_ms;
            s.failure_alert_ms = u.failure_alert_ms;
        }
    }
    state.last_tick_ms = globals.last_tick_ms;
    state.cap_alert_ms = globals.cap_alert_ms;
    state.missing_alert_ms = globals.missing_alert_ms;
}

pub fn is_valid_session_id(id: &str) -> bool {
    let rest = match id.strip_prefix("ses_") { Some(r) => r, None => return false };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
