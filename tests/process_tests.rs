use session_keeper_rust::process::{pid_alive, read_lock};

#[test]
fn own_pid_is_alive() {
    assert!(pid_alive(std::process::id() as i32));
}

#[test]
fn impossible_pid_is_dead() {
    assert!(!pid_alive(2_147_483_647));
}

#[test]
fn lock_file_parsing() {
    let path = std::path::Path::new("/tmp/sk-test.lock");
    std::fs::write(path, "12345\n").unwrap();
    assert_eq!(read_lock(path), Some(12345));
    std::fs::write(path, "garbage").unwrap();
    assert_eq!(read_lock(path), None);
    std::fs::remove_file(path).ok();
}

#[test]
fn unrelated_pid_is_not_a_keeper() {
    assert!(!pid_alive(1));
}
