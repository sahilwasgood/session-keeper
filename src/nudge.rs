use crate::config::Config;
use crate::db::Status;
use crate::state::{Session, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Nudge,
    Hold,
    Skip,
}

fn b64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (chunk.get(1).copied().unwrap_or(0) as u32) << 8
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

pub fn nudges_in_last_hour(sid: &str, now_ms: i64) -> i64 {
    let cutoff = now_ms - 3_600_000;
    std::fs::read_to_string(Config::history_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|v| v.get("sid").and_then(|s| s.as_str()) == Some(sid))
        .filter(|v| v.get("kind").and_then(|s| s.as_str()) == Some("nudge"))
        .filter(|v| v.get("t").and_then(|t| t.as_i64()).unwrap_or(0) >= cutoff)
        .count() as i64
}

pub fn nudges_in_last_hour_all(now_ms: i64) -> i64 {
    let cutoff = now_ms - 3_600_000;
    std::fs::read_to_string(Config::history_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|v| v.get("kind").and_then(|s| s.as_str()) == Some("nudge"))
        .filter(|v| v.get("t").and_then(|t| t.as_i64()).unwrap_or(0) >= cutoff)
        .count() as i64
}

pub fn notify(title: &str, message: &str) {
    if !Config::notifications() { return; }
    let _ = std::process::Command::new("notify-send")
        .arg(title).arg(message)
        .spawn();
}

pub fn decide_with_history(
    state: &State,
    session: &Session,
    status: &Status,
    now_ms: i64,
    recent_nudges: i64,
) -> Decision {
    if !state.enabled || !session.enabled {
        return Decision::Skip;
    }
    if *status != Status::Due {
        return Decision::Hold;
    }
    if session.last_nudge > 0 {
        let gap_sec = (now_ms - session.last_nudge) / 1000;
        if gap_sec < state.min_gap {
            return Decision::Hold;
        }
    }
    if state.max_nudges_per_hour > 0 && recent_nudges >= state.max_nudges_per_hour {
        return Decision::Hold;
    }
    Decision::Nudge
}

pub fn decide(state: &State, session: &Session, status: &Status, now_ms: i64) -> Decision {
    let recent = nudges_in_last_hour(&session.id, now_ms);
    decide_with_history(state, session, status, now_ms, recent)
}

pub fn prompt_url(api: &str, sid: &str) -> String {
    format!("{}/api/session/{}/prompt", api.trim_end_matches('/'), sid)
}

pub fn send_nudge(api: &str, password: &str, sid: &str, text: &str) -> Result<(), String> {
    send_nudge_with_timeout(api, password, sid, text, std::time::Duration::from_secs(10))
}

pub fn send_nudge_with_timeout(
    api: &str,
    password: &str,
    sid: &str,
    text: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let auth = format!("Basic {}", b64(format!("opencode:{}", password).as_bytes()));
    let body = serde_json::json!({ "text": text }).to_string();
    let url = prompt_url(api, sid);
    let resp = ureq::post(&url)
        .timeout(timeout)
        .set("Authorization", &auth)
        .set("Content-Type", "application/json")
        .send_string(&body)
        .map_err(|e| format!("{}", e))?;
    if (200..300).contains(&resp.status()) {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

pub fn append_history(sid: &str, kind: &str, detail: &str) {
    let path = Config::history_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let detail: String = detail.chars().take(200).collect();
    let line = serde_json::json!({ "t": now, "sid": sid, "kind": kind, "detail": detail });
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", line);
    }
    if let Ok(text) = std::fs::read_to_string(&path) {
        let all: Vec<&str> = text.lines().collect();
        if all.len() > 1000 {
            let tail = all[all.len() - 500..].join("\n") + "\n";
            let _ = std::fs::write(&path, tail);
        }
    }
}

pub fn log_line(message: &str) {
    use std::io::Write;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(Config::log_path())
    {
        let _ = writeln!(f, "{} {}", crate::config::format_log_time(now), message);
    }
}

