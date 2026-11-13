//! Renders the screenshots of the documentation, from a fixed recording and preset, in
//! the light and the dark theme, with wgpu:
//!
//! ```text
//! cd doc && make screenshots
//! ```
//!
//! which runs `TZ=UTC cargo test -p blip --release --features screenshots -- --ignored
//! screenshots` (a release build, so that the menu bar does not warn of a debug build). The PNGs go to `doc/source/screenshots`, `name.png` in the light theme
//! and `name-dark.png` in the dark one. `BLIP_SCREENSHOTS_DIR` (`make screenshots
//! SCREENSHOTS_DIR=…`) writes them to another directory instead, e.g. to compare them
//! with the committed ones. The times are shown in UTC, so they do not
//! depend on the time zone of the machine.
//!
//! The recording `tests/data/demo.blip` is 30 s of the outpost's simulator while a
//! script invoked some of its actions, the preset `tests/data/demo-presets.json`
//! shows it; both can also be opened in blip. The window is rendered connected to a
//! WebSocket server in the test that sends the recording, on port 3000 where blip
//! looks for an outpost by default (or another port while an outpost runs there); the
//! views and panels are rendered on their own.

use super::view::{NO_PLOTS, View, ViewContext, Viewable};
use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const RECORDING: &[u8] = include_bytes!("../../tests/data/demo.blip");
const PRESETS: &[u8] = include_bytes!("../../tests/data/demo-presets.json");

/// Pixels per point of the screenshots, for sharp images on high density screens.
const PIXELS_PER_POINT: f32 = 2.0;
/// The size of the window in points.
const WINDOW: egui::Vec2 = egui::vec2(1440.0, 900.0);
/// The size of a view on its own in points.
const VIEW: egui::Vec2 = egui::vec2(720.0, 380.0);
/// The messages per second the menu bar shows, about those of the simulator.
const RATE: f64 = 312.0;

/// `doc/source/screenshots`, or the directory in `BLIP_SCREENSHOTS_DIR`.
fn output_dir() -> PathBuf {
    std::env::var_os("BLIP_SCREENSHOTS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../doc/source/screenshots")
        })
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

/// The demo model with what targets say about two traces: the unit, description and
/// expected range of the motor speed, and the description and labels of a mode trace,
/// which has a sample every 2 s.
fn metadata_model() -> model::Model {
    let mut model = demo_model();
    let mut send = |message: proto::Message| {
        model.receive("", &serde_cbor_2::to_vec(&message).expect("a message"));
    };
    send(proto::Message::TraceMetadata(proto::TraceMetadata {
        path: "motor.speed_rpm".to_owned(),
        unit: "rpm".to_owned(),
        description: "Speed of the motor shaft, measured by the encoder".to_owned(),
        range: Some(proto::ExpectedRange {
            min: 0.0,
            max: 3000.0,
        }),
        ..Default::default()
    }));
    send(proto::Message::TraceMetadata(proto::TraceMetadata {
        path: "motor/mode".to_owned(),
        description: "Operating mode of the motor controller".to_owned(),
        labels: [(0, "Idle"), (1, "Ramp"), (2, "Run"), (3, "Fault")]
            .into_iter()
            .map(|(number, label)| (number, label.to_owned()))
            .collect(),
        ..Default::default()
    }));
    let (oldest, newest) = model.time_span().expect("the recording has data");
    let period = 2_000_000_000;
    let values = (0..=(newest - oldest) / period)
        .map(|i| proto::Data::Uint([0, 1, 2, 2, 2, 2, 1, 0, 3, 0][i as usize % 10]))
        .collect();
    model.receive(
        "",
        &serde_cbor_2::to_vec(&proto::Message::Samples(proto::Samples {
            path: "motor/mode".to_owned(),
            values,
            times: proto::SampleTimes::Period {
                timestamp: oldest,
                period,
            },
        }))
        .expect("a block"),
    );
    model
}

/// The metadata model with a long description of the mode trace, which wraps in a
/// tooltip.
fn long_description_model() -> model::Model {
    let mut model = metadata_model();
    model.receive(
        "",
        &serde_cbor_2::to_vec(&proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "motor/mode".to_owned(),
            description: "Operating mode of the motor controller: it idles until enabled, \
                          ramps the speed up to the setpoint, runs there, and drops into \
                          fault when the current limit trips"
                .to_owned(),
            labels: [(0, "Idle"), (1, "Ramp"), (2, "Run"), (3, "Fault")]
                .into_iter()
                .map(|(number, label)| (number, label.to_owned()))
                .collect(),
            ..Default::default()
        }))
        .expect("a message"),
    );
    model
}

