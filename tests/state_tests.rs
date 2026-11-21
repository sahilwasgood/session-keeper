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
        missing_alert_ms: 333,
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
    assert_eq!(loaded.sessions[0].missing_alert_ms, 333);
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

#[test]
fn corrupt_state_is_backed_up() {
    let path = std::path::Path::new("/tmp/sk-corrupt-state.json");
    std::fs::write(path, "{not json").unwrap();
    let s = session_keeper_rust::state::State::load_from_path(path);
    assert!(s.sessions.is_empty());
    let backed: Vec<_> = std::fs::read_dir("/tmp")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("sk-corrupt-state.json.corrupt-"))
        .collect();
    assert!(!backed.is_empty(), "corrupt state backup missing");
    std::fs::remove_file(path).ok();
    for b in backed { std::fs::remove_file(b.path()).ok(); }
}

#[test]
fn tick_merge_preserves_user_edits() {
    use session_keeper_rust::state::{apply_tick_fields, GlobalTickFields, TickFields};
    let mut st = session_keeper_rust::state::State::default();
    st.sessions.push(session_keeper_rust::state::Session {
        id: "ses_a".to_string(),
        enabled: false,
        prompt: Some("my prompt".to_string()),
        nudges: 7,
        ..Default::default()
    });
    let before_nudges = st.sessions[0].nudges;
    let updates = vec![TickFields {
        id: "ses_a".to_string(),
        handled: 100,
        prompted: 100,
        last_nudge: 100,
        nudges: before_nudges + 1,
        idle_streak: 0,
        last_hash: None,
        last_action: "nudged".to_string(),
        last_status: "Due".to_string(),
        last_alert_ms: 0,
        question_alert_ms: 0,
        stalled_alert_ms: 0,
        missing_alert_ms: 333,
    }];
    apply_tick_fields(&mut st, &updates, GlobalTickFields {
        last_tick_ms: 100,
        cap_alert_ms: 0,
        missing_alert_ms: 0,
    });
    assert_eq!(st.sessions[0].nudges, 8);
    assert!(!st.sessions[0].enabled, "user pause must survive tick merge");
    assert_eq!(st.sessions[0].prompt.as_deref(), Some("my prompt"));
    assert_eq!(st.last_tick_ms, 100);
}
