//! The connections panel: the WebSocket addresses of outposts and targets, to connect to
//! any number of them at once.

use super::theme::{self, TokensExt};
use egui_phosphor::regular;
use std::collections::{HashMap, HashSet};

/// Where an outpost started with its defaults listens.
const DEFAULT_ADDRESS: &str = "ws://127.0.0.1:3000/ws/outpost";

/// Why the connections are disabled while recording.
const RECORDING_HINT: &str = "Stop recording to change the connections";

/// The height of the list of connections from which it scrolls.
const MAX_LIST_HEIGHT: f32 = 320.0;

/// The connections of the active preset, see [`super::presets`].
pub struct Connections {
    /// The connections in the order they were added, their addresses differ.
    connections: Vec<Connection>,
    /// The open connections and those being opened.
    sockets: Vec<Socket>,
    /// Why connecting to an address failed, until it is connected again.
    failed: HashMap<String, String>,
    /// What went wrong since [`Self::take_errors`] was last called, to tell the user.
    errors: Vec<String>,
    editing: Option<Editing>,
    /// The addresses connected when the recording started, which can be connected
    /// again while recording if they break, `None` while not recording.
    recorded: Option<HashSet<String>>,
    /// The latest Hello of each address, kept after disconnecting until the next one.
    hellos: HashMap<String, proto::Hello>,
    /// What to warn the user of since [`Self::take_warnings`].
    warnings: Vec<String>,
}

impl Default for Connections {
    fn default() -> Self {
        Self {
            connections: Connection::defaults(),
            sockets: vec![],
            failed: HashMap::new(),
            errors: vec![],
            editing: None,
            recorded: None,
            hellos: HashMap::new(),
            warnings: vec![],
        }
    }
}

/// A connection as a preset saves it.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Debug)]
pub struct Connection {
    /// The name of the connection, which the actions of its outpost are sent to and
    /// which a recording keeps with each message. The names that are not empty differ.
    #[serde(default)]
    pub name: String,
    pub address: String,
}

impl Connection {
    /// `saved` kept apart as editing keeps them: trimmed, without blank addresses, the
    /// first of connections to the same address, and a name that an earlier
    /// connection has is cleared.
    pub fn sanitized(saved: Vec<Self>) -> Vec<Self> {
        let mut kept: Vec<Self> = Vec::with_capacity(saved.len());
        for mut connection in saved {
            connection.name = connection.name.trim().to_owned();
            connection.address = connection.address.trim().to_owned();
            let address = &connection.address;
            if address.is_empty() || kept.iter().any(|c| c.address == *address) {
                continue;
            }
            let name = &connection.name;
            if !name.is_empty() && kept.iter().any(|c| c.name == *name) {
                connection.name.clear();
            }
            kept.push(connection);
        }
        kept
    }

    /// The connections of a new preset: one to an outpost started with its defaults.
    pub fn defaults() -> Vec<Self> {
        vec![Connection {
            name: name_of(DEFAULT_ADDRESS).to_owned(),
            address: DEFAULT_ADDRESS.to_owned(),
        }]
    }
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
    /// Whether this connection sent its Hello, so the kept one is its own.
    greeted: bool,
    /// Whether a newer protocol was warned of on this connection.
    warned_newer: bool,
}

/// What [`Connections::receive`] passes on, in the order it arrived.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Received<'a> {
    /// A connection opened while no other was open: the first one, before any of its
    /// messages.
    FirstOpened,
    /// A message from the connection named `source`.
    Message { source: &'a str, message: &'a [u8] },
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
    /// Connected, the target speaks a newer protocol.
    Newer(u32),
    /// Connecting failed or the open connection broke, with the reason.
    Failed(String),
}

impl Status {
    /// Whether the connection is open or being opened.
    fn is_active(&self) -> bool {
        matches!(
            self,
            Status::Connected | Status::Newer(_) | Status::Connecting
        )
    }
}

