mod connections;
mod files;
mod layout;
mod model;
mod theme;
mod time;
mod toasts;
mod view;

use connections::Received;
use eframe::Frame;
use egui::{Context, Key, KeyboardShortcut, Modifiers};
use std::collections::HashSet;
use view::ViewContext;

const OPEN_RECORDING: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE_RECORDING: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct State {
    time: time::TimeRange,

    /// Whether the samples received from the connections are kept, else they are
    /// dropped. A recording starts with an open connection and clears the data.
    #[serde(skip)]
    recording: bool,

    /// Whether the data recorded since the recording started was not saved yet.
    #[serde(skip)]
    unsaved: bool,

    #[serde(skip)]
    model: model::Model,

    #[serde(skip)]
    invoker: model::actions::Invoker,
}

impl State {
    /// Applies a frame received from `source`, the name of its connection, at the time
    /// `now`: records it while recording, and passes answers to the invoker.
    fn receive(&mut self, source: &str, frame: &[u8], now: f64) {
        let result = if self.recording {
            self.model.receive(source, frame)
        } else {
            self.model.receive_unrecorded(source, frame)
        };
        if let Some(result) = result {
            self.invoker.receive(source, &result, now, &self.model);
        }
    }

    /// Applies what the connections received at the time `now`. Returns whether the
    /// first connection opened, which starts afresh with no data, unless recording.
    fn apply(&mut self, received: Received<'_>, now: f64) -> bool {
        match received {
            Received::FirstOpened if !self.recording => {
                self.start_fresh(model::Model::default());
                true
            }
            Received::FirstOpened => false,
            Received::Message { source, message } => {
                self.receive(source, message, now);
                false
            }
        }
    }

    /// Removes the received data, see [`model::Model::clear`].
    fn clear_data(&mut self) {
        self.model.clear();
        self.invoker.forget_readback_marks();
        self.unsaved = false;
    }

    /// Whether there is recorded data that was not saved, which starting afresh drops.
    fn has_unsaved_data(&self) -> bool {
        self.unsaved && self.model.has_samples()
    }

    /// Starts afresh with `model`, e.g. of an opened recording: drops all data, also the
    /// descriptions of the targets that [`Self::clear_data`] keeps, stops recording and
    /// forgets the invocations.
    fn start_fresh(&mut self, model: model::Model) {
        self.recording = false;
        self.unsaved = false;
        self.invoker.reset();
        self.model = model;
    }

    /// Starts or stops recording. A recording starts only while `connected`, clears the
    /// data and follows it live.
    fn set_recording(&mut self, recording: bool, connected: bool) {
        if recording && !self.recording {
            if !connected {
                return;
            }
            self.clear_data();
            self.time.go_live();
            self.unsaved = true;
        }
        self.recording = recording;
    }

    /// The bottom panel: the record button and the time range.
    fn bottom_ui(&mut self, ui: &mut egui::Ui, connected: bool) {
        // larger controls than elsewhere, as the bar is used all the time
        ui.spacing_mut().interact_size.y = 28.0;
        ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
        ui.horizontal(|ui| {
            if record_button(ui, self.recording, connected) {
                self.set_recording(!self.recording, connected);
            }
            ui.separator();
            self.time.ui(ui);
        });
    }
}

