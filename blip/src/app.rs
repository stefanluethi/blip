mod proto_parser;
mod view;
mod model;


use std::collections::BTreeMap;
use eframe::Frame;
use egui::Context;
use view::Viewable;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct State {
    stream_view: view::plot::Plot,
    log_view: view::log_table::LogTable,

    model: model::Model,

    target_address: String,

    #[serde(skip)]
    ws_receiver: Option<ewebsock::WsReceiver>,
    #[serde(skip)]
    ws_sender: Option<ewebsock::WsSender>,

    #[serde(skip)]
    panes_in_tabs: std::collections::HashSet<egui_tiles::TileId>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            target_address: "ws://localhost/ws/simulator".to_owned(),
            stream_view: view::plot::Plot::new(),
            log_view: view::log_table::LogTable::new(),
            model: model::Model::default(),
            ws_receiver: None,
            ws_sender: None,
            panes_in_tabs: std::collections::HashSet::new(),
        }
    }
}


impl State {
    fn websocket_connect(&mut self, ctx: egui::Context) {
        let wakeup = move || ctx.request_repaint();
        let mut options = ewebsock::Options::default();
        options.max_incoming_frame_size = 2048;
        let (sender, receiver) =
            ewebsock::connect_with_wakeup(self.target_address.clone(), options, wakeup).unwrap();
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
        });
    }
}



//--------------------------------------------------------------------------------------------------

#[derive(serde::Deserialize, serde::Serialize)]
enum Pane {
    Plots,
    TargetLogs,
}

impl From<&Pane> for &str {
    fn from(value: &Pane) -> Self {
        match value {
            Pane::Plots => "Data Streams",
            Pane::TargetLogs => "Target Logs",
        }
    }
}

impl From<&mut Pane> for &str {
    fn from(value: &mut Pane) -> Self {
        (value as &Pane).into()
    }
}

impl egui_tiles::Behavior<Pane> for State {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        let title: &str = pane.into();
        let mut drag_response = egui_tiles::UiResponse::None;

        let is_tab = self.panes_in_tabs.contains(&_tile_id);

        if !is_tab {
            let title_bar_color = if ui.visuals().dark_mode {
                ui.visuals().extreme_bg_color
            } else {
                ui.visuals().widgets.inactive.bg_fill
            };
            let title_bar_response = egui::Frame::NONE
                .fill(title_bar_color)
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(title);
                        ui.allocate_space(ui.available_size())
                    })
                        .response
                })
                .inner;

            if title_bar_response
                .interact(egui::Sense::click_and_drag())
                .dragged()
            {
                drag_response = egui_tiles::UiResponse::DragStarted;
            }
        }

        egui::Frame::NONE
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                match pane {
                    Pane::Plots => {
                        self.stream_view.view(ui, &mut self.model);
                    }
                    Pane::TargetLogs => {
                        self.log_view.view(ui, &mut self.model);
                    }

                }
            });

        drag_response
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        Into::<&str>::into(pane).into()
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    state: State,
    pane_tree: egui_tiles::Tree<Pane>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: Default::default(),
            pane_tree: App::create_tree(),
        }
    }
}

impl App {
    fn create_tree() -> egui_tiles::Tree<Pane> {
        let mut tiles = egui_tiles::Tiles::default();
        let plots = tiles.insert_pane(Pane::Plots);
        let target_logs = tiles.insert_pane(Pane::TargetLogs);

        let root = tiles.insert_horizontal_tile(vec![plots, target_logs]);

        egui_tiles::Tree::new("main_tree", root, tiles)
    }

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
        let mut app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        if app.pane_tree.root.is_none() {
            app.pane_tree = App::create_tree();
        }

        // renew tree if new panes have been added
        // if !app.pane_tree.inactive_tiles().is_empty() {
        //    app.pane_tree = App::create_tree();
        // }

        app
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &Context, _frame: &mut Frame) {
        if self.state.websocket_receive() {
            self.state.websocket_disconnect();
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // todo: find viable solution!!!
        self.state.panes_in_tabs.clear();
        for (_, tile) in self.pane_tree.tiles.iter() {
            if let egui_tiles::Tile::Container(c) = tile {
                if c.kind() == egui_tiles::ContainerKind::Tabs {
                    for &child in c.children() {
                        self.state.panes_in_tabs.insert(child);
                    }
                }
            }
        }

        egui::Panel::top("top panel").show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                self.state.settings_ui(ui);
            });
        });

        egui::Panel::bottom("bottom panel").show(ui, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                egui::warn_if_debug_build(ui);
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(egui::Margin::ZERO))
            .show(ui, |ui| {
            self.pane_tree.ui(&mut self.state, ui);
        });

    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
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
