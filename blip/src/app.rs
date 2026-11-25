#[cfg(all(test, feature = "screenshots"))]
mod screenshots;

use blip_kit::store as model;
use blip_kit::widgets::{theme, time, view};
use blip_kit::workbench::{connections, files, layout, opening, presets, toasts};

use connections::Received;
use eframe::Frame;
use egui::{Context, Key, KeyboardShortcut, Modifiers};
use model::session::Session;
use std::collections::HashSet;
use theme::TokensExt;
use view::{ViewContext, Viewable};

const OPEN_RECORDING: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE_RECORDING: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct State {
    time: time::TimeRange,

    /// The store, the invoker and whether the samples received are recorded.
    #[serde(skip)]
    session: Session,
}

impl State {
    /// Applies what the connections received at the time `now`. Returns whether the
    /// first connection opened, which starts afresh with no data, unless recording.
    fn apply(&mut self, received: Received<'_>, now: f64) -> bool {
        match received {
            Received::FirstOpened => self.session.first_opened(),
            Received::Message { source, message } => {
                self.session.receive(source, message, now);
                false
            }
        }
    }

    /// Removes the received data, see [`Session::clear_data`].
    fn clear_data(&mut self) {
        self.session.clear_data();
    }

    /// Starts afresh with `model`, e.g. of an opened recording, see
    /// [`Session::start_fresh`].
    fn start_fresh(&mut self, model: model::MemoryStore) {
        self.session.start_fresh(model);
    }

    /// Starts or stops recording. A recording starts only while `connected`, clears the
    /// data and follows it live.
    fn set_recording(&mut self, recording: bool, connected: bool) {
        if self.session.set_recording(recording, connected) {
            self.time.go_live();
        }
    }

    /// The bottom panel: the record button and the time range.
    fn bottom_ui(&mut self, ui: &mut egui::Ui, connected: bool) {
        // larger controls than elsewhere, as the bar is used all the time
        ui.spacing_mut().interact_size.y = 28.0;
        ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
        let recording = self.session.recording;
        self.time.set_connected(connected);
        let clicked = self.time.ui(ui, &self.session.store, recording, |ui| {
            record_button(ui, recording, connected)
        });
        if clicked {
            self.set_recording(!self.session.recording, connected);
        }
    }
}

/// A button showing whether `recording`, which can be started only while `connected`.
/// Returns whether it was clicked, to switch.
fn record_button(ui: &mut egui::Ui, recording: bool, connected: bool) -> bool {
    // plain while idle, red like a record key and filled with a tint of it while recording
    let tokens = ui.tokens();
    let red = tokens.error;
    let color = if recording {
        red
    } else {
        ui.visuals().text_color()
    };
    let dot = egui::Id::new("record dot");
    let text = egui::RichText::new("Record").color(color);
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
        // a dot, like the key of a recorder
        let alpha = if enabled {
            1.0
        } else {
            ui.visuals().disabled_alpha
        };
        ui.painter()
            .circle_filled(rect.center(), 4.5, color.gamma_multiply(alpha));
    }
    shown
        .response
        .on_hover_text(hint)
        .on_disabled_hover_text("Connect to record")
        .clicked()
}

//--------------------------------------------------------------------------------------------------

/// The storage of eframe as the presets read it.
struct KeptIn<'a>(&'a dyn eframe::Storage);

impl presets::Storage for KeptIn<'_> {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get_string(key)
    }

    fn set_string(&mut self, _key: &str, _value: String) {}
}

/// The storage of eframe as the presets are kept in it.
struct KeptInMut<'a>(&'a mut dyn eframe::Storage);

