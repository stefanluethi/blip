//! The connections panel: the WebSocket addresses of outposts and targets, to connect to
//! any number of them at once.

use crate::store::session::{Session, SourceEvent, SourceId, cannot_connect};
use crate::store::websocket::WebSocket;
use crate::store::{Ingest, Level, Store};
use crate::widgets::theme::{self, TokensExt};
use egui_phosphor::regular;
use std::collections::{HashMap, HashSet};

/// Where an outpost started with its defaults listens.
const DEFAULT_ADDRESS: &str = "ws://127.0.0.1:3000/ws/outpost";

/// Why the connections are disabled while recording.
const RECORDING_HINT: &str = "Stop recording to change the connections";

/// What a failed connection tells when the other side closed it.
const CLOSED_REASON: &str = "closed by the other side";

/// What to check when a connection cannot be made.
const OUTPOST_HINT: &str = "Is the outpost running?";

/// The scheme given to an address typed without one.
const DEFAULT_SCHEME: &str = "ws://";

/// The height of the list of connections from which it scrolls.
const MAX_LIST_HEIGHT: f32 = 320.0;

/// The connections of the active preset, see [`super::presets`]. Each open connection, and
/// each one being opened, is a WebSocket source of the session; the connections show what
/// the session tells of it.
pub struct Connections {
    /// The connections in the order they were added, their addresses differ.
    connections: Vec<Connection>,
    /// The open connections and those being opened.
    sockets: Vec<Socket>,
    /// Why connecting to an address failed or its connection broke, until it is
    /// connected again.
    failed: HashMap<String, Failure>,
    editing: Option<Editing>,
    /// The index of the connection whose deletion is asked to be confirmed.
    deleting: Option<usize>,
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
            editing: None,
            deleting: None,
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
    /// The source of the connection in the session.
    id: SourceId,
    address: String,
    /// The name of the connection when it was opened, the source of its messages.
    name: String,
    /// Whether the connection is open, it is being opened until then.
    open: bool,
    /// Whether this connection sent its Hello, so the kept one is its own.
    greeted: bool,
    /// Whether a newer protocol was warned of on this connection.
    warned_newer: bool,
}

/// What the connections do as a whole, e.g. for the status chip of the app, see
/// [`Connections::status`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// No connection is open or being opened, and none failed.
    None,
    /// A connection is being opened and none is open.
    Connecting,
    /// A connection is open. `problem` when one speaks a newer protocol or another
    /// one failed.
    Open { problem: bool },
    /// No connection is open or being opened, and one failed. `lost` when one broke or
    /// was closed after it was open, else connecting failed.
    Failed { lost: bool },
}

/// Why connecting to an address failed or its open connection ended.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Failure {
    reason: String,
    /// Whether the connection was open, so it was lost rather than not made.
    lost: bool,
}

/// The name or address being edited, or the address being added, and the text of its
/// field.
struct Editing {
    edited: Edited,
    field: Field,
    text: String,
    /// Whether finishing was refused, so a blank text tells why too.
    refused: bool,
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
enum State {
    Disconnected,
    Connecting,
    Connected,
    /// Connected, the target speaks a newer protocol.
    Newer(u32),
    /// Connecting failed, with the reason.
    Failed(String),
    /// The open connection broke or was closed by the other side, with the reason.
    Lost(String),
}

impl State {
    /// Whether the connection is open or being opened.
    fn is_active(&self) -> bool {
        matches!(self, State::Connected | State::Newer(_) | State::Connecting)
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
    /// Ends editing, with Enter if `entered`, else by a click elsewhere.
    FinishEdit {
        entered: bool,
    },
    CancelEdit,
    /// Asks to confirm deleting the connection at this index.
    AskRemove(usize),
    CancelRemove,
    Remove(usize),
}

impl Connections {
    /// Connects to the connection at `index`, unless it is connected.
    fn connect<S: Ingest + Store>(
        &mut self,
        index: usize,
        ctx: egui::Context,
        session: &mut Session<S>,
    ) {
        let Connection { name, address } = self.connections[index].clone();
        if self.socket(&address).is_some() {
            return;
        }
        self.failed.remove(&address);
        let wakeup = move || ctx.request_repaint();
        match WebSocket::connect(&address, wakeup) {
            Ok(socket) => {
                let id = session.add_source(&name, socket);
                session.set_label(id, &address);
                session.set_hint(id, OUTPOST_HINT);
                self.sockets.push(Socket {
                    id,
                    address,
                    name,
                    open: false,
                    greeted: false,
                    warned_newer: false,
                });
            }
            Err(error) => {
                log::warn!("ws cannot connect to {address}: {error}");
                let text = cannot_connect(&address, &error, Some(OUTPOST_HINT));
                session.notify(Level::Error, text);
                let failure = Failure {
                    reason: error,
                    lost: false,
                };
                self.failed.insert(address, failure);
            }
        }
    }

