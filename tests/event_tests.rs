use session_keeper_rust::events::is_relevant_event_line;

#[test]
fn relevant_event_lines_are_detected() {
    assert!(is_relevant_event_line(r#"data: {"type":"session.idle","data":{}}"#));
    assert!(is_relevant_event_line(r#"data: {"type":"permission.asked","data":{}}"#));
    assert!(is_relevant_event_line(r#"data: {"type":"form.created","data":{}}"#));
}

#[test]
fn unrelated_or_malformed_lines_are_ignored() {
    assert!(!is_relevant_event_line(r#"data: {"type":"server.connected","data":{}}"#));
    assert!(!is_relevant_event_line("event: session.idle"));
    assert!(!is_relevant_event_line("data: not json"));
    assert!(!is_relevant_event_line(""));
}
