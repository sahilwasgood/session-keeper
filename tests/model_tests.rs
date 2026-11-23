use session_keeper::nudge::{fetch_models, switch_model};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn serve_once(body: &'static str, status: &str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let status = status.to_string();
    thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut raw: Vec<u8> = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = s.read(&mut buf).unwrap_or(0);
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
            let response = format!("HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body);
            let _ = s.write_all(response.as_bytes());
        }
    });
    (format!("http://{}", addr), rx)
}

#[test]
fn fetches_models_from_api() {
    let body = r#"{"data":[{"id":"m1","providerID":"p1","name":"Model One"}]}"#;
    let (api, _rx) = serve_once(body, "200 OK");
    let models = fetch_models(&api, "pw");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "m1");
    assert_eq!(models[0].name, "Model One");
}

#[test]
fn switch_model_posts_expected_payload() {
    let (api, rx) = serve_once("{}", "200 OK");
    let result = switch_model(&api, "pw", "ses_test", "m2", "p2");
    assert!(result.is_ok(), "{:?}", result);
    let request = rx.recv().unwrap();
    assert!(
        request.starts_with("POST /api/session/ses_test/model"),
        "{}",
        request
    );
    assert!(request.contains("\"providerID\":\"p2\""), "{}", request);
}
