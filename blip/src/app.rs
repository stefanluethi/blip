mod layout;
mod proto_parser;
mod view;
mod model;
mod time;


use std::collections::BTreeMap;
use eframe::Frame;
use egui::Context;
use view::ViewContext;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct State {
    time: time::TimeRange,

    #[serde(skip)]
    model: model::Model,

    target_address: String,

    #[serde(skip)]
    ws_receiver: Option<ewebsock::WsReceiver>,
    #[serde(skip)]
    ws_sender: Option<ewebsock::WsSender>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            target_address: "ws://localhost/ws/simulator".to_owned(),
            time: time::TimeRange::default(),
            model: model::Model::default(),
            ws_receiver: None,
            ws_sender: None,
        }
    }
}


impl State {
    fn websocket_connect(&mut self, ctx: egui::Context) {
        let wakeup = move || ctx.request_repaint();
        let options = ewebsock::Options {
            max_incoming_frame_size: 2048,
            ..Default::default()
        };
        let (sender, receiver) =
            ewebsock::connect_with_wakeup(self.target_address.clone(), options, wakeup).unwrap();
        self.model = model::Model::default();
        self.ws_receiver = Some(receiver);
        self.ws_sender = Some(sender);
    }

    fn websocket_disconnect(&mut self) {
        if let Some(sender) = self.ws_sender.as_mut() {
            sender.close();
        }
        self.ws_receiver = None;
        self.ws_sender = None;
        log::info!("ws disconnected")
    }

    fn websocket_receive(&mut self) -> bool {
        if let Some(receiver) = self.ws_receiver.as_ref() {
            while let Some(event) = receiver.try_recv() {
                match event {
                    ewebsock::WsEvent::Opened => {
                        log::info!("ws connected");
                    }
                    ewebsock::WsEvent::Message(message) => match message {
                        ewebsock::WsMessage::Text(msg) => {
                            log::info!("{}", msg);
                        }
                        ewebsock::WsMessage::Binary(msg) => {
                            proto_parser::parse_message(msg, &mut self.model);
                        }
                        _ => {
                            log::warn!("ws unknown message received");
                        }
                    },
                    ewebsock::WsEvent::Error(e) => {
                        log::warn!("ws error: {}", e);
                        return true;
                    }
                    ewebsock::WsEvent::Closed => {
                        log::info!("ws disconnected");
                        return true;
                    }
                };
            }
        }
        false
    }

    #[expect(dead_code, reason = "no UI sends commands yet")]
    fn websocket_send(&mut self, command: proto::Action) {
        if let Some(sender) = self.ws_sender.as_mut() {
            let command_packet = serde_cbor_2::to_vec(&command).unwrap();
            log::debug!("send command {}", hex_string::HexString::from_bytes(&command_packet).as_string());
            sender.send(ewebsock::WsMessage::Binary(command_packet));
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("blip");

        ui.horizontal(|ui| {
            ui.label("websocket address:");
            ui.text_edit_singleline(&mut self.target_address);
            if ui
                .button(if self.ws_receiver.is_none() {
                    "Connect"
                } else {
                    "Disconnect"
                })
                .clicked()
            {
                if self.ws_receiver.is_none() {
                    self.websocket_connect(ui.ctx().clone());
                } else {
                    self.websocket_disconnect();
                }
            }
            if ui.button("Clear Data").clicked() {
                self.model = model::Model::default();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme_button(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}



//--------------------------------------------------------------------------------------------------

/// Storage key of the layout, saved apart from the app state in its own versioned format.
const LAYOUT_KEY: &str = "layout";

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    state: State,
    #[serde(skip)]
    layout: layout::Layout,
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

    fn streams_ui(&mut self, ui: &mut egui::Ui) {
        ui.strong("Streams");
        let view = self.layout.selected_view_mut();
        let hint = match &view {
            None => Some("Select a plot to choose its streams"),
            Some(view) if !view.shows_streams() => Some("The selected view shows no streams"),
            Some(_) => None,
        };
        if let Some(hint) = hint {
            ui.weak(hint);
        }
        egui::ScrollArea::vertical()
            .id_salt("streams")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                view::stream_selection::show(ui, &self.state.model, view.and_then(view::View::streams_mut));
            });
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &Context, _frame: &mut Frame) {
        if self.state.websocket_receive() {
            self.state.websocket_disconnect();
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top panel").show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                self.state.settings_ui(ui);
            });
        });

        self.state.time.update_span(&self.state.model);

        egui::Panel::bottom("bottom panel").show(ui, |ui| {
            self.state.time.ui(ui);
        });

        egui::Panel::left("left panel")
            .resizable(true)
            .default_size(220.0)
            .show(ui, |ui| {
                // the streams on top, resizable by dragging the border to the layout
                egui::Panel::top("streams panel")
                    .resizable(true)
                    .default_size(250.0)
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                    .show(ui, |ui| self.streams_ui(ui));
                ui.add_space(4.0);
                self.layout.panel_ui(ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::ZERO))
            .show(ui, |ui| {
                let mut context = ViewContext {
                    model: &self.state.model,
                    time: &mut self.state.time,
                };
                self.layout.ui(ui, &mut context);
            });
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
        ThemePreference::Light => (egui_phosphor::regular::SUN, "Light theme", ThemePreference::Dark),
        ThemePreference::Dark => (egui_phosphor::regular::MOON, "Dark theme", ThemePreference::System),
        ThemePreference::System => (egui_phosphor::regular::CIRCLE_HALF, "System theme", ThemePreference::Light),
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
