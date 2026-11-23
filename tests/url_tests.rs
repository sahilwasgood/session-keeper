use session_keeper::nudge::prompt_url;

#[test]
fn prompt_url_uses_api_prefix() {
    assert_eq!(
        prompt_url("http://127.0.0.1:49374", "ses_abc"),
        "http://127.0.0.1:49374/api/session/ses_abc/prompt"
    );
}

#[test]
fn prompt_url_handles_trailing_slash() {
    assert_eq!(
        prompt_url("http://127.0.0.1:49374/", "ses_abc"),
        "http://127.0.0.1:49374/api/session/ses_abc/prompt"
    );
}
