use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn connections(addresses: &[&str]) -> Connections {
    let mut connections = Connections {
        connections: vec![],
        ..Default::default()
    };
    for address in addresses {
        connections.set_address(Edited::New, address, &mut Session::default());
    }
    connections
}

fn names(connections: &Connections) -> Vec<&str> {
    connections
        .connections
        .iter()
        .map(|c| c.name.as_str())
        .collect()
}

fn addresses(connections: &Connections) -> Vec<&str> {
    connections
        .connections
        .iter()
        .map(|c| c.address.as_str())
        .collect()
}

#[test]
fn connections_of_a_preset_keep_their_addresses_and_names_apart() {
    let connection = |name: &str, address: &str| Connection {
        name: name.to_owned(),
        address: address.to_owned(),
    };
    let mut connections = Connections::default();
    connections.set_saved(
        vec![
            connection("a", " ws://a "),
            connection("b", "ws://a"),
            connection("a", "ws://b"),
            connection("c", " "),
        ],
        &mut Session::default(),
    );
    assert_eq!(names(&connections), ["a", ""]);
    assert_eq!(addresses(&connections), ["ws://a", "ws://b"]);
    assert_eq!(connections.saved().len(), 2);
}

#[test]
fn connections_are_named_after_their_address() {
    assert_eq!(name_of("ws://127.0.0.1:3000/ws/outpost"), "outpost");
    assert_eq!(name_of("ws://robby/"), "robby");
    assert_eq!(name_of("ws://host:80/a?b"), "a");
    assert_eq!(name_of(""), "connection");
    let connections = connections(&["ws://a/ws/outpost", "ws://b/ws/outpost", "ws://c/robby"]);
    assert_eq!(names(&connections), ["outpost", "outpost 2", "robby"]);
}

#[test]
fn names_differ_unless_they_are_empty() {
    let mut connections = connections(&["ws://a/x", "ws://b/y", "ws://c/z"]);
    connections.rename(0, "y");
    connections.rename(0, "");
    connections.rename(1, "");
    connections.rename(2, "x");
    assert_eq!(names(&connections), ["", "", "x"]);
}

#[test]
fn blank_and_listed_addresses_are_not_added() {
    let mut connections = connections(&["ws://a"]);
    let session = &mut Session::default();
    connections.set_address(Edited::New, "", session);
    connections.set_address(Edited::New, "ws://a", session);
    connections.set_address(Edited::New, "ws://b", session);
    assert_eq!(addresses(&connections), ["ws://a", "ws://b"]);
    connections.set_address(Edited::Connection(0), "ws://b", session);
    connections.set_address(Edited::Connection(0), "ws://c", session);
    assert_eq!(addresses(&connections), ["ws://c", "ws://b"]);
}

#[test]
fn changing_or_removing_a_failed_address_forgets_the_failure() {
    let mut connections = connections(&["ws://a", "ws://b"]);
    connections
        .failed
        .insert("ws://a".to_owned(), "refused".to_owned());
    assert_eq!(
        connections.status("ws://a"),
        Status::Failed("refused".to_owned())
    );
    assert_eq!(connections.status("ws://b"), Status::Disconnected);
    let mut session = Session::default();
    connections.remove(1, &mut session);
    assert!(!connections.failed.is_empty());
    connections.remove(0, &mut session);
    assert!(connections.failed.is_empty());
    assert!(connections.connections.is_empty());
}

#[test]
fn several_connections_are_opened_at_once() {
    let mut connections = connections(&["ws://127.0.0.1:1/a", "ws://127.0.0.1:1/b"]);
    let ctx = egui::Context::default();
    let mut session = Session::default();
    connections.apply(Action::Connect(0), &ctx, &mut session);
    connections.apply(Action::Connect(1), &ctx, &mut session);
    connections.apply(Action::Connect(1), &ctx, &mut session);
    assert_eq!(connections.sockets.len(), 2);
    let sources: Vec<&str> = connections
        .sockets
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(sources, ["a", "b"]);
    connections.apply(Action::Disconnect(0), &ctx, &mut session);
    assert_eq!(
        connections.status("ws://127.0.0.1:1/a"),
        Status::Disconnected
    );
    assert!(connections.socket("ws://127.0.0.1:1/b").is_some());
}

