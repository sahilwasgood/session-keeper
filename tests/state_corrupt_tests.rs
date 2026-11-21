use session_keeper_rust::state::{states_differ, Session, State};
use std::path::PathBuf;

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sk9-corrupt-{}-{}", std::process::id(), tag));
    let _ = std::fs::create_dir_all(&p);
    p
}

#[test]
fn corrupt_json_returns_defaults_and_keeps_backup() {
    let d = dir("backup");
    let path = d.join("state.json");
    let original = "{not json";
    std::fs::write(&path, original).unwrap();
    let s = State::load_from_path(&path);
    assert!(s.sessions.is_empty());
    assert!(s.enabled);
    let backups: Vec<_> = std::fs::read_dir(&d)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("state.json.corrupt-"))
        .collect();
    assert_eq!(backups.len(), 1, "exactly one corrupt backup expected");
    assert_eq!(std::fs::read_to_string(backups[0].path()).unwrap(), original);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn minimal_session_object_gets_serde_defaults() {
    let d = dir("minimal");
    let path = d.join("state.json");
    std::fs::write(&path, r#"{"sessions":[{"id":"ses_min"}]}"#).unwrap();
    let s = State::load_from_path(&path);
    assert_eq!(s.sessions.len(), 1);
    let sess = &s.sessions[0];
    assert_eq!(sess.id, "ses_min");
    assert!(sess.enabled);
    assert_eq!(sess.prompt, None);
    assert_eq!(sess.handled, 0);
    assert_eq!(sess.nudges, 0);
    assert_eq!(sess.idle_streak, 0);
    assert_eq!(sess.last_action, "");
    assert_eq!(sess.last_status, "");
    assert_eq!(sess.last_hash, None);
    assert_eq!(s.default_prompt, "continue, dont stop");
    assert_eq!(s.min_gap, 10);
    assert_eq!(s.max_nudges_per_hour, 30);
    assert_eq!(s.alert_cooldown_s, 300);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn empty_session_ids_are_dropped_on_load() {
    let d = dir("empty-id");
    let path = d.join("state.json");
    std::fs::write(&path, r#"{"sessions":[{"id":""},{"id":"ses_ok"}]}"#).unwrap();
    let s = State::load_from_path(&path);
    assert_eq!(s.sessions.len(), 1);
    assert_eq!(s.sessions[0].id, "ses_ok");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn save_creates_missing_parent_directories() {
    let d = dir("parents");
    let path = d.join("a/b/c/state.json");
    let st = State::default();
    st.save_to_path(&path).unwrap();
    assert!(path.exists());
    let loaded = State::load_from_path(&path);
    assert_eq!(loaded.default_prompt, st.default_prompt);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn save_overwrites_and_leaves_no_tmp_file() {
    let d = dir("tmp");
    let path = d.join("state.json");
    std::fs::write(&path, "old content").unwrap();
    let mut st = State::default();
    st.min_gap = 33;
    st.save_to_path(&path).unwrap();
    let loaded = State::load_from_path(&path);
    assert_eq!(loaded.min_gap, 33);
    assert!(!path.with_extension("tmp").exists(), "tmp file must be renamed away");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn unknown_extra_fields_are_ignored() {
    let d = dir("unknown");
    let path = d.join("state.json");
    std::fs::write(&path, r#"{"bogus":123,"min_gap":7,"sessions":[]}"#).unwrap();
    let s = State::load_from_path(&path);
    assert_eq!(s.min_gap, 7);
    assert!(s.sessions.is_empty());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn states_differ_detects_nested_session_change() {
    let mut a = State::default();
    a.sessions.push(Session { id: "ses_a".to_string(), nudges: 1, ..Default::default() });
    let mut b = a.clone();
    assert!(!states_differ(&a, &b));
    b.sessions[0].nudges = 2;
    assert!(states_differ(&a, &b));
    assert!(states_differ(&b, &a));
    std::fs::remove_dir_all(dir("unused")).ok();
}
