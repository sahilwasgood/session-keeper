use session_keeper_rust::ui::prompt_display;

#[test]
fn prompt_display_uses_default_when_unset() {
    assert_eq!(prompt_display(None::<&str>, "continue, dont stop"), "default: continue, dont stop");
}

#[test]
fn prompt_display_shows_custom_prompt() {
    assert_eq!(prompt_display(Some("keep going"), "continue, dont stop"), "keep going");
}

#[test]
fn prompt_display_ignores_blank_custom_prompt() {
    assert_eq!(prompt_display(Some("   "), "continue"), "default: continue");
}

#[test]
fn filter_matches_id_or_title() {
    assert!(session_keeper_rust::ui::matches_filter("ses_abc123", "tg", ""));
    assert!(session_keeper_rust::ui::matches_filter("ses_abc123", "tg", "TG"));
    assert!(session_keeper_rust::ui::matches_filter("ses_abc123", "tg", "abc"));
    assert!(!session_keeper_rust::ui::matches_filter("ses_abc123", "tg", "zzz"));
}

#[test]
fn attention_and_priority() {
    use session_keeper_rust::db::Status;
    use session_keeper_rust::ui::{needs_attention, status_priority};
    assert!(needs_attention(&Status::Due));
    assert!(needs_attention(&Status::Stalled));
    assert!(!needs_attention(&Status::Clean));
    assert!(status_priority(&Status::Due) < status_priority(&Status::Clean));
}
