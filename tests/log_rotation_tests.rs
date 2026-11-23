use session_keeper::config::{format_log_time, format_utc_time};
use session_keeper::nudge::rotate_text;

fn lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn empty_text_is_unchanged() {
    assert_eq!(rotate_text("", 2000, 1000), "");
}

#[test]
fn single_line_under_max_is_unchanged() {
    assert_eq!(rotate_text("only", 2000, 1000), "only");
}

#[test]
fn text_under_max_is_byte_identical() {
    let text = "a\nb\n";
    assert_eq!(rotate_text(text, 3, 2), text);
}

#[test]
fn text_exactly_at_max_is_unchanged() {
    let text = lines(3);
    assert_eq!(rotate_text(&text, 3, 2), text);
}

#[test]
fn one_line_over_max_keeps_recent_tail() {
    let text = lines(4); // line 0..3
    let out = rotate_text(&text, 3, 2);
    assert_eq!(out, "line 2\nline 3\n");
}

#[test]
fn keep_all_lines_when_keep_equals_line_count() {
    // 4 lines, max 2, keep 4 -> the tail slice is every line.
    let text = lines(4);
    let out = rotate_text(&text, 2, 4);
    assert_eq!(out.lines().count(), 4);
    assert_eq!(out, "line 0\nline 1\nline 2\nline 3\n");
}

#[test]
fn keep_zero_returns_empty_line() {
    let text = lines(5);
    assert_eq!(rotate_text(&text, 2, 0), "\n");
}

#[test]
fn boundary_max_one_keep_one() {
    let text = "old\nnew";
    assert_eq!(rotate_text(text, 1, 1), "new\n");
}

#[test]
fn final_line_without_newline_gets_one() {
    let text = "a\nb\nc";
    let out = rotate_text(text, 2, 1);
    assert_eq!(out, "c\n");
}

#[test]
fn under_max_preserves_trailing_newline() {
    let text = "a\nb\n";
    assert_eq!(rotate_text(text, 5, 1), "a\nb\n");
}

#[test]
fn log_time_midnight() {
    assert_eq!(format_utc_time(0), "00:00:00");
}

#[test]
fn log_time_end_of_day() {
    assert_eq!(format_utc_time(86_399), "23:59:59");
}

#[test]
fn log_time_wraps_each_day() {
    assert_eq!(format_utc_time(86_400), "00:00:00");
}

#[test]
fn log_time_negative_wraps() {
    assert_eq!(format_utc_time(-1), "23:59:59");
}

#[test]
fn log_time_hours_minutes_seconds() {
    assert_eq!(format_utc_time(3661), "01:01:01");
}

fn local_hms(epoch: i64) -> String {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut secs = epoch as libc::time_t;
    unsafe {
        libc::localtime_r(&secs, &mut tm);
    }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

#[test]
fn log_time_uses_machine_local_timezone() {
    // Pin display settings so only the timezone behavior is under test.
    session_keeper::config::set_time_prefs(&session_keeper::config::TimePrefs {
        timezone: "auto".to_string(),
        clock_24h: true,
        date_style: "none".to_string(),
    });
    for epoch in [0i64, 1_795_248_597, 1_700_000_000, -1] {
        assert_eq!(format_log_time(epoch), local_hms(epoch));
    }
}

#[test]
fn rotate_text_handles_keep_larger_than_lines() {
    let text = "a\nb\nc";
    assert_eq!(
        session_keeper::nudge::rotate_text(text, 2, 100),
        "a\nb\nc\n"
    );
}
