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

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

pub fn create_lock(path: &Path, pid: i32) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err("lock path is a symlink; refusing to write".to_string());
        }
        Ok(meta) if !meta.is_file() => {
            return Err("lock path is not a regular file".to_string());
        }
        Ok(_) => {
            if let Some(old) = read_lock(path) {
                if pid_alive(old) && old != pid {
                    return Err(format!("already running ({})", old));
                }
            }
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Err(_) => {}
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{}", pid).map_err(|e| e.to_string())?;
    Ok(())
}
