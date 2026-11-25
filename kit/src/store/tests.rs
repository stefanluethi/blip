use super::*;
use crate::store::actions::{ActionId, Outcome};
use crate::store::session::Session;
use crate::store::testing::{receive, receive_unrecorded};

fn frame(message: &proto::Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).unwrap()
}

#[test]
fn paths_are_kept_as_the_outposts_send_them() {
    let value = |path: &str| {
        frame(&proto::Message::Sample(proto::Sample {
            timestamp: 1,
            path: path.to_owned(),
            value: proto::Data::Uint(1),
        }))
    };
    let log = |path: &str| {
        frame(&proto::Message::Log(proto::LogRecord {
            timestamp: 1,
            path: path.to_owned(),
            level: 2,
            message: "hi".to_owned(),
        }))
    };
    let mut session = Session::recording();
    session.inbox("", 0.0).frame(&value("cpu"));
    session.inbox("a", 0.0).frame(&value("cpu"));
    session.inbox("b", 0.0).frame(&value("cpu"));
    session.inbox("b", 0.0).frame(&log("robot/arm"));
    session.inbox("b", 0.0).frame(&log(""));
    session.inbox("b", 0.0).frame(&log("motor"));
    let model = &session.store;
    // the same path from several outposts is one trace
    assert_eq!(model.traces.iter().count(), 1);
    assert_eq!(model.traces.get("cpu").unwrap().timestamps.len(), 3);
    assert_eq!(
        model.logs.get("robot/arm").unwrap().entries[0].message,
        "hi"
    );
    assert_eq!(
        model.logs.paths().collect::<Vec<_>>(),
        ["log/default", "motor", "robot/arm"]
    );
    assert_eq!(session.recorder.messages(), 6);
}

#[test]
fn activity_is_counted_per_bin_with_records_once() {
    let value = |timestamp: u64, path: &str, value: proto::Data| {
        proto::Message::Sample(proto::Sample {
            timestamp,
            path: path.to_owned(),
            value,
        })
    };
    let imu = proto::Data::Record(
        [("x", 1.0), ("y", 2.0)]
            .into_iter()
            .map(|(name, x)| (name.to_owned(), proto::Data::Float(x)))
            .collect(),
    );
    let mut model = MemoryStore::default();
    for t in [0, 10, 19, 20, 39, 40] {
        receive(&mut model, "", &value(t, "cpu", proto::Data::Uint(1)));
    }
    receive(&mut model, "", &value(25, "imu", imu.clone()));
    receive(&mut model, "", &value(35, "imu", imu));
    // of two paths, the first with a record after the bins of the second
    let log = |timestamp, path: &str, level| {
        proto::Message::Log(proto::LogRecord {
            timestamp,
            path: path.to_owned(),
            level,
            message: "hi".to_owned(),
        })
    };
    for (timestamp, path, level) in [(5, "", 2), (30, "", 2), (2, "motor", 3), (15, "motor", 4)] {
        receive(&mut model, "", &log(timestamp, path, level));
    }
    let values =
        |activity: Vec<Activity>| -> Vec<u32> { activity.iter().map(|bin| bin.values).collect() };
    // [0, 20), [20, 40)
    let halves = model.activity(0, 20, 2);
    assert_eq!(values(halves.clone()), [3, 4]);
    assert_eq!(halves[0].logs, [0, 0, 1, 1, 1]);
    assert_eq!(halves[1].logs, [0, 0, 1, 0, 0]);
    let tens = model.activity(0, 10, 2);
    assert_eq!(tens[0].logs, [0, 0, 1, 1, 0]);
    assert_eq!(tens[1].logs, [0, 0, 0, 0, 1]);
    assert_eq!(values(model.activity(0, 10, 4)), [1, 2, 2, 2]);
    assert_eq!(values(model.activity(10, 10, 1)), [2]);
    assert!(model.activity(0, 10, 0).is_empty());

    // a record without a field of the first is dropped and does not count
    let partial = proto::Data::Record(
        [("y".to_owned(), proto::Data::Float(3.0))]
            .into_iter()
            .collect(),
    );
    receive(&mut model, "", &value(45, "imu", partial));
    assert_eq!(values(model.activity(40, 10, 1)), [1]);
}