#[test]
fn an_address_is_added_with_the_plus_button() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (connections, session): &mut (Connections, Session)| {
                connections.ui(ui, session);
            },
            (connections(&[]), Session::default()),
        );
    harness.run();
    harness.get_by_label(regular::PLUS).click();
    harness.run();
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .type_text("ws://target/ws");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().0.connections,
        [Connection {
            name: "ws".to_owned(),
            address: "ws://target/ws".to_owned()
        }]
    );
    assert!(harness.state().0.editing.is_none());
}

#[test]
fn the_connections_cannot_be_changed_while_recording() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (connections, session): &mut (Connections, Session)| {
                connections.ui(ui, session);
            },
            (connections(&["ws://127.0.0.1:1/a"]), Session::recording()),
        );
    harness.run();
    harness.get_by_label(regular::PLUS).click();
    harness.get_by_label_contains("ws://127.0.0.1:1/a").click();
    harness.run();
    assert!(harness.state().0.editing.is_none());
    assert!(harness.state().0.sockets.is_empty());

    harness.state_mut().1.recording = false;
    harness.run();
    harness.get_by_label(regular::PLUS).click();
    harness.run();
    assert!(harness.state().0.editing.is_some());
    // starting to record drops the edit
    harness.state_mut().1.recording = true;
    harness.run();
    assert!(harness.state().0.editing.is_none());
    assert_eq!(addresses(&harness.state().0), ["ws://127.0.0.1:1/a"]);
}

#[test]
fn connections_that_broke_while_recording_can_be_connected_again() {
    let (a, b) = ("ws://127.0.0.1:1/a", "ws://127.0.0.1:1/b");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (connections, session): &mut (Connections, Session)| {
                connections.ui(ui, session);
            },
            (connections(&[a, b]), Session::default()),
        );
    let ctx = harness.ctx.clone();
    let (connections, session) = harness.state_mut();
    connections.apply(Action::Connect(0), &ctx, session);
    session.recording = true;
    harness.run_steps(2);
    // a breaks: it can be connected again, b was not connected and cannot be
    let (connections, session) = harness.state_mut();
    connections.disconnect(a, session);
    harness.run_steps(2);
    harness.get_by_label_contains(b).click();
    harness.get_by_label_contains(a).click();
    harness.run_steps(2);
    assert!(harness.state().0.status(a).is_active());
    assert!(!harness.state().0.status(b).is_active());
}

#[test]
fn a_connection_that_broke_while_the_list_was_closed_can_be_connected_again() {
    let (a, b) = ("ws://127.0.0.1:1/a", "ws://127.0.0.1:1/b");
    // the list is shown only once the recording runs
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (connections, session): &mut (Connections, Session)| {
                if session.recording {
                    connections.ui(ui, session);
                }
            },
            (connections(&[a, b]), Session::default()),
        );
    // the recording starts and a breaks before then
    let ctx = harness.ctx.clone();
    let (connections, session) = harness.state_mut();
    connections.apply(Action::Connect(0), &ctx, session);
    connections.set_recording(true);
    connections.disconnect(a, session);
    session.recording = true;
    harness.run_steps(2);
    harness.get_by_label_contains(b).click();
    harness.get_by_label_contains(a).click();
    harness.run_steps(2);
    assert!(harness.state().0.status(a).is_active());
    assert!(!harness.state().0.status(b).is_active());
    // the next recording starts with what is connected then
    let (connections, session) = harness.state_mut();
    connections.disconnect(a, session);
    connections.set_recording(false);
    harness.run_steps(2);
    harness.get_by_label_contains(a).click();
    harness.run_steps(2);
    assert!(!harness.state().0.status(a).is_active());
}

