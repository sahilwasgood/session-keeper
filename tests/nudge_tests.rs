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
        last_action: String::new(), idle_streak: 0, last_hash: None, last_status: String::new(), last_alert_ms: 0,
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
