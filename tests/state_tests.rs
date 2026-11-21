use session_keeper_rust::state::State;
use std::path::Path;

#[test]
fn missing_state_uses_defaults() {
    let s = State::load_from_path(Path::new("/tmp/nonexistent-sk-state.json"));
    assert!(s.enabled);
    assert_eq!(s.default_prompt, "continue, dont stop");
    assert_eq!(s.min_gap, 10);
    assert_eq!(s.max_nudges_per_hour, 30);
    assert!(!s.away_mode);
    assert!(s.sessions.is_empty());
}

#[test]
fn save_then_load_preserves_session_fields() {
    let mut s = State::default();
    s.tick_interval_s = 15;
    s.global_nudge_cap = 12;
    s.alert_cooldown_s = 600;
    s.persist_dry_run = true;
    s.persist_notifications = false;
    s.sessions.push(session_keeper_rust::state::Session {
        id: "ses_test".to_string(),
        enabled: true,
        prompt: Some("go on".to_string()),
        handled: 1000,
        prompted: 2000,
        last_nudge: 3000,
        nudges: 4,
        question_alert_ms: 111,
        stalled_alert_ms: 222,
        last_action: "nudged".to_string(), idle_streak: 0, last_hash: None, last_status: String::new(), last_alert_ms: 0,
    });
    let path = Path::new("/tmp/sk-roundtrip-state.json");
    s.save_to_path(path).unwrap();
    let loaded = State::load_from_path(path);
    assert_eq!(loaded.sessions.len(), 1);
    assert_eq!(loaded.sessions[0].id, "ses_test");
    assert_eq!(loaded.sessions[0].prompt.as_deref(), Some("go on"));
    assert_eq!(loaded.sessions[0].nudges, 4);
    assert_eq!(loaded.sessions[0].question_alert_ms, 111);
    assert_eq!(loaded.sessions[0].stalled_alert_ms, 222);
    assert_eq!(loaded.tick_interval_s, 15);
    assert_eq!(loaded.global_nudge_cap, 12);
    assert_eq!(loaded.alert_cooldown_s, 600);
    assert!(loaded.persist_dry_run);
    assert!(!loaded.persist_notifications);
    std::fs::remove_file(path).ok();
}

#[test]
fn states_differ_detects_changes() {
    let a = session_keeper_rust::state::State::default();
    let mut b = a.clone();
    assert!(!session_keeper_rust::state::states_differ(&a, &b));
    b.min_gap = 99;
    assert!(session_keeper_rust::state::states_differ(&a, &b));
}
