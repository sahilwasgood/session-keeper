use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn hung_api_returns_within_timeout() {
    // A local server that accepts the connection, reads the request, then never replies.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let _ = tx.send(());
                thread::sleep(Duration::from_secs(5)); // hold the socket open, never respond
            }
        }
    });

    let start = Instant::now();
    let result = session_keeper_rust::nudge::send_nudge_with_timeout(
        &format!("http://{}", addr),
        "pw",
        "ses_test",
        "continue",
        Duration::from_millis(300),
    );
    let elapsed = start.elapsed();

    assert!(result.is_err(), "hung server must produce an error, got {:?}", result);
    assert!(elapsed < Duration::from_secs(2), "call must return near timeout, took {:?}", elapsed);
    let _ = rx.try_recv();
}
