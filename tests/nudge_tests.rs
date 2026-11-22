use session_keeper_rust::db::Status;
use session_keeper_rust::nudge::{decide, decide_with_history, Decision};
use session_keeper_rust::state::{Session, State};

fn sess() -> Session {
    Session {
        id: "ses_test".to_string(),
        enabled: true,
        prompt: None,
        handled: 1000,
        prompted: 0,
        last_nudge: 0,
        nudges: 0,
        last_action: String::new(), idle_streak: 0, last_hash: None, prev_hash: None, last_status: String::new(), last_alert_ms: 0, question_alert_ms: 0, stalled_alert_ms: 0, missing_alert_ms: 0, failure_alert_ms: 0,
    }
}

#[test]
fn min_gap_holds_nudge() {
    let st = State::default();
    let mut s = sess();
    s.last_nudge = 20_000; // now = 25_000 -> 5s gap, min_gap 10
    assert_eq!(decide(&st, &s, &Status::Due, 25_000), Decision::Hold);
}

#[test]
fn hourly_cap_holds_nudge() {
    let st = State::default();
    assert_eq!(
        decide_with_history(&st, &sess(), &Status::Due, 100_000, 30),
        Decision::Hold
    );
}

#[test]
fn away_mode_question_is_held() {
    let mut st = State::default();
    st.away_mode = true;
    assert_eq!(decide(&st, &sess(), &Status::Waiting, 100_000), Decision::Hold);
}

#[test]
fn stalled_no_new_output_is_held() {
    let st = State::default();
    assert_eq!(decide(&st, &sess(), &Status::Stalled, 100_000), Decision::Hold);
}

#[test]
fn disabled_session_is_skipped() {
    let st = State::default();
    let mut s = sess();
    s.enabled = false;
    assert_eq!(decide(&st, &s, &Status::Due, 100_000), Decision::Skip);
}

#[test]
fn due_session_is_nudged() {
    let st = State::default();
    assert_eq!(decide(&st, &sess(), &Status::Due, 100_000), Decision::Nudge);
}

#[test]
fn notify_send_availability_is_cached_and_notify_does_not_panic() {
    let a = session_keeper_rust::nudge::notify_send_available();
    let b = session_keeper_rust::nudge::notify_send_available();
    assert_eq!(a, b, "availability check must be stable within a process");
    // Dispatch is best-effort: must never panic whether or not the transport exists.
    let _ = session_keeper_rust::nudge::notify("Session Keeper", "test");
}

#[test]
fn combined_global_count_survives_missing_history() {
    let mut st = State::default();
    let now = 20_000_000i64;
    st.recent_nudges.push(now - 1000);
    st.recent_nudges.push(now - 2000);
    st.recent_nudges.push(now - 4_000_000);
    let count = session_keeper_rust::nudge::nudges_in_last_hour_combined(now, &st);
    assert!(count >= 2, "in-state nudges must count when history is gone: {}", count);
}