#[test]
fn actions_are_kept_with_their_source_and_answers_returned() {
    let actions = proto::Message::Actions(vec![proto::ActionDescriptor {
        path: "motor/speed".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some("motor/rpm".to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]);
    let result = proto::ActionResult {
        id: 3,
        result: Ok(()),
    };
    let mut model = MemoryStore::default();
    assert_eq!(receive(&mut model, "b", &actions), None);
    let speed = model.actions.announced("b", "motor/speed").unwrap();
    assert_eq!(speed.readback.as_deref(), Some("motor/rpm"));
    assert!(model.actions.announced("a", "motor/speed").is_none());
    let answer = proto::Message::ActionResult(result.clone());
    assert_eq!(receive(&mut model, "b", &answer), Some(result));
}

/// Measures how many messages a second the model takes in: scalars on 100 paths
/// and a record of a float, an array and a string.
#[test]
#[ignore = "a benchmark, run with `cargo test --release -p blip ingest -- --ignored --nocapture`"]
fn ingest_rate() {
    let scalar = |timestamp: u64, path: &str| {
        frame(&proto::Message::Sample(proto::Sample {
            timestamp,
            path: path.to_owned(),
            value: proto::Data::Float(timestamp as f64 * 0.5),
        }))
    };
    let imu = |timestamp: u64| {
        frame(&proto::Message::Sample(proto::Sample {
            timestamp,
            path: "sensors/imu".to_owned(),
            value: proto::Data::Record(vec![
                ("t".to_owned(), proto::Data::Float(21.5)),
                (
                    "acc".to_owned(),
                    proto::Data::Array(vec![proto::Data::Float(0.1); 3]),
                ),
                ("mode".to_owned(), proto::Data::from("run")),
            ]),
        }))
    };
    let paths: Vec<String> = (0..100).map(|n| format!("cpu/core{n}")).collect();
    let frames: Vec<Vec<u8>> = (0..1_000_000u64)
        .map(|n| match n % 10 {
            0 => imu(n),
            _ => scalar(n, &paths[n as usize % 100]),
        })
        .collect();
    let mut session = Session::recording();
    let start = std::time::Instant::now();
    for frame in &frames {
        session.inbox("a", 0.0).frame(frame);
    }
    let model = &session.store;
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "{} messages in {seconds:.3} s: {:.0} messages/s",
        frames.len(),
        frames.len() as f64 / seconds
    );
    assert_eq!(
        model
            .traces
            .get("sensors/imu.mode")
            .unwrap()
            .timestamps
            .len(),
        100_000
    );
}

#[test]
fn the_origin_is_the_first_timestamp_a_target_sent() {
    let log = |timestamp| {
        proto::Message::Log(proto::LogRecord {
            timestamp,
            path: String::new(),
            level: 2,
            message: "hi".to_owned(),
        })
    };
    let mut model = MemoryStore::default();
    // a record without a timestamp gets the wall clock, which is no origin
    receive(&mut model, "a", &log(0));
    assert_eq!(model.origin(), None);
    receive(
        &mut model,
        "a",
        &proto::Message::Sample(proto::Sample {
            timestamp: 5_000_000_000,
            path: "rpm".to_owned(),
            value: proto::Data::Uint(1),
        }),
    );
    receive(&mut model, "a", &log(7_000_000_000));
    assert_eq!(model.origin(), Some(5_000_000_000));
}

#[test]
fn samples_of_another_time_base_than_the_origin_are_dropped() {
    let utc = 1_700_000_000_000_000_000;
    let mut model = MemoryStore::default();
    receive(
        &mut model,
        "target",
        &value(5_000_000_000, "rpm", proto::Data::Uint(1)),
    );
    // e.g. from a source with the time of its computer
    receive(&mut model, "pc", &value(utc, "rpm", proto::Data::Uint(2)));
    receive(
        &mut model,
        "pc",
        &value(utc, "pc/load", proto::Data::Uint(3)),
    );
    receive(
        &mut model,
        "pc",
        &proto::Message::Log(proto::LogRecord {
            timestamp: utc,
            path: String::new(),
            level: 2,
            message: "hi".to_owned(),
        }),
    );
    receive(
        &mut model,
        "target",
        &value(6_000_000_000, "rpm", proto::Data::Uint(4)),
    );
    assert_eq!(model.time_span(), Some((5_000_000_000, 6_000_000_000)));
    assert_eq!(model.logs.paths().next(), None);
    assert!(model.traces.get("pc/load").is_none());

    // a record without a timestamp before any data has the wall clock, and moves to
    // the first timestamp in another time base
    model.clear();
    receive(
        &mut model,
        "target",
        &proto::Message::Log(proto::LogRecord {
            timestamp: 0,
            path: String::new(),
            level: 2,
            message: "booting".to_owned(),
        }),
    );
    receive(
        &mut model,
        "target",
        &value(2_000_000_000, "rpm", proto::Data::Uint(1)),
    );
    assert_eq!(model.logs.entries().next().unwrap().time, 2_000_000_000);
    assert_eq!(model.time_span(), Some((2_000_000_000, 2_000_000_000)));

    // cleared, the next sample sets the time base anew
    model.clear();
    receive(&mut model, "pc", &value(utc, "rpm", proto::Data::Uint(5)));
    assert_eq!(model.time_span(), Some((utc, utc)));
}

fn value(timestamp: u64, path: &str, value: proto::Data) -> proto::Message {
    proto::Message::Sample(proto::Sample {
        timestamp,
        path: path.to_owned(),
        value,
    })
}

fn announce(readback: &str) -> proto::Message {
    proto::Message::Actions(vec![proto::ActionDescriptor {
        path: "set".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some(readback.to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }])
}

fn readback_value(model: &MemoryStore, path: &str) -> Option<String> {
    let trace = model.readback(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    Some(
        trace
            .samples
            .format(last, 4, None, numbers::DisplayFormat::Default, None),
    )
}

#[test]
fn readbacks_are_kept_also_while_not_recording() {
    let motor = |timestamp, speed| {
        let fields = vec![
            ("speed".to_owned(), proto::Data::Uint(speed)),
            ("state".to_owned(), proto::Data::from("run")),
        ];
        value(timestamp, "motor", proto::Data::Record(fields))
    };
    let mut model = MemoryStore::default();
    receive_unrecorded(&mut model, "b", &announce("motor.speed"));
    receive_unrecorded(&mut model, "b", &motor(1, 10));
    receive_unrecorded(&mut model, "b", &motor(2, 12));
    receive_unrecorded(&mut model, "b", &value(2, "rpm", proto::Data::Uint(3)));
    // only the latest value of the readback, the samples are dropped
    assert_eq!(readback_value(&model, "motor.speed").as_deref(), Some("12"));
    assert_eq!(model.readback("motor.speed").unwrap().timestamps.len(), 1);
    assert_eq!(model.readback_samples(Some("motor.speed")), 2);
    assert!(model.readback("rpm").is_none());
    assert_eq!(model.time_span(), None);

    // clearing the data keeps them and their counts
    model.clear();
    assert_eq!(readback_value(&model, "motor.speed").as_deref(), Some("12"));
    receive(&mut model, "b", &motor(3, 14));
    assert_eq!(readback_value(&model, "motor.speed").as_deref(), Some("14"));
    assert_eq!(model.readback_samples(Some("motor.speed")), 3);
}

#[test]
fn values_announced_as_readbacks_later_show() {
    let mut model = MemoryStore::default();
    receive_unrecorded(
        &mut model,
        "b",
        &value(1, "level", proto::Data::from("INFO")),
    );
    assert!(model.readback("level").is_none());
    receive_unrecorded(&mut model, "b", &announce("level"));
    assert_eq!(readback_value(&model, "level").as_deref(), Some("INFO"));
    // and no more once no action has it as its readback
    receive_unrecorded(&mut model, "b", &proto::Message::Actions(vec![]));
    assert!(model.readback("level").is_none());
}

#[test]
fn values_hold_their_fields_and_elements() {
    use crate::store::actions::holds;
    assert!(holds("motor", "motor"));
    assert!(holds("motor", "motor.speed"));
    assert!(holds("adc", "adc[1]"));
    assert!(!holds("motor", "motors.speed"));
    assert!(!holds("motor.speed", "motor"));
}

#[test]
fn samples_are_dropped_while_not_recording() {
    let value = frame(&proto::Message::Sample(proto::Sample {
        timestamp: 1,
        path: "rpm".to_owned(),
        value: proto::Data::Uint(1),
    }));
    let log = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 1,
        path: String::new(),
        level: 2,
        message: "hi".to_owned(),
    }));
    let metadata = frame(&proto::Message::TraceMetadata(proto::TraceMetadata {
        path: "rpm".to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    }));
    let actions = frame(&proto::Message::Actions(vec![]));
    let result = proto::ActionResult {
        id: 0,
        result: Ok(()),
    };
    let answer = frame(&proto::Message::ActionResult(result.clone()));

    let mut session = Session::default();
    // the answer goes to the invoker, as the session routes it
    let action = ActionId::new("b", "x");
    session.invoker.invoke(&action, None, vec![], 0.0);
    session.inbox("b", 0.0).frame(&value);
    session.inbox("b", 0.0).frame(&log);
    session.inbox("b", 0.0).frame(&metadata);
    session.inbox("b", 0.0).frame(&actions);
    session.inbox("b", 0.0).frame(&answer);
    assert_eq!(
        session.invoker.outcome(&action, 0.0),
        Some(&Outcome::Accepted(0.0))
    );
    assert_eq!(session.store.time_span(), None);
    assert_eq!(session.store.logs.paths().next(), None);
    // nothing is recorded, a recording starts with the kept units and actions
    assert_eq!(session.recorder.messages(), 0);
    session.clear_data();
    assert_eq!(session.recorder.messages(), 2);

    session.recording = true;
    session.inbox("b", 0.0).frame(&value);
    assert_eq!(session.store.traces.get("rpm").unwrap().timestamps.len(), 1);
    assert_eq!(session.store.traces.unit("rpm"), Some("rpm"));
}

