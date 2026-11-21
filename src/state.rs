use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

fn default_enabled() -> bool { true }
fn default_prompt() -> String { "continue, dont stop".to_string() }
fn default_min_gap() -> i64 { 10 }
fn default_max_nudges() -> i64 { 30 }

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
    #[serde(default)]
    pub sessions: Vec<Session>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: true,
            default_prompt: default_prompt(),
            min_gap: 10,
            max_nudges_per_hour: 30,
            away_mode: false,
            sessions: Vec::new(),
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
