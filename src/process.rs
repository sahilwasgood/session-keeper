use std::path::Path;

pub fn pid_alive(pid: i32) -> bool {
    if pid <= 1 {
        return false;
    }
    let cmdline = format!("/proc/{}/cmdline", pid);
    match std::fs::read(&cmdline) {
        Ok(bytes) => std::str::from_utf8(&bytes).unwrap_or("").contains("session-keeper-rust"),
        Err(_) => false,
    }
}

pub fn read_lock(path: &Path) -> Option<i32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
}
