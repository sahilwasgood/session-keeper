use session_keeper_rust::db::Status;
use session_keeper_rust::ui::{short_id, status_class, status_text, summary_line};

#[test]
fn status_text_is_short_and_human() {
    assert_eq!(status_text(&Status::Due), "due");
    assert_eq!(status_text(&Status::Nudged), "nudged");
    assert_eq!(status_text(&Status::Waiting), "waiting on question");
    assert_eq!(status_text(&Status::Stalled), "stalled");
    assert_eq!(status_text(&Status::Clean), "working");
}

#[test]
fn status_class_maps_to_css_badge() {
    assert_eq!(status_class(&Status::Due), "badge-due");
    assert_eq!(status_class(&Status::Nudged), "badge-nudged");
    assert_eq!(status_class(&Status::Waiting), "badge-waiting");
    assert_eq!(status_class(&Status::Stalled), "badge-stalled");
    assert_eq!(status_class(&Status::Clean), "badge-idle");
}

#[test]
fn short_id_keeps_the_tail() {
    assert_eq!(short_id("ses_e9c3a7de0ffeZmK488xq4jYSCf"), "K488xq4jYSCf");
    assert_eq!(short_id("short"), "short");
}

#[test]
fn summary_line_reports_actions() {
    assert_eq!(
        summary_line("12:34:56", 2, 1),
        "Last check 12:34:56   nudged 2   held 1"
    );
}

fn luminance(hex: &str) -> f64 {
    let channels: Vec<f64> = (1..=5)
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0)
        .map(|c| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
        .collect();
    0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
}

fn contrast(bg: &str, fg: &str) -> f64 {
    let (hi, lo) = if luminance(bg) > luminance(fg) {
        (luminance(bg), luminance(fg))
    } else {
        (luminance(fg), luminance(bg))
    };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn badge_and_button_colors_meet_wcag_aa() {
    // Mirrors the fixed colors in ui.rs CSS; keep in sync when the theme changes.
    let pairs = [
        ("due", "#8a5a00", "#ffe6b3"),
        ("nudged", "#1f5a8a", "#d6ecff"),
        ("waiting", "#5a3a8a", "#ece0ff"),
        ("stalled", "#8a2d2d", "#ffdede"),
        ("idle", "#2d6b3f", "#d8ffe2"),
        ("suggested", "#2d7d46", "#ffffff"),
        ("destructive", "#7d2d2d", "#ffffff"),
        ("unknown", "#555555", "#f0f0f0"),
    ];
    for (name, bg, fg) in pairs {
        let ratio = contrast(bg, fg);
        assert!(ratio >= 4.5, "{} contrast {:.2}:1 is below AA", name, ratio);
    }
}

#[test]
fn unknown_status_is_explicit_and_not_an_attention_state() {
    use session_keeper_rust::db::Status;
    use session_keeper_rust::ui::{needs_attention, status_class, status_text, status_tooltip};
    let s = Status::Unknown;
    assert_eq!(status_text(&s), "unknown");
    assert_eq!(status_class(&s), "badge-unknown");
    assert!(status_tooltip(&s).contains("database"));
    assert!(!needs_attention(&s));
}