fn demo_layout() -> layout::Layout {
    let mut presets = presets::read("demo-presets.json", PRESETS).expect("the demo presets read");
    layout::Layout::from_saved(presets.remove(0).layout)
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
    app.connections.set_saved(vec![connections::Connection {
        name: "outpost".to_owned(),
        address,
    }]);
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
    view_shot_of(name, demo_model(), json, None);
}

/// Renders the view of `json` on its own as [`view_shot`] does, with the last 20 s of
/// `model`, and the pointer at `hover` if given.
fn view_shot_of(name: &str, model: model::Model, json: &str, hover: Option<egui::Pos2>) {
    view_shot_with(name, model, json, |harness| {
        if let Some(at) = hover {
            harness.hover_at(at);
            // long enough for the tooltip to open
            harness.run_steps(40);
        }
    });
}

/// Renders the view as [`view_shot_of`] does, after `act` acted on the harness.
fn view_shot_with(
    name: &str,
    model: model::Model,
    json: &str,
    act: impl FnOnce(&mut Harness<'_, ViewShot>),
) {
    let mut time = time::TimeRange::default();
    time.fit(&model);
    let latest = time.latest().expect("the recording has data");
    time.pan_zoom(latest - 20.0, latest);
    time.set_cursor(latest - 6.0);
    let mut invoker = model::actions::Invoker::default();
    // the actions of the recording, which has no connection names, are connected
    invoker.connected.insert(String::new());
    invoker.recording = true;
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
                let tokens = ui.tokens();
                // the canvas behind the card, as in the app
                paint_background(ui, tokens.canvas);
                egui::Frame::NONE
                    .fill(tokens.card)
                    .stroke(egui::Stroke::new(1.0, tokens.border))
                    .corner_radius(4)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        let mut context =
                            ViewContext::new(&shot.model, &mut shot.time, &mut shot.invoker);
                        shot.view.view(ui, &mut context);
                    });
            },
            shot,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    act(&mut harness);
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
            r#"{"name": "Robot", "kind": {"Xy": {"traces": ["robot/pose.x", "robot/pose.y"], "xs": ["robot/pose.x"], "against": {"robot/pose.y": "robot/pose.x"}, "equal_axes": true}}}"#,
        ),
        (
            "view-distribution",
            r#"{"name": "Pose spread", "kind": {"Distribution": {"traces": ["robot/pose.x", "robot/pose.y"], "markers": "MeanSigma"}}}"#,
        ),
        (
            "view-distribution-boxes",
            r#"{"name": "Pose spread", "kind": {"Distribution": {"traces": ["robot/pose.x", "robot/pose.y", "robot/pose.heading"], "show": "Boxes"}}}"#,
        ),
        (
            "view-distribution-both",
            r#"{"name": "Pose spread", "kind": {"Distribution": {"traces": ["robot/pose.x", "robot/pose.y"], "show": "Both"}}}"#,
        ),
        (
            "view-plot-empty",
            r#"{"name": "Pose", "kind": {"Plot": {"traces": []}}}"#,
        ),
        (
            "view-table-empty",
            r#"{"name": "Values", "kind": {"Table": {"traces": []}}}"#,
        ),
        (
            "view-heatmap-grid",
            r#"{"name": "Thermal", "kind": {"Heatmap": {"traces": ["thermal/frame"]}}}"#,
        ),
        (
            "view-logs",
            r#"{"name": "Log", "kind": {"Logs": {}}, "wildcards": [""]}"#,
        ),
        (
            "view-actions",
            r#"{"name": "Actions", "kind": {"Actions": {"shown": ["motor/enable", "motor/speed", "thermal/ambient", "log/level", "log/message", "imu/calibrate"]}}}"#,
        ),
    ];
    for (name, json) in views {
        view_shot(name, json);
    }
    // bands that write the labels a target declares
    view_shot_of(
        "view-states-labels",
        metadata_model(),
        r#"{"name": "Motor", "kind": {"States": {"traces": ["motor.state", "motor/mode"]}}}"#,
        None,
    );
    // the tooltip over the lower band row: the label, the number faint, and the
    // description of the trace
    view_shot_of(
        "view-states-labels-tooltip",
        metadata_model(),
        r#"{"name": "Motor", "kind": {"States": {"traces": ["motor.state", "motor/mode"]}}}"#,
        Some(egui::pos2(360.0, 270.0)),
    );

    // the plot of a labelled trace: the tooltip writes the label, the number faint, and
    // the description wrapped
    view_shot_of(
        "view-plot-labels-tooltip",
        long_description_model(),
        r#"{"name": "Motor", "kind": {"Plot": {"traces": ["motor.speed_rpm", "motor/mode"]}}}"#,
        Some(egui::pos2(345.0, 345.0)),
    );
    // label cells in the table
    view_shot_of(
        "view-table-labels",
        metadata_model(),
        r#"{"name": "Values", "kind": {"Table": {"traces": ["motor.speed_rpm", "motor.state", "motor/mode"]}}}"#,
        None,
    );

    // boxes of loop timings at their edge cases: many outliers, a long name, no
    // samples in the window, all samples equal, and a fixed value range that one box
    // lies beyond and one whisker is cut at
    view_shot_of(
        "view-distribution-boxes-edges",
        timing_model(),
        r#"{"name": "Loop timing", "kind": {"Distribution": {"traces": ["loop/period", "motor/current_controller_integrator_feedback_period_measured", "sensor/sample_period", "timer/fixed_tick", "comms/poll_period", "adc/conversion"], "show": "Boxes", "range": [900.0, 1100.0]}}}"#,
        None,
    );
    // the tooltip of the middle one of three boxes, under the pointer
    view_shot_of(
        "view-distribution-box-tooltip",
        timing_model(),
        r#"{"name": "Loop timing", "kind": {"Distribution": {"traces": ["adc/conversion", "loop/period", "motor/current_controller_integrator_feedback_period_measured"], "show": "Boxes"}}}"#,
        Some(VIEW.to_pos2() * 0.5),
    );
    // the tooltip of a plot and of a heatmap cell, under the pointer
    view_shot_of(
        "view-plot-tooltip",
        demo_model(),
        r#"{"name": "Pose", "kind": {"Plot": {"traces": ["robot/pose.x", "robot/pose.y", "robot/pose.heading"]}}}"#,
        Some(VIEW.to_pos2() * 0.5),
    );
    view_shot_of(
        "view-heatmap-grid-tooltip",
        demo_model(),
        r#"{"name": "Thermal", "kind": {"Heatmap": {"traces": ["thermal/frame"]}}}"#,
        Some(VIEW.to_pos2() * 0.5),
    );
    // the filter of the path column, open, with records at several paths
    let mut model = demo_model();
    let (_, newest) = model.time_span().expect("the recording has data");
    for (seconds, path, level, message) in [
        (9, "log/imu", 3, "accelerometer saturated on z"),
        (7, "log/motor/left", 2, "speed limit reached"),
        (4, "log/motor/right", 4, "overcurrent 4.2 A"),
    ] {
        let record = proto::Message::Log(proto::LogRecord {
            timestamp: newest - seconds * 1_000_000_000,
            path: path.to_owned(),
            level,
            message: message.to_owned(),
        });
        model.receive("", &serde_cbor_2::to_vec(&record).expect("a record"));
    }
    view_shot_with(
        "view-logs-path-filter",
        model,
        r#"{"name": "Log", "kind": {"Logs": {}}, "wildcards": [""]}"#,
        |harness| {
            let header = format!("Path {}", egui_phosphor::regular::CARET_DOWN);
            harness.get_by_label(&header).click();
            harness.run_steps(4);
        },
    );
}

