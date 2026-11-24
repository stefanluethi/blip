//! Measures what the store and the plot cost, as a baseline for changes to how the
//! widgets read the store: how many samples a second the store takes in, and how long a
//! frame of a plot of a long trace takes. Ignored by default, run it with
//!
//! ```sh
//! cargo test --release -p blip-kit --test baseline -- --ignored --nocapture
//! ```
//!
//! The numbers depend on the machine: compare runs of the same machine only.

use blip_kit::store::MemoryStore;
use blip_kit::store::actions::Invoker;
use blip_kit::widgets::time::TimeRange;
use blip_kit::widgets::view::plot::Plot;
use blip_kit::widgets::view::{ViewContext, Viewable as _};
use std::time::Instant;

/// The frame of the float sample `i` of the trace `a`, the samples 1 ms apart.
fn frame(i: u64) -> Vec<u8> {
    let sample = proto::Message::Sample(proto::Sample {
        timestamp: i * 1_000_000,
        path: "a".to_owned(),
        value: proto::Data::Float((i % 1000) as f64),
    });
    serde_cbor_2::to_vec(&sample).expect("a frame")
}

/// A model that received `count` samples of the trace `a`.
fn model_of(count: u64) -> MemoryStore {
    let mut model = MemoryStore::default();
    for i in 0..count {
        model.receive("target", &frame(i));
    }
    model
}

#[test]
#[ignore = "a measurement, run on its own"]
fn ingest_rate() {
    const COUNT: u64 = 2_000_000;
    let frames: Vec<_> = (0..COUNT).map(frame).collect();
    let mut model = MemoryStore::default();
    let start = Instant::now();
    for frame in &frames {
        model.receive("target", frame);
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(model.has_samples());
    println!(
        "baseline ingest: {COUNT} samples in {seconds:.3} s, {:.2} million samples/s",
        COUNT as f64 / seconds / 1.0e6
    );
}

struct State {
    plot: Plot,
    model: MemoryStore,
    time: TimeRange,
    invoker: Invoker,
}

#[test]
#[ignore = "a measurement, run on its own"]
fn plot_frame_time() {
    const COUNT: u64 = 10_000_000;
    const FRAMES: u32 = 200;
    let mut plot = Plot::default();
    if let Some((traces, _)) = plot.selection() {
        traces.insert("a".to_owned());
    }
    let mut state = State {
        plot,
        model: model_of(COUNT),
        time: TimeRange::default(),
        invoker: Invoker::default(),
    };
    // show all of the trace, the most a plot reads
    state.time.update(&state.model, 0.0);
    state.time.fit(&state.model);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1200.0, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let mut context =
                    ViewContext::new(&state.model, &mut state.time, &mut state.invoker);
                state.plot.view(ui, &mut context);
            },
            state,
        );
    // the first frames lay out and fill caches
    for _ in 0..5 {
        harness.step();
    }
    let start = Instant::now();
    for _ in 0..FRAMES {
        harness.step();
    }
    let millis = start.elapsed().as_secs_f64() * 1.0e3 / f64::from(FRAMES);
    println!("baseline plot: {millis:.3} ms a frame of {COUNT} samples, 1200 points wide");
}
