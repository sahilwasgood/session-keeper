use std::time::Duration;
use std::sync::{Mutex, OnceLock};
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

// Rotation is a read-modify-write; serialise it so concurrent tick/kick/UI
// writers cannot drop each other's appended lines.
static IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn append_and_rotate(path: &std::path::Path, line: &str) {
    let _guard = IO_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", line);
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        let rotated = rotate_text(&text, 2000, 1000);
        if rotated != text {
            let _ = std::fs::write(path, rotated);
        }
    }
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
    if let Ok(child) = std::process::Command::new("notify-send")
        .arg("--")
        .arg(title)
        .arg(message)
        .spawn()
    {
        // Reap the helper so repeated alerts do not leave zombies.
        std::thread::spawn(move || { let mut child = child; let _ = child.wait(); });
    }
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

fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn prompt_url(api: &str, sid: &str) -> String {
    format!("{}/api/session/{}/prompt", api.trim_end_matches('/'), encode_segment(sid))
}

pub fn send_nudge(api: &str, password: &str, sid: &str, text: &str) -> Result<(), String> {
    send_nudge_with_retry(api, password, sid, text, std::time::Duration::from_secs(10), 2)
}

fn is_retryable_error(err: &str) -> bool {
    // ureq Error::Status displays "status code 503"; the fallback status path
    // in send_nudge_with_timeout returns "HTTP 503". Cover both, plus transport/timeout.
    err.contains("status code 5")
        || err.contains("status code 429")
        || err.starts_with("HTTP 5")
        || err.starts_with("HTTP 429")
        || err.contains("Network Error")
        || err.contains("timed out")
}

pub fn send_nudge_with_retry(
    api: &str,
    password: &str,
    sid: &str,
    text: &str,
    timeout: std::time::Duration,
    attempts: usize,
) -> Result<(), String> {
    let mut last = String::new();
    for i in 0..attempts.max(1) {
        match send_nudge_with_timeout(api, password, sid, text, timeout) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e;
                let retryable = is_retryable_error(&last);
                if !retryable || i + 1 >= attempts { break; }
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0);
                let jitter = Duration::from_nanos((nanos % 150_000_000) as u64);
                std::thread::sleep(Duration::from_millis(50) + jitter);
            }
        }
    }
    Err(last)
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let detail: String = detail.chars().take(200).collect();
    let line = serde_json::json!({ "t": now, "sid": sid, "kind": kind, "detail": detail });
    append_and_rotate(&Config::history_path(), &line.to_string());
}

pub fn rotate_text(text: &str, max: usize, keep: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    if all.len() <= max {
        return text.to_string();
    }
    let start = all.len().saturating_sub(keep.min(all.len()));
    all[start..].join("\n") + "\n"
}

pub fn log_line(message: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let line = format!("{} {}", crate::config::format_log_time(now), message);
    append_and_rotate(&Config::log_path(), &line);
}


#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub name: String,
}

pub fn fetch_models(api: &str, password: &str) -> Vec<ModelInfo> {
    let auth = format!("Basic {}", b64(format!("opencode:{}", password).as_bytes()));
    let url = format!("{}/api/model", api.trim_end_matches('/'));
    let resp = match ureq::get(&url)
        .timeout(Duration::from_secs(15))
        .set("Authorization", &auth)
        .call()
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if !(200..300).contains(&resp.status()) {
        return Vec::new();
    }
    let text = resp.into_string().unwrap_or_default();
    let value: serde_json::Value = match serde_json::from_str(&text) { Ok(v) => v, Err(_) => return Vec::new() };
    let items = value.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    items.iter().filter_map(|m| {
        Some(ModelInfo {
            id: m.get("id")?.as_str()?.to_string(),
            provider: m.get("providerID")?.as_str()?.to_string(),
            name: m.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
        })
    }).collect()
}

pub fn switch_model(api: &str, password: &str, sid: &str, model_id: &str, provider: &str) -> Result<(), String> {
    let auth = format!("Basic {}", b64(format!("opencode:{}", password).as_bytes()));
    let url = format!("{}/api/session/{}/model", api.trim_end_matches('/'), encode_segment(sid));
    let body = serde_json::json!({"model": {"id": model_id, "providerID": provider}}).to_string();
    let resp = ureq::post(&url)
        .timeout(Duration::from_secs(10))
        .set("Authorization", &auth)
        .set("Content-Type", "application/json")
        .send_string(&body)
        .map_err(|e| e.to_string())?;
    if (200..300).contains(&resp.status()) { Ok(()) } else { Err(format!("HTTP {}", resp.status())) }
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
}

pub fn fetch_sessions(api: &str, password: &str) -> Vec<SessionInfo> {
    let auth = format!("Basic {}", b64(format!("opencode:{}", password).as_bytes()));
    let url = format!("{}/api/session", api.trim_end_matches('/'));
    let resp = match ureq::get(&url)
        .timeout(Duration::from_secs(15))
        .set("Authorization", &auth)
        .call()
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let text = resp.into_string().unwrap_or_default();
    let value: serde_json::Value = match serde_json::from_str(&text) { Ok(v) => v, Err(_) => return Vec::new() };
    let items = value.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    items.iter().filter_map(|s| {
        Some(SessionInfo {
            id: s.get("id")?.as_str()?.to_string(),
            title: s.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        })
    }).collect()
}
