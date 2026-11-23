use session_keeper::ui::prompt_display;

#[test]
fn prompt_display_uses_default_when_unset() {
    assert_eq!(
        prompt_display(None::<&str>, "continue, dont stop"),
        "default: continue, dont stop"
    );
}

#[test]
fn prompt_display_shows_custom_prompt() {
    assert_eq!(
        prompt_display(Some("keep going"), "continue, dont stop"),
        "keep going"
    );
}

#[test]
fn prompt_display_ignores_blank_custom_prompt() {
    assert_eq!(prompt_display(Some("   "), "continue"), "default: continue");
}

#[test]
fn filter_matches_id_or_title() {
    assert!(session_keeper::ui::matches_filter("ses_abc123", "tg", ""));
    assert!(session_keeper::ui::matches_filter("ses_abc123", "tg", "TG"));
    assert!(session_keeper::ui::matches_filter(
        "ses_abc123",
        "tg",
        "abc"
    ));
    assert!(!session_keeper::ui::matches_filter(
        "ses_abc123",
        "tg",
        "zzz"
    ));
}

#[test]
fn attention_and_priority() {
    use session_keeper::db::Status;
    use session_keeper::ui::{needs_attention, status_priority};
    assert!(needs_attention(&Status::Due));
    assert!(needs_attention(&Status::Stalled));
    assert!(!needs_attention(&Status::Clean));
    assert!(status_priority(&Status::Due) < status_priority(&Status::Clean));
}

#[test]
fn status_change_detection() {
    assert!(session_keeper::ui::status_changed("working", "waiting"));
    assert!(!session_keeper::ui::status_changed("waiting", "waiting"));
}

#[test]
fn rotate_keeps_recent_lines() {
    let text = (0..3000)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let out = session_keeper::nudge::rotate_text(&text, 2000, 1000);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1000);
    assert_eq!(lines[0], "line 2000");
    assert_eq!(lines[999], "line 2999");
}

#[test]
fn sort_key_by_name_or_priority() {
    use session_keeper::db::Status;
    use session_keeper::ui::sort_key;
    assert_eq!(sort_key(&Status::Due, "tg", "ses_a", false).0, 0);
    assert_eq!(sort_key(&Status::Clean, "tg", "ses_a", false).0, 4);
    assert_eq!(sort_key(&Status::Due, "tg", "ses_a", true).0, 0);
    assert_eq!(sort_key(&Status::Due, "a", "ses_a", true).1, "ases_a");
}

#[test]
fn history_for_sid_filters_and_limits() {
    let text = [
        r#"{"t":1,"sid":"ses_a","kind":"nudge","detail":"x"}"#,
        r#"{"t":2,"sid":"ses_b","kind":"nudge","detail":"y"}"#,
        r#"{"t":3,"sid":"ses_a","kind":"nudge","detail":"z"}"#,
    ]
    .join("\n");
    let lines = session_keeper::ui::history_for_sid(&text, "ses_a", 5);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"z\""));
    assert!(lines[1].contains("\"x\""));
}
