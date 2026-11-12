use super::*;

fn frame(message: &proto::Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).unwrap()
}

#[test]
fn paths_are_kept_as_the_outposts_send_them() {
    let value = |path: &str| {
        frame(&proto::Message::Value(proto::Value {
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
    let mut model = Model::default();
    model.receive("", &value("cpu"));
    model.receive("a", &value("cpu"));
    model.receive("b", &value("cpu"));
    model.receive("b", &log("robot/arm"));
    model.receive("b", &log(""));
    model.receive("b", &log("motor"));
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
    assert_eq!(model.recording.messages(), 6);
}

#[test]
fn activity_is_counted_per_bin_with_records_once() {
    let value = |timestamp: u64, path: &str, value: proto::Data| {
        frame(&proto::Message::Value(proto::Value {
            timestamp,
            path: path.to_owned(),
            value,
        }))
    };
    let imu = proto::Data::Record(
        [("x", 1.0), ("y", 2.0)]
            .into_iter()
            .map(|(name, x)| (name.to_owned(), proto::Data::Float(x)))
            .collect(),
    );
    let mut model = Model::default();
    for t in [0, 10, 19, 20, 39, 40] {
        model.receive("", &value(t, "cpu", proto::Data::Uint(1)));
    }
    model.receive("", &value(25, "imu", imu.clone()));
    model.receive("", &value(35, "imu", imu));
    // of two paths, the first with a record after the bins of the second
    let log = |timestamp, path: &str, level| {
        frame(&proto::Message::Log(proto::LogRecord {
            timestamp,
            path: path.to_owned(),
            level,
            message: "hi".to_owned(),
        }))
    };
    for (timestamp, path, level) in [(5, "", 2), (30, "", 2), (2, "motor", 3), (15, "motor", 4)] {
        model.receive("", &log(timestamp, path, level));
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
    model.receive("", &value(45, "imu", partial));
    assert_eq!(values(model.activity(40, 10, 1)), [1]);
}

#[test]
fn actions_are_kept_with_their_source_and_answers_returned() {
    let actions = frame(&proto::Message::Actions(vec![proto::ActionDescriptor {
        path: "motor/speed".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some("motor/rpm".to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]));
    let result = proto::ActionResult {
        id: 3,
        result: Ok(()),
    };
    let mut model = Model::default();
    assert_eq!(model.receive("b", &actions), None);
    let speed = model.actions.announced("b", "motor/speed").unwrap();
    assert_eq!(speed.readback.as_deref(), Some("motor/rpm"));
    assert!(model.actions.announced("a", "motor/speed").is_none());
    let answer = frame(&proto::Message::ActionResult(result.clone()));
    assert_eq!(model.receive("b", &answer), Some(result));
}

/// Measures how many messages a second the model takes in: scalars on 100 paths
/// and a record of a float, an array and a string.
#[test]
#[ignore = "a benchmark, run with `cargo test --release -p blip ingest -- --ignored --nocapture`"]
fn ingest_rate() {
    let scalar = |timestamp: u64, path: &str| {
        frame(&proto::Message::Value(proto::Value {
            timestamp,
            path: path.to_owned(),
            value: proto::Data::Float(timestamp as f64 * 0.5),
        }))
    };
    let imu = |timestamp: u64| {
        frame(&proto::Message::Value(proto::Value {
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
    let mut model = Model::default();
    let start = std::time::Instant::now();
    for frame in &frames {
        model.receive("a", frame);
    }
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
        frame(&proto::Message::Log(proto::LogRecord {
            timestamp,
            path: String::new(),
            level: 2,
            message: "hi".to_owned(),
        }))
    };
    let mut model = Model::default();
    // a record without a timestamp gets the wall clock, which is no origin
    model.receive("a", &log(0));
    assert_eq!(model.origin(), None);
    model.receive(
        "a",
        &frame(&proto::Message::Value(proto::Value {
            timestamp: 5_000_000_000,
            path: "rpm".to_owned(),
            value: proto::Data::Uint(1),
        })),
    );
    model.receive("a", &log(7_000_000_000));
    assert_eq!(model.origin(), Some(5_000_000_000));
}

#[test]
fn samples_of_another_time_base_than_the_origin_are_dropped() {
    let utc = 1_700_000_000_000_000_000;
    let mut model = Model::default();
    model.receive("target", &value(5_000_000_000, "rpm", proto::Data::Uint(1)));
    // e.g. from a source with the time of its computer
    model.receive("pc", &value(utc, "rpm", proto::Data::Uint(2)));
    model.receive("pc", &value(utc, "pc/load", proto::Data::Uint(3)));
    model.receive(
        "pc",
        &frame(&proto::Message::Log(proto::LogRecord {
            timestamp: utc,
            path: String::new(),
            level: 2,
            message: "hi".to_owned(),
        })),
    );
    model.receive("target", &value(6_000_000_000, "rpm", proto::Data::Uint(4)));
    assert_eq!(model.time_span(), Some((5_000_000_000, 6_000_000_000)));
    assert_eq!(model.logs.paths().next(), None);
    assert!(model.traces.get("pc/load").is_none());

    // a record without a timestamp before any data has the wall clock, and moves to
    // the first timestamp in another time base
    model.clear();
    model.receive(
        "target",
        &frame(&proto::Message::Log(proto::LogRecord {
            timestamp: 0,
            path: String::new(),
            level: 2,
            message: "booting".to_owned(),
        })),
    );
    model.receive("target", &value(2_000_000_000, "rpm", proto::Data::Uint(1)));
    assert_eq!(model.logs.entries().next().unwrap().time, 2_000_000_000);
    assert_eq!(model.time_span(), Some((2_000_000_000, 2_000_000_000)));

    // cleared, the next sample sets the time base anew
    model.clear();
    model.receive("pc", &value(utc, "rpm", proto::Data::Uint(5)));
    assert_eq!(model.time_span(), Some((utc, utc)));
}

fn value(timestamp: u64, path: &str, value: proto::Data) -> Vec<u8> {
    frame(&proto::Message::Value(proto::Value {
        timestamp,
        path: path.to_owned(),
        value,
    }))
}

fn announce(readback: &str) -> Vec<u8> {
    frame(&proto::Message::Actions(vec![proto::ActionDescriptor {
        path: "set".to_owned(),
        description: String::new(),
        params: vec![],
        readback: Some(readback.to_owned()),
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]))
}

fn readback_value(model: &Model, path: &str) -> Option<String> {
    let trace = model.readback(path)?;
    let last = trace.timestamps.len().checked_sub(1)?;
    Some(
        trace
            .samples
            .format(last, 4, None, numbers::DisplayFormat::Default),
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
    let mut model = Model::default();
    model.receive_unrecorded("b", &announce("motor.speed"));
    model.receive_unrecorded("b", &motor(1, 10));
    model.receive_unrecorded("b", &motor(2, 12));
    model.receive_unrecorded("b", &value(2, "rpm", proto::Data::Uint(3)));
    // only the latest value of the readback, the samples are dropped
    assert_eq!(readback_value(&model, "motor.speed").as_deref(), Some("12"));
    assert_eq!(model.readback("motor.speed").unwrap().timestamps.len(), 1);
    assert_eq!(model.readback_samples(Some("motor.speed")), 2);
    assert!(model.readback("rpm").is_none());
    assert_eq!(model.time_span(), None);

    // clearing the data keeps them and their counts
    model.clear();
    assert_eq!(readback_value(&model, "motor.speed").as_deref(), Some("12"));
    model.receive("b", &motor(3, 14));
    assert_eq!(readback_value(&model, "motor.speed").as_deref(), Some("14"));
    assert_eq!(model.readback_samples(Some("motor.speed")), 3);
}

#[test]
fn values_announced_as_readbacks_later_show() {
    let mut model = Model::default();
    model.receive_unrecorded("b", &value(1, "level", proto::Data::from("INFO")));
    assert!(model.readback("level").is_none());
    model.receive_unrecorded("b", &announce("level"));
    assert_eq!(readback_value(&model, "level").as_deref(), Some("INFO"));
    // and no more once no action has it as its readback
    model.receive_unrecorded("b", &frame(&proto::Message::Actions(vec![])));
    assert!(model.readback("level").is_none());
}

#[test]
fn values_hold_their_fields_and_elements() {
    use crate::app::model::actions::holds;
    assert!(holds("motor", "motor"));
    assert!(holds("motor", "motor.speed"));
    assert!(holds("adc", "adc[1]"));
    assert!(!holds("motor", "motors.speed"));
    assert!(!holds("motor.speed", "motor"));
}

#[test]
fn samples_are_dropped_while_not_recording() {
    let value = frame(&proto::Message::Value(proto::Value {
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
    let metadata = frame(&proto::Message::ValueMetadata(proto::ValueMetadata {
        path: "rpm".to_owned(),
        unit: "rpm".to_owned(),
    }));
    let actions = frame(&proto::Message::Actions(vec![]));
    let result = proto::ActionResult {
        id: 3,
        result: Ok(()),
    };
    let answer = frame(&proto::Message::ActionResult(result.clone()));

    let mut model = Model::default();
    model.receive_unrecorded("b", &value);
    model.receive_unrecorded("b", &log);
    model.receive_unrecorded("b", &metadata);
    model.receive_unrecorded("b", &actions);
    assert_eq!(model.receive_unrecorded("b", &answer), Some(result));
    assert_eq!(model.time_span(), None);
    assert_eq!(model.logs.paths().next(), None);
    // nothing is recorded, a recording starts with the kept units and actions
    assert_eq!(model.recording.messages(), 0);
    model.clear();
    assert_eq!(model.recording.messages(), 2);

    model.receive("b", &value);
    assert_eq!(model.traces.get("rpm").unwrap().timestamps.len(), 1);
    assert_eq!(model.traces.unit("rpm"), Some("rpm"));
}

#[test]
fn clearing_keeps_the_latest_units_and_actions_and_records_them_again() {
    let unit = |unit: &str| {
        frame(&proto::Message::ValueMetadata(proto::ValueMetadata {
            path: "rpm".to_owned(),
            unit: unit.to_owned(),
        }))
    };
    let value = frame(&proto::Message::Value(proto::Value {
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
    let mut model = Model::default();
    model.receive("a", &unit("rpm"));
    model.receive("b", &actions);
    model.receive("a", &value);
    model.receive("a", &unit("1/min"));
    model.clear();

    assert_eq!(model.time_span(), None);
    assert_eq!(model.traces.unit("rpm"), Some("1/min"));
    assert!(model.actions.announced("b", "motor/speed").is_some());
    let mut recorded = vec![];
    recording::read_file(&model.recording.to_file(), |source, frame| {
        recorded.push((source.to_owned(), frame.to_vec()));
    })
    .unwrap();
    assert_eq!(
        recorded,
        [("a".to_owned(), unit("1/min")), ("b".to_owned(), actions)]
    );
}