    fn socket(&self, address: &str) -> Option<&Socket> {
        self.sockets.iter().find(|socket| socket.address == address)
    }

    fn disconnect<S: Ingest + Store>(&mut self, address: &str, session: &mut Session<S>) {
        self.sockets.retain_mut(|socket| {
            if socket.address != address {
                return true;
            }
            session.remove_source(socket.id);
            log::info!("ws disconnected from {address}");
            false
        });
    }

    /// The connections as a preset saves them.
    pub fn saved(&self) -> Vec<Connection> {
        self.connections.clone()
    }

    /// Replaces the connections with those of a preset, kept apart as
    /// [`Connection::sanitized`] does. A connection open or being opened stays when the
    /// preset has a connection of the same name and address, the others are
    /// disconnected: their sources are removed from `session`.
    pub fn set_saved<S: Ingest + Store>(
        &mut self,
        saved: Vec<Connection>,
        session: &mut Session<S>,
    ) {
        let connections = Connection::sanitized(saved);
        // a kept socket goes on to name its messages after the connection
        self.sockets.retain(|socket| {
            let kept =
                (connections.iter()).any(|c| c.address == socket.address && c.name == socket.name);
            if !kept {
                session.remove_source(socket.id);
                log::info!("ws disconnected from {}", socket.address);
            }
            kept
        });
        let listed = |address: &String| connections.iter().any(|c| c.address == *address);
        self.failed.retain(|address, _| listed(address));
        self.hellos.retain(|address, _| listed(address));
        self.editing = None;
        self.deleting = None;
        self.connections = connections;
    }

    /// Forgets why connections failed or broke, e.g. once an opened recording shows in
    /// place of their data, so that the status no longer tells of them.
    pub fn clear_failures(&mut self) {
        self.failed.clear();
    }

    /// Disconnects every connection: removes its source from `session`. Why
    /// connections failed stays, see [`Self::clear_failures`].
    pub fn disconnect_all<S: Ingest + Store>(&mut self, session: &mut Session<S>) {
        for socket in self.sockets.drain(..) {
            session.remove_source(socket.id);
            log::info!("ws disconnected from {}", socket.address);
        }
    }