/// A button showing whether `recording`, which can be started only while `connected`.
/// Returns whether it was clicked, to switch.
fn record_button(ui: &mut egui::Ui, recording: bool, connected: bool) -> bool {
    // red like a record key, filled with a tint of it while recording
    let tokens = theme::tokens(ui.visuals());
    let red = tokens.error;
    let dot = egui::Id::new("record dot");
    let text = egui::RichText::new("Record").color(red);
    let (fill, stroke) = if recording {
        (tokens.control.lerp_to_gamma(red, 0.2), red)
    } else {
        (tokens.control, tokens.border)
    };
    let button = egui::Button::new((egui::Atom::custom(dot, egui::vec2(9.0, 9.0)), text))
        .selected(recording)
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke));
    let hint = if recording {
        "Recording the received samples, click to stop"
    } else {
        "Click to clear the data and record the received samples"
    };
    let enabled = recording || connected;
    let shown = ui.add_enabled_ui(enabled, |ui| button.atom_ui(ui)).inner;
    if let Some(rect) = shown.rect(dot) {
        // a red dot, like the key of a recorder
        let alpha = if enabled {
            1.0
        } else {
            ui.visuals().disabled_alpha
        };
        ui.painter()
            .circle_filled(rect.center(), 4.5, red.gamma_multiply(alpha));
    }
    shown
        .response
        .on_hover_text(hint)
        .on_disabled_hover_text("Connect to record")
        .clicked()
}

//--------------------------------------------------------------------------------------------------

/// Storage key of the layout, saved apart from the app state in its own versioned format.
const LAYOUT_KEY: &str = "layout";

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    state: State,
    connections: connections::Connections,
    #[serde(skip)]
    layout: layout::Layout,
    /// The search of the traces panel.
    #[serde(skip)]
    trace_search: String,
    /// Whether the traces panel shows only the traces and actions the selected view can
    /// show.
    compatible_only: bool,
    /// Counts the fresh starts of the traces panel. The state of its widgets, e.g.
    /// which branches of the tree are open, is kept apart per start, see
    /// [`Self::reset_traces_panel`].
    traces_generation: u64,
    #[serde(skip)]
    files: files::Files,
    /// The notices for the user, e.g. why a file could not be opened.
    #[serde(skip)]
    toasts: toasts::Toasts,
    /// How many messages the connections receive, shown in the title bar.
    #[serde(skip)]
    rate: MessageRate,
}

/// The number of messages received per second, measured second by second.
#[derive(Default)]
struct MessageRate {
    /// When the second being measured started, and the messages received since.
    start: Option<f64>,
    count: usize,
    /// The rate of the last measured second.
    per_second: Option<f64>,
}

