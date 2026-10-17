//! The connections panel: the WebSocket addresses of outposts and targets, to connect to
//! any number of them at once.

use egui_phosphor::regular;
use std::collections::{HashMap, HashSet};

/// Where an outpost started with its defaults listens.
const DEFAULT_ADDRESS: &str = "ws://127.0.0.1:3000/ws/outpost";

const CONNECTED_COLOR: egui::Color32 = egui::Color32::from_rgb(39, 174, 96);
const FAILED_COLOR: egui::Color32 = egui::Color32::from_rgb(235, 87, 87);

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct Connections {
    /// The connections in the order they were added, their addresses differ.
    connections: Vec<Connection>,
    /// The open connections and those being opened.
    #[serde(skip)]
    sockets: Vec<Socket>,
    /// Why connecting to an address failed, until it is connected again.
    #[serde(skip)]
    failed: HashMap<String, String>,
    /// What went wrong since [`Self::take_errors`] was last called, to tell the user.
    #[serde(skip)]
    errors: Vec<String>,
    #[serde(skip)]
    editing: Option<Editing>,
}

impl Default for Connections {
    fn default() -> Self {
        Self {
            connections: vec![Connection {
                name: name_of(DEFAULT_ADDRESS).to_owned(),
                address: DEFAULT_ADDRESS.to_owned(),
            }],
            sockets: vec![],
            failed: HashMap::new(),
            errors: vec![],
            editing: None,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Debug)]
struct Connection {
    /// The traces and log components received from the connection are put below the
    /// name, unless it is empty. The names that are not empty differ.
    name: String,
    address: String,
}

/// The connection to one of the addresses.
struct Socket {
    address: String,
    /// The name of the connection when it was opened, the source of its messages.
    name: String,
    sender: ewebsock::WsSender,
    receiver: ewebsock::WsReceiver,
    /// Whether the connection is open, it is being opened until then.
    open: bool,
}

/// The name or address being edited, or the address being added, and the text of its
/// field.
struct Editing {
    edited: Edited,
    field: Field,
    text: String,
}

/// Which connection is edited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Edited {
    /// The connection at this index.
    Connection(usize),
    /// A new connection, added at the end.
    New,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Name,
    Address,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Status {
    Disconnected,
    Connecting,
    Connected,
    /// Connecting failed or the open connection broke, with the reason.
    Failed(String),
}

impl Status {
    /// Whether the connection is open or being opened.
    fn is_active(&self) -> bool {
        matches!(self, Status::Connected | Status::Connecting)
    }
}

/// A change of the connections, applied after the panel is shown.
enum Action {
    Connect(usize),
    Disconnect(usize),
    StartEdit(Edited, Field),
    FinishEdit,
    CancelEdit,
    Remove(usize),
}

impl Connections {
    /// Connects to the connection at `index`, unless it is connected.
    fn connect(&mut self, index: usize, ctx: egui::Context) {
        let Connection { name, address } = self.connections[index].clone();
        if self.socket(&address).is_some() {
            return;
        }
        self.failed.remove(&address);
        let wakeup = move || ctx.request_repaint();
        let options = ewebsock::Options {
            max_incoming_frame_size: 2048,
            ..Default::default()
        };
        match ewebsock::connect_with_wakeup(address.clone(), options, wakeup) {
            Ok((sender, receiver)) => self.sockets.push(Socket {
                address,
                name,
                sender,
                receiver,
                open: false,
            }),
            Err(error) => {
                log::warn!("ws cannot connect to {address}: {error}");
                self.errors
                    .push(format!("Cannot connect to {address}: {error}"));
                self.failed.insert(address, error);
            }
        }
    }

    fn socket(&self, address: &str) -> Option<&Socket> {
        self.sockets.iter().find(|socket| socket.address == address)
    }

    fn disconnect(&mut self, address: &str) {
        self.sockets.retain_mut(|socket| {
            if socket.address != address {
                return true;
            }
            socket.sender.close();
            log::info!("ws disconnected from {address}");
            false
        });
    }