#[test]
fn a_connected_row_is_not_highlighted() {
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (connections, session): &mut (Connections, Session)| {
                connections.ui(ui, session);
            },
            (connections(&["ws://127.0.0.1:1/a"]), Session::default()),
        );
    harness.run();
    let ctx = harness.ctx.clone();
    let (connections, session) = harness.state_mut();
    connections.apply(Action::Connect(0), &ctx, session);
    // the spinner of the connecting row keeps repainting
    harness.run_steps(2);
    assert!(harness.state().0.status("ws://127.0.0.1:1/a").is_active());
    let row = harness.get_by_label_contains("ws://127.0.0.1:1/a");
    assert_ne!(row.accesskit_node().toggled(), Some(Toggled::True));
}

/// Serves WebSocket clients one after the other on a free local port, sending each
/// `frame` and then reading until the client leaves. Returns the address, ending
/// with `path`.
fn serve(path: &str, frame: Vec<u8>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("ws://{}/{path}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut socket) = tungstenite::accept(stream.unwrap()) else {
                continue;
            };
            socket
                .send(tungstenite::Message::Binary(frame.clone()))
                .unwrap();
            while socket.read().is_ok() {}
        }
    });
    address
}

/// Updates `session` and lets `connections` observe it until `count` messages arrived, or
/// 5 s passed. Returns the messages and whether an update started afresh.
fn receive_some(
    connections: &mut Connections,
    session: &mut Session,
    count: usize,
) -> (usize, bool) {
    let (mut messages, mut fresh) = (0, false);
    let start = std::time::Instant::now();
    while messages < count && start.elapsed().as_secs() < 5 {
        let update = session.update(0.0);
        connections.observe(session);
        messages += update.messages;
        fresh |= update.started_fresh;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    (messages, fresh)
}

fn announce(path: &str) -> Vec<u8> {
    serde_cbor_2::to_vec(&proto::Message::Actions(vec![proto::ActionDescriptor {
        path: path.to_owned(),
        description: String::new(),
        params: vec![],
        readback: None,
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]))
    .unwrap()
}

#[test]
fn the_first_connection_to_open_is_passed_on_before_its_messages() {
    let mut connections = connections(&[&serve("a", announce("x")), &serve("b", announce("y"))]);
    let mut session = Session::default();
    let ctx = egui::Context::default();
    connections.apply(Action::Connect(0), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 1), (1, true));
    assert!(session.store.actions.announced("a", "x").is_some());
    connections.apply(Action::Connect(1), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 1), (1, false));
    assert!(session.store.actions.announced("a", "x").is_some());
    assert!(session.store.actions.announced("b", "y").is_some());

    connections.apply(Action::Disconnect(0), &ctx, &mut session);
    connections.apply(Action::Disconnect(1), &ctx, &mut session);
    connections.apply(Action::Connect(1), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 1), (1, true));
    assert!(session.store.actions.announced("a", "x").is_none());
    assert!(session.store.actions.announced("b", "y").is_some());
    assert!(session.take_notices().is_empty());
}

#[test]
fn an_invalid_address_fails_to_connect() {
    let mut connections = connections(&["no address"]);
    let mut session = Session::default();
    connections.apply(Action::Connect(0), &egui::Context::default(), &mut session);
    // natively, the error arrives from the thread connecting
    let start = std::time::Instant::now();
    while !connections.sockets.is_empty() && start.elapsed().as_secs() < 5 {
        session.update(0.0);
        connections.observe(&session);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(
        connections.status("no address"),
        Status::Failed(_)
    ));
    let notices = session.take_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, Level::Error);
    assert!(notices[0].1.starts_with("Cannot connect to no address: "));
    assert!(session.take_notices().is_empty());
}

fn hello(version: u32, name: Option<&str>, clock: Option<proto::Clock>) -> proto::Hello {
    proto::Hello {
        version,
        name: name.map(str::to_owned),
        clock,
        sender: Some("sonde 1.2".to_owned()),
    }
}

