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