    pub fn disconnect_all(&mut self) {
        for mut socket in self.sockets.drain(..) {
            socket.sender.close();
            log::info!("ws disconnected from {}", socket.address);
        }
    }

    /// Passes the name of the connection and each message received since the last call
    /// to `receive`, ending connections when they are closed or fail. A frame holds one
    /// message or several as a CBOR sequence, e.g. a batch of a target.
    pub fn receive(&mut self, mut receive: impl FnMut(&str, &[u8])) {
        let failed = &mut self.failed;
        let errors = &mut self.errors;
        self.sockets.retain_mut(|socket| {
            while let Some(event) = socket.receiver.try_recv() {
                match event {
                    ewebsock::WsEvent::Opened => {
                        log::info!("ws connected to {}", socket.address);
                        socket.open = true;
                    }
                    ewebsock::WsEvent::Message(ewebsock::WsMessage::Binary(frame)) => {
                        for message in crate::app::model::cbor_items(&frame) {
                            match message {
                                Ok(message) => receive(&socket.name, message),
                                Err(error) => log::warn!(
                                    "error reading a message of {}: {error}",
                                    socket.address
                                ),
                            }
                        }
                    }
                    ewebsock::WsEvent::Message(ewebsock::WsMessage::Text(text)) => {
                        log::info!("{text}");
                    }
                    ewebsock::WsEvent::Message(_) => log::warn!("ws unknown message received"),
                    ewebsock::WsEvent::Error(error) => {
                        log::warn!("ws error of {}: {error}", socket.address);
                        socket.sender.close();
                        errors.push(if socket.open {
                            format!("The connection to {} broke: {error}", socket.address)
                        } else {
                            format!("Cannot connect to {}: {error}", socket.address)
                        });
                        failed.insert(socket.address.clone(), error);
                        return false;
                    }
                    ewebsock::WsEvent::Closed => {
                        log::info!("ws disconnected from {}", socket.address);
                        // the user's disconnecting drops the socket before it is closed
                        errors.push(format!("{} closed the connection", socket.address));
                        return false;
                    }
                }
            }
            true
        });
    }

    /// Takes what went wrong since the last call, e.g. why connecting failed, to tell the
    /// user.
    pub fn take_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }

    /// Sends `request` to the open connection named `name`, or drops it if there is none.
    pub fn send(&mut self, name: &str, request: &proto::Request) {
        let Some(socket) = self
            .sockets
            .iter_mut()
            .find(|socket| socket.open && socket.name == name)
        else {
            log::warn!("dropping a request to {name:?}, which is not connected");
            return;
        };
        let packet = serde_cbor_2::to_vec(request).expect("requests encode");
        log::debug!(
            "send request {}",
            hex_string::HexString::from_bytes(&packet).as_string()
        );
        socket.sender.send(ewebsock::WsMessage::Binary(packet));
    }

    /// The names of the open connections, the sources whose actions can be invoked.
    pub fn open_names(&self) -> HashSet<String> {
        self.sockets
            .iter()
            .filter(|socket| socket.open)
            .map(|socket| socket.name.clone())
            .collect()
    }

    /// Whether a connection is open.
    pub fn is_connected(&self) -> bool {
        self.sockets.iter().any(|socket| socket.open)
    }

    fn status(&self, address: &str) -> Status {
        match (self.socket(address), self.failed.get(address)) {
            (Some(socket), _) if socket.open => Status::Connected,
            (Some(_), _) => Status::Connecting,
            (None, Some(error)) => Status::Failed(error.clone()),
            (None, None) => Status::Disconnected,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut actions = vec![];
        ui.horizontal(|ui| {
            ui.strong("Connections");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(regular::PLUS)
                    .on_hover_text("Add the WebSocket address of an outpost or target")
                    .clicked()
                {
                    actions.push(Action::StartEdit(Edited::New, Field::Address));
                }
            });
        });

