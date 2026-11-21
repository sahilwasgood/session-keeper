use session_keeper_rust::cli::{parse_args, Mode};

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn default_mode_is_gui() {
    let o = parse_args(&args(&["session-keeper-rust"]));
    assert_eq!(o.mode, Mode::Gui);
    assert!(!o.dry_run);
}

#[test]
fn check_now_mode() {
    let o = parse_args(&args(&["session-keeper-rust", "--check-now"]));
    assert_eq!(o.mode, Mode::CheckNow);
}

#[test]
fn status_mode_with_dry_run() {
    let o = parse_args(&args(&["session-keeper-rust", "--status", "--dry-run"]));
    assert_eq!(o.mode, Mode::Status);
    assert!(o.dry_run);
}

#[test]
fn help_mode_for_unknown_flag() {
    let o = parse_args(&args(&["session-keeper-rust", "--wat"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn kick_mode_requires_session_id() {
    let o = parse_args(&args(&["session-keeper-rust", "--kick", "ses_abc"]));
    assert_eq!(o.mode, Mode::Kick("ses_abc".to_string()));
    let bad = parse_args(&args(&["session-keeper-rust", "--kick"]));
    assert_eq!(bad.mode, Mode::Help);
}

#[test]
fn set_model_mode_reads_three_values() {
    let o = parse_args(&args(&[
        "session-keeper-rust", "--set-model", "ses_abc", "model-x", "provider-y",
    ]));
    assert_eq!(
        o.mode,
        Mode::SetModel("ses_abc".to_string(), "model-x".to_string(), "provider-y".to_string())
    );
}

#[test]
fn models_mode_with_optional_filter() {
    let o = parse_args(&args(&["session-keeper-rust", "--models", "hy4"]));
    assert_eq!(o.mode, Mode::Models(Some("hy4".to_string())));
    let all = parse_args(&args(&["session-keeper-rust", "--models"]));
    assert_eq!(all.mode, Mode::Models(None));
}

#[test]
fn export_audit_mode() {
    let o = parse_args(&args(&["session-keeper-rust", "--export-audit"]));
    assert_eq!(o.mode, Mode::ExportAudit);
}