    /// Takes what the connections told in the last update of `session`: which opened, their
    /// Hello, which closed or failed and why. Call it after every [`Session::update`]. The
    /// session tells the user what went wrong, by the address of the connection.
    pub fn observe<S: Ingest + Store>(&mut self, session: &Session<S>) {
        for (id, event) in session.source_events() {
            let Some(index) = self.sockets.iter().position(|socket| socket.id == *id) else {
                continue;
            };
            let socket = &mut self.sockets[index];
            match event {
                SourceEvent::Opened => {
                    log::info!("ws connected to {}", socket.address);
                    socket.open = true;
                }
                SourceEvent::Hello(hello) => {
                    keep_hello(socket, hello.clone(), &mut self.hellos, &mut self.warnings);
                }
                SourceEvent::Failed(error) => {
                    log::warn!("ws error of {}: {error}", socket.address);
                    let failure = Failure {
                        reason: error.clone(),
                        lost: socket.open,
                    };
                    self.failed.insert(socket.address.clone(), failure);
                    self.sockets.remove(index);
                }
                SourceEvent::Closed => {
                    log::info!("ws disconnected from {}", socket.address);
                    let failure = Failure {
                        reason: CLOSED_REASON.to_owned(),
                        lost: socket.open,
                    };
                    self.failed.insert(socket.address.clone(), failure);
                    self.sockets.remove(index);
                }
            }
        }
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
    #[cfg(any(test, feature = "test-util"))]
    pub fn connect_first<S: Ingest + Store>(
        &mut self,
        ctx: &egui::Context,
        session: &mut Session<S>,
    ) {
        self.connect(0, ctx.clone(), session);
    }

    /// `<name>: protocol N is newer than blip` of each open connection that speaks one.
    pub fn newer(&self) -> Vec<String> {
        self.connections
            .iter()
            .filter_map(|connection| match self.state(&connection.address) {
                State::Newer(version) => {
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

    fn state(&self, address: &str) -> State {
        match (self.socket(address), self.failed.get(address)) {
            (Some(socket), _) if socket.open => match self.hellos.get(address) {
                Some(hello) if socket.greeted && hello.version > proto::VERSION => {
                    State::Newer(hello.version)
                }
                _ => State::Connected,
            },
            (Some(_), _) => State::Connecting,
            (None, Some(failure)) if failure.lost => State::Lost(failure.reason.clone()),
            (None, Some(failure)) => State::Failed(failure.reason.clone()),
            (None, None) => State::Disconnected,
        }
    }

    /// What the connections do as a whole, to show it in one place, e.g. a status chip:
    /// open beats being opened, which beats failed.
    pub fn status(&self) -> Status {
        // the failures of the listed connections, others are not shown
        let mut failures = (self.connections.iter())
            .filter(|connection| self.socket(&connection.address).is_none())
            .filter_map(|connection| self.failed.get(&connection.address));
        if self.is_connected() {
            let newer = self
                .connections
                .iter()
                .any(|connection| matches!(self.state(&connection.address), State::Newer(_)));
            let problem = newer || failures.next().is_some();
            Status::Open { problem }
        } else if !self.sockets.is_empty() {
            Status::Connecting
        } else {
            let mut failed = false;
            let mut lost = false;
            for failure in failures {
                failed = true;
                lost |= failure.lost;
            }
            if failed {
                Status::Failed { lost }
            } else {
                Status::None
            }
        }
    }

    /// Whether the address or the name of a connection is being edited, or deleting one
    /// waits to be confirmed: Escape ends that first.
    pub fn is_editing(&self) -> bool {
        self.editing.is_some() || self.deleting.is_some()
    }

    /// Tells whether the app is `recording`, every frame: remembers the connections
    /// that are active when a recording starts, also while they are not shown, as only
    /// those can be connected again when they break.
    pub fn set_recording(&mut self, recording: bool) {
        if !recording {
            self.recorded = None;
        } else if self.recorded.is_none() {
            let addresses = self
                .connections
                .iter()
                .map(|connection| &connection.address);
            let active = addresses.filter(|address| self.state(address).is_active());
            self.recorded = Some(active.cloned().collect());
        }
    }

    /// Shows the connections to connect, disconnect and edit them, unless `session` is
    /// recording: a recording keeps the connections it started with, so they are disabled
    /// but for connecting again those that broke, and an edit is dropped.
    pub fn ui<S: Ingest + Store>(&mut self, ui: &mut egui::Ui, session: &mut Session<S>) {
        let recording = session.recording;
        self.set_recording(recording);
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
            self.deleting = None;
        }
        let mut editing = self.editing.take();
        egui::ScrollArea::vertical()
            .id_salt("connections")
            .auto_shrink([false, true])
            .max_height(MAX_LIST_HEIGHT)
            .show(ui, |ui| {
                for (index, connection) in self.connections.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let state = self.state(&connection.address);
                        status_ui(ui, &state);
                        match editing.as_mut() {
                            Some(editing) if editing.edited == Edited::Connection(index) => {
                                let note = self.edit_note(editing);
                                edit_ui(ui, editing, note, &mut actions);
                            }
                            _ if self.deleting == Some(index) => {
                                let name = display_name(&connection.name, &connection.address);
                                delete_ui(ui, name, state.is_active(), index, &mut actions);
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
                            status_ui(ui, &State::Disconnected);
                            let note = self.edit_note(editing);
                            edit_ui(ui, editing, note, &mut actions);
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
            self.apply(action, ui.ctx(), session);
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
        let active = self.state(&connection.address).is_active();
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
            if ui.button(format!("{} Delete…", regular::TRASH)).clicked() {
                actions.push(Action::AskRemove(index));
            }
        });
    }

    fn apply<S: Ingest + Store>(
        &mut self,
        action: Action,
        ctx: &egui::Context,
        session: &mut Session<S>,
    ) {
        match action {
            Action::Connect(index) => self.connect(index, ctx.clone(), session),
            Action::Disconnect(index) => {
                let address = self.connections[index].address.clone();
                self.disconnect(&address, session);
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
                    refused: false,
                });
                self.deleting = None;
            }
            Action::FinishEdit { entered } => {
                if let Some(editing) = self.editing.take() {
                    self.finish_edit(editing, entered, session);
                }
            }
            Action::CancelEdit => self.editing = None,
            Action::AskRemove(index) => {
                self.deleting = Some(index);
                self.editing = None;
            }
            Action::CancelRemove => self.deleting = None,
            Action::Remove(index) => self.remove(index, session),
        }
    }

    /// Applies the text of `editing`, ended with Enter if `entered`. Text that cannot
    /// be applied keeps the field open to tell why, unless a new address was left blank
    /// by a click elsewhere, which drops it.
    fn finish_edit<S: Ingest + Store>(
        &mut self,
        mut editing: Editing,
        entered: bool,
        session: &mut Session<S>,
    ) {
        if self.problem(&editing).is_some() {
            let blank = editing.text.trim().is_empty();
            if entered || !blank || editing.edited != Edited::New {
                editing.refused = true;
                self.editing = Some(editing);
            }
            return;
        }
        let text = editing.text.trim();
        match (editing.edited, editing.field) {
            (Edited::Connection(index), Field::Name) => self.rename(index, text),
            (edited, _) => self.set_address(edited, &with_scheme(text), session),
        }
    }

    /// Why the text of `editing` cannot be applied: a blank address, or a name or an
    /// address that another connection has.
    fn problem(&self, editing: &Editing) -> Option<&'static str> {
        let text = editing.text.trim();
        let mut others = (self.connections.iter().enumerate())
            .filter(|&(index, _)| editing.edited != Edited::Connection(index))
            .map(|(_, connection)| connection);
        match editing.field {
            Field::Name => (!text.is_empty() && others.any(|c| c.name == text))
                .then_some("Another connection has this name"),
            Field::Address if text.is_empty() => Some("Enter the address to connect to"),
            Field::Address => {
                let address = with_scheme(text);
                (others.any(|c| c.address == address))
                    .then_some("Another connection has this address")
            }
        }
    }

    /// What to show below the field of `editing` and whether it is an error: why its
    /// text cannot be applied, or that a new address disconnects the connection. A
    /// blank text tells why only once finishing it was refused.
    fn edit_note(&self, editing: &Editing) -> Option<(&'static str, bool)> {
        let problem = self
            .problem(editing)
            .filter(|_| editing.refused || !editing.text.trim().is_empty());
        if let Some(problem) = problem {
            return Some((problem, true));
        }
        match (editing.edited, editing.field) {
            (Edited::Connection(index), Field::Address)
                if self.state(&self.connections[index].address).is_active() =>
            {
                Some(("A new address disconnects this connection", false))
            }
            _ => None,
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
    fn set_address<S: Ingest + Store>(
        &mut self,
        edited: Edited,
        address: &str,
        session: &mut Session<S>,
    ) {
        if address.is_empty() || self.connections.iter().any(|c| c.address == address) {
            return;
        }
        match edited {
            Edited::Connection(index) => {
                self.forget(index, session);
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

    fn remove<S: Ingest + Store>(&mut self, index: usize, session: &mut Session<S>) {
        self.forget(index, session);
        self.connections.remove(index);
        self.editing = None;
        self.deleting = None;
    }

    /// Disconnects the connection at `index` and drops why it failed.
    fn forget<S: Ingest + Store>(&mut self, index: usize, session: &mut Session<S>) {
        let address = self.connections[index].address.clone();
        self.disconnect(&address, session);
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
    hello: proto::Hello,
    hellos: &mut HashMap<String, proto::Hello>,
    warnings: &mut Vec<String>,
) {
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

/// `address` with the scheme `ws://` if it has none.
fn with_scheme(address: &str) -> String {
    if address.contains("://") {
        address.to_owned()
    } else {
        format!("{DEFAULT_SCHEME}{address}")
    }
}

/// A field to edit a name or an address, ending with Enter or a click elsewhere, or
/// Escape to cancel, with the `note` below it, an error if its flag is set.
fn edit_ui(
    ui: &mut egui::Ui,
    editing: &mut Editing,
    note: Option<(&str, bool)>,
    actions: &mut Vec<Action>,
) {
    let hint = match editing.field {
        Field::Name => "No name: traces are not put below it",
        Field::Address => DEFAULT_ADDRESS,
    };
    ui.vertical(|ui| {
        let field = theme::wide_field(&mut editing.text).hint_text(hint);
        match theme::focused_edit(ui, field) {
            Some(true) => {
                let entered = ui.input(|input| input.key_pressed(egui::Key::Enter));
                actions.push(Action::FinishEdit { entered });
            }
            Some(false) => actions.push(Action::CancelEdit),
            None => {}
        }
        if let Some((note, error)) = note {
            let color = if error {
                ui.tokens().error
            } else {
                ui.visuals().weak_text_color()
            };
            let font = egui::TextStyle::Small.resolve(ui.style());
            ui.label(egui::RichText::new(note).font(font).color(color));
        }
    });
}

/// Asks to confirm deleting the connection `name` at `index`, which is disconnected
/// first if `active`. Enter deletes it while nothing else has the focus, Escape keeps
/// it.
fn delete_ui(ui: &mut egui::Ui, name: &str, active: bool, index: usize, actions: &mut Vec<Action>) {
    ui.vertical(|ui| {
        let question = if active {
            format!("Disconnect and delete {name}?")
        } else {
            format!("Delete {name}?")
        };
        ui.add(egui::Label::new(question).wrap());
        // Enter goes to a widget that has the focus, e.g. a field or a button
        let unfocused = ui.memory(|memory| memory.focused().is_none());
        let mut pressed = ui.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                Some(Action::CancelRemove)
            } else if unfocused && input.key_pressed(egui::Key::Enter) {
                Some(Action::Remove(index))
            } else {
                None
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let delete = egui::RichText::new("Delete").color(ui.tokens().error);
            if ui.button(delete).clicked() {
                pressed = Some(Action::Remove(index));
            }
            if ui.button("Cancel").clicked() {
                pressed = Some(Action::CancelRemove);
            }
        });
        actions.extend(pressed);
    });
}

/// A dot showing the state of a connection: hollow when disconnected, a spinner while
/// connecting, green when connected, amber when it speaks a newer protocol and red when
/// it failed or was lost.
fn status_ui(ui: &mut egui::Ui, state: &State) {
    let size = egui::Vec2::splat(ui.spacing().icon_width);
    let hint = match state {
        State::Disconnected => "Disconnected".to_owned(),
        State::Connecting => "Connecting…".to_owned(),
        State::Connected => "Connected".to_owned(),
        State::Newer(version) => format!("Connected, {}", newer_text(*version)),
        State::Failed(error) => format!("Failed: {error}"),
        State::Lost(error) => format!("Connection lost: {error}"),
    };
    let response = if *state == State::Connecting {
        ui.add_sized(size, egui::Spinner::new())
    } else {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
        let radius = size.x * 0.3;
        let painter = ui.painter();
        let center = rect.center();
        match state {
            State::Connected => {
                painter.circle_filled(center, radius, ui.tokens().ok);
            }
            State::Newer(_) => {
                painter.circle_filled(center, radius, ui.tokens().warn);
            }
            State::Failed(_) | State::Lost(_) => {
                painter.circle_filled(center, radius, ui.tokens().error);
            }
            State::Disconnected | State::Connecting => {
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
