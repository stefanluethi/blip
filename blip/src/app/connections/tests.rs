use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn connections(addresses: &[&str]) -> Connections {
    let mut connections = Connections {
        connections: vec![],
        ..Default::default()
    };
    for address in addresses {
        connections.set_address(Edited::New, address);
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
    connections.set_saved(vec![
        connection("a", " ws://a "),
        connection("b", "ws://a"),
        connection("a", "ws://b"),
        connection("c", " "),
    ]);
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
    connections.set_address(Edited::New, "");
    connections.set_address(Edited::New, "ws://a");
    connections.set_address(Edited::New, "ws://b");
    assert_eq!(addresses(&connections), ["ws://a", "ws://b"]);
    connections.set_address(Edited::Connection(0), "ws://b");
    connections.set_address(Edited::Connection(0), "ws://c");
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
    connections.remove(1);
    assert!(!connections.failed.is_empty());
    connections.remove(0);
    assert!(connections.failed.is_empty());
    assert!(connections.connections.is_empty());
}

#[test]
fn several_connections_are_opened_at_once() {
    let mut connections = connections(&["ws://127.0.0.1:1/a", "ws://127.0.0.1:1/b"]);
    let ctx = egui::Context::default();
    connections.apply(Action::Connect(0), &ctx);
    connections.apply(Action::Connect(1), &ctx);
    connections.apply(Action::Connect(1), &ctx);
    assert_eq!(connections.sockets.len(), 2);
    let sources: Vec<&str> = connections
        .sockets
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(sources, ["a", "b"]);
    connections.apply(Action::Disconnect(0), &ctx);
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
            |ui, connections: &mut Connections| connections.ui(ui, false),
            connections(&[]),
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
        harness.state().connections,
        [Connection {
            name: "ws".to_owned(),
            address: "ws://target/ws".to_owned()
        }]
    );
    assert!(harness.state().editing.is_none());
}

#[test]
fn the_connections_cannot_be_changed_while_recording() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (connections, recording): &mut (Connections, bool)| {
                connections.ui(ui, *recording);
            },
            (connections(&["ws://127.0.0.1:1/a"]), true),
        );
    harness.run();
    harness.get_by_label(regular::PLUS).click();
    harness.get_by_label_contains("ws://127.0.0.1:1/a").click();
    harness.run();
    assert!(harness.state().0.editing.is_none());
    assert!(harness.state().0.sockets.is_empty());

    harness.state_mut().1 = false;
    harness.run();
    harness.get_by_label(regular::PLUS).click();
    harness.run();
    assert!(harness.state().0.editing.is_some());
    // starting to record drops the edit
    harness.state_mut().1 = true;
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
            |ui, (connections, recording): &mut (Connections, bool)| {
                connections.ui(ui, *recording);
            },
            (connections(&[a, b]), false),
        );
    let ctx = harness.ctx.clone();
    harness.state_mut().0.apply(Action::Connect(0), &ctx);
    harness.state_mut().1 = true;
    harness.run_steps(2);
    // a breaks: it can be connected again, b was not connected and cannot be
    harness.state_mut().0.disconnect(a);
    harness.run_steps(2);
    harness.get_by_label_contains(b).click();
    harness.get_by_label_contains(a).click();
    harness.run_steps(2);
    assert!(harness.state().0.status(a).is_active());
    assert!(!harness.state().0.status(b).is_active());
}

#[test]
fn a_connected_row_is_not_highlighted() {
    use egui::accesskit::Toggled;
    use egui_kittest::kittest::NodeT;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, connections: &mut Connections| connections.ui(ui, false),
            connections(&["ws://127.0.0.1:1/a"]),
        );
    harness.run();
    let ctx = harness.ctx.clone();
    harness.state_mut().apply(Action::Connect(0), &ctx);
    // the spinner of the connecting row keeps repainting
    harness.run_steps(2);
    assert!(harness.state().status("ws://127.0.0.1:1/a").is_active());
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

/// What `connections` receive until `count` things arrived, or 5 s passed.
fn receive_some(connections: &mut Connections, count: usize) -> Vec<String> {
    let mut received = vec![];
    let start = std::time::Instant::now();
    while received.len() < count && start.elapsed().as_secs() < 5 {
        connections.receive(|r| {
            received.push(match r {
                Received::FirstOpened => "first opened".to_owned(),
                Received::Message { source, message } => format!("{source} {message:?}"),
            });
        });
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    received
}

#[test]
fn the_first_connection_to_open_is_passed_on_before_its_messages() {
    let mut connections = connections(&[&serve("a", vec![1]), &serve("b", vec![2])]);
    let ctx = egui::Context::default();
    connections.apply(Action::Connect(0), &ctx);
    assert_eq!(receive_some(&mut connections, 2), ["first opened", "a [1]"]);
    connections.apply(Action::Connect(1), &ctx);
    assert_eq!(receive_some(&mut connections, 1), ["b [2]"]);

    connections.apply(Action::Disconnect(0), &ctx);
    connections.apply(Action::Disconnect(1), &ctx);
    connections.apply(Action::Connect(1), &ctx);
    assert_eq!(receive_some(&mut connections, 2), ["first opened", "b [2]"]);
    assert!(connections.take_errors().is_empty());
}

#[test]
fn an_invalid_address_fails_to_connect() {
    let mut connections = connections(&["no address"]);
    connections.apply(Action::Connect(0), &egui::Context::default());
    // natively, the error arrives from the thread connecting
    let start = std::time::Instant::now();
    while !connections.sockets.is_empty() && start.elapsed().as_secs() < 5 {
        connections.receive(|_| {});
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(
        connections.status("no address"),
        Status::Failed(_)
    ));
    let errors = connections.take_errors();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("Cannot connect to no address: "));
    assert!(connections.take_errors().is_empty());
}