#[test]
fn clearing_keeps_the_latest_units_and_actions_and_records_them_again() {
    let unit = |unit: &str| {
        frame(&proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "rpm".to_owned(),
            unit: unit.to_owned(),
            ..Default::default()
        }))
    };
    let value = frame(&proto::Message::Sample(proto::Sample {
        timestamp: 1,
        path: "rpm".to_owned(),
        value: proto::Data::Uint(1),
    }));
    let actions = frame(&proto::Message::Actions(vec![proto::ActionDescriptor {
        path: "motor/speed".to_owned(),
        description: String::new(),
        params: vec![],
        readback: None,
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]));
    let mut session = Session::recording();
    session.inbox("a", 0.0).frame(&unit("rpm"));
    session.inbox("b", 0.0).frame(&actions);
    session.inbox("a", 0.0).frame(&value);
    session.inbox("a", 0.0).frame(&unit("1/min"));
    session.clear_data();
    let model = &session.store;

    assert_eq!(model.time_span(), None);
    assert_eq!(model.traces.unit("rpm"), Some("1/min"));
    assert!(model.actions.announced("b", "motor/speed").is_some());
    let mut recorded = vec![];
    recording::read_file(&session.recorder.to_file(), |source, frame| {
        recorded.push((source.to_owned(), frame.to_vec()));
    })
    .unwrap();
    assert_eq!(
        recorded,
        [("a".to_owned(), unit("1/min")), ("b".to_owned(), actions)]
    );
}