/// The frame of the Hellos, a CBOR sequence.
fn hellos_frame(hellos: &[proto::Hello]) -> Vec<u8> {
    hellos
        .iter()
        .flat_map(|hello| serde_cbor_2::to_vec(&proto::Message::Hello(hello.clone())).unwrap())
        .collect()
}

#[test]
fn hello_lines_name_the_target_sender_and_clock() {
    let lines = hello_lines(&hello(1, Some("rig-7"), Some(proto::Clock::Boot)), "bench");
    assert_eq!(
        lines,
        HelloLines {
            target: Some("rig-7".to_owned()),
            detail: Some("sonde 1.2, time since boot".to_owned()),
            newer: None,
        }
    );

    let epoch = proto::Hello {
        version: 1,
        name: Some("bench".to_owned()),
        clock: Some(proto::Clock::Epoch),
        sender: None,
    };
    let lines = hello_lines(&epoch, "bench");
    assert_eq!(lines.target, None);
    assert_eq!(lines.detail.as_deref(), Some("time of day"));

    let newer = proto::Hello {
        version: 2,
        sender: Some("blip-outpost 0.4".to_owned()),
        ..Default::default()
    };
    let lines = hello_lines(&newer, "bench");
    assert_eq!(lines.detail.as_deref(), Some("blip-outpost 0.4"));
    assert_eq!(
        lines.newer.as_deref(),
        Some("protocol 2 is newer than blip")
    );

    let bare = proto::Hello {
        version: 1,
        ..Default::default()
    };
    assert_eq!(hello_lines(&bare, "bench"), HelloLines::default());
    let empty = proto::Hello {
        name: Some(String::new()),
        ..bare
    };
    assert_eq!(hello_lines(&empty, "bench").target, None);
}

#[test]
fn a_hello_is_kept_and_replaced() {
    let frame = hellos_frame(&[
        hello(1, Some("one"), Some(proto::Clock::Boot)),
        hello(1, Some("two"), Some(proto::Clock::Boot)),
    ]);
    let address = serve("a", frame);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 200.0))
        .build_ui_state(
            |ui, (connections, session): &mut (Connections, Session)| {
                session.update(0.0);
                connections.observe(session);
                connections.ui(ui, session);
            },
            (connections(&[&address]), Session::default()),
        );
    let ctx = harness.ctx.clone();
    let (connections, session) = harness.state_mut();
    connections.apply(Action::Connect(0), &ctx, session);
    let start = std::time::Instant::now();
    while !harness.state().0.greeted(&address) && start.elapsed().as_secs() < 5 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // both Hellos arrive in one frame
    while harness.state().0.hellos[&address].name.as_deref() != Some("two")
        && start.elapsed().as_secs() < 5
    {
        harness.step();
    }
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.hellos[&address].name.as_deref(),
        Some("two")
    );
    harness.get_by_label_contains("sonde 1.2, time since boot");

    let (connections, session) = harness.state_mut();
    connections.apply(Action::Disconnect(0), &ctx, session);
    harness.run_steps(2);
    assert!(harness.state().0.hellos.contains_key(&address));
    assert!(!harness.state().0.greeted(&address));
    harness.get_by_label_contains("sonde 1.2, time since boot");

    let (connections, session) = harness.state_mut();
    connections.apply(Action::Connect(0), &ctx, session);
    let start = std::time::Instant::now();
    while !harness.state().0.greeted(&address) && start.elapsed().as_secs() < 5 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(harness.state().0.greeted(&address));
}