        let mut editing = self.editing.take();
        egui::ScrollArea::vertical()
            .id_salt("connections")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (index, connection) in self.connections.iter().enumerate() {
                    ui.horizontal(|ui| {
                        status_ui(ui, &self.status(&connection.address));
                        match editing.as_mut() {
                            Some(editing) if editing.edited == Edited::Connection(index) => {
                                edit_ui(ui, editing, &mut actions);
                            }
                            _ => self.row_ui(ui, index, connection, &mut actions),
                        }
                    });
                }
                match editing.as_mut() {
                    Some(editing) if editing.edited == Edited::New => {
                        ui.horizontal(|ui| {
                            status_ui(ui, &Status::Disconnected);
                            edit_ui(ui, editing, &mut actions);
                        });
                    }
                    _ if self.connections.is_empty() => {
                        ui.weak("Add the address of an outpost or target with +");
                    }
                    _ => {}
                }
            });
        self.editing = editing;

        for action in actions {
            self.apply(action, ui.ctx());
        }
    }

    /// The row of the connection at `index`, clicking it connects or disconnects it.
    fn row_ui(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        connection: &Connection,
        actions: &mut Vec<Action>,
    ) {
        let active = self.status(&connection.address).is_active();
        let mut text = egui::text::LayoutJob::default();
        let style = ui.style();
        if !connection.name.is_empty() {
            egui::RichText::new(format!("{}  ", connection.name))
                .strong()
                .append_to(
                    &mut text,
                    style,
                    egui::FontSelection::Default,
                    egui::Align::Center,
                );
        }
        egui::RichText::new(&connection.address)
            .color(style.visuals.weak_text_color())
            .append_to(
                &mut text,
                style,
                egui::FontSelection::Default,
                egui::Align::Center,
            );
        let response = ui
            .add(egui::Button::selectable(active, text).truncate())
            .on_hover_text(if active {
                "Click to disconnect, right-click to edit"
            } else {
                "Click to connect, right-click to edit"
            });
        let toggle = || {
            if active {
                Action::Disconnect(index)
            } else {
                Action::Connect(index)
            }
        };
        if response.clicked() {
            actions.push(toggle());
        }
        response.context_menu(|ui| {
            let (icon, text) = if active {
                (regular::PLUGS, "Disconnect")
            } else {
                (regular::PLUGS_CONNECTED, "Connect")
            };
            if ui.button(format!("{icon} {text}")).clicked() {
                actions.push(toggle());
            }
            ui.separator();
            // the name of the messages received so far stays
            if ui
                .add_enabled(
                    !active,
                    egui::Button::new(format!("{} Rename", regular::TAG)),
                )
                .on_disabled_hover_text("Disconnect to rename")
                .clicked()
            {
                actions.push(Action::StartEdit(Edited::Connection(index), Field::Name));
            }
            if ui
                .button(format!("{} Edit address", regular::PENCIL_SIMPLE))
                .clicked()
            {
                actions.push(Action::StartEdit(Edited::Connection(index), Field::Address));
            }
            if ui.button(format!("{} Delete", regular::TRASH)).clicked() {
                actions.push(Action::Remove(index));
            }
        });
    }

    fn apply(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Connect(index) => self.connect(index, ctx.clone()),
            Action::Disconnect(index) => {
                let address = self.connections[index].address.clone();
                self.disconnect(&address);
            }
            Action::StartEdit(edited, field) => {
                let text = match (edited, field) {
                    (Edited::Connection(index), Field::Name) => &self.connections[index].name,
                    (Edited::Connection(index), Field::Address) => &self.connections[index].address,
                    (Edited::New, _) => "",
                };
                self.editing = Some(Editing {
                    edited,
                    field,
                    text: text.to_owned(),
                });
            }
            Action::FinishEdit => {
                if let Some(editing) = self.editing.take() {
                    let text = editing.text.trim();
                    match (editing.edited, editing.field) {
                        (Edited::Connection(index), Field::Name) => self.rename(index, text),
                        (edited, _) => self.set_address(edited, text),
                    }
                }
            }
            Action::CancelEdit => self.editing = None,
            Action::Remove(index) => self.remove(index),
        }
    }

    /// Renames the connection at `index`, unless another connection has the name.
    fn rename(&mut self, index: usize, name: &str) {
        if self.is_named(name) {
            return;
        }
        name.clone_into(&mut self.connections[index].name);
    }

    /// Whether a connection is named `name`, which is not empty.
    fn is_named(&self, name: &str) -> bool {
        !name.is_empty() && self.connections.iter().any(|c| c.name == name)
    }

    /// Changes the address of a connection or adds a connection named after its
    /// address, unless the address is blank or of another connection. A connection
    /// whose address changes is disconnected.
    fn set_address(&mut self, edited: Edited, address: &str) {
        if address.is_empty() || self.connections.iter().any(|c| c.address == address) {
            return;
        }
        match edited {
            Edited::Connection(index) => {
                self.forget(index);
                self.connections[index].address = address.to_owned();
            }
            Edited::New => {
                let base = name_of(address);
                let name = std::iter::once(base.to_owned())
                    .chain((2..).map(|n| format!("{base} {n}")))
                    .find(|name| !self.is_named(name))
                    .expect("there is always an unused name");
                self.connections.push(Connection {
                    name,
                    address: address.to_owned(),
                });
            }
        }
    }

    fn remove(&mut self, index: usize) {
        self.forget(index);
        self.connections.remove(index);
        self.editing = None;
    }

    /// Disconnects the connection at `index` and drops why it failed.
    fn forget(&mut self, index: usize) {
        let address = self.connections[index].address.clone();
        self.disconnect(&address);
        self.failed.remove(&address);
    }
}

