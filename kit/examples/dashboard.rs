//! A dashboard built in code on blip-kit, without the workbench: no traces panel, no
//! properties and no layout to arrange. It shows two tanks filling, simulated in the
//! application itself, in a plot, a table with a button for each tank, and the
//! timeline.
//!
//! ```sh
//! cargo run --release -p blip-kit --example dashboard
//! ```
//!
//! What it takes:
//!
//! - a [`Source`], here the [`Simulator`]: it tells what the target sends and takes the
//!   invocations of its actions, with no outpost and no socket in between;
//! - a [`Session`], which receives from the sources into its store and sends the
//!   invocations;
//! - a [`TimeRange`], the time all widgets show;
//! - the widgets, made with the traces they show and drawn where the application
//!   wants them.

use blip_kit::proto;
use blip_kit::store::session::Session;
use blip_kit::store::source::{Event, Source};
use blip_kit::widgets::theme;
use blip_kit::widgets::time::{self, TimeRange};
use blip_kit::widgets::view::plot::Plot;
use blip_kit::widgets::view::table::Table;
use blip_kit::widgets::view::{self, ViewContext, Viewable as _};
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

/// The name of the simulator among the sources, which its actions are invoked by.
const SIMULATOR: &str = "simulator";
/// The simulator sends a sample of each tank this often, in seconds.
const PERIOD: f64 = 0.02;
/// The tanks with how fast each fills, in litres a second.
const TANKS: [(&str, f64); 2] = [("a", 4.0), ("b", 7.0)];
const CAPACITY: f64 = 100.0;

/// Two tanks that fill until they are full, and are drained by the action `drain`.
/// It reads the time from `clock`, which the application sets each frame, so it runs
/// the same natively and in the browser.
struct Simulator {
    clock: Rc<Cell<f64>>,
    /// The time in seconds up to which samples were sent, `None` before it opened.
    sent_until: Option<f64>,
    levels: [f64; TANKS.len()],
    queue: VecDeque<Event>,
}

impl Simulator {
    fn new(clock: Rc<Cell<f64>>) -> Self {
        Self {
            clock,
            sent_until: None,
            levels: [0.0; TANKS.len()],
            queue: VecDeque::new(),
        }
    }

    fn tell(&mut self, message: proto::Message) {
        self.queue.push_back(Event::message(&message));
    }

    fn log(&mut self, time: f64, level: u8, message: String) {
        self.tell(proto::Message::Log(proto::LogRecord {
            timestamp: nanoseconds(time),
            path: "log/tanks".to_owned(),
            level,
            message,
        }));
    }

    /// What a target says once: who it is, what its traces are and what it can do.
    fn describe(&mut self) {
        self.tell(proto::Message::Hello(proto::Hello {
            version: proto::VERSION,
            name: Some("Tanks".to_owned()),
            clock: Some(proto::Clock::Boot),
            sender: None,
        }));
        for (tank, _) in TANKS {
            self.tell(proto::Message::TraceMetadata(proto::TraceMetadata {
                path: format!("tank/{tank}/level"),
                unit: "l".to_owned(),
                description: format!("How much is in tank {tank}"),
                range: Some(proto::ExpectedRange {
                    min: 0.0,
                    max: CAPACITY,
                }),
                ..Default::default()
            }));
        }
        self.tell(proto::Message::Actions(vec![proto::ActionDescriptor {
            path: "drain".to_owned(),
            description: "Empties a tank".to_owned(),
            params: vec![proto::Param {
                name: "tank".to_owned(),
                kind: proto::ParamKind::Enum(
                    TANKS.iter().map(|(tank, _)| (*tank).to_owned()).collect(),
                ),
                unit: String::new(),
                default: None,
            }],
            readback: None,
            confirm: false,
            enabled: true,
            disabled_reason: None,
        }]));
    }

