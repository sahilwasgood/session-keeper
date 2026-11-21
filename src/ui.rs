use crate::db::Status;

pub fn status_text(status: &Status) -> &'static str {
    match status {
        Status::Due => "due",
        Status::Nudged => "nudged",
        Status::Waiting => "waiting on question",
        Status::Stalled => "stalled",
        Status::Clean => "working",
        Status::Unknown => "unknown",
    }
}

pub fn status_class(status: &Status) -> &'static str {
    match status {
        Status::Due => "badge-due",
        Status::Nudged => "badge-nudged",
        Status::Waiting => "badge-waiting",
        Status::Stalled => "badge-stalled",
        Status::Clean => "badge-idle",
        Status::Unknown => "badge-unknown",
    }
}

/// Longer, sentence-style description for tooltips and accessibility names.
/// `status_text` stays short for the badge label; this adds context on hover.
pub fn status_tooltip(status: &Status) -> &'static str {
    match status {
        Status::Due => "Due for a nudge - no assistant activity since the last check.",
        Status::Nudged => "Nudged - waiting for the assistant to respond.",
        Status::Waiting => "Waiting on a question - the assistant asked for input.",
        Status::Stalled => "Stalled - no progress for longer than the idle threshold.",
        Status::Clean => "Working - recent assistant activity detected.",
        Status::Unknown => "Unknown - the OpenCode database is unavailable.",
    }
}

pub fn short_id(id: &str) -> String {
    if id.len() <= 12 {
        return id.to_string();
    }
    id.chars().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect()
}

pub fn history_for_sid(text: &str, sid: &str, limit: usize) -> Vec<String> {
    text.lines()
        .filter(|line| serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| v.get("sid").and_then(|s| s.as_str()).map(str::to_string))
            .as_deref() == Some(sid))
        .rev()
        .take(limit)
        .map(str::to_string)
        .collect()
}

pub fn format_age(ts_ms: i64, now_ms: i64) -> String {
    // saturating_sub clamps future timestamps and avoids i64 overflow panics.
    let secs = now_ms.saturating_sub(ts_ms) / 1000;
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

pub fn sort_key(status: &Status, title: &str, id: &str, by_name: bool) -> (i32, String) {
    let prio = if by_name { 0 } else { status_priority(status) };
    (prio, format!("{}{}", title, id))
}

pub fn status_changed(prev: &str, next: &str) -> bool {
    prev != next
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
        Status::Unknown => 5,
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
/* shade() keeps hover/active legible in both light and dark themes;
   a fixed dark hex once left dark-theme text unreadable on hover. */
button:hover { background-color: shade(@theme_bg_color, 0.92); }
button:active { background-color: shade(@theme_bg_color, 0.84); }
button.suggested-action {
    background-color: #2d7d46;
    border-color: #3aa359;
    color: #ffffff;
    font-weight: bold;
}
button.suggested-action:hover { background-color: #31804a; }
button.suggested-action:active { background-color: #24663b; }
button.destructive-action {
    background-color: #7d2d2d;
    border-color: #a33a3a;
    color: #ffffff;
}
button.destructive-action:hover { background-color: #993838; }
button.destructive-action:active { background-color: #612424; }
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
/* insensitive_fg_color measured ~3:1; alpha fg keeps hierarchy at >=6:1 in both themes. */
.session-meta { font-size: 13px; color: alpha(@theme_fg_color, 0.8); }
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
.badge-unknown { background-color: #555555; color: #f0f0f0; }
/* was @theme_selected_bg_color as a foreground: 1.61:1, unreadable. */
.summary { font-size: 15px; font-weight: bold; color: @theme_fg_color; padding: 6px 2px; }
.note { font-size: 12px; color: alpha(@theme_fg_color, 0.8); }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_age_clamps_future_timestamps() {
        assert_eq!(format_age(2000, 1000), "just now");
        assert_eq!(format_age(1000, 1000), "just now");
    }

    #[test]
    fn format_age_survives_extreme_timestamps() {
        let _ = format_age(i64::MIN, i64::MAX);
        let _ = format_age(i64::MAX, i64::MIN);
    }

    #[test]
    fn format_age_formats_units() {
        assert_eq!(format_age(0, 5_000), "5s");
        assert_eq!(format_age(0, 90_000), "1m 30s");
        assert_eq!(format_age(0, 3_600_000), "1h");
        assert_eq!(format_age(0, 5_400_000), "1h 30m");
    }

    #[test]
    fn status_tooltip_adds_context_without_changing_badge_label() {
        assert!(status_tooltip(&Status::Waiting).len() > status_text(&Status::Waiting).len());
        assert!(status_tooltip(&Status::Due).contains("nudge"));
    }
}
