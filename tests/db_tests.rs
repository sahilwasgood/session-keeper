use rusqlite::Connection;
use session_keeper::db::{classify, is_question, Status};
use session_keeper::state::{Session, State};

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
    format!(
        r#"{{"content":[{{"type":"text","text":{}}}]}}"#,
        serde_json::to_string(text).unwrap()
    )
}

#[test]
fn assistant_question_is_waiting() {
    let conn = fixture(&[
        (
            "assistant",
            "ses_test",
            900,
            &assistant("Which file should I edit?"),
        ),
        ("idle", "ses_test", 1000, r#"{}"#),
    ]);
    assert_eq!(
        classify(&conn, "ses_test", &State::default()),
        Status::Waiting
    );
}

#[test]
fn question_detection_trims_and_handles_fullwidth_mark() {
    assert!(is_question("Which file?"));
    assert!(is_question("Which file?\n"));
    assert!(is_question("Which file？"));
    assert!(is_question("Which file？  \n"));
    assert!(!is_question("Which file"));
    assert!(!is_question(""));
    assert!(!is_question("? but then more text"));
}

#[test]
fn question_with_trailing_newline_is_waiting() {
    let conn = fixture(&[
        (
            "assistant",
            "ses_test",
            900,
            &assistant("Which file should I edit?\n"),
        ),
        ("idle", "ses_test", 1000, r#"{}"#),
    ]);
    assert_eq!(
        classify(&conn, "ses_test", &State::default()),
        Status::Waiting
    );
}

#[test]
fn idle_without_assistant_is_stalled() {
    let conn = fixture(&[("idle", "ses_test", 1000, r#"{}"#)]);
    assert_eq!(
        classify(&conn, "ses_test", &State::default()),
        Status::Stalled
    );
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
    assert_eq!(
        classify(&conn, "ses_test", &State::default()),
        Status::Clean
    );
}

#[test]
fn assistant_text_falls_back_to_v24_part_rows() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE session_message (
           id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER,
           time_created INTEGER, time_updated INTEGER, data TEXT);
         CREATE TABLE session_part (
           message_id TEXT, session_id TEXT, ordinal INTEGER, type TEXT,
           call_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_message VALUES ('msg1','ses_test','assistant',1,1000,1000,'{\"content\":[]}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_part VALUES ('msg1','ses_test',0,'reasoning','',1000,1000,'{\"type\":\"reasoning\",\"text\":\"internal\"}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_part VALUES ('msg1','ses_test',1,'text','',1000,1000,'{\"type\":\"text\",\"text\":\"Which file should I edit?\"}')",
        [],
    )
    .unwrap();
    let text = session_keeper::db::last_assistant_text(&conn, "ses_test");
    assert_eq!(text, "Which file should I edit?");
}

#[test]
fn assistant_text_prefers_legacy_content_when_present() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE session_message (
           id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER,
           time_created INTEGER, time_updated INTEGER, data TEXT);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO session_message VALUES ('msg2','ses_test','assistant',1,1000,1000,'{\"content\":[{\"type\":\"text\",\"text\":\"legacy answer\"}]}')",
        [],
    )
    .unwrap();
    assert_eq!(
        session_keeper::db::last_assistant_text(&conn, "ses_test"),
        "legacy answer"
    );
}
