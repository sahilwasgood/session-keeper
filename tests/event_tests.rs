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

#[test]
fn event_url_uses_repeated_type_filters() {
    let url = session_keeper_rust::events::event_url("http://localhost:49374/");
    assert_eq!(
        url,
        "http://localhost:49374/api/event?types=session.idle&types=session.status&types=permission.asked&types=form.created"
    );
}