/// A model of 30 s of loop timings in µs at 100 Hz, made up for the edge cases of the
/// boxes, see [`screenshots_views`].
fn timing_model() -> model::Model {
    // a fixed sequence of pseudo random numbers in [0, 1), xorshift
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    // about normal around 0 with a spread of about `sigma`
    let mut normal = move |sigma: f64| (random() + random() + random() - 1.5) * 2.0 * sigma;
    let mut model = model::Model::default();
    let append = |model: &mut model::Model, path: &str, seconds: f64, value: f64| {
        model.traces.append(&proto::Sample {
            timestamp: (seconds * 1.0e9).round() as u64,
            path: path.to_owned(),
            value: proto::Data::Float(value),
        });
    };
    for index in 0..3000 {
        let seconds = f64::from(index) * 0.01;
        // jitter with a spike in every tenth sample, early or late
        let spike = if index % 10 == 3 {
            let size = 35.0 + 55.0 * (f64::from(index % 70) / 70.0);
            if index % 20 == 3 { size } else { -size }
        } else {
            0.0
        };
        append(
            &mut model,
            "loop/period",
            seconds,
            1003.7 + normal(4.0) + spike,
        );
        append(
            &mut model,
            "motor/current_controller_integrator_feedback_period_measured",
            seconds,
            1024.3 + normal(12.0),
        );
        // only in the first 5 s, before the shown window
        if seconds < 5.0 {
            append(
                &mut model,
                "sensor/sample_period",
                seconds,
                997.0 + normal(3.0),
            );
        }
        append(&mut model, "timer/fixed_tick", seconds, 987.0);
        // beyond the value range
        append(
            &mut model,
            "comms/poll_period",
            seconds,
            1183.0 + normal(9.0),
        );
        // the lower whisker below the value range
        append(&mut model, "adc/conversion", seconds, 945.6 + normal(30.0));
    }
    for path in [
        "loop/period",
        "motor/current_controller_integrator_feedback_period_measured",
        "sensor/sample_period",
        "timer/fixed_tick",
        "comms/poll_period",
        "adc/conversion",
    ] {
        model.receive(
            "",
            &serde_cbor_2::to_vec(&proto::Message::TraceMetadata(proto::TraceMetadata {
                path: path.to_owned(),
                unit: "µs".to_owned(),
                ..Default::default()
            }))
            .expect("metadata"),
        );
    }
    model
}

