use session_keeper_rust::state::{
    apply_tick_fields, states_differ, GlobalTickFields, Session, State, TickFields,
};

fn tf(id: &str) -> TickFields {
    TickFields {
        id: id.to_string(),
        handled: 0,
        prompted: 0,
        last_nudge: 0,
        nudges: 0,
        idle_streak: 0,
        last_hash: None,
        last_action: String::new(),
        last_status: String::new(),
        last_alert_ms: 0,
        question_alert_ms: 0,
        stalled_alert_ms: 0,
        missing_alert_ms: 0, failure_alert_ms: 0,
    }
}

fn globals(last_tick_ms: i64) -> GlobalTickFields {
    GlobalTickFields { last_tick_ms, cap_alert_ms: 0, missing_alert_ms: 0 }
}

fn path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("sk9-merge-{}-{}", std::process::id(), tag))
}

#[test]
fn empty_updates_still_apply_globals() {
    let mut st = State::default();
    apply_tick_fields(
        &mut st,
        &[],
        GlobalTickFields { last_tick_ms: 999, cap_alert_ms: 11, missing_alert_ms: 22 },
    );
    assert_eq!(st.last_tick_ms, 999);
    assert_eq!(st.cap_alert_ms, 11);
    assert_eq!(st.missing_alert_ms, 22);
}

#[test]
fn unknown_session_ids_are_ignored() {
    let mut st = State::default();
    st.sessions.push(Session { id: "ses_known".to_string(), ..Default::default() });
    let mut u = tf("ses_unknown");
    u.nudges = 7;
    apply_tick_fields(&mut st, &[u], globals(5));
    assert_eq!(st.sessions.len(), 1);
    assert_eq!(st.sessions[0].nudges, 0);
    assert_eq!(st.last_tick_ms, 5);
}

#[test]
fn duplicate_updates_last_one_wins() {
    let mut st = State::default();
    st.sessions.push(Session { id: "ses_a".to_string(), ..Default::default() });
    let mut first = tf("ses_a");
    first.nudges = 1;
    first.last_action = "first".to_string();
    let mut second = tf("ses_a");
    second.nudges = 9;
    second.last_action = "second".to_string();
    apply_tick_fields(&mut st, &[first, second], globals(1));
    assert_eq!(st.sessions[0].nudges, 9);
    assert_eq!(st.sessions[0].last_action, "second");
}

#[test]
fn tick_fields_do_not_overwrite_user_settings() {
    let mut st = State::default();
    st.enabled = false;
    st.default_prompt = "custom default".to_string();
    st.min_gap = 77;
    st.max_nudges_per_hour = 5;
    st.away_mode = true;
    st.global_nudge_cap = 9;
    st.tick_interval_s = 11;
    st.persist_dry_run = true;
    st.persist_notifications = false;
    st.alert_cooldown_s = 42;
    st.sort_by_name = true;
    st.sessions.push(Session {
        id: "ses_a".to_string(),
        enabled: false,
        prompt: Some("my prompt".to_string()),
        ..Default::default()
    });
    let mut u = tf("ses_a");
    u.nudges = 3;
    apply_tick_fields(&mut st, &[u], globals(0));
    assert!(!st.enabled);
    assert_eq!(st.default_prompt, "custom default");
    assert_eq!(st.min_gap, 77);
    assert_eq!(st.max_nudges_per_hour, 5);
    assert!(st.away_mode);
    assert_eq!(st.global_nudge_cap, 9);
    assert_eq!(st.tick_interval_s, 11);
    assert!(st.persist_dry_run);
    assert!(!st.persist_notifications);
    assert_eq!(st.alert_cooldown_s, 42);
    assert!(st.sort_by_name);
    assert!(!st.sessions[0].enabled);
    assert_eq!(st.sessions[0].prompt.as_deref(), Some("my prompt"));
}

#[test]
fn all_tick_fields_are_applied() {
    let mut st = State::default();
    st.sessions.push(Session { id: "ses_a".to_string(), ..Default::default() });
    let mut u = tf("ses_a");
    u.handled = 101;
    u.prompted = 102;
    u.last_nudge = 103;
    u.nudges = 104;
    u.idle_streak = 2;
    u.last_hash = Some("abc".to_string());
    u.last_action = "nudged".to_string();
    u.last_status = "Due".to_string();
    u.last_alert_ms = 105;
    u.question_alert_ms = 106;
    u.stalled_alert_ms = 107;
    u.missing_alert_ms = 108;
    apply_tick_fields(&mut st, &[u], globals(109));
    let s = &st.sessions[0];
    assert_eq!(s.handled, 101);
    assert_eq!(s.prompted, 102);
    assert_eq!(s.last_nudge, 103);
    assert_eq!(s.nudges, 104);
    assert_eq!(s.idle_streak, 2);
    assert_eq!(s.last_hash.as_deref(), Some("abc"));
    assert_eq!(s.last_action, "nudged");
    assert_eq!(s.last_status, "Due");
    assert_eq!(s.last_alert_ms, 105);
    assert_eq!(s.question_alert_ms, 106);
    assert_eq!(s.stalled_alert_ms, 107);
    assert_eq!(s.missing_alert_ms, 108);
    assert_eq!(st.last_tick_ms, 109);
}

#[test]
fn states_differ_is_symmetric_and_sees_nested_changes() {
    let a = State::default();
    let mut b = a.clone();
    assert!(!states_differ(&a, &b));
    assert!(!states_differ(&b, &a));
    b.sessions.push(Session { id: "ses_a".to_string(), nudges: 1, ..Default::default() });
    assert!(states_differ(&a, &b));
    assert!(states_differ(&b, &a));
    let mut c = b.clone();
    c.sessions[0].prompt = Some("x".to_string());
    assert!(states_differ(&b, &c));
}

#[test]
fn concurrent_saves_to_distinct_paths_are_independent() {
    let handles: Vec<_> = (0..8)
        .map(|i| {
            std::thread::spawn(move || {
                let mut st = State::default();
                st.default_prompt = format!("prompt-{}", i);
                st.min_gap = i as i64;
                st.sessions.push(Session {
                    id: format!("ses_{}", i),
                    nudges: i as i64,
                    ..Default::default()
                });
                let p = path(&format!("concurrent-{}", i));
                st.save_to_path(&p).unwrap();
                (p, st)
            })
        })
        .collect();
    for (i, h) in handles.into_iter().enumerate() {
        let (p, written) = h.join().unwrap();
        let loaded = State::load_from_path(&p);
        assert_eq!(loaded.default_prompt, format!("prompt-{}", i));
        assert_eq!(loaded.min_gap, i as i64);
        assert_eq!(loaded.sessions[0].id, format!("ses_{}", i));
        assert_eq!(loaded.sessions[0].nudges, i as i64);
        assert_eq!(written.sessions[0].nudges, i as i64);
        let _ = std::fs::remove_file(&p);
    }
}