impl MessageRate {
    /// Counts `received` messages at the time `now` in seconds, and ends the second
    /// being measured when it is over.
    fn update(&mut self, now: f64, received: usize) {
        let start = *self.start.get_or_insert(now);
        self.count += received;
        let elapsed = now - start;
        if elapsed >= 1.0 {
            self.per_second = Some(self.count as f64 / elapsed);
            self.start = Some(now);
            self.count = 0;
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let Some(storage) = cc.storage else {
            return Default::default();
        };
        let mut app: Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        if let Some(layout) = eframe::get_value::<layout::LayoutFile>(storage, LAYOUT_KEY)
            .and_then(layout::Layout::migrate)
        {
            app.layout = layout;
        }
        app
    }

    fn menu_ui(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            logo_ui(ui);
            ui.label(egui::RichText::new("blip").size(15.0).strong());
            ui.add_space(6.0);
            ui.menu_button("File", |ui| {
                let ctx = ui.ctx().clone();
                let shortcut = |text, shortcut| {
                    egui::Button::new(text).shortcut_text(ctx.format_shortcut(shortcut))
                };
                if ui
                    .add(shortcut("Open recording…", &OPEN_RECORDING))
                    .clicked()
                {
                    self.files.open(files::Kind::Recording, ui.ctx());
                }
                let recorded = self.state.model.has_samples();
                let messages = self.state.model.recording.messages();
                let save = shortcut("Save recording…", &SAVE_RECORDING);
                if ui
                    .add_enabled(recorded, save)
                    .on_hover_text(format!("{messages} messages received"))
                    .clicked()
                {
                    self.save_recording();
                }
                if ui
                    .add_enabled(recorded, egui::Button::new("Clear data"))
                    .on_hover_text("Remove all received data")
                    .clicked()
                {
                    self.state.clear_data();
                }
                ui.separator();
                if ui.button("Open layout…").clicked() {
                    self.files.open(files::Kind::Layout, ui.ctx());
                }
                if ui.button("Save layout…").clicked() {
                    self.save_layout();
                }
                if ui.button("Reset layout").clicked() {
                    self.layout = layout::Layout::default();
                }
            });
            ui.menu_button("View", |ui| {
                egui::gui_zoom::zoom_menu_buttons(ui);
                ui.weak(format!("Zoom {:.0} %", ui.ctx().zoom_factor() * 100.0));
            });
            ui.menu_button("Help", |ui| {
                ui.label(format!("blip {}", env!("CARGO_PKG_VERSION")));
                ui.hyperlink_to("Documentation", "https://stefanluethi.github.io/blip/docs/");
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme_button(ui);
                let open = self.connections.open_names();
                status_chip(ui, &open, self.rate.per_second);
                egui::warn_if_debug_build(ui);
            });
        });
    }

    /// Handles the shortcuts of the menu.
    fn shortcuts(&mut self, ctx: &Context) {
        if ctx.input_mut(|input| input.consume_shortcut(&OPEN_RECORDING)) {
            self.files.open(files::Kind::Recording, ctx);
        }
        if ctx.input_mut(|input| input.consume_shortcut(&SAVE_RECORDING))
            && self.state.model.has_samples()
        {
            self.save_recording();
        }
    }

    /// Starts the traces panel anew for other traces, e.g. of an opened recording: the
    /// search is cleared and the tree is scrolled to the top with its branches open.
    fn reset_traces_panel(&mut self) {
        self.trace_search.clear();
        self.traces_generation += 1;
    }

    /// The traces panel, with the state of its widgets apart from that of other
    /// generations.
    fn traces_panel_ui(&mut self, ui: &mut egui::Ui) {
        ui.push_id(self.traces_generation, |ui| self.traces_ui(ui));
    }

    /// Receives from the connections at the time `now`. The first connection to open
    /// starts afresh, see [`State::apply`].
    fn receive(&mut self, now: f64) {
        let state = &mut self.state;
        let mut fresh = false;
        let mut messages = 0;
        self.connections.receive(|received| {
            messages += usize::from(matches!(received, Received::Message { .. }));
            fresh |= state.apply(received, now);
        });
        self.rate.update(now, messages);
        if fresh {
            self.reset_traces_panel();
        }
    }

    fn save_recording(&mut self) {
        let name = format!("{}.blip", chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
        let file = self.state.model.recording.to_file();
        match files::save(files::Kind::Recording, &name, &file) {
            // a recording going on is saved only up to now
            Ok(saved) => self.state.unsaved &= !saved || self.state.recording,
            Err(error) => self.toasts.error(error),
        }
    }

    fn save_layout(&mut self) {
        let file = serde_json::to_vec_pretty(&self.layout.to_file()).expect("layouts are JSON");
        if let Err(error) = files::save(files::Kind::Layout, "layout.json", &file) {
            self.toasts.error(error);
        }
    }

    /// Opens a recording or a layout, as which the contents of `file` tell.
    fn open(&mut self, file: &files::File) -> Result<(), String> {
        if model::recording::is_recording(&file.bytes) {
            let mut model = model::Model::default();
            let loaded = model::recording::read_file(&file.bytes, |source, frame| {
                model.receive(source, frame);
            })
            .map_err(|error| format!("{} cannot be opened: {error}", file.name))?;
            self.connections.disconnect_all();
            self.state.start_fresh(model);
            self.reset_traces_panel();
            self.state.time.fit(&self.state.model);
            if let Some(problem) = loaded.problem {
                self.toasts.warning(format!(
                    "{} was opened up to message {}, {problem}",
                    file.name, loaded.messages
                ));
            }
        } else {
            let layout =
                serde_json::from_slice::<layout::LayoutFile>(&file.bytes).map_err(|error| {
                    format!("{} is no blip recording or layout: {error}", file.name)
                })?;
            self.layout = layout::Layout::migrate(layout)
                .ok_or_else(|| format!("{} is a layout of a newer version of blip", file.name))?;
        }
        Ok(())
    }

    /// Sends the invocations of the views to the connections of the outposts chosen
    /// for their actions, unless the outpost no longer announces the action.
    fn send_invocations(&mut self) {
        for (source, request) in self.state.invoker.take_outbox() {
            let proto::Request::Invoke(invocation) = &request;
            let actions = &self.state.model.actions;
            if actions.announced(&source, &invocation.path).is_none() {
                log::warn!(
                    "dropping an invocation of {} on {source:?}, which no longer announces it",
                    invocation.path
                );
                continue;
            }
            self.connections.send(&source, &request);
        }
    }

    /// The tree of the traces and actions, to choose those of the selected view.
    fn traces_ui(&mut self, ui: &mut egui::Ui) {
        theme::section_header(ui, "Traces & actions", |_| {});
        let view = self.layout.selected_view_mut();
        let hint = match &view {
            None => Some("Select a view to choose its traces or actions"),
            Some(view) if !view.has_selection() => {
                Some("The selected view shows no traces or actions")
            }
            Some(_) => None,
        };
        if let Some(hint) = hint {
            ui.weak(hint);
        }
        // a view without traces or actions, like the logs, has nothing to choose; the
        // panel keeps its height, so that the layout panel below stays where it is
        if view.as_ref().is_some_and(|view| !view.has_selection()) {
            ui.allocate_space(ui.available_size());
            return;
        }
        let filterable = view.as_ref().is_some_and(|view| view.has_selection());
        view::trace_selection::search_ui(
            ui,
            &mut self.trace_search,
            &mut self.compatible_only,
            filterable,
        );
        egui::ScrollArea::vertical()
            .id_salt("traces")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let model = &self.state.model;
                let search = &self.trace_search;
                let selection = view.and_then(|view| view.selection_mut());
                view::trace_selection::show(ui, model, search, self.compatible_only, selection);
            });
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut egui::RawInput) {
        view::zoom_with_wheel(ctx, raw_input);
    }

    fn logic(&mut self, ctx: &Context, _frame: &mut Frame) {
        let now = ctx.input(|input| input.time);
        self.receive(now);
        let state = &mut self.state;
        state.invoker.connected = self.connections.open_names();
        state.invoker.update(now);
        for error in self.connections.take_errors() {
            self.toasts.error(error);
        }
        self.files.receive_dropped(ctx);
        while let Some(file) = self.files.try_recv() {
            if let Err(error) = file.and_then(|file| self.open(&file)) {
                self.toasts.error(error);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shortcuts(ui.ctx());
        let top_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 8));
        egui::Panel::top("top panel")
            .frame(top_frame)
            .show(ui, |ui| self.menu_ui(ui));

        let now = ui.input(|input| input.time);
        self.state.time.update(&self.state.model, now);

        let bottom_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 8));
        egui::Panel::bottom("bottom panel")
            .frame(bottom_frame)
            .show(ui, |ui| {
                let connected = self.connections.is_connected();
                self.state.bottom_ui(ui, connected);
            });

        egui::Panel::left("left panel")
            .resizable(true)
            .default_size(220.0)
            .show(ui, |ui| {
                // the traces on top, resizable by dragging the border to the layout
                egui::Panel::top("traces panel")
                    .resizable(true)
                    .default_size(250.0)
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                    .show(ui, |ui| self.traces_panel_ui(ui));
                ui.add_space(4.0);
                self.layout.panel_ui(ui);
            });

        egui::Panel::right("properties panel")
            .resizable(true)
            .default_size(264.0)
            .show(ui, |ui| {
                // the connections below, resizable by dragging the border to the properties
                egui::Panel::bottom("connections panel")
                    .resizable(true)
                    .default_size(160.0)
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                    .show(ui, |ui| {
                        let unsaved = self.state.has_unsaved_data();
                        self.connections.ui(ui, self.state.recording, unsaved);
                    });
                ui.add_space(4.0);
                let context = view::PropertiesContext {
                    model: &self.state.model,
                    connected: &self.state.invoker.connected,
                };
                self.layout.properties_ui(ui, &context);
            });

        let central = egui::CentralPanel::default()
            .frame(
                // the views are cards on the canvas, with a gap around them
                egui::Frame::central_panel(ui.style())
                    .fill(theme::tokens(ui.visuals()).canvas)
                    .inner_margin(egui::Margin::same(theme::GAP as i8)),
            )
            .show(ui, |ui| {
                let mut context = ViewContext {
                    model: &self.state.model,
                    time: &mut self.state.time,
                    invoker: &mut self.state.invoker,
                    formats: &view::NO_FORMATS,
                };
                self.layout.ui(ui, &mut context);
            });
        self.send_invocations();
        if self.state.invoker.is_waiting(now) {
            ui.ctx()
                .request_repaint_after_secs(model::actions::DRAG_INTERVAL as f32);
        }

        self.toasts.ui(ui.ctx(), central.response.rect);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        eframe::set_value(storage, LAYOUT_KEY, &self.layout.to_file());
    }
}

