use session_keeper_rust::db::Status;
use session_keeper_rust::ui::status_text;

#[test]
fn status_text_is_short_and_human() {
    assert_eq!(status_text(&Status::Due), "due");
    assert_eq!(status_text(&Status::Nudged), "nudged");
    assert_eq!(status_text(&Status::Waiting), "waiting on question");
    assert_eq!(status_text(&Status::Stalled), "stalled");
    assert_eq!(status_text(&Status::Clean), "idle");
}
