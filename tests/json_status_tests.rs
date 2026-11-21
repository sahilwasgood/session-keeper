use serde_json::Value;
use session_keeper_rust::state::{Session, State};
use std::path::PathBuf;
use std::process::{Command, Output};

fn dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sk9-json-{}-{}", std::process::id(), tag));
    let _ = std::fs::create_dir_all(&p);
    p
}

fn run(d: &PathBuf, state_json: &str) -> Output {
    let state = d.join("state.json");
    std::fs::write(&state, state_json).unwrap();
    Command::new(env!("CARGO_BIN_EXE_session-keeper-rust"))
        .args(["--status", "--json"])
        .env("SESSION_KEEPER_STATE", &state)
        .env("SESSION_KEEPER_DB", d.join("no-db.db"))
        .env("SESSION_KEEPER_LOCK", d.join("keeper.lock"))
        .output()
        .unwrap()
}

fn fixture() -> State {
    let mut st = State::default();
    st.min_gap = 13;
    st.max_nudges_per_hour = 4;
    st.away_mode = true;
    st.sessions.push(Session {
        id: "ses_json_a".to_string(),
        enabled: true,
        nudges: 9,
        last_nudge: 1234,
        last_action: "nudged".to_string(),
        ..Default::default()
    });
    st.sessions.push(Session {
        id: "ses_json_b".to_string(),
        enabled: false,
        ..Default::default()
    });
    st
}

#[test]
fn json_status_has_expected_schema_and_values() {
    let d = dir("schema");
    let out = run(&d, &serde_json::to_string_pretty(&fixture()).unwrap());
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap();
    let items = parsed.as_array().expect("status output must be a JSON array");
    assert_eq!(items.len(), 2);
    let keys = [
        "id", "enabled", "status", "reason", "model", "nudges", "last_action",
        "last_nudge_ms", "min_gap_s", "session_cap_per_hour", "away_mode",
    ];
    for item in items {
        let obj = item.as_object().expect("each status entry must be an object");
        for key in keys {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
        assert!(obj["id"].is_string());
        assert!(obj["enabled"].is_boolean());
        assert!(obj["status"].is_string());
        assert!(obj["reason"].is_string());
        assert!(obj["model"].is_string());
        assert!(obj["nudges"].is_number());
        assert!(obj["last_action"].is_string());
        assert!(obj["last_nudge_ms"].is_number());
        assert!(obj["min_gap_s"].is_number());
        assert!(obj["session_cap_per_hour"].is_number());
        assert!(obj["away_mode"].is_boolean());
        assert_eq!(obj["min_gap_s"].as_i64(), Some(13));
        assert_eq!(obj["session_cap_per_hour"].as_i64(), Some(4));
        assert_eq!(obj["away_mode"].as_bool(), Some(true));
    }
    assert_eq!(items[0]["id"].as_str(), Some("ses_json_a"));
    assert_eq!(items[0]["enabled"].as_bool(), Some(true));
    assert_eq!(items[0]["nudges"].as_i64(), Some(9));
    assert_eq!(items[0]["last_nudge_ms"].as_i64(), Some(1234));
    assert_eq!(items[0]["last_action"].as_str(), Some("nudged"));
    assert_eq!(items[1]["id"].as_str(), Some("ses_json_b"));
    assert_eq!(items[1]["enabled"].as_bool(), Some(false));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn empty_state_yields_empty_array() {
    let d = dir("empty");
    let mut st = State::default();
    st.sessions.clear();
    let out = run(&d, &serde_json::to_string(&st).unwrap());
    assert!(out.status.success());
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 0);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn malformed_state_yields_empty_array() {
    let d = dir("malformed");
    let out = run(&d, "not json");
    assert!(out.status.success());
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 0);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn json_output_is_a_single_line() {
    let d = dir("oneline");
    let out = run(&d, &serde_json::to_string(&fixture()).unwrap());
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let body = stdout.trim_end_matches('\n');
    assert!(!body.contains('\n'), "compact JSON must not be pretty printed");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn check_now_json_is_valid_with_expected_shape() {
    let d = dir("checknow");
    let state = d.join("state.json");
    std::fs::write(&state, r#"{"sessions":[]}"#).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_session-keeper-rust"))
        .args(["--check-now", "--json", "--dry-run"])
        .env("SESSION_KEEPER_STATE", &state)
        .env("SESSION_KEEPER_DB", d.join("no-db.db"))
        .env("SESSION_KEEPER_DRIVE", &d)
        .env("SESSION_KEEPER_LOCK", d.join("keeper.lock"))
        .env("SESSION_KEEPER_API", "http://127.0.0.1:1")
        .output().unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(parsed.get("summary").and_then(|v| v.as_str()).is_some());
    assert!(parsed.get("cap_remaining").and_then(|v| v.as_i64()).is_some());
    assert!(parsed.get("views").and_then(|v| v.as_array()).is_some());
    std::fs::remove_dir_all(&d).ok();
}
