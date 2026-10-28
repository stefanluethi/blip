//! Renders the screenshots of the documentation, from a fixed recording and layout, in
//! the light and the dark theme, with wgpu:
//!
//! ```text
//! cd doc && make screenshots
//! ```
//!
//! which runs `TZ=UTC cargo test -p blip --release --features screenshots -- --ignored
//! screenshots` (a release build, so that the menu bar does not warn of a debug build). The PNGs go to `doc/source/screenshots`, `name.png` in the light theme
//! and `name-dark.png` in the dark one. The times are shown in UTC, so they do not
//! depend on the time zone of the machine.
//!
//! The recording `tests/data/demo.blip` is 30 s of the outpost's simulator while a
//! script invoked some of its actions, the layout `tests/data/demo-layout.json` shows
//! it; both can also be opened in blip. The window is rendered connected to a
//! WebSocket server in the test that sends the recording, on port 3000 where blip
//! looks for an outpost by default (or another port while an outpost runs there); the
//! views and panels are rendered on their own.

use super::view::{NO_FORMATS, View, ViewContext, Viewable};
use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const RECORDING: &[u8] = include_bytes!("../../tests/data/demo.blip");
const LAYOUT: &str = include_str!("../../tests/data/demo-layout.json");

/// Pixels per point of the screenshots, for sharp images on high density screens.
const PIXELS_PER_POINT: f32 = 2.0;
/// The size of the window in points.
const WINDOW: egui::Vec2 = egui::vec2(1440.0, 900.0);
/// The size of a view on its own in points.
const VIEW: egui::Vec2 = egui::vec2(720.0, 380.0);
/// The messages per second the menu bar shows, about those of the simulator.
const RATE: f64 = 312.0;

fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../doc/source/screenshots")
}

/// The messages of the recording, one after another.
fn messages() -> Vec<Vec<u8>> {
    let mut messages = vec![];
    model::recording::read_file(RECORDING, |_, message| messages.push(message.to_vec()))
        .expect("the demo recording can be read");
    messages
}

/// A model that received the recording, while recording.
fn demo_model() -> model::Model {
    let mut model = model::Model::default();
    for message in messages() {
        model.receive("", &message);
    }
    model
}

fn demo_layout() -> layout::Layout {
    let file = serde_json::from_str(LAYOUT).expect("the demo layout is a layout");
    layout::Layout::migrate(file).expect("the demo layout has a known version")
}

/// Renders the harness in the light and the dark theme, to `name.png` and
/// `name-dark.png`.
fn save_both<State>(harness: &mut Harness<'_, State>, name: &str) {
    let dir = output_dir();
    std::fs::create_dir_all(&dir).expect("the screenshots directory can be created");
    for (theme, suffix) in [(egui::Theme::Light, ""), (egui::Theme::Dark, "-dark")] {
        harness.ctx.set_theme(theme);
        harness.run_steps(4);
        let image = harness
            .render()
            .expect("rendering with wgpu works, see doc/README.md");
        let path = dir.join(format!("{name}{suffix}.png"));
        // opaque, so without the alpha channel, which makes the files smaller
        let image = image::DynamicImage::ImageRgba8(image).into_rgb8();
        image.save(&path).expect("the screenshot can be saved");
        println!("wrote {}", path.display());
    }
}

/// Paints the whole screen in `color`, also where the frame of a harness leaves it
/// unpainted.
fn paint_background(ui: &egui::Ui, color: egui::Color32) {
    let screen = ui.ctx().content_rect();
    (ui.ctx().layer_painter(egui::LayerId::background())).rect_filled(screen, 0.0, color);
}

