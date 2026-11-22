use session_keeper_rust::state::{Session, State};
use std::path::PathBuf;
use std::process::{Command, Output};

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sk9-sync-{}-{}", std::process::id(), tag));
    let _ = std::fs::create_dir_all(&p);
    p
}

fn fixture() -> State {
    let mut st = State::default();
    st.default_prompt = "sync default".to_string();
    st.min_gap = 17;
    st.global_nudge_cap = 23;
    st.sessions.push(Session {
        id: "ses_sync_a".to_string(),
        prompt: Some("go sync".to_string()),
        nudges: 5,
        ..Default::default()
    });
    st.sessions.push(Session {
        id: "ses_sync_b".to_string(),
        ..Default::default()
    });
    st
}

fn assert_same(a: &State, b: &State) {
    assert_eq!(a.default_prompt, b.default_prompt);
    assert_eq!(a.min_gap, b.min_gap);
    assert_eq!(a.global_nudge_cap, b.global_nudge_cap);
    assert_eq!(a.sessions.len(), b.sessions.len());
    for (x, y) in a.sessions.iter().zip(b.sessions.iter()) {
        assert_eq!(x.id, y.id);
        assert_eq!(x.prompt, y.prompt);
        assert_eq!(x.nudges, y.nudges);
    }
}

fn run(args: &[&str], state: &PathBuf, lock: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_session-keeper-rust"))
        .args(args)
        .env("SESSION_KEEPER_STATE", state)
        .env("SESSION_KEEPER_LOCK", lock)
        .env("SESSION_KEEPER_DB", lock.with_file_name("no-db.db"))
        .output()
        .unwrap()
}

#[test]
fn export_writes_current_state_to_backup_path() {
    let d = dir("export");
    let state = d.join("state.json");
    let backup = d.join("backup.json");
    let lock = d.join("keeper.lock");
    let original = fixture();
    std::fs::write(&state, serde_json::to_string_pretty(&original).unwrap()).unwrap();
    let out = run(&["--export-state", backup.to_str().unwrap()], &state, &lock);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let exported: State = serde_json::from_slice(&std::fs::read(&backup).unwrap()).unwrap();
    assert_same(&original, &exported);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn import_restores_state_from_backup_path() {
    let d = dir("import");
    let state = d.join("state.json");
    let backup = d.join("backup.json");
    let lock = d.join("keeper.lock");
    let original = fixture();
    std::fs::write(&backup, serde_json::to_string_pretty(&original).unwrap()).unwrap();

    let mut changed = fixture();
    changed.default_prompt = "overwritten".to_string();
    changed.min_gap = 1;
    changed.sessions.clear();
    std::fs::write(&state, serde_json::to_string_pretty(&changed).unwrap()).unwrap();

    let out = run(&["--import-state", backup.to_str().unwrap()], &state, &lock);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let restored: State = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    assert_same(&original, &restored);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn invalid_import_leaves_state_untouched() {
    let d = dir("invalid");
    let state = d.join("state.json");
    let bad = d.join("bad.json");
    let lock = d.join("keeper.lock");
    let original = fixture();
    let bytes = serde_json::to_string_pretty(&original).unwrap();
    std::fs::write(&state, &bytes).unwrap();
    std::fs::write(&bad, "not json").unwrap();

    let out = run(&["--import-state", bad.to_str().unwrap()], &state, &lock);
    assert!(!out.status.success(), "invalid import must fail");
    assert_eq!(std::fs::read_to_string(&state).unwrap(), bytes);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn export_to_missing_parent_directory_fails() {
    let d = dir("missing-parent");
    let state = d.join("state.json");
    let lock = d.join("keeper.lock");
    std::fs::write(&state, serde_json::to_string(&fixture()).unwrap()).unwrap();
    let target = d.join("no-such-dir/backup.json");
    let out = run(&["--export-state", target.to_str().unwrap()], &state, &lock);
    assert!(
        !out.status.success(),
        "export into a missing directory must fail"
    );
    std::fs::remove_dir_all(&d).ok();
}
