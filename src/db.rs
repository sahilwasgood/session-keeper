use crate::state::{Session, State};
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Due,
    Nudged,
    Waiting,
    Stalled,
    Clean,
}

fn last_event(conn: &Connection, sid: &str) -> (Option<String>, i64, String) {
    conn.query_row(
        "SELECT type, time_created, COALESCE(json_extract(data,'$.outcome'),'')
         FROM session_message WHERE session_id=?1
         ORDER BY time_created DESC, rowid DESC LIMIT 1",
        [sid],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .unwrap_or(None)
    .unwrap_or((None, 0, String::new()))
}

fn last_assistant(conn: &Connection, sid: &str) -> (i64, String) {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT time_created, data FROM session_message
             WHERE session_id=?1 AND type='assistant'
             ORDER BY time_created DESC, rowid DESC LIMIT 1",
            [sid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap_or(None);
    match row {
        None => (0, String::new()),
        Some((ts, data)) => {
            let text = serde_json::from_str::<serde_json::Value>(&data)
                .ok()
                .and_then(|v| v.get("content").and_then(|c| c.as_array()).cloned())
                .and_then(|parts| {
                    parts
                        .iter()
                        .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .last()
                        .map(str::to_string)
                })
                .unwrap_or_default();
            (ts, text.trim().to_string())
        }
    }
}

pub fn session_exists(conn: &Connection, sid: &str) -> bool {
    conn.query_row("SELECT 1 FROM session_v2 WHERE id=?1", [sid], |_| Ok(()))
        .optional()
        .unwrap_or(None)
        .is_some()
}

pub fn last_assistant_ts(conn: &Connection, sid: &str) -> i64 {
    last_assistant(conn, sid).0
}

pub fn last_assistant_text(conn: &Connection, sid: &str) -> String {
    last_assistant(conn, sid).1
}

pub fn session_title(conn: &Connection, sid: &str) -> String {
    conn.query_row("SELECT title FROM session_v2 WHERE id=?1", [sid], |r| r.get::<_, Option<String>>(0))
        .optional()
        .unwrap_or(None)
        .and_then(|title| title)
        .unwrap_or_default()
}

pub fn last_idle_ts(conn: &Connection, sid: &str) -> i64 {
    conn.query_row(
        "SELECT time_created FROM session_message WHERE session_id=?1 AND type='idle'
         ORDER BY time_created DESC, rowid DESC LIMIT 1",
        [sid],
        |r| r.get(0),
    )
    .optional()
    .unwrap_or(None)
    .unwrap_or(0)
}

pub fn classify(conn: &Connection, sid: &str, state: &State) -> Status {
    let fallback = Session {
        id: sid.to_string(),
        enabled: true,
        prompt: None,
        handled: 0,
        prompted: 0,
        last_nudge: 0,
        nudges: 0,
        last_action: String::new(), idle_streak: 0, last_hash: None, last_status: String::new(), last_alert_ms: 0,
    };
    let sess = state.sessions.iter().find(|s| s.id == sid).unwrap_or(&fallback);
    if !sess.enabled {
        return Status::Clean;
    }
    let (typ, ts, _outcome) = last_event(conn, sid);
    if typ.as_deref() != Some("idle") || ts == 0 {
        return Status::Clean;
    }
    if ts == sess.handled {
        return Status::Nudged;
    }
    let (ast, text) = last_assistant(conn, sid);
    if state.away_mode && text.ends_with('?') {
        return Status::Waiting;
    }
    if sess.prompted > 0 && ast <= sess.prompted {
        return Status::Stalled;
    }
    if text.ends_with('?') {
        return Status::Waiting;
    }
    if ast == 0 {
        return Status::Stalled;
    }
    Status::Due
}
