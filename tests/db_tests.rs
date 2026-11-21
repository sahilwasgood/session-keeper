use rusqlite::Connection;
use session_keeper_rust::db::{classify, Status};
use session_keeper_rust::state::{Session, State};

fn session() -> Session {
    Session {
        id: "ses_test".to_string(),
        enabled: true,
        prompt: None,
        handled: 0,
        prompted: 0,
        last_nudge: 0,
        nudges: 0,
        last_action: String::new(),
    }
}

fn fixture(rows: &[(&str, &str, i64, &str)]) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE session_message (
           id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER,
           time_created INTEGER, time_updated INTEGER, data TEXT);",
    )
    .unwrap();
    for (i, (typ, _sid, ts, data)) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO session_message (id,session_id,type,seq,time_created,time_updated,data)
             VALUES (?1,'ses_test',?2,?3,?4,?4,?5)",
            rusqlite::params![format!("msg_{i}"), typ, i as i64, ts, data],
        )
        .unwrap();
    }
    conn
}

fn assistant(text: &str) -> String {
    format!(r#"{{"content":[{{"type":"text","text":{}}}]}}"#, serde_json::to_string(text).unwrap())
}

#[test]
fn assistant_question_is_waiting() {
    let conn = fixture(&[
        ("assistant", "ses_test", 900, &assistant("Which file should I edit?")),
        ("idle", "ses_test", 1000, r#"{}"#),
    ]);
    assert_eq!(classify(&conn, "ses_test", &State::default()), Status::Waiting);
}

#[test]
fn idle_without_assistant_is_stalled() {
    let conn = fixture(&[("idle", "ses_test", 1000, r#"{}"#)]);
    assert_eq!(classify(&conn, "ses_test", &State::default()), Status::Stalled);
}

#[test]
fn idle_with_assistant_is_due() {
    let conn = fixture(&[
        ("assistant", "ses_test", 900, &assistant("done")),
        ("idle", "ses_test", 1000, r#"{}"#),
    ]);
    assert_eq!(classify(&conn, "ses_test", &State::default()), Status::Due);
}

#[test]
fn handled_idle_is_nudged() {
    let conn = fixture(&[("idle", "ses_test", 1000, r#"{}"#)]);
    let mut st = State::default();
    let mut s = session();
    s.handled = 1000;
    st.sessions.push(s);
    assert_eq!(classify(&conn, "ses_test", &st), Status::Nudged);
}

#[test]
fn no_idle_event_is_clean() {
    let conn = fixture(&[("assistant", "ses_test", 900, &assistant("working"))]);
    assert_eq!(classify(&conn, "ses_test", &State::default()), Status::Clean);
}
