use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[test]
fn retries_once_after_server_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut count = 0;
        for stream in listener.incoming() {
            count += 1;
            if let Ok(mut s) = stream {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let response = if count == 1 {
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                };
                let _ = s.write_all(response.as_bytes());
                let _ = tx.send(count);
            }
            if count >= 2 { break; }
        }
    });

    let result = session_keeper_rust::nudge::send_nudge_with_retry(
        &format!("http://{}", addr), "pw", "ses_test", "continue",
        Duration::from_millis(2000), 2,
    );
    assert!(result.is_ok(), "retry should succeed: {:?}", result);
    let counts: Vec<i32> = rx.try_iter().collect();
    assert!(counts.len() >= 2, "server should see two requests, saw {:?}", counts);
}