/// The mark of blip: a blip on a line, on the accent.
fn logo_ui(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
    let tokens = theme::tokens(ui.visuals());
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, tokens.accent);
    let at = |x: f32, y: f32| rect.min + egui::vec2(x, y);
    let line = [
        (3.5, 11.0),
        (6.5, 11.0),
        (8.1, 6.5),
        (10.7, 14.0),
        (12.3, 11.0),
        (16.5, 11.0),
    ];
    let points = line.iter().map(|&(x, y)| at(x, y)).collect();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.7, tokens.on_accent),
    ));
}

/// A chip with the status of the connections: a green dot and the name of the open
/// connection or their number, and the messages received per second, `rate`.
fn status_chip(ui: &mut egui::Ui, open: &HashSet<String>, rate: Option<f64>) {
    let tokens = theme::tokens(ui.visuals());
    let text = match open.len() {
        0 => "Not connected".to_owned(),
        1 => open.iter().next().cloned().unwrap_or_default(),
        count => format!("{count} connections"),
    };
    let frame = egui::Frame::NONE
        .fill(tokens.control)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(theme::CONTROL_RADIUS)
        .inner_margin(egui::Margin::symmetric(8, 3));
    let response = frame
        .show(ui, |ui| {
            // from the right, as it sits at the right end of the menu bar
            let layout = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(layout, |ui| {
                if let Some(rate) = rate.filter(|_| !open.is_empty()) {
                    let rate = format!("{} msg/s", group_thousands(rate.round() as u64));
                    ui.label(egui::RichText::new(rate).monospace().weak());
                }
                ui.weak(text);
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                if open.is_empty() {
                    let stroke = egui::Stroke::new(1.0, ui.visuals().weak_text_color());
                    ui.painter().circle_stroke(dot.center(), 3.5, stroke);
                } else {
                    ui.painter().circle_filled(dot.center(), 3.5, tokens.ok);
                }
            });
        })
        .response;
    if open.len() > 1 {
        let mut names: Vec<&String> = open.iter().collect();
        names.sort();
        let names: Vec<&str> = names.into_iter().map(String::as_str).collect();
        response.on_hover_text(names.join("\n"));
    }
}