fn block(path: &str, values: Vec<proto::Data>, times: proto::SampleTimes) -> proto::Samples {
    proto::Samples {
        path: path.to_owned(),
        values,
        times,
    }
}

/// The timestamps and values of the numbers at `path`, as text to compare NaNs.
fn series_of(model: &MemoryStore, path: &str) -> (Vec<u64>, Vec<String>) {
    let series = model.traces.series(path).unwrap();
    let values = (0..series.len())
        .map(|index| series.value(index).to_string())
        .collect();
    (series.timestamps.to_vec(), values)
}

/// Models that received the block, and its samples one by one.
fn block_and_samples(block: &proto::Samples) -> (MemoryStore, MemoryStore) {
    let (mut blocks, mut singles) = (MemoryStore::default(), MemoryStore::default());
    receive(&mut blocks, "a", &proto::Message::Samples(block.clone()));
    for sample in block.to_samples() {
        receive(&mut singles, "a", &proto::Message::Sample(sample));
    }
    (blocks, singles)
}

#[test]
fn blocks_give_the_traces_of_single_samples() {
    use proto::Data::{Null, Uint};
    let adc = block(
        "adc",
        vec![Uint(1), Null, Uint(3)],
        proto::SampleTimes::Period {
            timestamp: 1000,
            period: 10,
        },
    );
    let (blocks, singles) = block_and_samples(&adc);
    let (timestamps, values) = series_of(&blocks, "adc");
    assert_eq!(timestamps, [1000, 1010, 1020]);
    assert_eq!(values, ["1", "NaN", "3"]);
    assert_eq!(series_of(&singles, "adc"), (timestamps, values));
    let record = |x, y| {
        proto::Data::Record(
            [("x", x), ("y", y)]
                .into_iter()
                .map(|(name, value)| (name.to_owned(), Uint(value)))
                .collect(),
        )
    };
    let imu = block(
        "imu",
        vec![record(1, 2), record(3, 4)],
        proto::SampleTimes::Each(vec![5, 7]),
    );
    let (blocks, singles) = block_and_samples(&imu);
    for path in ["imu.x", "imu.y"] {
        assert_eq!(series_of(&blocks, path), series_of(&singles, path));
        assert_eq!(series_of(&blocks, path).0, [5, 7]);
    }
}

