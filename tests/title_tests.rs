use rusqlite::Connection;
use session_keeper::db::session_title;

#[test]
fn title_reads_from_session_v2() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE session_v2 (id TEXT PRIMARY KEY, title TEXT);
         INSERT INTO session_v2 VALUES ('ses_a','tg'), ('ses_b', NULL);",
    )
    .unwrap();
    assert_eq!(session_title(&conn, "ses_a"), "tg");
    assert_eq!(session_title(&conn, "ses_b"), "");
    assert_eq!(session_title(&conn, "ses_missing"), "");
}