#[test]
fn a_newer_protocol_is_warned_once_per_connect() {
    let sample = serde_cbor_2::to_vec(&proto::Message::Samples(proto::Samples {
        path: "x".to_owned(),
        values: vec![proto::Data::Uint(1)],
        times: proto::SampleTimes::Period {
            timestamp: 0,
            period: 1,
        },
    }))
    .unwrap();
    let v2 = hello(2, None, None);
    let mut frame = hellos_frame(std::slice::from_ref(&v2));
    frame.extend(&sample);
    let mut twice = hellos_frame(&[v2.clone(), v2]);
    twice.extend(&sample);
    let mut old = hellos_frame(&[hello(1, None, None)]);
    old.extend(&sample);
    let ctx = egui::Context::default();
    let mut session = Session::default();

    let mut connections = connections(&[&serve("a", frame), &serve("b", twice), &serve("c", old)]);
    connections.apply(Action::Connect(0), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 2).0, 2);
    let warning = "a sends protocol 2, blip reads up to 1. Update blip to see all of it.";
    assert_eq!(connections.take_warnings(), [warning]);
    let address = connections.connections[0].address.clone();
    assert_eq!(connections.status(&address), Status::Newer(2));
    assert_eq!(connections.newer(), ["a: protocol 2 is newer than blip"]);

    connections.apply(Action::Connect(1), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 3).0, 3);
    assert_eq!(connections.take_warnings().len(), 1);

    connections.apply(Action::Disconnect(0), &ctx, &mut session);
    connections.apply(Action::Connect(0), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 2).0, 2);
    assert_eq!(connections.take_warnings(), [warning]);

    connections.apply(Action::Connect(2), &ctx, &mut session);
    assert_eq!(receive_some(&mut connections, &mut session, 2).0, 2);
    assert!(connections.take_warnings().is_empty());
    let address = connections.connections[2].address.clone();
    assert_eq!(connections.status(&address), Status::Connected);
    assert_eq!(connections.newer().len(), 2);
}

/// Like [`serve`], and answers each invocation it reads with an accepted result.
fn serve_answering(path: &str, frame: Vec<u8>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("ws://{}/{path}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut socket) = tungstenite::accept(stream.unwrap()) else {
                continue;
            };
            socket
                .send(tungstenite::Message::Binary(frame.clone()))
                .unwrap();
            while let Ok(message) = socket.read() {
                let tungstenite::Message::Binary(bytes) = message else {
                    continue;
                };
                let Ok(proto::Request::Invoke(invocation)) = serde_cbor_2::from_slice(&bytes)
                else {
                    continue;
                };
                let result = proto::Message::ActionResult(proto::ActionResult {
                    id: invocation.id,
                    result: Ok(()),
                });
                let answer = serde_cbor_2::to_vec(&result).unwrap();
                socket.send(tungstenite::Message::Binary(answer)).unwrap();
            }
        }
    });
    address
}

