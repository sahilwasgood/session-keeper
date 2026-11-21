use crate::db::Status;

pub fn status_text(status: &Status) -> &'static str {
    match status {
        Status::Due => "due",
        Status::Nudged => "nudged",
        Status::Waiting => "waiting on question",
        Status::Stalled => "stalled",
        Status::Clean => "idle",
    }
}
