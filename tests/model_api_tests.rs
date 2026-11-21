use session_keeper_rust::nudge::{fetch_models, switch_model};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

fn mock(status: &str, body: &str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let status = status.to_string();
    let body = body.to_string();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut raw = Vec::new();
            let mut buf = [0u8; 2048];
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw);
                if let Some(pos) = text.find("\r\n\r\n") {
                    let need = text
                        .lines()
                        .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                        .and_then(|l| l.split(':').nth(1))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if text[pos + 4..].len() >= need {
                        break;
                    }
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&raw).to_string());
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{}", addr), rx)
}

#[test]
fn empty_data_array_yields_no_models() {
    let (api, _rx) = mock("200 OK", r#"{"data":[]}"#);
    assert!(fetch_models(&api, "pw").is_empty());
}

#[test]
fn missing_name_defaults_to_empty() {
    let (api, _rx) = mock("200 OK", r#"{"data":[{"id":"m1","providerID":"p1"}]}"#);
    let models = fetch_models(&api, "pw");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].name, "");
}

#[test]
fn item_missing_required_field_is_skipped() {
    let (api, _rx) = mock(
        "200 OK",
        r#"{"data":[{"providerID":"p1","name":"no id"},{"id":"m2","name":"no provider"}]}"#,
    );
    assert!(fetch_models(&api, "pw").is_empty());
}

#[test]
fn malformed_json_yields_no_models() {
    let (api, _rx) = mock("200 OK", "not json");
    assert!(fetch_models(&api, "pw").is_empty());
}

#[test]
fn response_without_data_key_yields_no_models() {
    let (api, _rx) = mock("200 OK", r#"{"models":[]}"#);
    assert!(fetch_models(&api, "pw").is_empty());
}

#[test]
fn http_error_yields_no_models() {
    let (api, _rx) = mock("500 Internal Server Error", "{}");
    assert!(fetch_models(&api, "pw").is_empty());
}

#[test]
fn trailing_slash_api_base_calls_model_endpoint() {
    let (api, rx) = mock("200 OK", r#"{"data":[]}"#);
    let _ = fetch_models(&format!("{}/", api), "pw");
    let request = rx.recv().unwrap();
    assert!(request.starts_with("GET /api/model "), "{}", request);
}

#[test]
fn switch_model_http_error_returns_status() {
    let (api, _rx) = mock("500 Internal Server Error", "{}");
    let err = switch_model(&api, "pw", "ses_x", "m1", "p1").unwrap_err();
    // ureq may surface non-2xx as its own error or the crate's "HTTP <code>" fallback.
    assert!(err.contains("500"), "unexpected error: {}", err);
}

#[test]
fn switch_model_posts_expected_payload_and_headers() {
    let (api, rx) = mock("200 OK", "{}");
    let result = switch_model(&api, "pw", "ses_x", "m2", "p2");
    assert!(result.is_ok(), "{:?}", result);
    let request = rx.recv().unwrap();
    assert!(request.starts_with("POST /api/session/ses_x/model "), "{}", request);
    assert!(request.contains("\"id\":\"m2\""), "{}", request);
    assert!(request.contains("\"providerID\":\"p2\""), "{}", request);
    assert!(request.contains("Authorization: Basic b3BlbmNvZGU6cHc="), "{}", request);
    assert!(request.to_ascii_lowercase().contains("content-type: application/json"), "{}", request);
}

#[test]
fn session_id_is_encoded_in_url() {
    let url = session_keeper_rust::nudge::prompt_url("http://localhost:49374", "ses_a/b?c");
    assert_eq!(url, "http://localhost:49374/api/session/ses_a%2Fb%3Fc/prompt");
}

#[test]
fn fetch_sessions_parses_id_and_title() {
    let body = r#"{"data":[{"id":"ses_abc","title":"tg"}]}"#;
    let (api, _rx) = mock("200 OK", body);
    let items = session_keeper_rust::nudge::fetch_sessions(&api, "pw");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "ses_abc");
    assert_eq!(items[0].title, "tg");
}

#[test]
fn result_api_distinguishes_empty_list_from_server_error() {
    let (api, _rx) = mock("200 OK", r#"{"data":[]}"#);
    assert!(session_keeper_rust::nudge::fetch_models_result(&api, "pw").unwrap().is_empty());
    let (api2, _rx2) = mock("500 Internal Server Error", "{}");
    let err = session_keeper_rust::nudge::fetch_models_result(&api2, "pw").unwrap_err();
    assert!(err.contains("HTTP 500"), "unexpected error: {}", err);
    let (api3, _rx3) = mock("200 OK", r#"{"data":[{"id":"s1","title":"Demo"}]}"#);
    let sessions = session_keeper_rust::nudge::fetch_sessions_result(&api3, "pw").unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Demo");
}

#[test]
fn result_api_reports_unreachable_host() {
    let err = session_keeper_rust::nudge::fetch_sessions_result("http://127.0.0.1:1", "pw").unwrap_err();
    assert!(err.contains("cannot reach OpenCode API"), "unexpected error: {}", err);
}