/// Steps the harness until `done` or panics after a few seconds.
fn step_until<State>(harness: &mut Harness<'_, State>, what: &str, done: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(harness.state()) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        harness.step();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Serves one client on `listener`: once `go` arrives, sends it all `messages` in
/// batches, as an outpost passing on a target's batches would, then reads what it
/// sends until it disconnects.
fn serve(listener: TcpListener, messages: Vec<Vec<u8>>, go: mpsc::Receiver<()>) {
    let (stream, _) = listener.accept().expect("blip connects");
    let mut socket = tungstenite::accept(stream).expect("a WebSocket handshake");
    go.recv().ok();
    for batch in messages.chunks(100) {
        if socket
            .send(tungstenite::Message::Binary(batch.concat()))
            .is_err()
        {
            return;
        }
    }
    while socket.read().is_ok() {}
}

#[test]
#[ignore = "renders the screenshots of the documentation, see the module"]
fn screenshots_window() {
    // where blip looks for an outpost by default, or any free port if an outpost is
    // running there, which then shows in the address
    let listener = TcpListener::bind("127.0.0.1:3000")
        .or_else(|_| TcpListener::bind("127.0.0.1:0"))
        .expect("a port to serve the recording on");
    let address = format!(
        "ws://127.0.0.1:{}/ws/outpost",
        listener.local_addr().expect("a bound port").port()
    );
    let messages = messages();
    let total = messages.len();
    let (go, wait) = mpsc::channel();
    std::thread::spawn(move || serve(listener, messages, wait));

    let mut harness = Harness::builder()
        .with_size(WINDOW)
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_eframe(|cc| App::new(cc));
    let app = harness.state_mut();
    app.layout = demo_layout();
    app.connections = serde_json::from_value(serde_json::json!({
        "connections": [{"name": "outpost", "address": address}]
    }))
    .expect("connections");
    harness.run_steps(2);

    // connect as the user does, from the list of the connection chip, then record
    harness.get_by_label("Connections").click();
    harness.run_steps(2);
    harness.get_by_label_contains("ws://127.0.0.1:").click();
    harness.run_steps(2);
    harness.key_press(egui::Key::Escape);
    // no pointer left over the panel in the screenshots
    harness.event(egui::Event::PointerGone);
    step_until(&mut harness, "the connection", |app| {
        app.connections.is_connected()
    });
    harness.state_mut().state.set_recording(true, true);
    go.send(()).expect("the server waits");
    step_until(&mut harness, "the recording", |app| {
        app.state.model.recording.messages() >= total
    });

    let app = harness.state_mut();
    app.layout.select_named("Motor speed");
    // the rate measured in the test says nothing, as the recording arrives at once
    app.rate = MessageRate {
        start: Some(f64::INFINITY),
        count: 0,
        per_second: Some(RATE),
    };
    harness.run_steps(4);
    save_both(&mut harness, "window");

    let app = harness.state_mut();
    app.layout.select_named("Sensors");
    app.layout.select_named("Robot");
    harness.run_steps(4);
    save_both(&mut harness, "window-sensors");
}

/// What a view on its own is shown with.
struct ViewShot {
    model: model::Model,
    time: time::TimeRange,
    invoker: model::actions::Invoker,
    view: View,
}

/// Renders the view of `json`, the saved form of a view, on its own, with the last
/// 20 s of the recording and the cursor 6 s before its end.
fn view_shot(name: &str, json: &str) {
    let model = demo_model();
    let mut time = time::TimeRange::default();
    time.fit(&model);
    let latest = time.latest().expect("the recording has data");
    time.pan_zoom(latest - 20.0, latest);
    time.set_cursor(latest - 6.0);
    let mut invoker = model::actions::Invoker::default();
    // the actions of the recording, which has no connection names, are connected
    invoker.connected.insert(String::new());
    let view = serde_json::from_str(json).expect("a view");
    let shot = ViewShot {
        model,
        time,
        invoker,
        view,
    };

    let mut harness = Harness::builder()
        .with_size(VIEW)
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            |ui, shot: &mut ViewShot| {
                let now = ui.input(|input| input.time);
                shot.time.update(&shot.model, now);
                let tokens = theme::tokens(ui.visuals());
                // the canvas behind the card, as in the app
                paint_background(ui, tokens.canvas);
                egui::Frame::NONE
                    .fill(tokens.card)
                    .stroke(egui::Stroke::new(1.0, tokens.border))
                    .corner_radius(4)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        let mut context = ViewContext {
                            model: &shot.model,
                            time: &mut shot.time,
                            invoker: &mut shot.invoker,
                            formats: &NO_FORMATS,
                            titled: false,
                        };
                        shot.view.view(ui, &mut context);
                    });
            },
            shot,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, name);
}

#[test]
#[ignore = "renders the screenshots of the documentation, see the module"]
fn screenshots_views() {
    let views = [
        (
            "view-plot",
            r#"{"name": "Pose", "kind": {"Plot": {"traces": ["robot/pose.x", "robot/pose.y", "robot/pose.heading"]}}}"#,
        ),
        (
            "view-states",
            r#"{"name": "States", "kind": {"States": {"traces": ["motor.enabled", "motor.state", "simulator/log_level"]}}}"#,
        ),
        (
            "view-table",
            r#"{"name": "Values", "kind": {"Table": {"traces": ["motor.enabled", "motor.speed_rpm", "motor.state", "robot/pose.heading", "robot/pose.x", "robot/pose.y", "sensor/temperature", "imu.acc", "imu.gyro"]}}}"#,
        ),
        (
            "view-xy",
            r#"{"name": "Robot", "kind": {"Xy": {"x": "robot/pose.x", "traces": ["robot/pose.x", "robot/pose.y"], "equal_axes": true}}}"#,
        ),
        (
            "view-heatmap-grid",
            r#"{"name": "Thermal", "kind": {"Heatmap": {"traces": ["thermal/frame"]}}}"#,
        ),
        ("view-logs", r#"{"name": "Log", "kind": {"Logs": {}}}"#),
        (
            "view-actions",
            r#"{"name": "Actions", "kind": {"Actions": {"shown": ["motor/enable", "motor/speed", "thermal/ambient", "log/level", "log/message", "imu/calibrate"]}}}"#,
        ),
    ];
    for (name, json) in views {
        view_shot(name, json);
    }
}

#[test]
#[ignore = "renders the screenshots of the documentation, see the module"]
fn screenshots_panels() {
    // the traces panel, choosing the traces of a plot
    let mut app = App::default();
    app.state.model = demo_model();
    app.layout = demo_layout();
    app.layout.select_named("Acceleration");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 520.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            |ui, app: &mut App| {
                // the panels are drawn on the card colour
                paint_background(ui, theme::tokens(ui.visuals()).card);
                app.traces_panel_ui(ui);
            },
            app,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "panel-traces");

    // the bottom panel while recording, live
    let mut state = State {
        model: demo_model(),
        recording: true,
        ..State::default()
    };
    state.time.update(&state.model, 0.0);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 76.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            |ui, state: &mut State| {
                let now = ui.input(|input| input.time);
                state.time.update(&state.model, now);
                paint_background(ui, theme::tokens(ui.visuals()).card);
                state.bottom_ui(ui, true);
            },
            state,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "panel-time");
}