impl presets::Storage for KeptInMut<'_> {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get_string(key)
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.set_string(key, value);
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    state: State,
    /// The presets, kept apart from the app state in their own versioned format. The
    /// layout and the connections are those of the active one.
    #[serde(skip)]
    presets: presets::Presets,
    #[serde(skip)]
    connections: connections::Connections,
    #[serde(skip)]
    layout: layout::Layout,
    /// The search of the traces panel.
    #[serde(skip)]
    trace_search: String,
    /// Whether the traces panel shows only the traces and actions the selected view can
    /// show.
    compatible_only: bool,
    /// Whether the traces panel shows only what is checked for the selected view.
    checked_only: bool,
    /// Counts the fresh starts of the traces panel. The state of its widgets, e.g.
    /// which branches of the tree are open, is kept apart per start, see
    /// [`Self::reset_traces_panel`].
    traces_generation: u64,
    /// Whether the left panel, with the traces and the layout, is collapsed.
    left_collapsed: bool,
    /// Whether the right panel, with the properties, is collapsed.
    right_collapsed: bool,
    #[serde(skip)]
    files: files::Files,
    /// The recording being opened, read a part in each frame.
    #[serde(skip)]
    opening: Option<opening::Opening>,
    /// The notices for the user, e.g. why a file could not be opened.
    #[serde(skip)]
    toasts: toasts::Toasts,
    /// How many messages the connections receive, shown in the title bar.
    #[serde(skip)]
    rate: MessageRate,
    /// Whether the list of connections is open below the status chip.
    #[serde(skip)]
    connections_shown: bool,
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
        app.presets = presets::Presets::load(&KeptIn(storage));
        if let Some(problem) = app.presets.take_problem() {
            app.toasts.warning(problem);
        }
        app.show_preset();
        app
    }

    /// Stores the shown layout and connections in the active preset.
    fn store_preset(&mut self) {
        let (layout, connections) = (self.layout.to_saved(), self.connections.saved());
        self.presets.store(layout, connections);
    }

    /// Shows the layout and the connections of the active preset, disconnected.
    fn show_preset(&mut self) {
        let preset = self.presets.active();
        self.layout = layout::Layout::from_saved(preset.layout.clone());
        self.connections.set_saved(preset.connections.clone());
    }

    /// Applies a change of the presets asked for in their menu or a dialog. Those that
    /// change the active preset are not offered while recording.
    fn apply_preset(&mut self, action: presets::Action, ctx: &Context) {
        use presets::Action;
        self.store_preset();
        match action {
            Action::Switch(index) => self.presets.switch(index),
            Action::New => self.presets.add_new(),
            Action::Duplicate => self.presets.duplicate(),
            Action::Delete => self.presets.delete(),
            Action::Import => {
                self.files.open(files::Kind::Presets, ctx);
                return;
            }
            Action::Export(indices) => {
                let bytes = self.presets.export(&indices);
                if let Err(error) = files::save(files::Kind::Presets, "blip-presets.json", &bytes) {
                    self.toasts.error(error);
                }
                return;
            }
            Action::Imported(presets) => {
                if !self.presets.import(presets) {
                    return;
                }
            }
        }
        self.show_preset();
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
                let recorded = self.state.session.store.has_samples();
                let messages = self.state.session.store.recording.messages();
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
            });
            ui.menu_button("View", |ui| {
                ui.menu_button("Add view", |ui| {
                    for view_type in view::ViewType::all() {
                        let text = format!("{} {}", view_type.icon(), view_type.name());
                        if ui.button(text).clicked() {
                            self.layout.add_view(view_type);
                        }
                    }
                });
                if ui
                    .button("Clear layout")
                    .on_hover_text("Remove all views and tabs of the active preset")
                    .clicked()
                {
                    self.layout = layout::Layout::default();
                }
                ui.separator();
                egui::gui_zoom::zoom_menu_buttons(ui);
                ui.weak(format!("Zoom {:.0} %", ui.ctx().zoom_factor() * 100.0));
            });
            ui.menu_button("Presets", |ui| {
                if let Some(action) = self.presets.menu_ui(ui, self.state.session.recording) {
                    self.apply_preset(action, ui.ctx());
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label(format!("blip {}", env!("CARGO_PKG_VERSION")));
                ui.hyperlink_to("Documentation", "https://stefanluethi.github.io/blip/docs/");
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme_button(ui);
                panel_button(ui, PanelSide::Right, &mut self.right_collapsed);
                panel_button(ui, PanelSide::Left, &mut self.left_collapsed);
                ui.add_space(4.0);
                let open = self.connections.open_names();
                let newer = self.connections.newer();
                let chip = status_chip(ui, &open, &newer, self.rate.per_second);
                if chip.clicked() {
                    self.connections_shown = !self.connections_shown;
                }
                if self.connections_shown {
                    self.connections_ui(ui, &chip);
                }
                egui::warn_if_debug_build(ui);
            });
        });
    }

    /// Shows the list of connections below the status `chip`. It stays open to connect
    /// several or to type an address, also while the menu of a connection is used, and
    /// closes on a click into the window or on Escape.
    fn connections_ui(&mut self, ui: &egui::Ui, chip: &egui::Response) {
        // Escape first ends editing an address
        let editing = self.connections.is_editing();
        let area = egui::Area::new(egui::Id::new("connections list"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(chip.rect.right_bottom() + egui::vec2(0.0, 4.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(CONNECTIONS_WIDTH);
                    self.connections.ui(ui, self.state.session.recording);
                });
            });
        let pressed = ui.input(|input| {
            input
                .pointer
                .any_pressed()
                .then(|| input.pointer.press_origin())
                .flatten()
        });
        // menus opened from the list are popups in front, like the list
        let into_window = pressed.is_some_and(|pos| {
            !area.response.rect.contains(pos)
                && !chip.rect.contains(pos)
                && ui
                    .ctx()
                    .layer_id_at(pos)
                    .is_none_or(|layer| layer.order < egui::Order::Foreground)
        });
        let escape = !editing && ui.input(|input| input.key_pressed(egui::Key::Escape));
        if into_window || escape {
            self.connections_shown = false;
        }
    }

    /// Handles the shortcuts of the menu.
    fn shortcuts(&mut self, ctx: &Context) {
        if ctx.input_mut(|input| input.consume_shortcut(&OPEN_RECORDING)) {
            self.files.open(files::Kind::Recording, ctx);
        }
        if ctx.input_mut(|input| input.consume_shortcut(&SAVE_RECORDING))
            && self.state.session.store.has_samples()
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
        let file = self.state.session.store.recording.to_file();
        if let Err(error) = files::save(files::Kind::Recording, &name, &file) {
            self.toasts.error(error);
        }
    }

    /// Opens a recording or a presets file, as which the contents of `file` tell. A
    /// recording is read over the next frames, see [`Self::continue_opening`], replacing
    /// one still being read. The connections are closed and recording stops right away,
    /// so the data shown while it is read is what it replaces. Of a presets file, the
    /// user chooses which presets to import.
    fn open(&mut self, file: files::File) -> Result<(), String> {
        if model::recording::is_recording(&file.bytes) {
            self.opening = Some(opening::Opening::new(file.name, file.bytes)?);
            self.connections.disconnect_all();
            self.state.set_recording(false, false);
        } else if presets::is_presets(&file.bytes) {
            let presets = presets::read(&file.name, &file.bytes)?;
            self.presets.start_import(file.name, presets)?;
        } else {
            return Err(format!(
                "{} is no blip recording or presets file",
                file.name
            ));
        }
        Ok(())
    }

    /// Reads the next part of the recording being opened, and shows it once all is
    /// read: the views start afresh with its data. Connecting while it is read cancels
    /// it, so that its data does not replace that of the connection.
    fn continue_opening(&mut self, ctx: &Context) {
        let Some(opening) = &mut self.opening else {
            return;
        };
        if !self.connections.is_idle() {
            self.toasts.warning(format!(
                "Opening {} was cancelled by connecting",
                opening.name
            ));
            self.opening = None;
            return;
        }
        if !opening.step() {
            ctx.request_repaint();
            return;
        }
        if let Some(opening) = self.opening.take() {
            self.show_opened(opening);
        }
    }

    /// Shows the recording read by `opening`.
    fn show_opened(&mut self, opening: opening::Opening) {
        let name = opening.name.clone();
        let (model, loaded) = opening.finish();
        self.state.start_fresh(model);
        self.reset_traces_panel();
        self.state.time.fit(&self.state.session.store);
        if let Some(problem) = loaded.problem {
            self.toasts.warning(format!(
                "{name} was opened up to message {}, {problem}",
                loaded.messages
            ));
        }
    }

    /// Opens `file` and reads all of a recording at once.
    #[cfg(test)]
    fn open_all(&mut self, file: files::File) -> Result<(), String> {
        self.open(file)?;
        if let Some(mut opening) = self.opening.take() {
            while !opening.read_while(|| true) {}
            self.show_opened(opening);
        }
        Ok(())
    }

    /// Sends the invocations of the views to the connections of the outposts chosen
    /// for their actions, unless the outpost no longer announces the action.
    fn send_invocations(&mut self) {
        for (source, request) in self.state.session.take_requests() {
            self.connections.send(&source, &request);
        }
    }

    /// The tree of the traces and actions, to choose those of the selected view.
    fn traces_ui(&mut self, ui: &mut egui::Ui) {
        theme::section_header(ui, "Traces, actions and logs", |_| {});
        // a distribution linked to a plot shows the traces of the plot
        let linked = (self.layout.selected_link())
            .map(|plot| format!("Shows the traces of the linked plot {plot}"));
        let view = self.layout.selected_view_mut();
        let hint = match &view {
            None => Some("Select a view to choose its traces, actions or logs"),
            Some(view) if !view.has_selection() => {
                Some("The selected view shows no traces, actions or logs")
            }
            Some(_) => None,
        };
        if let Some(hint) = hint.or(linked.as_deref()) {
            theme::panel_hint(ui, hint);
        }
        // a view without traces or actions, like the logs, has nothing to choose
        if view.as_ref().is_some_and(|view| !view.has_selection()) {
            return;
        }
        let filterable = view.as_ref().is_some_and(|view| view.has_selection());
        view::trace_selection::search_ui(
            ui,
            &mut self.trace_search,
            &mut self.compatible_only,
            &mut self.checked_only,
            filterable,
        );
        let model = &self.state.session.store;
        let colored = (view.as_ref()).map_or_else(Vec::new, |view| view.colored_paths(model));
        let selection = view.and_then(|view| view.selection_mut());
        let filter = view::trace_selection::TreeFilter {
            search: &self.trace_search,
            compatible_only: self.compatible_only,
            checked_only: self.checked_only,
        };
        // a linked view shows its checks, which the user cannot change
        view::trace_selection::show(ui, model, filter, selection, &colored, linked.is_none());
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut egui::RawInput) {
        view::zoom_with_wheel(ctx, raw_input);
        time::take_wheel(ctx, raw_input);
    }

    fn logic(&mut self, ctx: &Context, _frame: &mut Frame) {
        let now = ctx.input(|input| input.time);
        // before receiving, which may find a connection broken
        self.connections.set_recording(self.state.session.recording);
        self.receive(now);
        for (level, text) in self.state.session.take_notices() {
            match level {
                toasts::Level::Error => self.toasts.error(text),
                toasts::Level::Warning => self.toasts.warning(text),
            }
        }
        let connected = self.connections.open_names();
        self.state.session.update_invoker(connected, now);
        for error in self.connections.take_errors() {
            self.toasts.error(error);
        }
        for warning in self.connections.take_warnings() {
            self.toasts.warning(warning);
        }
        self.files.receive_dropped(ctx);
        while let Some(file) = self.files.try_recv() {
            if let Err(error) = file.and_then(|file| self.open(file)) {
                self.toasts.error(error);
            }
        }
        self.continue_opening(ctx);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shortcuts(ui.ctx());
        // before the traces panel shows what a linked distribution shows
        self.layout.take_linked_traces();
        let top_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 8));
        egui::Panel::top("top panel")
            .frame(top_frame)
            .show(ui, |ui| self.menu_ui(ui));

        let now = ui.input(|input| input.time);
        self.state.time.update(&self.state.session.store, now);

        let bottom_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(10, 8));
        egui::Panel::bottom("bottom panel")
            .frame(bottom_frame)
            .show(ui, |ui| {
                let connected = self.connections.is_connected();
                self.state.bottom_ui(ui, connected);
            });

        // the side panels leave the room at their sides to their sections, so that the
        // lines between the sections go from edge to edge
        let side_frame =
            egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(0, 2));
        let section_frame = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 4));
        let rest_frame = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 0));
        let mut left_open = !self.left_collapsed;
        egui::Panel::left("left panel")
            .resizable(true)
            .default_size(220.0)
            .frame(side_frame)
            .show_collapsible(ui, &mut left_open, |ui| {
                section_frame.show(ui, |ui| self.traces_panel_ui(ui));
            });
        // dragging its edge past the smallest size collapses a panel too
        self.left_collapsed = !left_open;

        let mut right_open = !self.right_collapsed;
        egui::Panel::right("properties panel")
            .resizable(true)
            .default_size(264.0)
            .frame(side_frame)
            .show_collapsible(ui, &mut right_open, |ui| {
                ui.add_space(4.0);
                let context = view::PropertiesContext {
                    store: &self.state.session.store,
                    connected: &self.state.session.invoker.connected,
                    // the layout lists its plots
                    plots: &view::NO_PLOTS,
                };
                rest_frame.show(ui, |ui| self.layout.properties_ui(ui, &context));
            });
        self.right_collapsed = !right_open;

        let central = egui::CentralPanel::default()
            .frame(
                // the views are cards on the canvas, with a gap around them, the tab bar
                // on top brings its own room above the tabs
                egui::Frame::central_panel(ui.style())
                    .fill(ui.tokens().canvas)
                    .inner_margin(egui::Margin {
                        top: 2,
                        ..egui::Margin::same(theme::GAP as i8)
                    }),
            )
            .show(ui, |ui| {
                let mut context = ViewContext::new(
                    &self.state.session.store,
                    &mut self.state.time,
                    &mut self.state.session.invoker,
                );
                self.layout.ui(ui, &mut context);
            });
        self.send_invocations();
        if let Some(seconds) = self.state.session.invoker.repaint_after(now) {
            ui.ctx().request_repaint_after_secs(seconds as f32);
        }

        let progress = self.opening.as_ref().map(|opening| toasts::Progress {
            text: format!("Opening {}", opening.name),
            fraction: opening.progress(),
        });
        if self.toasts.ui(ui.ctx(), central.response.rect, progress) {
            self.opening = None;
        }
        if let Some(action) = self
            .presets
            .dialog_ui(ui.ctx(), self.state.session.recording)
        {
            self.apply_preset(action, ui.ctx());
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        self.store_preset();
        self.presets.save(&mut KeptInMut(storage));
        // the layout kept apart before presets
        storage.remove_string("layout");
    }
}