/// What a layout is shown with.
struct LayoutShot {
    model: model::Model,
    time: time::TimeRange,
    invoker: model::actions::Invoker,
    layout: layout::Layout,
}

/// Renders the layout of `json`, the saved form of a layout, `size` large, with the
/// last 20 s of the recording and the cursor 6 s before its end.
fn layout_shot(name: &str, size: egui::Vec2, json: &str) {
    let model = demo_model();
    let mut time = time::TimeRange::default();
    time.fit(&model);
    let latest = time.latest().expect("the recording has data");
    time.pan_zoom(latest - 20.0, latest);
    time.set_cursor(latest - 6.0);
    let saved = serde_json::from_str(json).expect("a layout");
    let shot = LayoutShot {
        model,
        time,
        invoker: model::actions::Invoker::default(),
        layout: layout::Layout::from_saved(saved),
    };
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            |ui, shot: &mut LayoutShot| {
                let now = ui.input(|input| input.time);
                shot.time.update(&shot.model, now);
                paint_background(ui, ui.tokens().canvas);
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        top: 2,
                        ..egui::Margin::same(theme::GAP as i8)
                    })
                    .show(ui, |ui| {
                        let mut context =
                            ViewContext::new(&shot.model, &mut shot.time, &mut shot.invoker);
                        shot.layout.ui(ui, &mut context);
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
fn screenshots_layouts() {
    // a sideways distribution linked to the plot beside it, view 0, showing its traces,
    // as bars and as boxes
    for (name, show) in [
        ("view-distribution-sideways", "Bars"),
        ("view-distribution-boxes-sideways", "Boxes"),
    ] {
        layout_shot(name, egui::vec2(1000.0, 380.0), &linked_distribution(show));
    }
}

/// The saved form of a layout of the plot "Motor speed" and the sideways distribution
/// "Speed spread" linked to it, showing `show`.
fn linked_distribution(show: &str) -> String {
    format!(
        r#"{{"name": "", "kind": "Tabs", "active": 0, "children": [{{"Group": {{
        "name": "Motor", "kind": "Horizontal", "shares": [1.8, 1.0], "children": [
            {{"View": {{"name": "Motor speed", "kind": {{"Plot": {{"traces": ["motor.speed_rpm"]}}}}}}}},
            {{"View": {{"name": "Speed spread", "kind": {{"Distribution": {{"show": "{show}", "orientation": "Sideways", "link": 0}}}}}}}}
        ]}}}}]}}"#
    )
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
                paint_background(ui, ui.tokens().card);
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
                paint_background(ui, ui.tokens().card);
                state.bottom_ui(ui, true);
            },
            state,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "panel-time");

    // the properties of a sideways distribution showing boxes, linked to a plot
    let mut layout = layout::Layout::from_saved(
        serde_json::from_str(&linked_distribution("Boxes")).expect("a layout"),
    );
    layout.select_named("Speed spread");
    let model = demo_model();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 560.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            move |ui, layout: &mut layout::Layout| {
                paint_background(ui, ui.tokens().card);
                let connected = Default::default();
                let context = view::PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &NO_PLOTS,
                };
                layout.properties_ui(ui, &context);
            },
            layout,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "panel-properties-distribution");

    // the properties of a plot of traces the target describes
    let mut layout = layout::Layout::from_saved(
        serde_json::from_str(
            r#"{"name": "", "kind": "Tabs", "active": 0, "children": [{"View": {"name": "Motor", "kind": {"Plot": {"traces": ["motor.speed_rpm", "motor/mode"]}}}}]}"#,
        )
        .expect("a layout"),
    );
    layout.select_named("Motor");
    let model = metadata_model();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 900.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            move |ui, layout: &mut layout::Layout| {
                paint_background(ui, ui.tokens().card);
                let connected = Default::default();
                let context = view::PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &NO_PLOTS,
                };
                layout.properties_ui(ui, &context);
            },
            layout,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "panel-properties-traces");

    // the properties of a records table, with the facts a target declares under a column
    let mut layout = layout::Layout::from_saved(
        serde_json::from_str(
            r#"{"name": "", "kind": "Tabs", "active": 0, "children": [{"View": {"name": "Motor", "kind": {"Table": {"layout": "Records", "traces": ["motor"]}}}}]}"#,
        )
        .expect("a layout"),
    );
    layout.select_named("Motor");
    let model = metadata_model();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 420.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            move |ui, layout: &mut layout::Layout| {
                paint_background(ui, ui.tokens().card);
                let connected = Default::default();
                let context = view::PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &NO_PLOTS,
                };
                layout.properties_ui(ui, &context);
            },
            layout,
        );
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "panel-properties-records-facts");

    // the status chip while a connection speaks a newer protocol
    let open: HashSet<String> = ["lab".to_owned()].into();
    let newer = vec!["lab: protocol 2 is newer than blip".to_owned()];
    let mut harness = Harness::builder()
        .with_size(egui::vec2(260.0, 36.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui(move |ui| {
            paint_background(ui, ui.tokens().canvas);
            ui.horizontal_centered(|ui| {
                status_chip(ui, &open, &newer, Some(RATE));
            });
        });
    theme::install(&harness.ctx);
    harness.run_steps(4);
    save_both(&mut harness, "chip-connections-newer");
}

/// Serves one `hello` to the first client on `port`. Returns the address. The port is
/// fixed so that the screenshot shows the same address on every render.
fn serve_hello(port: u16, hello: proto::Hello) -> String {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("the port is free");
    let message = serde_cbor_2::to_vec(&proto::Message::Hello(hello)).expect("a Hello");
    let (go, wait) = mpsc::channel();
    go.send(()).expect("the server waits");
    std::thread::spawn(move || serve(listener, vec![message], wait));
    format!("ws://127.0.0.1:{port}/ws/outpost")
}

#[test]
#[ignore = "renders the screenshots of the documentation, see the module"]
fn screenshots_connections() {
    let bench = serve_hello(
        47101,
        proto::Hello {
            version: 1,
            name: Some("rig-7".to_owned()),
            clock: Some(proto::Clock::Boot),
            sender: Some("sonde 1.2".to_owned()),
        },
    );
    let lab = serve_hello(
        47102,
        proto::Hello {
            version: 2,
            name: Some("lab-pc".to_owned()),
            clock: Some(proto::Clock::Epoch),
            sender: Some("blip-outpost 0.4".to_owned()),
        },
    );
    let mut connections = connections::Connections::default();
    connections.set_saved(
        [
            ("bench", bench),
            ("lab", lab),
            ("spare", "ws://127.0.0.1:9/spare".to_owned()),
        ]
        .into_iter()
        .map(|(name, address)| connections::Connection {
            name: name.to_owned(),
            address,
        })
        .collect(),
    );
    let mut harness = Harness::builder()
        .with_size(egui::vec2(330.0, 260.0))
        .with_pixels_per_point(PIXELS_PER_POINT)
        .wgpu()
        .build_ui_state(
            |ui, connections: &mut connections::Connections| {
                paint_background(ui, ui.tokens().canvas);
                connections.receive(|_| {});
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(300.0);
                    connections.ui(ui, false);
                });
            },
            connections,
        );
    theme::install(&harness.ctx);
    harness.run_steps(2);
    harness.get_by_label_contains("bench").click();
    harness.run_steps(2);
    harness.get_by_label_contains("lab").click();
    harness.event(egui::Event::PointerGone);
    step_until(&mut harness, "the Hellos", |connections| {
        connections.newer().len() == 1 && connections.is_connected()
    });
    harness.run_steps(4);
    save_both(&mut harness, "popup-connections");
}
