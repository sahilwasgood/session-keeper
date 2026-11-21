use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// True for SSE data frames that mean a session may need attention.
pub fn is_relevant_event_line(line: &str) -> bool {
    let rest = match line.strip_prefix("data:") {
        Some(r) => r.trim(),
        None => return false,
    };
    let value: serde_json::Value = match serde_json::from_str(rest) {
        Ok(v) => v,
        Err(_) => return false,
    };
    matches!(
        value.get("type").and_then(|t| t.as_str()),
        Some("session.idle") | Some("session.status") | Some("permission.asked") | Some("form.created")
    )
}

fn auth_header(password: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = format!("opencode:{}", password).into_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (chunk.get(1).copied().unwrap_or(0) as u32) << 8
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    format!("Basic {}", out)
}

pub fn event_url(api: &str) -> String {
    format!(
        "{}/api/event?types=session.idle&types=session.status&types=permission.asked&types=form.created",
        api.trim_end_matches('/')
    )
}

/// Streams OpenCode events and sends a wake signal for relevant frames.
/// Reconnects with a short delay on failures; `stop` ends the loop when set.
pub fn watch_events(
    api: &str,
    password: &str,
    wake: Sender<()>,
    stop: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) {
    let url = event_url(api);
    let mut was_connected = false;
    let mut last_error: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        let request = ureq::get(&url)
            .timeout(Duration::from_secs(30))
            .set("Authorization", &auth_header(password))
            .set("Accept", "text/event-stream");
        match request.call() {
            Ok(response) => {
                if !was_connected {
                    crate::nudge::log_line("event stream connected");
                    connected.store(true, Ordering::Relaxed);
                    was_connected = true;
                    last_error = None;
                }
                let reader = BufReader::new(response.into_reader());
                for line in reader.lines() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    match line {
                        Ok(text) if is_relevant_event_line(&text) => {
                            if wake.send(()).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                // EOF or read error ends the stream even though request.call() succeeded.
                if was_connected {
                    crate::nudge::log_line("event stream disconnected");
                    connected.store(false, Ordering::Relaxed);
                    was_connected = false;
                }
            }
            Err(e) => {
                if was_connected {
                    crate::nudge::log_line("event stream disconnected");
                    connected.store(false, Ordering::Relaxed);
                    was_connected = false;
                }
                let now = Instant::now();
                if last_error.map(|t| now.duration_since(t) >= Duration::from_secs(60)).unwrap_or(true) {
                    crate::nudge::log_line(&format!("event stream retry: {}", e));
                    last_error = Some(now);
                }
            }
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}
