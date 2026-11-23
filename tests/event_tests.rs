use session_keeper::events::is_relevant_event_line;

#[test]
fn relevant_event_lines_are_detected() {
    assert!(is_relevant_event_line(
        r#"data: {"type":"session.idle","data":{}}"#
    ));
    assert!(is_relevant_event_line(
        r#"data: {"type":"permission.asked","data":{}}"#
    ));
    assert!(is_relevant_event_line(
        r#"data: {"type":"form.created","data":{}}"#
    ));
}

#[test]
fn unrelated_or_malformed_lines_are_ignored() {
    assert!(!is_relevant_event_line(
        r#"data: {"type":"server.connected","data":{}}"#
    ));
    assert!(!is_relevant_event_line("event: session.idle"));
    assert!(!is_relevant_event_line("data: not json"));
    assert!(!is_relevant_event_line(""));
}

#[test]
fn event_url_uses_repeated_type_filters() {
    let url = session_keeper::events::event_url("http://localhost:49374/");
    assert_eq!(
        url,
        "http://localhost:49374/api/event?types=session.idle&types=session.status&types=permission.asked&types=form.created"
    );
}

#[test]
fn watcher_marks_disconnected_when_stream_ends_cleanly() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = "data: {\"type\":\"session.idle\",\"data\":{}}\n\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let (wake_tx, wake_rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let connected = Arc::new(AtomicBool::new(false));
    let watcher_stop = stop.clone();
    let watcher_connected = connected.clone();
    let handle = std::thread::spawn(move || {
        session_keeper::events::watch_events(&api, "pw", wake_tx, watcher_stop, watcher_connected);
    });
    assert!(
        wake_rx.recv_timeout(Duration::from_secs(3)).is_ok(),
        "wake expected"
    );
    let mut offline = false;
    for _ in 0..40 {
        if !connected.load(Ordering::Relaxed) {
            offline = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    stop.store(true, Ordering::Relaxed);
    let _ = handle.join();
    assert!(offline, "connected flag must clear when the stream ends");
}