/// What the row of the connection at `index` allows: none of the changes while
/// `recording`.
#[derive(Clone, Copy)]
struct Row {
    index: usize,
    recording: bool,
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
        let options = ewebsock::Options::default();
        match ewebsock::connect_with_wakeup(address.clone(), options, wakeup) {
            Ok((sender, receiver)) => self.sockets.push(Socket {
                address,
                name,
                sender,
                receiver,
                open: false,
                greeted: false,
                warned_newer: false,
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

    /// The connections as a preset saves them.
    pub fn saved(&self) -> Vec<Connection> {
        self.connections.clone()
    }

    /// Replaces the connections with those of a preset, disconnected, kept apart as
    /// [`Connection::sanitized`] does.
    pub fn set_saved(&mut self, saved: Vec<Connection>) {
        self.disconnect_all();
        self.failed.clear();
        self.hellos.clear();
        self.editing = None;
        self.connections = Connection::sanitized(saved);
    }

    pub fn disconnect_all(&mut self) {
        for mut socket in self.sockets.drain(..) {
            socket.sender.close();
            log::info!("ws disconnected from {}", socket.address);
        }
    }

    /// Passes each message received since the last call to `receive` with the name of
    /// its connection, and when the first connection opens, ending connections when they
    /// are closed or fail. A frame holds one message or several as a CBOR sequence, e.g.
    /// a batch of a target.
    pub fn receive(&mut self, mut receive: impl FnMut(Received<'_>)) {
        let failed = &mut self.failed;
        let errors = &mut self.errors;
        let hellos = &mut self.hellos;
        let warnings = &mut self.warnings;
        let mut open = self.sockets.iter().filter(|socket| socket.open).count();
        self.sockets.retain_mut(|socket| {
            while let Some(event) = socket.receiver.try_recv() {
                match event {
                    ewebsock::WsEvent::Opened => {
                        log::info!("ws connected to {}", socket.address);
                        if open == 0 {
                            receive(Received::FirstOpened);
                        }
                        open += 1;
                        socket.open = true;
                    }
                    ewebsock::WsEvent::Message(ewebsock::WsMessage::Binary(frame)) => {
                        for message in crate::app::model::cbor::items(&frame) {
                            match message {
                                Ok(message) => {
                                    if crate::app::model::decode::message_name(message)
                                        == Some("Hello")
                                    {
                                        keep_hello(socket, message, hellos, warnings);
                                    }
                                    receive(Received::Message {
                                        source: &socket.name,
                                        message,
                                    });
                                }
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
                        open -= usize::from(socket.open);
                        return false;
                    }
                    ewebsock::WsEvent::Closed => {
                        log::info!("ws disconnected from {}", socket.address);
                        // the user's disconnecting drops the socket before it is closed
                        errors.push(format!("{} closed the connection", socket.address));
                        open -= usize::from(socket.open);
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
        log::debug!("send request {request:?} to {name}");
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

    /// Whether no connection is open or being opened.
    pub fn is_idle(&self) -> bool {
        self.sockets.is_empty()
    }

    /// Connects to the first connection, as a click on its row does.
    #[cfg(test)]
    pub fn connect_first(&mut self, ctx: &egui::Context) {
        self.connect(0, ctx.clone());
    }

    /// `<name>: protocol N is newer than blip` of each open connection that speaks one.
    pub fn newer(&self) -> Vec<String> {
        self.connections
            .iter()
            .filter_map(|connection| match self.status(&connection.address) {
                Status::Newer(version) => {
                    let name = display_name(&connection.name, &connection.address);
                    Some(format!("{name}: {}", newer_text(version)))
                }
                _ => None,
            })
            .collect()
    }

    /// Takes what to warn the user of since the last call, e.g. a newer protocol.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    /// Whether the kept Hello of `address` is of its open connection, else it is faint.
    fn greeted(&self, address: &str) -> bool {
        self.socket(address)
            .is_some_and(|socket| socket.open && socket.greeted)
    }

    fn status(&self, address: &str) -> Status {
        match (self.socket(address), self.failed.get(address)) {
            (Some(socket), _) if socket.open => match self.hellos.get(address) {
                Some(hello) if socket.greeted && hello.version > proto::VERSION => {
                    Status::Newer(hello.version)
                }
                _ => Status::Connected,
            },
            (Some(_), _) => Status::Connecting,
            (None, Some(error)) => Status::Failed(error.clone()),
            (None, None) => Status::Disconnected,
        }
    }

    /// Whether the address or the name of a connection is being edited.
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Shows the connections to connect, disconnect and edit them, unless `recording`:
    /// a recording keeps the connections it started with, so they are disabled but for
    /// connecting again those that broke, and an edit is dropped.
    pub fn ui(&mut self, ui: &mut egui::Ui, recording: bool) {
        if !recording {
            self.recorded = None;
        } else if self.recorded.is_none() {
            let addresses = self
                .connections
                .iter()
                .map(|connection| &connection.address);
            let active = addresses.filter(|address| self.status(address).is_active());
            self.recorded = Some(active.cloned().collect());
        }
        let mut actions = vec![];
        theme::section_header(ui, "Connections", |ui| {
            if ui
                .add_enabled(!recording, theme::icon_button(regular::PLUS))
                .on_hover_text("Add the WebSocket address of an outpost or target")
                .on_disabled_hover_text(RECORDING_HINT)
                .clicked()
            {
                actions.push(Action::StartEdit(Edited::New, Field::Address));
            }
        });

        if recording {
            self.editing = None;
        }
        let mut editing = self.editing.take();
        egui::ScrollArea::vertical()
            .id_salt("connections")
            .auto_shrink([false, true])
            .max_height(MAX_LIST_HEIGHT)
            .show(ui, |ui| {
                for (index, connection) in self.connections.iter().enumerate() {
                    ui.horizontal(|ui| {
                        status_ui(ui, &self.status(&connection.address));
                        match editing.as_mut() {
                            Some(editing) if editing.edited == Edited::Connection(index) => {
                                edit_ui(ui, editing, &mut actions);
                            }
                            _ => {
                                let row = Row { index, recording };
                                self.row_ui(ui, connection, row, &mut actions);
                            }
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
                        theme::panel_hint(ui, "Add the address of an outpost or target with +");
                    }
                    _ => {}
                }
            });
        self.editing = editing;

        for action in actions {
            self.apply(action, ui.ctx());
        }
    }

    /// The row of a connection, clicking it connects or disconnects it, see [`Row`].
    fn row_ui(
        &self,
        ui: &mut egui::Ui,
        connection: &Connection,
        row: Row,
        actions: &mut Vec<Action>,
    ) {
        let Row { index, recording } = row;
        let active = self.status(&connection.address).is_active();
        let reconnect = recording
            && !active
            && self
                .recorded
                .as_ref()
                .is_some_and(|recorded| recorded.contains(&connection.address));
        // the name above the address, which is cut to the width of the panel
        let width = ui.available_width();
        let mut text = egui::text::LayoutJob::default();
        let style = ui.style();
        let named = !connection.name.is_empty();
        let title = if named {
            &connection.name
        } else {
            &connection.address
        };
        // the Hello of the connection, faint unless its own connection is open
        let faint = !self.greeted(&connection.address);
        let lines = self
            .hellos
            .get(&connection.address)
            .map(|hello| hello_lines(hello, &connection.name))
            .unwrap_or_default();
        let weak = if faint {
            ui.tokens().text_faint
        } else {
            style.visuals.weak_text_color()
        };
        let padding = ui.spacing().button_padding;
        let title_font = egui::TextStyle::Button.resolve(style);
        // the target name is painted at the right of the first line
        let target = lines.target.as_ref().map(|target| {
            ui.ctx()
                .fonts_mut(|fonts| fonts.layout_no_wrap(target.clone(), title_font.clone(), weak))
        });
        let title = match &target {
            Some(target) => {
                let room = width - 2.0 * padding.x - target.size().x - 8.0;
                &elide(ui, title, &title_font, room)
            }
            None => title,
        };
        egui::RichText::new(title).strong().append_to(
            &mut text,
            style,
            egui::FontSelection::Default,
            egui::Align::Min,
        );
        if named {
            let font = egui::FontId::monospace(11.0);
            let room = width - 2.0 * ui.spacing().button_padding.x;
            let address = elide(ui, &connection.address, &font, room);
            egui::RichText::new(format!("\n{address}"))
                .font(font)
                .color(style.visuals.weak_text_color())
                .append_to(
                    &mut text,
                    style,
                    egui::FontSelection::Default,
                    egui::Align::Min,
                );
        }
        if lines.detail.is_some() || lines.newer.is_some() {
            let font = egui::TextStyle::Small.resolve(style);
            let mut part = |piece: String, color: egui::Color32| {
                egui::RichText::new(piece)
                    .font(font.clone())
                    .color(color)
                    .append_to(
                        &mut text,
                        style,
                        egui::FontSelection::Default,
                        egui::Align::Min,
                    );
            };
            part("\n".to_owned(), weak);
            if let Some(detail) = &lines.detail {
                part(detail.clone(), weak);
            }
            if let Some(newer) = &lines.newer {
                let separator = if lines.detail.is_some() { ", " } else { "" };
                let color = if faint { weak } else { ui.tokens().warn };
                part(format!("{separator}{newer}"), color);
            }
        }
        // not highlighted while connected, the status dot shows it, the
        // growing atom after the text fills the rest of the width so the
        // text stays left
        let button = theme::hover_frame(
            egui::Button::new((text, egui::Atom::grow()))
                .wrap_mode(egui::TextWrapMode::Wrap)
                .min_size(egui::vec2(width, 0.0)),
        );
        let response = ui
            .add_enabled(!recording || reconnect, button)
            .on_hover_text(if reconnect {
                "Click to connect again, it was connected when the recording started"
            } else if active {
                "Click to disconnect, right-click to edit"
            } else {
                "Click to connect, right-click to edit"
            })
            .on_disabled_hover_text(RECORDING_HINT);
        if let Some(target) = target {
            let pos = egui::pos2(
                response.rect.max.x - padding.x,
                response.rect.min.y + padding.y,
            );
            ui.painter()
                .galley(pos - egui::vec2(target.size().x, 0.0), target, weak);
        }
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
        if recording {
            return;
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
                let name = super::unused_name(name_of(address), false, |name| self.is_named(name));
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
        self.hellos.remove(&address);
    }
}

/// `<newer>` as the row and the hints say it.
fn newer_text(version: u32) -> String {
    format!("protocol {version} is newer than blip")
}

/// The name of a connection, its address if it has none.
fn display_name<'a>(name: &'a str, address: &'a str) -> &'a str {
    if name.is_empty() { address } else { name }
}

/// Keeps the Hello in `message` as that of the address of `socket`, and warns once per
/// connection when its protocol is newer than blip's.
fn keep_hello(
    socket: &mut Socket,
    message: &[u8],
    hellos: &mut HashMap<String, proto::Hello>,
    warnings: &mut Vec<String>,
) {
    let hello = match serde_cbor_2::from_slice::<proto::Message>(message) {
        Ok(proto::Message::Hello(hello)) => hello,
        Ok(_) => return,
        Err(error) => {
            log::warn!("error reading the Hello of {}: {error}", socket.address);
            return;
        }
    };
    socket.greeted = true;
    if hello.version > proto::VERSION && !socket.warned_newer {
        socket.warned_newer = true;
        let name = display_name(&socket.name, &socket.address);
        let warning = format!(
            "{name} sends protocol {}, blip reads up to {}. Update blip to see all of it.",
            hello.version,
            proto::VERSION
        );
        log::warn!("{warning}");
        warnings.push(warning);
    }
    hellos.insert(socket.address.clone(), hello);
}

/// The lines a Hello adds to the row of the connection named `name`.
#[derive(Debug, PartialEq, Default)]
struct HelloLines {
    /// The target's name, unless empty or `name`.
    target: Option<String>,
    /// The sender and the clock, e.g. "sonde 1.2, time since boot".
    detail: Option<String>,
    /// "protocol N is newer than blip".
    newer: Option<String>,
}

fn hello_lines(hello: &proto::Hello, name: &str) -> HelloLines {
    let target = hello
        .name
        .as_deref()
        .filter(|target| !target.is_empty() && *target != name)
        .map(str::to_owned);
    let clock = hello.clock.map(|clock| match clock {
        proto::Clock::Epoch => "time of day",
        proto::Clock::Boot => "time since boot",
    });
    let parts: Vec<&str> = hello
        .sender
        .as_deref()
        .filter(|sender| !sender.is_empty())
        .into_iter()
        .chain(clock)
        .collect();
    HelloLines {
        target,
        detail: (!parts.is_empty()).then(|| parts.join(", ")),
        newer: (hello.version > proto::VERSION).then(|| newer_text(hello.version)),
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
    let field = theme::wide_field(&mut editing.text).hint_text(hint);
    match theme::focused_edit(ui, field) {
        Some(true) => actions.push(Action::FinishEdit),
        Some(false) => actions.push(Action::CancelEdit),
        None => {}
    }
}

/// A dot showing the status of a connection: hollow when disconnected, a spinner while
/// connecting, green when connected, amber when it speaks a newer protocol and red when it failed.
fn status_ui(ui: &mut egui::Ui, status: &Status) {
    let size = egui::Vec2::splat(ui.spacing().icon_width);
    let hint = match status {
        Status::Disconnected => "Disconnected".to_owned(),
        Status::Connecting => "Connecting…".to_owned(),
        Status::Connected => "Connected".to_owned(),
        Status::Newer(version) => format!("Connected, {}", newer_text(*version)),
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
                painter.circle_filled(center, radius, ui.tokens().ok);
            }
            Status::Newer(_) => {
                painter.circle_filled(center, radius, ui.tokens().warn);
            }
            Status::Failed(_) => {
                painter.circle_filled(center, radius, ui.tokens().error);
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

/// `text` in `font`, cut with an ellipsis to fit `width`.
fn elide(ui: &egui::Ui, text: &str, font: &egui::FontId, width: f32) -> String {
    let fits = |text: &str| {
        let galley = ui.ctx().fonts_mut(|fonts| {
            fonts.layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER)
        });
        galley.size().x <= width
    };
    if fits(text) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len())
        .rev()
        .map(|count| {
            chars[..count]
                .iter()
                .chain(['…'].iter())
                .collect::<String>()
        })
        .find(|elided| fits(elided))
        .unwrap_or_else(|| "…".to_owned())
}

#[cfg(test)]
mod tests;