/// The name of a connection to `address`: the last part of its path, or its host.
fn name_of(address: &str) -> &str {
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    path.split('/')
        .rfind(|part| !part.is_empty())
        .unwrap_or("connection")
}

/// A field to edit a name or an address, ending with Enter or a click elsewhere, or
/// Escape to cancel.
fn edit_ui(ui: &mut egui::Ui, editing: &mut Editing, actions: &mut Vec<Action>) {
    let hint = match editing.field {
        Field::Name => "No name: traces are not put below it",
        Field::Address => DEFAULT_ADDRESS,
    };
    let field = egui::TextEdit::singleline(&mut editing.text)
        .hint_text(hint)
        .desired_width(f32::INFINITY);
    let response = ui.add(field);
    if response.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            actions.push(Action::CancelEdit);
        } else {
            actions.push(Action::FinishEdit);
        }
    } else {
        response.request_focus();
    }
}

/// A dot showing the status of a connection: hollow when disconnected, a spinner while
/// connecting, green when connected and red when it failed.
fn status_ui(ui: &mut egui::Ui, status: &Status) {
    let size = egui::Vec2::splat(ui.spacing().icon_width);
    let hint = match status {
        Status::Disconnected => "Disconnected".to_owned(),
        Status::Connecting => "Connecting…".to_owned(),
        Status::Connected => "Connected".to_owned(),
        Status::Failed(error) => format!("Failed: {error}"),
    };
    let response = if *status == Status::Connecting {
        ui.add_sized(size, egui::Spinner::new())
    } else {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
        let radius = size.x * 0.3;
        let painter = ui.painter();
        let center = rect.center();
        match status {
            Status::Connected => {
                painter.circle_filled(center, radius, CONNECTED_COLOR);
            }
            Status::Failed(_) => {
                painter.circle_filled(center, radius, FAILED_COLOR);
            }
            Status::Disconnected | Status::Connecting => {
                let stroke = egui::Stroke::new(1.0, ui.visuals().weak_text_color());
                painter.circle_stroke(center, radius, stroke);
            }
        }
        response
    };
    response.on_hover_text(hint);
}

#[cfg(test)]
mod tests {
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
                |ui, connections: &mut Connections| connections.ui(ui),
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
    fn an_invalid_address_fails_to_connect() {
        let mut connections = connections(&["no address"]);
        connections.apply(Action::Connect(0), &egui::Context::default());
        // natively, the error arrives from the thread connecting
        let start = std::time::Instant::now();
        while !connections.sockets.is_empty() && start.elapsed().as_secs() < 5 {
            connections.receive(|_, _| {});
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
}
