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
use blip_kit::store::session::Session;
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
    let mut session = Session::recording();
    for i in 0..count {
        session.inbox("target", 0.0).frame(&frame(i));
    }
    session.store
}

#[test]
#[ignore = "a measurement, run on its own"]
fn ingest_rate() {
    const COUNT: u64 = 2_000_000;
    let frames: Vec<_> = (0..COUNT).map(frame).collect();
    let mut session = Session::recording();
    let start = Instant::now();
    for frame in &frames {
        session.inbox("target", 0.0).frame(frame);
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(session.store.has_samples());
    println!(
        "baseline ingest: {COUNT} samples in {seconds:.3} s, {:.2} million samples/s",
        COUNT as f64 / seconds / 1.0e6
    );
}

#[test]
#[ignore = "a measurement, run on its own"]
fn ingest_typed() {
    const COUNT: u64 = 2_000_000;
    let mut session = Session::recording();
    let start = Instant::now();
    {
        let mut inbox = session.inbox("target", 0.0);
        for i in 0..COUNT {
            inbox.sample(i * 1_000_000, "a", (i % 1000) as f64);
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(session.store.has_samples());
    println!(
        "baseline ingest typed: {COUNT} samples in {seconds:.3} s, {:.2} million samples/s",
        COUNT as f64 / seconds / 1.0e6
    );
}

#[test]
#[ignore = "a measurement, run on its own"]
fn ingest_unrecorded() {
    const COUNT: u64 = 2_000_000;
    let frames: Vec<_> = (0..COUNT).map(frame).collect();
    let mut session = Session::default();
    let start = Instant::now();
    for frame in &frames {
        session.inbox("target", 0.0).frame(frame);
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(!session.store.has_samples());
    println!(
        "baseline ingest unrecorded: {COUNT} samples in {seconds:.3} s, {:.2} million samples/s",
        COUNT as f64 / seconds / 1.0e6
    );
}

/// The frame of the block `i` of 100 float samples of the trace `adc`, 1 ms apart.
fn floats_block(i: u64) -> Vec<u8> {
    let block = proto::Message::Samples(proto::Samples {
        path: "adc".to_owned(),
        values: (0..100)
            .map(|k| proto::Data::Float(((i * 100 + k) % 1000) as f64))
            .collect(),
        times: proto::SampleTimes::Period {
            timestamp: i * 100_000_000,
            period: 1_000_000,
        },
    });
    serde_cbor_2::to_vec(&block).expect("a frame")
}

/// The frame of the block `i` of 100 record samples of the trace `imu`, 1 ms apart.
fn records_block(i: u64) -> Vec<u8> {
    let block = proto::Message::Samples(proto::Samples {
        path: "imu".to_owned(),
        values: (0..100)
            .map(|k| {
                proto::Data::Record(vec![
                    (
                        "t".to_owned(),
                        proto::Data::Float(((i * 100 + k) % 1000) as f64 * 0.5),
                    ),
                    (
                        "acc".to_owned(),
                        proto::Data::Array(vec![
                            proto::Data::Float(0.1),
                            proto::Data::Float(0.2),
                            proto::Data::Float(9.8),
                        ]),
                    ),
                    ("mode".to_owned(), proto::Data::String("run".to_owned())),
                ])
            })
            .collect(),
        times: proto::SampleTimes::Period {
            timestamp: i * 100_000_000,
            period: 1_000_000,
        },
    });
    serde_cbor_2::to_vec(&block).expect("a frame")
}

/// Times how long a fresh store takes to receive `frames` of `samples` samples, and
/// prints it as `what`.
fn ingest_blocks_of(what: &str, frames: &[Vec<u8>], samples: u64) {
    let mut session = Session::recording();
    let start = Instant::now();
    for frame in frames {
        session.inbox("target", 0.0).frame(frame);
    }
    let seconds = start.elapsed().as_secs_f64();
    assert!(session.store.has_samples());
    println!(
        "baseline ingest blocks of {what}: {samples} samples in {seconds:.3} s, {:.2} million samples/s",
        samples as f64 / seconds / 1.0e6
    );
}

#[test]
#[ignore = "a measurement, run on its own"]
fn ingest_blocks() {
    let floats: Vec<_> = (0..20_000).map(floats_block).collect();
    ingest_blocks_of("floats", &floats, 2_000_000);
    let records: Vec<_> = (0..10_000).map(records_block).collect();
    ingest_blocks_of("records", &records, 1_000_000);
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
