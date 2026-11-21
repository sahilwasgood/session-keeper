use std::path::Path;

pub fn pid_alive(pid: i32) -> bool {
    if pid <= 1 {
        return false;
    }
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn read_lock(path: &Path) -> Option<i32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
}