/// Shows the app icon at the start of the menu bar.
fn logo_ui(ui: &mut egui::Ui) {
    let texture = logo_texture(ui.ctx());
    ui.add(egui::Image::new(&texture).fit_to_exact_size(egui::vec2(20.0, 20.0)));
}

/// The app icon as a texture, decoded on first use and then kept in the memory of
/// `ctx`.
fn logo_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("logo_texture");
    if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let png = include_bytes!("../assets/icon-64.png");
    let image = image::load_from_memory(png)
        .expect("the icon is a valid PNG")
        .into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    // mipmaps keep it smooth when shrunk at low pixel densities
    let options = egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear));
    let texture = ctx.load_texture("logo", image, options);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

/// Width of the list of connections that opens from the [`status_chip`].
const CONNECTIONS_WIDTH: f32 = 300.0;

/// A chip with the status of the connections: a green dot (amber while a connection
/// speaks a newer protocol, `newer` has its lines) and the name of the open
/// connection or their number, and the messages received per second, `rate`. It is a
/// button that opens the list of connections.
fn status_chip(
    ui: &mut egui::Ui,
    open: &HashSet<String>,
    newer: &[String],
    rate: Option<f64>,
) -> egui::Response {
    let tokens = ui.tokens();
    let id = ui.id().with("status chip");
    // the frame shows the state of the previous frame, as it is drawn before the click
    // is sensed on it
    let hovered = ui
        .ctx()
        .read_response(id)
        .is_some_and(|chip| chip.hovered());
    let text = match open.len() {
        0 => "Not connected".to_owned(),
        1 => open.iter().next().cloned().unwrap_or_default(),
        count => format!("{count} connections"),
    };
    let (fill, stroke) = if hovered {
        (tokens.hover, tokens.border_strong)
    } else {
        (tokens.control, tokens.border)
    };
    let frame = egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(theme::CONTROL_RADIUS)
        .inner_margin(egui::Margin::symmetric(8, 3));
    let response = frame
        .show(ui, |ui| {
            // from the right, as it sits at the right end of the menu bar
            let layout = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(layout, |ui| {
                ui.weak(egui_phosphor::regular::CARET_DOWN);
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
                    let color = if newer.is_empty() {
                        tokens.ok
                    } else {
                        tokens.warn
                    };
                    ui.painter().circle_filled(dot.center(), 3.5, color);
                }
            });
        })
        .response;
    let response = ui
        .interact(response.rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Connections"));
    let mut hint = String::new();
    if open.len() > 1 {
        let mut names: Vec<&String> = open.iter().collect();
        names.sort();
        let names: Vec<&str> = names.into_iter().map(String::as_str).collect();
        hint = names.join("\n") + "\n\n";
    }
    hint.push_str("Click to show the connections");
    if !newer.is_empty() {
        hint = newer.join("\n") + "\n\n" + &hint;
    }
    response.on_hover_text(hint)
}

