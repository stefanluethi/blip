mod connections;
mod files;
mod layout;
mod model;
mod time;
mod view;

use eframe::Frame;
use egui::{Context, Key, KeyboardShortcut, Modifiers};
use std::collections::BTreeMap;
use view::ViewContext;

const OPEN_RECORDING: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE_RECORDING: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct State {
    time: time::TimeRange,

    #[serde(skip)]
    model: model::Model,
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
    #[serde(skip)]
    files: files::Files,
    /// A message for the user, e.g. why a file could not be opened.
    #[serde(skip)]
    notice: Option<String>,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "phosphor".into(),
            egui::FontData::from_static(egui_phosphor::Variant::Regular.font_bytes()).into(),
        );
        if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            font_keys.insert(1, "phosphor".into());
        }
        if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            font_keys.push("phosphor".into());
        }
        cc.egui_ctx.set_fonts(fonts);

        configure_text_styles(&cc.egui_ctx);

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
                let recorded = !self.state.model.recording.is_empty();
                let messages = self.state.model.recording.len();
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
                    self.state.model = model::Model::default();
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
            && !self.state.model.recording.is_empty()
        {
            self.save_recording();
        }
    }

    fn save_recording(&mut self) {
        let name = format!("{}.blip", chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
        let file = self.state.model.recording.to_file();
        if let Err(error) = files::save(files::Kind::Recording, &name, &file) {
            self.notice = Some(error);
        }
    }

    fn save_layout(&mut self) {
        let file = serde_json::to_vec_pretty(&self.layout.to_file()).expect("layouts are JSON");
        if let Err(error) = files::save(files::Kind::Layout, "layout.json", &file) {
            self.notice = Some(error);
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
            self.state.model = model;
            self.state.time.fit(&self.state.model);
            if let Some(problem) = loaded.problem {
                self.notice = Some(format!(
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

    /// Shows the notice, if any, until the user closes it.
    fn notice_ui(&mut self, ctx: &Context) {
        let Some(notice) = &self.notice else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("notice")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.label(notice);
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("OK").clicked() {
                    ui.close();
                }
            });
        });
        if modal.should_close() {
            self.notice = None;
        }
    }

    fn traces_ui(&mut self, ui: &mut egui::Ui) {
        ui.strong("Traces");
        let view = self.layout.selected_view_mut();
        let hint = match &view {
            None => Some("Select a view to choose its traces"),
            Some(view) if !view.shows_traces() => Some("The selected view shows no traces"),
            Some(_) => None,
        };
        if let Some(hint) = hint {
            ui.weak(hint);
        }
        view::trace_selection::search_ui(ui, &mut self.trace_search);
        egui::ScrollArea::vertical()
            .id_salt("traces")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                view::trace_selection::show(
                    ui,
                    &self.state.model,
                    &self.trace_search,
                    view.and_then(view::View::selection_mut),
                );
            });
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.connections
            .receive(|source, frame| self.state.model.receive(source, frame));
        self.files.receive_dropped(ctx);
        while let Some(file) = self.files.try_recv() {
            if let Err(error) = file.and_then(|file| self.open(&file)) {
                self.notice = Some(error);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shortcuts(ui.ctx());
        egui::Panel::top("top panel").show(ui, |ui| self.menu_ui(ui));

        self.state.time.update(&self.state.model);

        egui::Panel::bottom("bottom panel").show(ui, |ui| {
            self.state.time.ui(ui);
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
                    .show(ui, |ui| self.traces_ui(ui));
                ui.add_space(4.0);
                self.layout.panel_ui(ui);
            });

        egui::Panel::right("properties panel")
            .resizable(true)
            .default_size(240.0)
            .show(ui, |ui| {
                // the connections below, resizable by dragging the border to the properties
                egui::Panel::bottom("connections panel")
                    .resizable(true)
                    .default_size(160.0)
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                    .show(ui, |ui| self.connections.ui(ui));
                ui.add_space(4.0);
                self.layout.properties_ui(ui, &self.state.model);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::ZERO))
            .show(ui, |ui| {
                let mut context = ViewContext {
                    model: &self.state.model,
                    time: &mut self.state.time,
                    formats: &view::NO_FORMATS,
                };
                self.layout.ui(ui, &mut context);
            });

        self.notice_ui(ui.ctx());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        eframe::set_value(storage, LAYOUT_KEY, &self.layout.to_file());
    }
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
    if ui.button(icon).on_hover_text(hint).clicked() {
        ui.ctx().set_theme(next);
    }
}

fn configure_text_styles(ctx: &egui::Context) {
    use egui::FontFamily::{Monospace, Proportional};
    use egui::FontId;
    use egui::style::TextStyle;

    let text_styles: BTreeMap<TextStyle, FontId> = [
        (TextStyle::Heading, FontId::new(24.0, Proportional)),
        (TextStyle::Body, FontId::new(14.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
        (TextStyle::Button, FontId::new(14.0, Proportional)),
        (TextStyle::Small, FontId::new(10.0, Proportional)),
    ]
    .into();
    ctx.all_styles_mut(move |style| style.text_styles = text_styles.clone());
}