#[test]
fn an_empty_block_gives_nothing() {
    let empty = block(
        "adc",
        vec![],
        proto::SampleTimes::Period {
            timestamp: 1,
            period: 1,
        },
    );
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &proto::Message::Samples(empty));
    assert!(model.traces.get("adc").is_none());
    assert_eq!(model.take_notices(), []);
    assert_eq!(model.time_span(), None);
}

#[test]
fn the_latest_value_of_a_block_is_its_last() {
    use proto::Data::Uint;
    let mut model = MemoryStore::default();
    receive_unrecorded(&mut model, "a", &announce("adc"));
    let adc = block(
        "adc",
        vec![Uint(1), Uint(2), Uint(3)],
        proto::SampleTimes::Each(vec![1, 2, 3]),
    );
    receive_unrecorded(&mut model, "a", &proto::Message::Samples(adc));
    assert_eq!(readback_value(&model, "adc").as_deref(), Some("3"));
    assert_eq!(model.readback_samples(Some("adc")), 1);
}

#[test]
fn a_block_counts_once_for_readbacks() {
    use proto::Data::Uint;
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &announce("adc"));
    let adc = block(
        "adc",
        vec![Uint(1), Uint(2), Uint(3)],
        proto::SampleTimes::Each(vec![1, 2, 3]),
    );
    receive(&mut model, "a", &proto::Message::Samples(adc));
    assert_eq!(model.readback_samples(Some("adc")), 1);
    assert_eq!(readback_value(&model, "adc").as_deref(), Some("3"));
}

