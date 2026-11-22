use session_keeper_rust::nudge::{send_nudge_with_retry, send_nudge_with_timeout};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn reply(status: &str) -> String {
    format!("HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}", status)
}

fn scripted(responses: Vec<String>) -> (String, mpsc::Receiver<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for (i, response) in responses.iter().enumerate() {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = tx.send(i + 1);
                let _ = stream.write_all(response.as_bytes());
            }
        }
    });
    (format!("http://{}", addr), rx)
}

#[test]
fn retries_5xx_until_success() {
    let (api, rx) = scripted(vec![
        reply("500 Internal Server Error"),
        reply("500 Internal Server Error"),
        reply("200 OK"),
    ]);
    let result = send_nudge_with_retry(
        &api,
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(2000),
        3,
    );
    assert!(result.is_ok(), "{:?}", result);
    let counts: Vec<usize> = rx.try_iter().collect();
    assert_eq!(counts.len(), 3, "expected three attempts, got {:?}", counts);
}

#[test]
fn single_attempt_5xx_returns_error() {
    let (api, rx) = scripted(vec![reply("503 Service Unavailable")]);
    let result = send_nudge_with_retry(
        &api,
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(2000),
        1,
    );
    assert!(result.is_err());
    assert_eq!(rx.try_iter().count(), 1);
}

#[test]
fn client_error_is_not_retried() {
    let (api, rx) = scripted(vec![reply("400 Bad Request")]);
    let result = send_nudge_with_retry(
        &api,
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(2000),
        3,
    );
    assert!(result.is_err());
    assert_eq!(
        rx.try_iter().count(),
        1,
        "4xx other than 429 must not be retried"
    );
}

#[test]
fn too_many_requests_is_retried() {
    let (api, rx) = scripted(vec![reply("429 Too Many Requests"), reply("200 OK")]);
    let result = send_nudge_with_retry(
        &api,
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(2000),
        2,
    );
    assert!(result.is_ok(), "{:?}", result);
    assert_eq!(rx.try_iter().count(), 2);
}

#[test]
fn connection_refused_returns_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let result = send_nudge_with_timeout(
        &format!("http://{}", addr),
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(500),
    );
    assert!(result.is_err(), "closed port must fail: {:?}", result);
}

#[test]
fn hung_server_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            thread::sleep(Duration::from_secs(2));
        }
    });
    let start = Instant::now();
    let result = send_nudge_with_timeout(
        &format!("http://{}", addr),
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(250),
    );
    assert!(result.is_err(), "hung server must fail: {:?}", result);
    assert!(
        start.elapsed() < Duration::from_millis(1500),
        "took {:?}",
        start.elapsed()
    );
}

#[test]
fn malformed_success_body_is_not_a_success() {
    let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot json".to_string();
    let (api, _rx) = scripted(vec![response]);
    let result = session_keeper_rust::nudge::send_nudge_with_timeout(
        &api,
        "pw",
        "ses_x",
        "continue",
        std::time::Duration::from_millis(2000),
    );
    assert!(
        result.is_err(),
        "malformed 200 body must be rejected: {:?}",
        result
    );
}

#[test]
fn pending_permission_detects_human_input() {
    let body = r#"{"data":[{"id":"perm1"}]}"#;
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
    let (api, _rx) = scripted(vec![response]);
    assert!(session_keeper_rust::nudge::has_pending_human_input(
        &api, "pw", "ses_x"
    ));
}

#[test]
fn empty_permission_means_no_human_input() {
    let body = r#"{"data":[]}"#;
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
    let (api, _rx) = scripted(vec![response]);
    assert!(!session_keeper_rust::nudge::has_pending_human_input(
        &api, "pw", "ses_x"
    ));
}
