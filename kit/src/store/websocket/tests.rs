use super::*;
use crate::store::recording::read_file;
use crate::store::session::{Session, SourceEvent};
use std::time::{Duration, Instant};

/// Serves one WebSocket client on a free local port: sends `frame` as one binary
/// message, then closes. Returns the address.
fn serve_once(frame: Vec<u8>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("ws://{}/ws", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.send(tungstenite::Message::Binary(frame)).unwrap();
        socket.close(None).unwrap();
        while socket.read().is_ok() {}
    });
    address
}

/// Polls `socket` through `session` every 10 ms until it ended or 5 s passed. Returns
/// the events and the number of messages.
fn poll_until_ended(session: &mut Session, socket: &mut WebSocket) -> (Vec<SourceEvent>, usize) {
    let (mut events, mut messages) = (vec![], 0);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        let polled = session.poll_source("ws", socket, 0.0, true);
        messages += polled.messages;
        let ended = polled
            .events
            .iter()
            .any(|event| matches!(event, SourceEvent::Closed | SourceEvent::Failed(_)));
        events.extend(polled.events);
        if ended {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    (events, messages)
}

#[test]
fn a_websocket_hands_over_its_frames_as_they_are() {
    let log = proto::Message::Log(proto::LogRecord {
        timestamp: 1,
        path: String::new(),
        level: 2,
        message: "hi".to_owned(),
    });
    let sample = proto::Message::Sample(proto::Sample {
        timestamp: 2,
        path: "a".to_owned(),
        value: proto::Data::Float(1.5),
    });
    let log_bytes = serde_cbor_2::to_vec(&log).unwrap();
    let sample_bytes = serde_cbor_2::to_vec(&sample).unwrap();
    let frame = [log_bytes.clone(), sample_bytes.clone()].concat();
    let address = serve_once(frame);

    let mut session = Session::recording();
    let mut socket = WebSocket::connect(&address, || {}).unwrap();
    let (events, messages) = poll_until_ended(&mut session, &mut socket);

    assert_eq!(events, [SourceEvent::Opened, SourceEvent::Closed]);
    assert_eq!(messages, 2);
    let mut found = vec![];
    read_file(&session.recorder.to_file(), |source, bytes| {
        found.push((source.to_owned(), bytes.to_vec()));
    })
    .unwrap();
    assert_eq!(
        found,
        [
            ("ws".to_owned(), log_bytes),
            ("ws".to_owned(), sample_bytes)
        ]
    );
}

#[test]
fn a_websocket_that_cannot_connect_fails() {
    let mut session = Session::recording();
    let Ok(mut socket) = WebSocket::connect("ws://127.0.0.1:1/x", || {}) else {
        return;
    };
    let (events, messages) = poll_until_ended(&mut session, &mut socket);
    assert_eq!(messages, 0);
    assert!(
        matches!(events.as_slice(), [SourceEvent::Failed(_)]),
        "{events:?}"
    );
}
