use session_keeper_rust::ui::format_age;

#[test]
fn age_formats_seconds_minutes_hours() {
    assert_eq!(format_age(100_000, 100_000), "just now");
    assert_eq!(format_age(97_000, 100_000), "3s");
    assert_eq!(format_age(40_000, 100_000), "1m");
    assert_eq!(format_age(10_000, 100_000), "1m 30s");
    assert_eq!(format_age(0, 7_200_000), "2h");
}