    /// Fills the tanks up to the time of the clock and sends their levels.
    fn run(&mut self) {
        let now = self.clock.get();
        let Some(mut time) = self.sent_until else {
            return;
        };
        // after a long pause, e.g. a hidden tab, the tanks go on from now
        if now - time > 5.0 {
            time = now - 5.0;
        }
        while time + PERIOD <= now {
            time += PERIOD;
            for (index, (tank, rate)) in TANKS.into_iter().enumerate() {
                let before = self.levels[index];
                let level = (before + rate * PERIOD).min(CAPACITY);
                self.levels[index] = level;
                if level >= CAPACITY && before < CAPACITY {
                    self.log(time, proto::level::WARN, format!("Tank {tank} is full"));
                }
                self.tell(proto::Message::Sample(proto::Sample {
                    timestamp: nanoseconds(time),
                    path: format!("tank/{tank}/level"),
                    value: proto::Data::Float(level),
                }));
            }
        }
        self.sent_until = Some(time);
    }

    /// Carries out the action `drain` on the tank named in `args`.
    fn drain(&mut self, args: &proto::Data) -> Result<(), String> {
        let proto::Data::Record(args) = args else {
            return Err("drain takes the tank to drain".to_owned());
        };
        let tank = args.iter().find_map(|(name, value)| match value {
            proto::Data::String(tank) if name == "tank" => Some(tank.as_str()),
            _ => None,
        });
        let index = TANKS
            .iter()
            .position(|(known, _)| Some(*known) == tank)
            .ok_or("there is no such tank")?;
        self.levels[index] = 0.0;
        let time = self.clock.get();
        self.log(
            time,
            proto::level::INFO,
            format!("Tank {} drained", TANKS[index].0),
        );
        Ok(())
    }
}

impl Source for Simulator {
    fn poll(&mut self) -> Option<Event> {
        if self.sent_until.is_none() {
            self.sent_until = Some(self.clock.get());
            self.describe();
            return Some(Event::Opened);
        }
        if self.queue.is_empty() {
            self.run();
        }
        self.queue.pop_front()
    }

    fn send(&mut self, request: &proto::Request) {
        let proto::Request::Invoke(invocation) = request;
        let result = match invocation.path.as_str() {
            "drain" => self.drain(&invocation.args),
            other => Err(format!("there is no action {other}")),
        };
        self.tell(proto::Message::ActionResult(proto::ActionResult {
            id: invocation.id,
            result,
        }));
    }
}

/// `seconds` since the simulator started as a timestamp since boot.
fn nanoseconds(seconds: f64) -> u64 {
    (seconds * 1.0e9) as u64
}

/// The tank of the trace at `path`, e.g. `a` of `tank/a/level`.
fn tank_of(path: &str) -> Option<&str> {
    path.strip_prefix("tank/")?.strip_suffix("/level")
}

struct Dashboard {
    /// The time of egui in seconds, which the simulator runs by.
    clock: Rc<Cell<f64>>,
    session: Session,
    time: TimeRange,
    plot: Plot,
    table: Table,
}

impl Dashboard {
    fn new(ctx: &egui::Context) -> Self {
        // the fonts and colours the widgets are drawn with
        theme::install(ctx);
        let clock = Rc::new(Cell::new(0.0));
        // there is no record button: the samples are kept from the start
        let mut session = Session::recording();
        session.add_source(SIMULATOR, Simulator::new(clock.clone()));
        let levels = TANKS.map(|(tank, _)| format!("tank/{tank}/level"));
        Self {
            clock,
            session,
            time: TimeRange::default(),
            plot: view::showing(levels.clone()),
            // a table of the levels with a column of the application: a button in the
            // row of each tank, which invokes the action of the simulator
            table: view::showing::<Table>(levels).column("Drain", |ui, cell| {
                let Some(tank) = tank_of(cell.trace.path) else {
                    return;
                };
                if ui.button("Drain").clicked() {
                    let args = vec![("tank".to_owned(), proto::Data::from(tank))];
                    cell.invoke(SIMULATOR, "drain", args);
                }
            }),
        }
    }
}

