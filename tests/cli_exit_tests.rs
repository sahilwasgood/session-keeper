use std::process::Command;

fn env_setup(dir: &std::path::Path) -> Vec<(String, String)> {
    vec![
        (
            "SESSION_KEEPER_STATE".into(),
            dir.join("state.json").display().to_string(),
        ),
        (
            "SESSION_KEEPER_LOCK".into(),
            dir.join("keeper.lock").display().to_string(),
        ),
        (
            "SESSION_KEEPER_LOG".into(),
            dir.join("keeper.log").display().to_string(),
        ),
        (
            "SESSION_KEEPER_HISTORY".into(),
            dir.join("history.jsonl").display().to_string(),
        ),
        ("SESSION_KEEPER_NOTIFY".into(), "0".into()),
    ]
}

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_session-keeper-rust"));
    cmd.args(args);
    for (k, v) in env_setup(dir) {
        cmd.env(k, v);
    }
    cmd.output().expect("run binary")
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("sk-cliexit-{}-{}", std::process::id(), tag));
    let _ = std::fs::create_dir_all(&p);
    p
}

#[test]
fn kick_unknown_session_exits_nonzero() {
    let d = tmp("kick");
    let out = run(&d, &["--kick", "ses_unknown_here"]);
    assert!(
        !out.status.success(),
        "unknown kick must fail, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("not in the watch list"));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn set_model_unknown_session_exits_nonzero_without_api_call() {
    let d = tmp("model");
    let out = run(
        &d,
        &["--set-model", "ses_unknown_here", "model-x", "provider-y"],
    );
    assert!(
        !out.status.success(),
        "unknown set-model must fail, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("not in the watch list"));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn set_setting_updates_time_preferences() {
    let d = tmp("timeprefs");
    let state = d.join("state.json");
    std::fs::write(&state, "{}").unwrap();
    for (key, value) in [
        ("timezone", "+05:30"),
        ("clock_24h", "true"),
        ("date_style", "year_month_day"),
    ] {
        let out = run(&d, &["--set-setting", key, value]);
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    let prefs = &saved["time_prefs"];
    assert_eq!(prefs["timezone"].as_str(), Some("+05:30"));
    assert_eq!(prefs["clock_24h"].as_bool(), Some(true));
    assert_eq!(prefs["date_style"].as_str(), Some("year_month_day"));
    std::fs::remove_dir_all(&d).ok();
}