fn hello(clock: Option<proto::Clock>) -> proto::Message {
    proto::Message::Hello(proto::Hello {
        version: 1,
        name: None,
        clock,
        sender: None,
    })
}

fn log_at(timestamp: u64) -> proto::Message {
    proto::Message::Log(proto::LogRecord {
        timestamp,
        path: String::new(),
        level: 2,
        message: "hi".to_owned(),
    })
}

#[test]
fn a_declared_clock_decides_the_time_base() {
    use proto::Clock;
    let uint = proto::Data::Uint(1);
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &hello(Some(Clock::Boot)));
    receive(
        &mut model,
        "a",
        &value(1_700_000_000_000_000_000, "rpm", uint.clone()),
    );
    assert_eq!(model.time_base(), Some(TimeBase::SinceBoot));
    assert!(model.traces.get("rpm").is_some());

    let mut model = MemoryStore::default();
    receive(&mut model, "a", &hello(Some(Clock::Epoch)));
    assert_eq!(model.time_base(), Some(TimeBase::Unix));
    receive(&mut model, "a", &value(5_000_000_000, "rpm", uint.clone()));
    assert_eq!(model.time_base(), Some(TimeBase::Unix));
    assert!(model.traces.get("rpm").is_some());

    // without a Hello the value guesses
    let mut model = MemoryStore::default();
    receive(
        &mut model,
        "a",
        &value(999_999_999_999_999_999, "rpm", uint.clone()),
    );
    assert_eq!(model.time_base(), Some(TimeBase::SinceBoot));
    let mut model = MemoryStore::default();
    receive(
        &mut model,
        "a",
        &value(1_000_000_000_000_000_000, "rpm", uint.clone()),
    );
    assert_eq!(model.time_base(), Some(TimeBase::Unix));

    // a Hello without a clock replaces the one with it, so the source is guessed again
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &hello(Some(Clock::Boot)));
    receive(&mut model, "a", &hello(None));
    receive(
        &mut model,
        "a",
        &value(1_700_000_000_000_000_000, "rpm", uint),
    );
    assert_eq!(model.time_base(), Some(TimeBase::Unix));
}

#[test]
fn different_clocks_share_the_utc_axis() {
    use proto::Clock;
    let uint = proto::Data::Uint(1);
    let notice = (
        Level::Warning,
        "Connections send different clocks: blip shows the time of day and drops the data \
         timed since boot"
            .to_owned(),
    );
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &hello(Some(Clock::Boot)));
    receive(&mut model, "a", &value(5_000_000_000, "rpm", uint.clone()));
    receive(&mut model, "a", &value(6_000_000_000, "rpm", uint.clone()));
    receive(&mut model, "a", &log_at(5_000_000_000));
    assert!(model.take_notices().is_empty());
    receive(&mut model, "b", &hello(Some(Clock::Epoch)));
    assert_eq!(model.time_span(), None);
    assert!(model.traces.get("rpm").is_none());
    assert_eq!(model.logs.paths().next(), None);
    assert_eq!(model.time_base(), Some(TimeBase::Unix));
    assert_eq!(model.take_notices(), [notice]);

    receive(&mut model, "a", &value(7_000_000_000, "rpm", uint.clone()));
    assert!(model.traces.get("rpm").is_none());
    receive(
        &mut model,
        "b",
        &value(1_700_000_000_000_000_000, "temp", uint.clone()),
    );
    assert!(model.traces.get("temp").is_some());
    receive(&mut model, "b", &hello(Some(Clock::Epoch)));
    assert!(model.take_notices().is_empty());

    // a new recording starts with the Hellos, without a notice
    model.clear();
    assert!(model.take_notices().is_empty());
    assert_eq!(model.time_base(), Some(TimeBase::Unix));

    // the same clock twice is no conflict
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &hello(Some(Clock::Boot)));
    receive(&mut model, "b", &hello(Some(Clock::Boot)));
    assert!(model.take_notices().is_empty());

    // the first sample sets the axis as its Hello declares
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &hello(Some(Clock::Boot)));
    receive(
        &mut model,
        "a",
        &value(1_700_000_000_000_000_000, "rpm", uint),
    );
    assert_eq!(model.time_base(), Some(TimeBase::SinceBoot));
}