impl Dashboard {
    /// Receives from the simulator into the store at the time `now` in seconds, once a
    /// frame before the widgets are shown.
    fn update(&mut self, now: f64) {
        self.clock.set(now);
        self.session.update(now);
        for (_, notice) in self.session.take_notices() {
            log::warn!("{notice}");
        }
    }

    /// Shows the dashboard: the time controls with the timeline at the bottom, the
    /// table at the right and the plot in the room that is left.
    fn show(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|input| input.time);
        self.time.update(&self.session.store, now);

        egui::Panel::bottom("time").show(ui, |ui| {
            // play, pause, go live and the window length, with the timeline beside them
            self.time.ui(ui, &self.session.store, true, |_| ());
        });
        egui::Panel::right("tanks")
            .default_size(320.0)
            .show(ui, |ui| {
                ui.heading("Tanks");
                let session = &mut self.session;
                let mut context =
                    ViewContext::new(&session.store, &mut self.time, &mut session.invoker);
                self.table.view(ui, &mut context);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let session = &mut self.session;
            let mut context =
                ViewContext::new(&session.store, &mut self.time, &mut session.invoker);
            self.plot.view(ui, &mut context);
        });

        // what the buttons queued goes to the simulator in this frame
        self.session.send_invocations();
    }
}

impl eframe::App for Dashboard {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // the wheel zooms the plot and the timeline under the pointer
        view::zoom_with_wheel(ctx, raw_input);
        time::take_wheel(ctx, raw_input);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update(ctx.input(|input| input.time));
        // the simulator has new samples by then, a socket would wake the app up itself
        ctx.request_repaint_after_secs(PERIOD as f32);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "blip-kit dashboard",
        options,
        Box::new(|cc| Ok(Box::new(Dashboard::new(&cc.egui_ctx)))),
    )
}

// In the browser the dashboard runs on the canvas `the_canvas_id` of its page, as
// blip does, see `blip/index.html` and `blip/src/main.rs`.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("the_canvas_id"))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("a canvas with the id the_canvas_id");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(Dashboard::new(&cc.egui_ctx)))),
            )
            .await;
        if let Err(error) = started {
            log::error!("the dashboard did not start: {error:?}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable as _;

    /// The level of tank `tank` as the store has it last.
    fn level(dashboard: &Dashboard, tank: &str) -> f64 {
        let path = format!("tank/{tank}/level");
        let series = (dashboard.session.store.traces.series(&path)).expect("the level");
        series.value(series.len() - 1)
    }

    #[test]
    fn the_dashboard_shows_the_tanks_and_drains_them() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000.0, 600.0))
            .with_step_dt(0.1)
            .build_ui_state(
                |ui, dashboard: &mut Option<Dashboard>| {
                    let dashboard = dashboard.get_or_insert_with(|| Dashboard::new(ui.ctx()));
                    dashboard.update(ui.input(|input| input.time));
                    dashboard.show(ui);
                },
                None,
            );
        harness.run_steps(20);

        // the simulator of the application fed the store, with no socket
        let dashboard = harness.state().as_ref().expect("the dashboard");
        assert!(dashboard.session.open_sources().contains(SIMULATOR));
        assert!(level(dashboard, "a") > 1.0);
        assert!(level(dashboard, "b") > level(dashboard, "a"));
        assert_eq!(
            dashboard.session.store.traces.unit("tank/a/level"),
            Some("l")
        );

        // a button of the application in the table invokes the action of its row
        let buttons: Vec<_> = harness.query_all_by_label("Drain").collect();
        // the title of the column and a button for each tank
        assert_eq!(buttons.len(), 1 + TANKS.len());
        let before = level(harness.state().as_ref().unwrap(), "b");
        buttons.last().expect("the button of tank b").click();
        harness.run_steps(3);
        let dashboard = harness.state().as_ref().expect("the dashboard");
        assert!(level(dashboard, "b") < before);
        assert!(level(dashboard, "a") > 1.0);
    }
}