/// Updates `session` and lets `connections` observe it until `done`, or 5 s passed.
fn update_until(
    connections: &mut Connections,
    session: &mut Session,
    done: impl Fn(&Connections, &Session) -> bool,
) {
    let start = std::time::Instant::now();
    while !done(connections, session) && start.elapsed().as_secs() < 5 {
        session.update(0.0);
        connections.observe(session);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn connections_are_sources_of_the_session() {
    use crate::store::actions::{ActionId, Outcome};

    let mut messages = vec![
        proto::Message::Hello(hello(1, Some("rig-7"), Some(proto::Clock::Boot))),
        proto::Message::Actions(vec![proto::ActionDescriptor {
            path: "reset".to_owned(),
            description: String::new(),
            params: vec![],
            readback: None,
            confirm: false,
            enabled: true,
            disabled_reason: None,
        }]),
        proto::Message::Sample(proto::Sample {
            timestamp: 1_000,
            path: "rpm".to_owned(),
            value: proto::Data::Float(1.0),
        }),
        proto::Message::Log(proto::LogRecord {
            timestamp: 2_000,
            path: "log/rig".to_owned(),
            level: proto::level::WARN,
            message: "hot".to_owned(),
        }),
    ];
    let frame: Vec<u8> = messages
        .drain(..)
        .flat_map(|message| serde_cbor_2::to_vec(&message).unwrap())
        .collect();
    let address = serve_answering("rig", frame.clone());
    let mut connections = connections(&[&address]);
    let mut session = Session::recording();
    let ctx = egui::Context::default();
    connections.apply(Action::Connect(0), &ctx, &mut session);

    // the connections poll nothing: only the session does
    for _ in 0..5 {
        connections.observe(&session);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(session.store.traces().count(), 0);
    assert_eq!(session.recorder.messages(), 0);
    assert!(!connections.is_connected());

    assert_eq!(receive_some(&mut connections, &mut session, 4).0, 4);
    assert!(session.store.traces().any(|trace| trace.path == "rpm"));
    assert_eq!(
        session
            .store
            .logs(crate::store::Window::ALL)
            .data
            .entries()
            .count(),
        1
    );
    assert!(session.store.actions.announced("rig", "reset").is_some());
    assert_eq!(session.open_sources(), HashSet::from(["rig".to_owned()]));
    assert_eq!(connections.hellos[&address].name.as_deref(), Some("rig-7"));
    assert!(connections.greeted(&address));

    // the same frame through an inbox makes the same recording
    let mut other = Session::recording();
    let mut inbox = other.inbox("rig", 0.0);
    inbox.opened();
    inbox.frame(&frame);
    assert_eq!(other.recorder.to_file(), session.recorder.to_file());

    let action = ActionId::new("rig", "reset");
    session.invoker.invoke(&action, None, vec![], 0.0);
    session.send_invocations();
    update_until(&mut connections, &mut session, |_, session| {
        session.invoker.outcome(&action, 0.0) != Some(&Outcome::Pending)
    });
    assert!(matches!(
        session.invoker.outcome(&action, 0.0),
        Some(Outcome::Accepted(_))
    ));
    assert!(session.take_notices().is_empty());
}

#[test]
fn two_connections_without_a_name_are_told_apart() {
    let sample = |path: &str| {
        serde_cbor_2::to_vec(&proto::Message::Sample(proto::Sample {
            timestamp: 1_000,
            path: path.to_owned(),
            value: proto::Data::Float(1.0),
        }))
        .unwrap()
    };
    let mut frame_a = hellos_frame(&[hello(1, Some("one"), None)]);
    frame_a.extend(sample("x"));
    let mut frame_b = hellos_frame(&[hello(1, Some("two"), None)]);
    frame_b.extend(sample("y"));
    let a = serve("a", frame_a);
    let b = serve("b", frame_b);
    let c = "ws://127.0.0.1:1/c".to_owned();
    let mut connections = connections(&[&a, &b, &c]);
    let mut session = Session::recording();
    for index in 0..3 {
        connections.rename(index, "");
    }
    let ctx = egui::Context::default();
    connections.apply(Action::Connect(0), &ctx, &mut session);
    connections.apply(Action::Connect(1), &ctx, &mut session);
    update_until(&mut connections, &mut session, |connections, _| {
        connections.greeted(&a) && connections.greeted(&b)
    });
    assert_eq!(connections.hellos[&a].name.as_deref(), Some("one"));
    assert_eq!(connections.hellos[&b].name.as_deref(), Some("two"));
    update_until(&mut connections, &mut session, |_, session| {
        session.store.traces().count() == 2
    });
    let paths: Vec<&str> = session.store.traces().map(|trace| trace.path).collect();
    assert!(paths.contains(&"x") && paths.contains(&"y"), "{paths:?}");
    let unnamed = HashSet::from([String::new()]);
    assert_eq!(session.open_sources(), unnamed);

    connections.apply(Action::Connect(2), &ctx, &mut session);
    update_until(&mut connections, &mut session, |connections, _| {
        matches!(connections.status(&c), Status::Failed(_))
    });
    let notices = session.take_notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, Level::Error);
    assert!(
        notices[0]
            .1
            .starts_with("Cannot connect to ws://127.0.0.1:1/c: "),
        "{notices:?}"
    );
    assert_eq!(connections.status(&a), Status::Connected);
    assert_eq!(connections.status(&b), Status::Connected);

    connections.apply(Action::Disconnect(0), &ctx, &mut session);
    assert_eq!(connections.status(&a), Status::Disconnected);
    assert_eq!(connections.status(&b), Status::Connected);
    assert!(connections.is_connected());
    assert_eq!(session.open_sources(), unnamed);
    assert!(session.take_notices().is_empty());
}
