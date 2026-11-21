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
