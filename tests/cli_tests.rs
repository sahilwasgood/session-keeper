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

#[test]
fn pause_and_resume_modes() {
    assert_eq!(
        parse_args(&args(&["session-keeper-rust", "--pause", "ses_a"])).mode,
        Mode::Pause("ses_a".to_string())
    );
    assert_eq!(
        parse_args(&args(&["session-keeper-rust", "--resume", "ses_a"])).mode,
        Mode::Resume("ses_a".to_string())
    );
}

#[test]
fn set_prompt_mode() {
    let o = parse_args(&args(&[
        "session-keeper-rust", "--set-prompt", "ses_a", "keep working",
    ]));
    assert_eq!(
        o.mode,
        Mode::SetPrompt("ses_a".to_string(), "keep working".to_string())
    );
}

#[test]
fn watch_and_version_modes() {
    assert_eq!(parse_args(&args(&["session-keeper-rust", "--watch"])).mode, Mode::Watch);
    assert_eq!(parse_args(&args(&["session-keeper-rust", "--version"])).mode, Mode::Version);
}

#[test]
fn export_state_mode_with_path() {
    let o = parse_args(&args(&["session-keeper-rust", "--export-state", "/tmp/state.json"]));
    assert_eq!(o.mode, Mode::ExportState("/tmp/state.json".to_string()));
    let bad = parse_args(&args(&["session-keeper-rust", "--export-state"]));
    assert_eq!(bad.mode, Mode::Help);
}

#[test]
fn json_flag_for_status() {
    let o = parse_args(&args(&["session-keeper-rust", "--status", "--json"]));
    assert_eq!(o.mode, Mode::Status);
    assert!(o.json);
}

#[test]
fn import_state_mode_with_path() {
    let o = parse_args(&args(&["session-keeper-rust", "--import-state", "/tmp/state.json"]));
    assert_eq!(o.mode, Mode::ImportState("/tmp/state.json".to_string()));
    let bad = parse_args(&args(&["session-keeper-rust", "--import-state"]));
    assert_eq!(bad.mode, Mode::Help);
}

#[test]
fn explicit_dry_run_is_tracked() {
    let o = parse_args(&args(&["session-keeper-rust", "--kick", "ses_a", "--dry-run"]));
    assert!(o.dry_run);
    assert!(o.explicit_dry);
    let p = parse_args(&args(&["session-keeper-rust", "--status"]));
    assert!(!p.explicit_dry);
}

#[test]
fn kick_does_not_consume_a_flag_as_session_id() {
    let o = parse_args(&args(&["session-keeper-rust", "--kick", "--dry-run"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn set_setting_mode_parses_key_value() {
    let o = parse_args(&args(&[
        "session-keeper-rust", "--set-setting", "min_gap", "25",
    ]));
    assert_eq!(o.mode, Mode::SetSetting("min_gap".to_string(), "25".to_string()));
}

#[test]
fn sessions_mode() {
    assert_eq!(parse_args(&args(&["session-keeper-rust", "--sessions"])).mode, Mode::Sessions);
}

#[test]
fn help_lists_all_implemented_flags() {
    let help = session_keeper_rust::cli::HELP;
    assert!(help.contains("--set-setting"));
    assert!(help.contains("--sessions"));
}
