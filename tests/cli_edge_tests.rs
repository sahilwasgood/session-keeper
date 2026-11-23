use session_keeper::cli::{parse_args, Mode, HELP};

fn a(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn json_flag_is_recorded_without_status() {
    let o = parse_args(&a(&["sk", "--json"]));
    assert_eq!(o.mode, Mode::Gui);
    assert!(o.json);
    assert!(!o.explicit_dry);
}

#[test]
fn dry_run_before_mode_is_preserved() {
    let o = parse_args(&a(&["sk", "--dry-run", "--status"]));
    assert_eq!(o.mode, Mode::Status);
    assert!(o.dry_run);
    assert!(o.explicit_dry);
}

#[test]
fn short_help_stops_parsing() {
    let o = parse_args(&a(&["sk", "-h"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn help_after_a_mode_wins() {
    let o = parse_args(&a(&["sk", "--status", "--help"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn help_before_a_mode_wins() {
    let o = parse_args(&a(&["sk", "--help", "--status"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn unknown_flag_after_a_valid_mode_falls_back_to_help() {
    let o = parse_args(&a(&["sk", "--status", "--wat"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn set_prompt_with_one_value_is_help() {
    let o = parse_args(&a(&["sk", "--set-prompt", "ses_a"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn set_prompt_consumes_flag_looking_text() {
    let o = parse_args(&a(&["sk", "--set-prompt", "ses_a", "--dry-run"]));
    assert_eq!(
        o.mode,
        Mode::SetPrompt("ses_a".to_string(), "--dry-run".to_string())
    );
    assert!(!o.dry_run, "prompt text must not be parsed as a flag");
}

#[test]
fn set_model_with_too_few_values_is_help() {
    let o = parse_args(&a(&["sk", "--set-model", "a", "b"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn flag_after_set_model_values_is_still_parsed() {
    let o = parse_args(&a(&["sk", "--set-model", "a", "b", "c", "--dry-run"]));
    assert_eq!(
        o.mode,
        Mode::SetModel("a".to_string(), "b".to_string(), "c".to_string())
    );
    assert!(o.dry_run);
}

#[test]
fn pause_without_value_is_help() {
    assert_eq!(parse_args(&a(&["sk", "--pause"])).mode, Mode::Help);
}

#[test]
fn resume_consumes_a_flag_looking_value() {
    let o = parse_args(&a(&["sk", "--resume", "--json"]));
    assert_eq!(o.mode, Mode::Resume("--json".to_string()));
    assert!(!o.json);
}

#[test]
fn export_state_with_dash_path_is_help() {
    let o = parse_args(&a(&["sk", "--export-state", "--dry-run"]));
    assert_eq!(o.mode, Mode::Help);
}

#[test]
fn models_with_dash_filter_is_models_all() {
    let o = parse_args(&a(&["sk", "--models", "--dry-run"]));
    assert_eq!(o.mode, Mode::Models(None));
    assert!(o.dry_run);
}

#[test]
fn flags_after_models_filter_are_parsed() {
    let o = parse_args(&a(&["sk", "--models", "hy4", "--json"]));
    assert_eq!(o.mode, Mode::Models(Some("hy4".to_string())));
    assert!(o.json);
}

#[test]
fn later_mode_flag_overrides_earlier_one() {
    let o = parse_args(&a(&["sk", "--check-now", "--status", "--watch"]));
    assert_eq!(o.mode, Mode::Watch);
    let p = parse_args(&a(&["sk", "--set-prompt", "ses_a", "go", "--status"]));
    assert_eq!(p.mode, Mode::Status);
}

#[test]
fn repeated_dry_run_remains_explicit() {
    let o = parse_args(&a(&["sk", "--dry-run", "--dry-run", "--status"]));
    assert!(o.dry_run);
    assert!(o.explicit_dry);
}

#[test]
fn help_text_lists_the_documented_modes() {
    for flag in [
        "--check-now",
        "--kick",
        "--export-state",
        "--import-state",
        "--json",
        "--dry-run",
    ] {
        assert!(HELP.contains(flag), "help is missing {}", flag);
    }
}
