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

/// A shape cue paired with the text label, so status never relies on colour alone.
pub fn status_glyph(status: &Status) -> &'static str {
    match status {
        Status::Due => "◷ ",
        Status::Nudged => "➤ ",
        Status::Waiting => "? ",
        Status::Stalled => "✕ ",
        Status::Clean => "● ",
        Status::Unknown => "○ ",
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
    id.chars()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn history_for_sid(text: &str, sid: &str, limit: usize) -> Vec<String> {
    text.lines()
        .filter(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|v| v.get("sid").and_then(|s| s.as_str()).map(str::to_string))
                .as_deref()
                == Some(sid)
        })
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
        return if s == 0 {
            format!("{}m", m)
        } else {
            format!("{}m {}s", m, s)
        };
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if m == 0 {
        format!("{}h", h)
    } else {
        format!("{}h {}m", h, m)
    }
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
    if filter.trim().is_empty() {
        return true;
    }
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
/* Design tokens. Theme-derived colours keep the app at home in light, dark
   and HighContrast themes; colour is reserved for status, never decoration. */
@define-color ink @theme_fg_color;
@define-color ink2 alpha(@theme_fg_color, 0.78);
@define-color ink3 alpha(@theme_fg_color, 0.58);
@define-color hairline alpha(@theme_fg_color, 0.13);
@define-color quietline alpha(@theme_fg_color, 0.07);
@define-color wash alpha(@theme_fg_color, 0.05);
@define-color wash2 alpha(@theme_fg_color, 0.09);
@define-color focus @theme_selected_bg_color;

window {
    background-color: @theme_bg_color;
    background-image: none;
    color: @ink;
    font-family: Inter, "Noto Sans", "DejaVu Sans", sans-serif;
    font-size: 10.5pt;
}
button:focus, entry:focus, checkbutton:focus, combobox:focus {
    outline-style: solid; outline-width: 2px; outline-color: @focus; outline-offset: 1px;
}

headerbar {
    background-color: @theme_bg_color;
    background-image: none;
    color: @ink;
    border-bottom: 1px solid @hairline;
    box-shadow: none;
    padding: 6px 12px;
    min-height: 44px;
}
headerbar .title { font-size: 12pt; font-weight: 600; }
headerbar .subtitle { font-size: 9pt; color: @ink3; }

button {
    background-color: transparent;
    background-image: none;
    color: @ink;
    border: 1px solid @hairline;
    border-radius: 6px;
    padding: 6px 12px;
    min-height: 24px;
    font-size: 10pt;
    font-weight: 500;
    transition: background-color 120ms ease-out, border-color 120ms ease-out;
}
button:hover { background-color: @wash; border-color: @wash2; }
button:active { background-color: @wash2; }
button:disabled { opacity: 0.5; }

button.suggested-action {
    background-color: @focus;
    border-color: @focus;
    color: @theme_selected_fg_color;
    font-weight: 600;
}
button.suggested-action:hover { background-color: shade(@focus, 1.08); }
button.suggested-action:active { background-color: shade(@focus, 0.92); }

button.destructive-action {
    background-color: alpha(@error_color, 0.16);
    border-color: alpha(@error_color, 0.35);
    color: @error_color;
}
button.destructive-action:hover { background-color: alpha(@error_color, 0.24); }

button.flat-action {
    background-color: transparent;
    border-color: transparent;
    color: @ink2;
}
button.flat-action:hover { background-color: @wash; color: @ink; }

button.remove-action {
    background-color: transparent;
    border-color: transparent;
    color: alpha(@error_color, 0.8);
}
button.remove-action:hover {
    background-color: alpha(@error_color, 0.14);
    color: @error_color;
}

.card-calm { background-color: alpha(@theme_fg_color, 0.015); }

entry {
    background-color: @theme_base_color;
    background-image: none;
    color: @ink;
    border: 1px solid @hairline;
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 10pt;
}
entry:focus { border-color: @focus; }
entry.placeholder { color: @ink3; }

combobox {
    font-size: 10pt;
}

.card {
    background-color: @theme_base_color;
    background-image: none;
    border: 1px solid @quietline;
    border-radius: 8px;
    padding: 12px 14px;
    margin: 3px 4px;
    transition: background-color 120ms ease-out, border-color 120ms ease-out;
}
.card:hover { background-color: @wash; border-color: @hairline; }
.card-urgent { border-left: 3px solid @warning_color; }
.card-critical { border-left: 3px solid @error_color; }

.session-id { font-size: 10.5pt; font-weight: 600; color: @ink; }
.session-meta { font-size: 9pt; color: @ink2; }
.mono { font-family: "DejaVu Sans Mono", "Noto Sans Mono", monospace; font-size: 9pt; }

.badge {
    border-radius: 6px;
    padding: 3px 9px;
    font-size: 9pt;
    font-weight: 600;
    min-width: 108px;
}
.badge-due { background-color: alpha(@warning_color, 0.18); color: @warning_color; }
.badge-nudged { background-color: alpha(@theme_selected_bg_color, 0.16); color: @theme_selected_bg_color; }
.badge-waiting { background-color: alpha(@warning_color, 0.14); color: @warning_color; }
.badge-stalled { background-color: alpha(@error_color, 0.18); color: @error_color; }
.badge-idle { background-color: alpha(@success_color, 0.16); color: @success_color; }
.badge-unknown { background-color: @wash2; color: @ink2; }

.summary { font-size: 11pt; font-weight: 600; color: @ink; padding: 4px 2px; }
.note { font-size: 9pt; color: @ink3; }

expander title { font-size: 10pt; font-weight: 500; color: @ink2; }
scrollbar { background-color: transparent; border: none; }
scrollbar trough { background-color: transparent; }
scrollbar slider {
    background-color: alpha(@ink3, 0.45);
    border-radius: 4px;
    min-width: 7px;
    min-height: 7px;
}
scrollbar slider:hover { background-color: alpha(@ink3, 0.7); }
scrollbar button { border: none; background-color: transparent; }
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
