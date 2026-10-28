/// Defines a fieldless enum with the name shown for each variant, written
/// `Variant => "Name"`, and gives it `ALL` its variants in their order, `CHOICES` each
/// with its name, e.g. for [`view::choice_row`], and `name()`.
macro_rules! named_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident => $text:literal),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$variant_meta])* $variant),*
        }

        #[allow(dead_code)]
        impl $name {
            pub const ALL: [Self; [$($text),*].len()] = [$(Self::$variant),*];
            pub const CHOICES: [(Self, &'static str); [$($text),*].len()] =
                [$((Self::$variant, $text)),*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),*
                }
            }
        }
    };
}

mod connections;
mod files;
mod layout;
mod model;
#[cfg(all(test, feature = "screenshots"))]
mod screenshots;
mod theme;
mod time;
mod toasts;
mod view;

use connections::Received;
use eframe::Frame;
use egui::{Context, Key, KeyboardShortcut, Modifiers};
use std::collections::HashSet;
use view::{ViewContext, Viewable};

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
    }

    /// Starts afresh with `model`, e.g. of an opened recording: drops all data, also the
    /// descriptions of the targets that [`Self::clear_data`] keeps, stops recording and
    /// forgets the invocations.
    fn start_fresh(&mut self, model: model::Model) {
        self.recording = false;
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
        }
        self.recording = recording;
    }

    /// The bottom panel: the record button and the time range.
    fn bottom_ui(&mut self, ui: &mut egui::Ui, connected: bool) {
        // larger controls than elsewhere, as the bar is used all the time
        ui.spacing_mut().interact_size.y = 28.0;
        ui.spacing_mut().button_padding = egui::vec2(10.0, 4.0);
        let recording = self.recording;
        let clicked = self.time.ui(ui, &self.model, recording, |ui| {
            record_button(ui, recording, connected)
        });
        if clicked {
            self.set_recording(!self.recording, connected);
        }
    }
}

/// A button showing whether `recording`, which can be started only while `connected`.
/// Returns whether it was clicked, to switch.
fn record_button(ui: &mut egui::Ui, recording: bool, connected: bool) -> bool {
    // plain while idle, red like a record key and filled with a tint of it while recording
    let tokens = theme::tokens(ui.visuals());
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
    /// Whether the left panel, with the traces and the layout, is collapsed.
    left_collapsed: bool,
    /// Whether the right panel, with the properties, is collapsed.
    right_collapsed: bool,
    #[serde(skip)]
    files: files::Files,
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
                ui.menu_button("Add view", |ui| {
                    for view_type in view::ViewType::ALL {
                        let text = format!("{} {}", view_type.icon(), view_type.name());
                        if ui.button(text).clicked() {
                            self.layout.add_view(view_type);
                        }
                    }
                });
                ui.separator();
                egui::gui_zoom::zoom_menu_buttons(ui);
                ui.weak(format!("Zoom {:.0} %", ui.ctx().zoom_factor() * 100.0));
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
                let chip = status_chip(ui, &open, self.rate.per_second);
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
                    self.connections.ui(ui, self.state.recording);
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
        if let Err(error) = files::save(files::Kind::Recording, &name, &file) {
            self.toasts.error(error);
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
        // a view without traces or actions, like the logs, has nothing to choose
        if view.as_ref().is_some_and(|view| !view.has_selection()) {
            return;
        }
        let filterable = view.as_ref().is_some_and(|view| view.has_selection());
        view::trace_selection::search_ui(
            ui,
            &mut self.trace_search,
            &mut self.compatible_only,
            filterable,
        );
        let model = &self.state.model;
        let colored = (view.as_ref()).map_or_else(Vec::new, |view| view.colored_paths(model));
        let selection = view.and_then(|view| view.selection_mut());
        view::trace_selection::show(
            ui,
            model,
            &self.trace_search,
            self.compatible_only,
            selection,
            &colored,
        );
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut egui::RawInput) {
        view::zoom_with_wheel(ctx, raw_input);
        time::take_wheel(ctx, raw_input);
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
                    model: &self.state.model,
                    connected: &self.state.invoker.connected,
                };
                rest_frame.show(ui, |ui| self.layout.properties_ui(ui, &context));
            });
        self.right_collapsed = !right_open;

        let central = egui::CentralPanel::default()
            .frame(
                // the views are cards on the canvas, with a gap around them, the tab bar
                // on top brings its own room above the tabs
                egui::Frame::central_panel(ui.style())
                    .fill(theme::tokens(ui.visuals()).canvas)
                    .inner_margin(egui::Margin {
                        top: 2,
                        ..egui::Margin::same(theme::GAP as i8)
                    }),
            )
            .show(ui, |ui| {
                let mut context = ViewContext {
                    model: &self.state.model,
                    time: &mut self.state.time,
                    invoker: &mut self.state.invoker,
                    formats: &view::NO_FORMATS,
                    titled: false,
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

/// A chip with the status of the connections: a green dot and the name of the open
/// connection or their number, and the messages received per second, `rate`. It is a
/// button that opens the list of connections.
fn status_chip(ui: &mut egui::Ui, open: &HashSet<String>, rate: Option<f64>) -> egui::Response {
    let tokens = theme::tokens(ui.visuals());
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
                    ui.painter().circle_filled(dot.center(), 3.5, tokens.ok);
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
    response.on_hover_text(hint)
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
    let tokens = theme::tokens(ui.visuals());
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
            .with_size(egui::vec2(600.0, 60.0))
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
    fn the_connections_open_from_the_chip_and_stay_open_in_their_menus() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 500.0))
            .build_ui_state(|ui, app: &mut App| app.menu_ui(ui), App::default());
        harness.run();
        assert!(harness.query_by_label_contains("ws://").is_none());

        harness.get_by_label("Connections").click();
        harness.run();
        assert!(harness.query_by_label_contains("ws://").is_some());

        // the menu of a connection is part of the list, using it keeps the list open
        harness.get_by_label_contains("ws://").click_secondary();
        harness.run();
        harness.get_by_label_contains("Edit address").click();
        harness.run();
        assert!(
            harness
                .query_by_role(egui::accesskit::Role::TextInput)
                .is_some()
        );

        // a click elsewhere closes it
        let elsewhere = egui::pos2(20.0, 400.0);
        harness.hover_at(elsewhere);
        harness.drag_at(elsewhere);
        harness.drop_at(elsewhere);
        harness.run();
        assert!(harness.query_by_label_contains("ws://").is_none());
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
