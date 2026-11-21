use crate::db::Status;

pub fn status_text(status: &Status) -> &'static str {
    match status {
        Status::Due => "due",
        Status::Nudged => "nudged",
        Status::Waiting => "waiting on question",
        Status::Stalled => "stalled",
        Status::Clean => "working",
    }
}

pub fn status_class(status: &Status) -> &'static str {
    match status {
        Status::Due => "badge-due",
        Status::Nudged => "badge-nudged",
        Status::Waiting => "badge-waiting",
        Status::Stalled => "badge-stalled",
        Status::Clean => "badge-idle",
    }
}

pub fn short_id(id: &str) -> String {
    if id.len() <= 12 {
        return id.to_string();
    }
    id.chars().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect()
}

pub fn prompt_display(prompt: Option<&str>, default_prompt: &str) -> String {
    match prompt {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => format!("default: {}", default_prompt),
    }
}

pub fn summary_line(stamp: &str, nudged: i64, held: i64) -> String {
    format!("Last check {}   nudged {}   held {}", stamp, nudged, held)
}

pub const CSS: &str = r#"
window {
    background-color: #14181d;
    color: #e8edf2;
    font-family: "DejaVu Sans", sans-serif;
    font-size: 14px;
}
headerbar {
    background-color: #1d242c;
    color: #f2f5f8;
    padding: 8px 12px;
    font-size: 16px;
    font-weight: bold;
    min-height: 44px;
}
button {
    background-color: #2a333d;
    color: #f2f5f8;
    border: 1px solid #3d4a57;
    border-radius: 8px;
    padding: 7px 14px;
    font-size: 14px;
}
button:hover { background-color: #364452; }
button:active { background-color: #46607a; }
.suggested-action {
    background-color: #2d7d46;
    border-color: #3aa359;
    color: #ffffff;
    font-weight: bold;
}
.destructive-action { background-color: #7d2d2d; border-color: #a33a3a; }
entry {
    background-color: #0f1318;
    color: #f2f5f8;
    border: 1px solid #3d4a57;
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 14px;
}
.card {
    background-color: #1d242c;
    border: 1px solid #33404c;
    border-radius: 10px;
    padding: 12px;
    margin: 4px 6px;
}
.session-id { font-size: 15px; font-weight: bold; color: #eef3f8; }
.session-meta { font-size: 13px; color: #a9b6c3; }
.badge {
    border-radius: 10px;
    padding: 3px 10px;
    font-size: 13px;
    font-weight: bold;
    min-width: 84px;
}
.badge-due { background-color: #8a5a00; color: #ffe6b3; }
.badge-nudged { background-color: #1f5a8a; color: #d6ecff; }
.badge-waiting { background-color: #5a3a8a; color: #ece0ff; }
.badge-stalled { background-color: #8a2d2d; color: #ffdede; }
.badge-idle { background-color: #2d6b3f; color: #d8ffe2; }
.summary { font-size: 15px; font-weight: bold; color: #cfe6ff; padding: 6px 2px; }
.note { font-size: 12px; color: #90a0ae; }
.scrolledwindow { background-color: #14181d; }
"#;
