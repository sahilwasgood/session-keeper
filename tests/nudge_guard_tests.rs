use session_keeper_rust::db::Status;
use session_keeper_rust::nudge::{decide_with_history, Decision};
use session_keeper_rust::safety::{fingerprint, should_nudge_after_debounce};
use session_keeper_rust::state::{Session, State};

fn sess() -> Session {
    Session {
        id: "ses_guard".to_string(),
        enabled: true,
        ..Default::default()
    }
}

#[test]
fn gap_equal_to_min_gap_nudges() {
    let mut st = State::default();
    st.min_gap = 10;
    let mut s = sess();
    s.last_nudge = 20_000; // now 30_000 -> exactly 10s
    assert_eq!(
        decide_with_history(&st, &s, &Status::Due, 30_000, 0),
        Decision::Nudge
    );
}

#[test]
fn gap_one_second_less_holds() {
    let mut st = State::default();
    st.min_gap = 10;
    let mut s = sess();
    s.last_nudge = 20_001; // now 30_000 -> 9s
    assert_eq!(
        decide_with_history(&st, &s, &Status::Due, 30_000, 0),
        Decision::Hold
    );
}

#[test]
fn zero_last_nudge_skips_gap_check() {
    let st = State::default();
    let s = sess();
    assert_eq!(s.last_nudge, 0);
    assert_eq!(
        decide_with_history(&st, &s, &Status::Due, 30_000, 0),
        Decision::Nudge
    );
}

#[test]
fn future_last_nudge_holds() {
    let st = State::default();
    let mut s = sess();
    s.last_nudge = 40_000; // now 30_000 -> negative gap
    assert_eq!(
        decide_with_history(&st, &s, &Status::Due, 30_000, 0),
        Decision::Hold
    );
}

#[test]
fn per_session_cap_of_one() {
    let mut st = State::default();
    st.max_nudges_per_hour = 1;
    assert_eq!(
        decide_with_history(&st, &sess(), &Status::Due, 1_000, 1),
        Decision::Hold
    );
    assert_eq!(
        decide_with_history(&st, &sess(), &Status::Due, 1_000, 0),
        Decision::Nudge
    );
}

#[test]
fn zero_cap_means_unlimited() {
    let mut st = State::default();
    st.max_nudges_per_hour = 0;
    assert_eq!(
        decide_with_history(&st, &sess(), &Status::Due, 1_000, 100_000),
        Decision::Nudge
    );
}

#[test]
fn globally_disabled_state_skips() {
    let mut st = State::default();
    st.enabled = false;
    assert_eq!(
        decide_with_history(&st, &sess(), &Status::Due, 1_000, 0),
        Decision::Skip
    );
}

#[test]
fn disabled_session_skips_even_when_due() {
    let st = State::default();
    let mut s = sess();
    s.enabled = false;
    assert_eq!(
        decide_with_history(&st, &s, &Status::Due, 1_000, 0),
        Decision::Skip
    );
}

#[test]
fn every_non_due_status_holds() {
    let st = State::default();
    for status in [
        Status::Clean,
        Status::Nudged,
        Status::Waiting,
        Status::Stalled,
    ] {
        assert_eq!(
            decide_with_history(&st, &sess(), &status, 1_000, 0),
            Decision::Hold,
            "{:?}",
            status
        );
    }
}

#[test]
fn away_mode_does_not_override_non_due_hold() {
    let mut st = State::default();
    st.away_mode = true;
    assert_eq!(
        decide_with_history(&st, &sess(), &Status::Waiting, 1_000, 0),
        Decision::Hold
    );
}

#[test]
fn debounce_boundaries() {
    assert!(!should_nudge_after_debounce(-1));
    assert!(!should_nudge_after_debounce(0));
    assert!(!should_nudge_after_debounce(1));
    assert!(should_nudge_after_debounce(2));
    assert!(should_nudge_after_debounce(3));
}

#[test]
fn fingerprint_backs_loop_detection() {
    assert_eq!(fingerprint("same output"), fingerprint("same output"));
    assert_ne!(fingerprint("same output"), fingerprint("same output "));
    assert_ne!(fingerprint(""), fingerprint(" "));
}
