use session_keeper_rust::config::{format_log_time, format_utc_time, Config};
use std::path::PathBuf;

#[test]
fn log_time_is_readable_utc() {
    // 2026-11-21 08:09:57 UTC
    assert_eq!(format_utc_time(1795248597), "08:09:57");
}

#[test]
fn env_overrides_db_and_state() {
    std::env::set_var("SESSION_KEEPER_DB", "/tmp/sk-test-db.db");
    std::env::set_var("SESSION_KEEPER_STATE", "/tmp/sk-test-state.json");
    assert_eq!(Config::db_path(), PathBuf::from("/tmp/sk-test-db.db"));
    assert_eq!(Config::state_path(), PathBuf::from("/tmp/sk-test-state.json"));
    std::env::remove_var("SESSION_KEEPER_DB");
    std::env::remove_var("SESSION_KEEPER_STATE");
}

#[test]
fn dry_run_flag_from_env() {
    std::env::set_var("SESSION_KEEPER_DRY_RUN", "1");
    assert!(Config::dry_run());
    std::env::remove_var("SESSION_KEEPER_DRY_RUN");
}

#[test]
fn day_start_matches_local_midnight() {
    let now = 1_795_248_597_000i64;
    let start = session_keeper_rust::config::local_day_start_ms(now);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut secs = (now / 1000) as libc::time_t;
    unsafe { libc::localtime_r(&mut secs, &mut tm); }
    let elapsed = (tm.tm_hour as i64 * 3600 + tm.tm_min as i64 * 60 + tm.tm_sec as i64) * 1000;
    assert_eq!(start, now - elapsed);
    assert!(start <= now && now - start < 86_400_000);
}