/// `number` with its digits in groups of three, split by narrow spaces.
fn group_thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
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
    let tokens = ui.tokens();
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

/// A side of the window with a panel.
#[derive(Clone, Copy)]
enum PanelSide {
    Left,
    Right,
}

/// A button collapsing the panel at `side`, or expanding it again. Its icon is a window
/// with the panel filled in while the panel is shown.
fn panel_button(ui: &mut egui::Ui, side: PanelSide, collapsed: &mut bool) {
    use egui::emath::GuiRounding as _;
    let hint = match (side, *collapsed) {
        (PanelSide::Left, false) => "Hide the traces and the layout",
        (PanelSide::Left, true) => "Show the traces and the layout",
        (PanelSide::Right, false) => "Hide the properties and the connections",
        (PanelSide::Right, true) => "Show the properties and the connections",
    };
    let square = egui::Vec2::splat(ui.spacing().interact_size.y);
    let tokens = ui.tokens();
    let response = ui
        .add(
            egui::Button::new("")
                .fill(tokens.control)
                .stroke(egui::Stroke::new(1.0, tokens.border))
                .min_size(square),
        )
        .on_hover_text(hint);
    if response.clicked() {
        *collapsed = !*collapsed;
    }

    let color = ui.style().interact(&response).text_color();
    let window = egui::Rect::from_center_size(response.rect.center(), egui::vec2(14.0, 12.0))
        .round_to_pixels(ui.pixels_per_point());
    let painter = ui.painter();
    let stroke = egui::Stroke::new(1.0, color);
    painter.rect_stroke(window, 2, stroke, egui::StrokeKind::Inside);
    let width = 5.0;
    let (panel, corners) = match side {
        PanelSide::Left => (
            window.with_max_x(window.min.x + width),
            egui::CornerRadius {
                nw: 2,
                sw: 2,
                ..Default::default()
            },
        ),
        PanelSide::Right => (
            window.with_min_x(window.max.x - width),
            egui::CornerRadius {
                ne: 2,
                se: 2,
                ..Default::default()
            },
        ),
    };
    if *collapsed {
        // only the line at the edge of the panel
        let x = match side {
            PanelSide::Left => panel.max.x - 0.5,
            PanelSide::Right => panel.min.x + 0.5,
        };
        painter.vline(x, window.y_range(), stroke);
    } else {
        painter.rect_filled(panel, corners, color);
    }
}

#[cfg(test)]
mod tests;
