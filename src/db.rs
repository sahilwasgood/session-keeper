use crate::state::{Session, State};
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Due,
    Nudged,
    Waiting,
    Stalled,
    Clean,
    /// Database unavailable, so the real state cannot be determined.
    Unknown,
}

fn last_event(conn: &Connection, sid: &str) -> (Option<String>, i64, String) {
    conn.query_row(
        "SELECT type, time_created,
         CASE WHEN json_valid(data) THEN COALESCE(json_extract(data,'$.outcome'),'') ELSE '' END
         FROM session_message WHERE session_id=?1
         ORDER BY seq DESC LIMIT 1",
        [sid],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .unwrap_or(None)
    .unwrap_or((None, 0, String::new()))
}

fn text_from_part_data(data: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string))
}

/// OpenCode 2.4 stores assistant parts in `session_part` instead of the
/// message's `content` array. Older installs keep the array path below.
fn part_text(conn: &Connection, message_id: &str, sid: &str) -> Option<String> {
    let by_message: Option<String> = conn
        .query_row(
            "SELECT data FROM session_part WHERE message_id=?1 AND type='text'
             ORDER BY ordinal DESC LIMIT 1",
            [message_id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten();
    if let Some(text) = by_message.and_then(|d| text_from_part_data(&d)) {
        return Some(text);
    }
    // Some writes carry only the session id; use the newest text part then.
    conn.query_row(
        "SELECT data FROM session_part WHERE session_id=?1 AND type='text'
         ORDER BY time_created DESC, ordinal DESC LIMIT 1",
        [sid],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .and_then(|d| text_from_part_data(&d))
}

// Public so callers can fetch timestamp+text in one query instead of two.
pub fn last_assistant(conn: &Connection, sid: &str) -> (i64, String) {
    let row: Option<(String, i64, String)> = conn
        .query_row(
            "SELECT id, time_created, data FROM session_message
             WHERE session_id=?1 AND type='assistant'
             ORDER BY seq DESC LIMIT 1",
            [sid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .unwrap_or(None);
    match row {
        None => (0, String::new()),
        Some((message_id, ts, data)) => {
            let legacy = serde_json::from_str::<serde_json::Value>(&data)
                .ok()
                .and_then(|v| v.get("content").and_then(|c| c.as_array()).cloned())
                .and_then(|parts| {
                    parts
                        .iter()
                        .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .next_back()
                        .map(str::to_string)
                });
            let text = legacy
                .filter(|t| !t.trim().is_empty())
                .or_else(|| part_text(conn, &message_id, sid))
                .unwrap_or_default();
            (ts, text.trim().to_string())
        }
    }
}

pub fn session_model(conn: &Connection, sid: &str) -> String {
    let raw: Option<String> = conn
        .query_row("SELECT model FROM session_v2 WHERE id=?1", [sid], |r| {
            r.get(0)
        })
        .optional()
        .unwrap_or(None);
    raw.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("id").and_then(|x| x.as_str()).map(str::to_string))
        .unwrap_or_default()
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
    conn.query_row("SELECT title FROM session_v2 WHERE id=?1", [sid], |r| {
        r.get::<_, Option<String>>(0)
    })
    .optional()
    .unwrap_or(None)
    .and_then(|title| title)
    .unwrap_or_default()
}

pub fn last_idle_ts(conn: &Connection, sid: &str) -> i64 {
    conn.query_row(
        "SELECT time_created FROM session_message WHERE session_id=?1 AND type='idle'
         ORDER BY seq DESC LIMIT 1",
        [sid],
        |r| r.get::<_, i64>(0),
    )
    .optional()
    .unwrap_or(None)
    .unwrap_or(0)
}

/// True when an assistant message is waiting on the operator's answer.
/// Assistant text often carries a trailing newline, so trim before checking.
pub fn is_question(text: &str) -> bool {
    let t = text.trim_end();
    t.ends_with('?') || t.ends_with('？')
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
    };
    let sess = state
        .sessions
        .iter()
        .find(|s| s.id == sid)
        .unwrap_or(&fallback);
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
    if state.away_mode && is_question(&text) {
        return Status::Waiting;
    }
    if sess.prompted > 0 && ast <= sess.prompted {
        return Status::Stalled;
    }
    if is_question(&text) {
        return Status::Waiting;
    }
    if ast == 0 {
        return Status::Stalled;
    }
    Status::Due
}
