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

pub fn format_age(ts_ms: i64, now_ms: i64) -> String {
    let secs = ((now_ms - ts_ms).max(0)) / 1000;
    if secs < 2 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{}s", secs);
    }
    if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        return if s == 0 { format!("{}m", m) } else { format!("{}m {}s", m, s) };
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if m == 0 { format!("{}h", h) } else { format!("{}h {}m", h, m) }
}

pub fn needs_attention(status: &Status) -> bool {
    matches!(status, Status::Due | Status::Waiting | Status::Stalled)
}

pub fn status_priority(status: &Status) -> i32 {
    match status {
        Status::Due => 0,
        Status::Stalled => 1,
        Status::Waiting => 2,
        Status::Nudged => 3,
        Status::Clean => 4,
    }
}

pub fn matches_filter(id: &str, title: &str, filter: &str) -> bool {
    if filter.trim().is_empty() { return true; }
    let f = filter.trim().to_lowercase();
    id.to_lowercase().contains(&f) || title.to_lowercase().contains(&f)
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
    background-color: @theme_bg_color;
    background-image: none;
    color: @theme_fg_color;
    font-family: "DejaVu Sans", sans-serif;
    font-size: 14px;
}
headerbar {
    background-color: @theme_bg_color;
    background-image: none;
    color: @theme_fg_color;
    padding: 8px 12px;
    font-size: 16px;
    font-weight: bold;
    min-height: 44px;
}
button {
    background-color: @theme_bg_color;
    background-image: none;
    color: @theme_fg_color;
    border: 1px solid @borders;
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
    background-color: @theme_base_color;
    background-image: none;
    color: @theme_text_color;
    border: 1px solid @borders;
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 14px;
}
.card {
    background-color: @theme_base_color;
    background-image: none;
    border: 1px solid @borders;
    border-radius: 10px;
    padding: 12px;
    margin: 4px 6px;
}
.session-id { font-size: 15px; font-weight: bold; color: @theme_fg_color; }
.session-meta { font-size: 13px; color: @insensitive_fg_color; }
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
.summary { font-size: 15px; font-weight: bold; color: @theme_selected_bg_color; padding: 6px 2px; }
.note { font-size: 12px; color: @insensitive_fg_color; }
.scrolledwindow { background-color: #14181d; }
"#;