#[test]
fn a_record_at_0_follows_the_newest_data() {
    let mut model = MemoryStore::default();
    receive(
        &mut model,
        "a",
        &value(7_000_000_000, "rpm", proto::Data::Uint(1)),
    );
    receive(&mut model, "a", &log_at(0));
    assert_eq!(model.logs.entries().next().unwrap().time, 7_000_000_000);
}

#[test]
fn a_record_at_0_without_data_shows_the_time_of_day() {
    let mut model = MemoryStore::default();
    receive(&mut model, "a", &log_at(0));
    assert!(model.logs.entries().next().unwrap().time >= 1_000_000_000_000_000_000);
    assert_eq!(model.time_base(), Some(TimeBase::Unix));
}

#[test]
fn the_store_takes_typed_input() {
    let action = proto::ActionDescriptor {
        path: "set".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some("rpm".to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    };
    let metadata = proto::TraceMetadata {
        path: "rpm".to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    };
    let mut rows = value::RowsBuf::default();
    for n in [1u64, 2, 3] {
        rows.push_with(|to| to.uint(n));
    }
    let one = |n: u64| value::Value::from(&proto::Data::Uint(n));
    let mut model = MemoryStore::default();
    model.ingest("a", Input::Clock(Some(TimeBase::SinceBoot)));
    model.ingest("a", Input::Metadata(&metadata));
    model.ingest("a", Input::Actions(&[action]));
    model.ingest(
        "a",
        Input::Sample {
            timestamp: 10,
            path: "rpm",
            value: one(5).as_ref(),
        },
    );
    model.ingest(
        "a",
        Input::Samples {
            path: "adc",
            times: Times::Period {
                timestamp: 20,
                period: 5,
            },
            values: rows.rows(),
        },
    );
    model.ingest(
        "a",
        Input::Log {
            timestamp: 30,
            path: "motor",
            level: proto::level::WARN,
            message: "hot",
        },
    );
    model.ingest(
        "a",
        Input::Latest {
            timestamp: 40,
            path: "rpm",
            value: one(7).as_ref(),
        },
    );
    assert_eq!(model.traces.get("rpm").unwrap().timestamps.len(), 1);
    assert_eq!(model.traces.get("adc").unwrap().timestamps, [20, 25, 30]);
    assert_eq!(model.logs.entries().count(), 1);
    assert_eq!(model.traces.unit("rpm"), Some("rpm"));
    assert_eq!(readback_value(&model, "rpm").as_deref(), Some("7"));
    assert_eq!(model.readback_samples(Some("rpm")), 2);
    assert_eq!(model.time_base(), Some(TimeBase::SinceBoot));
}

#[test]
fn clearing_keeps_what_describes_the_sources() {
    let action = proto::ActionDescriptor {
        path: "set".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some("rpm".to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    };
    let metadata = proto::TraceMetadata {
        path: "rpm".to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    };
    let five = value::Value::from(&proto::Data::Uint(5));
    let mut model = MemoryStore::default();
    model.ingest("a", Input::Clock(Some(TimeBase::Unix)));
    model.ingest("a", Input::Metadata(&metadata));
    model.ingest("a", Input::Actions(&[action]));
    model.ingest(
        "a",
        Input::Sample {
            timestamp: 1_700_000_000_000_000_000,
            path: "rpm",
            value: five.as_ref(),
        },
    );
    model.clear();
    assert_eq!(model.time_span(), None);
    assert_eq!(model.traces.unit("rpm"), Some("rpm"));
    assert!(model.actions.announced("a", "set").is_some());
    assert_eq!(model.time_base(), Some(TimeBase::Unix));
    assert_eq!(readback_value(&model, "rpm").as_deref(), Some("5"));
}