/// `number` with its digits in groups of three, split by narrow spaces.
fn group_thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push('\u{202f}');
        }
        grouped.push(digit);
    }
    grouped
}

/// A button showing the theme preference, clicking it switches to the next one.
fn theme_button(ui: &mut egui::Ui) {
    use egui::ThemePreference;
    let preference = ui.ctx().options(|options| options.theme_preference);
    let (icon, hint, next) = match preference {
        ThemePreference::Light => (
            egui_phosphor::regular::SUN,
            "Light theme",
            ThemePreference::Dark,
        ),
        ThemePreference::Dark => (
            egui_phosphor::regular::MOON,
            "Dark theme",
            ThemePreference::System,
        ),
        ThemePreference::System => (
            egui_phosphor::regular::CIRCLE_HALF,
            "System theme",
            ThemePreference::Light,
        ),
    };
    let square = egui::Vec2::splat(ui.spacing().interact_size.y);
    let tokens = theme::tokens(ui.visuals());
    if ui
        .add(
            // framed like the other controls, which the menu bar leaves unframed
            egui::Button::new(icon)
                .fill(tokens.control)
                .stroke(egui::Stroke::new(1.0, tokens.border))
                .min_size(square),
        )
        .on_hover_text(hint)
        .clicked()
    {
        ui.ctx().set_theme(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_rate_is_measured_per_second() {
        let mut rate = MessageRate::default();
        rate.update(10.0, 0);
        rate.update(10.5, 300);
        assert_eq!(rate.per_second, None);
        rate.update(11.0, 200);
        assert_eq!(rate.per_second, Some(500.0));
        rate.update(11.2, 50);
        assert_eq!(rate.per_second, Some(500.0));
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(group_thousands(7), "7");
        assert_eq!(group_thousands(1284), "1\u{202f}284");
        assert_eq!(group_thousands(1_234_567), "1\u{202f}234\u{202f}567");
    }

    fn value(timestamp: u64) -> Vec<u8> {
        value_of("rpm", timestamp)
    }

    fn value_of(path: &str, timestamp: u64) -> Vec<u8> {
        serde_cbor_2::to_vec(&proto::Message::Value(proto::Value {
            timestamp,
            path: path.to_owned(),
            value: proto::Data::Uint(1),
        }))
        .unwrap()
    }

    fn unit() -> Vec<u8> {
        unit_of("rpm")
    }

    fn unit_of(path: &str) -> Vec<u8> {
        serde_cbor_2::to_vec(&proto::Message::ValueMetadata(proto::ValueMetadata {
            path: path.to_owned(),
            unit: "rpm".to_owned(),
        }))
        .unwrap()
    }

    #[test]
    fn a_recording_starts_only_while_connected_and_clears_the_data() {
        let mut state = State::default();
        assert!(!state.recording);
        state.receive("a", &unit(), 0.0);
        state.receive("a", &value(1_000_000_000), 0.0);
        assert!(!state.model.has_samples());
        state.set_recording(true, false);
        assert!(!state.recording);

        state.set_recording(true, true);
        state.receive("a", &value(2_000_000_000), 0.0);
        state.receive("a", &value(3_000_000_000), 0.0);
        state.time.update(&state.model, 0.0);
        state.time.set_cursor(2.5);
        // stopping keeps the data, also when no longer connected
        state.set_recording(false, false);
        assert!(!state.recording);
        assert!(state.model.has_samples());

        state.set_recording(true, true);
        assert!(state.recording);
        assert!(!state.model.has_samples());
        assert_eq!(state.model.traces.unit("rpm"), Some("rpm"));
        assert!(state.time.is_live() && state.time.is_playing());
    }

    #[test]
    fn the_record_button_is_disabled_until_connected() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 40.0))
            .build_ui_state(
                |ui, (state, connected): &mut (State, bool)| state.bottom_ui(ui, *connected),
                (State::default(), false),
            );
        harness.run();
        harness.get_by_label_contains("Record").click();
        harness.run();
        assert!(!harness.state().0.recording);

        harness.state_mut().1 = true;
        harness.run();
        harness.get_by_label_contains("Record").click();
        harness.run();
        assert!(harness.state().0.recording);

        harness.state_mut().1 = false;
        harness.run();
        harness.get_by_label_contains("Record").click();
        harness.run();
        assert!(!harness.state().0.recording);
    }

    #[test]
    fn opening_a_recording_stops_recording_and_starts_afresh() {
        let mut recorded = model::Model::default();
        recorded.receive("a", &value(1));
        let file = files::File {
            name: "a.blip".to_owned(),
            bytes: recorded.recording.to_file(),
        };
        let mut app = App::default();
        app.state.receive("old", &unit_of("temp"), 0.0);
        app.state.recording = true;
        app.trace_search = "temp".to_owned();
        let generation = app.traces_generation;
        app.open(&file).unwrap();
        assert!(!app.state.recording);
        assert!(app.state.model.has_samples());
        assert!(app.trace_search.is_empty());
        assert_ne!(app.traces_generation, generation);
        // the unit before is gone: clearing the data, which keeps the units and
        // actions to record them again, has none of the recording file to keep
        app.state.clear_data();
        assert_eq!(app.state.model.recording.messages(), 0);
    }

    #[test]
    fn the_first_connection_to_open_starts_afresh_unless_recording() {
        let mut state = State::default();
        state.set_recording(true, true);
        state.receive("old", &unit(), 0.0);
        state.receive("old", &value(1), 0.0);
        // a connection opening while recording keeps the recording going
        assert!(!state.apply(Received::FirstOpened, 0.0));
        assert!(state.recording && state.model.has_samples());

        state.set_recording(false, false);
        assert!(state.apply(Received::FirstOpened, 0.0));
        assert!(state.model.traces.iter().next().is_none());
        assert_eq!(state.model.recording.messages(), 0);
        let message = unit();
        let received = Received::Message {
            source: "new",
            message: &message,
        };
        assert!(!state.apply(received, 0.0));
        // a recording starts with the unit of the new connection only, not the old one
        state.set_recording(true, true);
        assert_eq!(state.model.recording.messages(), 1);
    }

    #[test]
    fn recorded_data_is_unsaved_until_saved_or_dropped() {
        let mut state = State::default();
        state.set_recording(true, true);
        assert!(!state.has_unsaved_data());
        state.receive("a", &value(1), 0.0);
        state.set_recording(false, true);
        assert!(state.has_unsaved_data());
        state.clear_data();
        assert!(!state.has_unsaved_data());

        state.set_recording(true, true);
        state.receive("a", &value(2), 0.0);
        state.set_recording(false, true);
        assert!(state.apply(Received::FirstOpened, 0.0));
        assert!(!state.has_unsaved_data());
    }

    #[test]
    fn starting_afresh_opens_the_branches_of_the_traces_again() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut app = App::default();
        app.layout.select_view_showing_traces();
        app.state.set_recording(true, true);
        app.state.receive("a", &value_of("motor/rpm", 1), 0.0);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 400.0))
            .build_ui_state(|ui, app: &mut App| app.traces_panel_ui(ui), app);
        harness.run();
        assert!(harness.query_by_label_contains("rpm").is_some());

        // collapse the branch with its button left of its checkbox
        let motor = harness.get_by_label("motor").rect();
        let spacing = egui::Spacing::default();
        let x = motor.left() - spacing.item_spacing.x - spacing.indent / 2.0;
        let toggle = egui::pos2(x, motor.center().y);
        harness.hover_at(toggle);
        harness.drag_at(toggle);
        harness.drop_at(toggle);
        harness.run_steps(20);
        assert!(harness.query_by_label_contains("rpm").is_none());

        harness.state_mut().reset_traces_panel();
        harness.run();
        assert!(harness.query_by_label_contains("rpm").is_some());
    }

    #[test]
    fn a_recording_opened_in_part_is_toasted() {
        let mut recorded = model::Model::default();
        for timestamp in 0..1000 {
            recorded.receive("a", &value(timestamp));
        }
        let mut bytes = recorded.recording.to_file();
        bytes.truncate(bytes.len() / 2);
        let file = files::File {
            name: "a.blip".to_owned(),
            bytes,
        };
        let mut app = App::default();
        app.open(&file).unwrap();
        let shown = app.toasts.shown();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].0, toasts::Level::Warning);
        assert!(shown[0].1.starts_with("a.blip was opened up to message"));
    }
}
