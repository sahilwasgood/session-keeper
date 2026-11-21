use session_keeper_rust::process::{pid_alive, read_lock};
use std::path::PathBuf;
use std::process::Command;

fn tmp(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sk9-lock-{}-{}", std::process::id(), tag))
}

#[test]
fn missing_lock_file_is_none() {
    assert_eq!(read_lock(&tmp("missing")), None);
}

#[test]
fn whitespace_around_pid_is_trimmed() {
    let p = tmp("whitespace");
    std::fs::write(&p, " 12345 \n").unwrap();
    assert_eq!(read_lock(&p), Some(12345));
    std::fs::remove_file(&p).ok();
}

#[test]
fn empty_lock_file_is_none() {
    let p = tmp("empty");
    std::fs::write(&p, "").unwrap();
    assert_eq!(read_lock(&p), None);
    std::fs::remove_file(&p).ok();
}

#[test]
fn zero_and_negative_pids_are_parsed() {
    let p = tmp("zero");
    std::fs::write(&p, "0").unwrap();
    assert_eq!(read_lock(&p), Some(0));
    std::fs::write(&p, "-5").unwrap();
    assert_eq!(read_lock(&p), Some(-5));
    std::fs::remove_file(&p).ok();
}

#[test]
fn oversized_pid_is_none() {
    let p = tmp("oversized");
    std::fs::write(&p, "9999999999").unwrap();
    assert_eq!(read_lock(&p), None);
    std::fs::remove_file(&p).ok();
}

#[test]
fn leading_plus_is_accepted() {
    let p = tmp("plus");
    std::fs::write(&p, "+7").unwrap();
    assert_eq!(read_lock(&p), Some(7));
    std::fs::remove_file(&p).ok();
}

#[test]
fn trailing_garbage_is_none() {
    let p = tmp("garbage");
    std::fs::write(&p, "12abc").unwrap();
    assert_eq!(read_lock(&p), None);
    std::fs::remove_file(&p).ok();
}

#[test]
fn directory_instead_of_file_is_none() {
    let p = tmp("dir");
    std::fs::create_dir_all(&p).unwrap();
    assert_eq!(read_lock(&p), None);
    std::fs::remove_dir_all(&p).ok();
}

#[test]
fn non_positive_pids_are_not_alive() {
    assert!(!pid_alive(0));
    assert!(!pid_alive(-1));
}

#[test]
fn own_pid_is_alive() {
    assert!(pid_alive(std::process::id() as i32));
}

#[test]
fn version_mode_does_not_create_lock() {
    let lock = tmp("version-lock");
    let _ = std::fs::remove_file(&lock);
    let out = Command::new(env!("CARGO_BIN_EXE_session-keeper-rust"))
        .arg("--version")
        .env("SESSION_KEEPER_LOCK", &lock)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!lock.exists(), "Version mode must not take the single-instance lock");
}

#[test]
fn status_mode_does_not_create_lock() {
    let lock = tmp("status-lock");
    let state = tmp("status-state");
    let _ = std::fs::remove_file(&lock);
    let out = Command::new(env!("CARGO_BIN_EXE_session-keeper-rust"))
        .args(["--status", "--json"])
        .env("SESSION_KEEPER_LOCK", &lock)
        .env("SESSION_KEEPER_STATE", &state)
        .env("SESSION_KEEPER_DB", tmp("status-no-db"))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!lock.exists(), "Status mode must not take the single-instance lock");
}
