use session_keeper_rust::db::Status;
use session_keeper_rust::ui::{short_id, status_class, status_text, summary_line};

#[test]
fn status_text_is_short_and_human() {
    assert_eq!(status_text(&Status::Due), "due");
    assert_eq!(status_text(&Status::Nudged), "nudged");
    assert_eq!(status_text(&Status::Waiting), "waiting on question");
    assert_eq!(status_text(&Status::Stalled), "stalled");
    assert_eq!(status_text(&Status::Clean), "working");
}

#[test]
fn status_class_maps_to_css_badge() {
    assert_eq!(status_class(&Status::Due), "badge-due");
    assert_eq!(status_class(&Status::Nudged), "badge-nudged");
    assert_eq!(status_class(&Status::Waiting), "badge-waiting");
    assert_eq!(status_class(&Status::Stalled), "badge-stalled");
    assert_eq!(status_class(&Status::Clean), "badge-idle");
}

#[test]
fn short_id_keeps_the_tail() {
    assert_eq!(short_id("ses_e9c3a7de0ffeZmK488xq4jYSCf"), "K488xq4jYSCf");
    assert_eq!(short_id("short"), "short");
}

#[test]
fn summary_line_reports_actions() {
    assert_eq!(summary_line("12:34:56", 2, 1), "Last check 12:34:56   nudged 2   held 1");
}
