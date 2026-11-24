use super::*;
use proto::Data;

fn record(fields: &[(&str, Data)]) -> Data {
    Data::Record(
        fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.clone()))
            .collect(),
    )
}

fn array(elements: &[f64]) -> Data {
    Data::Array(elements.iter().copied().map(Data::Float).collect())
}

fn traces(samples: &[(&str, Data)]) -> Traces {
    let mut traces = Traces::default();
    for (timestamp, (path, value)) in samples.iter().enumerate() {
        traces.append(&proto::Sample {
            timestamp: timestamp as u64,
            path: path.to_string(),
            value: value.clone(),
        });
    }
    traces
}

fn values(traces: &Traces, path: &str) -> Vec<f64> {
    let series = traces.series(path).unwrap();
    (0..series.len()).map(|index| series.value(index)).collect()
}

/// Compares the bit patterns, so NaN equals NaN.
fn assert_values(actual: Vec<f64>, expected: &[f64]) {
    // any NaN, also the one of a null
    let bits = |values: &[f64]| {
        values
            .iter()
            .map(|value| {
                if value.is_nan() {
                    u64::MAX
                } else {
                    value.to_bits()
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&actual), bits(expected));
}

#[test]
fn series_know_the_range_of_all_their_values() {
    let traces = traces(&[
        ("a", Data::Null),
        ("a", Data::Float(2.0)),
        ("a", Data::Float(-1.0)),
        ("a", Data::Float(f64::NAN)),
        ("a", Data::Int(5)),
        ("v", array(&[1.0, 10.0])),
        ("v", array(&[-3.0, 20.0])),
        ("n", Data::Null),
    ]);
    let a = traces.series("a").unwrap();
    assert_eq!(a.range, Some((-1.0, 5.0)));
    // also in a slice of the samples
    assert_eq!(a.slice(0..1).range, Some((-1.0, 5.0)));
    assert_eq!(traces.series("v[0]").unwrap().range, Some((-3.0, 1.0)));
    assert_eq!(traces.series("v[1]").unwrap().range, Some((10.0, 20.0)));
    assert!(traces.series("n").is_none_or(|n| n.range.is_none()));
    assert_eq!(traces.get("v").unwrap().value_range(), Some((-3.0, 20.0)));
    assert_eq!(traces.get("n").unwrap().value_range(), None);
}

#[test]
fn records_split_into_field_traces() {
    let imu = record(&[
        ("x", Data::Float(0.5)),
        ("acc", record(&[("z", Data::Uint(9))])),
    ]);
    let traces = traces(&[("imu", imu)]);
    let paths: Vec<&str> = traces.iter().map(|trace| trace.path).collect();
    assert_eq!(paths, ["imu.acc.z", "imu.x"]);
    assert_values(values(&traces, "imu.x"), &[0.5]);
    assert_eq!(
        traces.series("imu.acc.z").unwrap().kind,
        NumberKind::Integer
    );
}

#[test]
fn fields_start_marks_where_record_fields_begin() {
    let imu = record(&[("acc", record(&[("z", Data::Uint(9))]))]);
    let traces = traces(&[("sensors/imu", imu), ("v1.2", Data::Float(1.0))]);
    let fields_start = |path| traces.get(path).unwrap().fields_start;
    assert_eq!(fields_start("sensors/imu.acc.z"), Some(11));
    // a dot in a path that is not a record is part of the name
    assert_eq!(fields_start("v1.2"), None);
}

#[test]
fn late_samples_are_put_in_the_order_of_their_timestamps() {
    let mut traces = Traces::default();
    let mut push = |timestamp, path: &str, data: Data| {
        traces.append(&proto::Sample {
            timestamp,
            path: path.to_owned(),
            value: data,
        });
    };
    for (timestamp, value) in [(10, 1.0), (30, 3.0), (20, 2.0), (5, 0.5), (30, 4.0)] {
        push(timestamp, "n", Data::Float(value));
        push(timestamp, "s", Data::String(value.to_string()));
        push(timestamp, "a", array(&[value, -value]));
    }
    push(25, "n", Data::Null);

    let n = traces.get("n").unwrap();
    assert_eq!(n.timestamps, [5, 10, 20, 25, 30, 30]);
    assert_values(values(&traces, "n"), &[0.5, 1.0, 2.0, f64::NAN, 3.0, 4.0]);
    let Samples::Numbers(numbers) = &n.samples else {
        panic!("numbers expected");
    };
    assert!(numbers.is_null(3) && !numbers.is_null(4));
    let s = traces.get("s").unwrap();
    let texts: Vec<String> = (0..5)
        .map(|index| {
            s.samples
                .format(index, 4, None, DisplayFormat::Default, None)
        })
        .collect();
    assert_eq!(texts, ["0.5", "1", "2", "3", "4"]);
    let a = traces.get("a").unwrap();
    let second: Vec<Number> = (0..5)
        .map(|index| a.samples.array_sample(index).unwrap().get(1).unwrap())
        .collect();
    assert_eq!(second, [-0.5, -1.0, -2.0, -3.0, -4.0].map(Number::Float));
}

#[test]
fn records_keep_the_fields_of_their_first() {
    let mut traces = traces(&[
        ("a", record(&[("b", record(&[("c", Data::Uint(1))]))])),
        (
            "x",
            record(&[("y", Data::Float(1.0)), ("s", Data::from("on"))]),
        ),
    ]);
    fn append(traces: &mut Traces, timestamp: u64, path: &str, value: Data) {
        traces.append(&proto::Sample {
            timestamp,
            path: path.to_owned(),
            value,
        });
    }
    // the field of a record has the path `a.b.c`
    append(&mut traces, 10, "a.b", record(&[("c", Data::Uint(2))]));
    // a string does not fit the float field, which is null
    let text = record(&[("y", Data::from("text")), ("s", Data::from("off"))]);
    append(&mut traces, 11, "x", text);
    // other fields than those of the first record
    append(&mut traces, 12, "x", record(&[("y", Data::Float(3.0))]));
    let more = record(&[
        ("y", Data::Float(4.0)),
        ("s", Data::from("on")),
        ("z", Data::Uint(1)),
    ]);
    append(&mut traces, 13, "x", more);
    // a missing record leaves a gap
    append(&mut traces, 14, "x", Data::Null);

    assert!(traces.get("a.b").is_none());
    assert_eq!(traces.get("a.b.c").unwrap().timestamps, [0]);
    assert_eq!(traces.tables().count(), 2);
    assert_eq!(traces.get("x.y").unwrap().timestamps, [1, 11, 14]);
    assert_values(values(&traces, "x.y"), &[1.0, f64::NAN, f64::NAN]);
    assert!(traces.get("x.z").is_none());
    assert!(traces.warned("x.y"));
}

#[test]
fn values_far_older_than_the_newest_are_dropped() {
    let mut traces = Traces::default();
    let mut append = |timestamp: u64| {
        traces.append(&proto::Sample {
            timestamp,
            path: "a".to_owned(),
            value: Data::Float(timestamp as f64),
        });
    };
    for timestamp in 1..=MAX_LATE as u64 + 10 {
        append(timestamp * 10);
    }
    // late, but not too late
    let newest = (MAX_LATE as u64 + 10) * 10;
    append(newest - 5);
    // too late
    append(5);
    let a = traces.get("a").unwrap();
    assert_eq!(a.timestamps.len(), MAX_LATE + 11);
    assert_eq!(a.timestamps[0], 10);
    assert_eq!(a.timestamps[MAX_LATE + 9], newest - 5);
    assert!(a.timestamps.is_sorted());
    assert!(traces.warned("a"));
}

#[test]
fn kinds_change_when_a_trace_turns_out_of_another_kind() {
    let mut traces = Traces::default();
    let mut append = |value: Data| {
        traces.append(&proto::Sample {
            timestamp: 0,
            path: "a".to_owned(),
            value,
        });
        traces.kinds()
    };
    let nulls = append(Data::Null);
    let ints = append(Data::Uint(1));
    assert_ne!(ints, nulls);
    assert_eq!(append(Data::Uint(2)), ints);
    assert_ne!(append(Data::Float(0.5)), ints);
}

#[test]
fn integer_traces_switch_to_float() {
    let traces = traces(&[
        ("a", Data::Uint(0)),
        ("a", Data::Int(-1)),
        ("a", Data::Float(0.5)),
    ]);
    let series = traces.series("a").unwrap();
    assert_eq!(series.kind, NumberKind::Float);
    assert_values(values(&traces, "a"), &[0.0, -1.0, 0.5]);
}

#[test]
fn samples_of_another_kind_are_dropped() {
    let traces = traces(&[
        ("a", Data::Float(1.0)),
        ("a", Data::from("text")),
        ("a", Data::Bool(true)),
        ("a", Data::Float(2.0)),
    ]);
    assert_values(values(&traces, "a"), &[1.0, 2.0]);
    assert_eq!(traces.get("a").unwrap().timestamps, [0, 3]);
    assert!(traces.warned("a"));
}

#[test]
fn a_scalar_trace_does_not_turn_into_a_record() {
    let traces = traces(&[
        ("a", Data::Float(1.0)),
        ("a", record(&[("x", Data::Float(2.0))])),
    ]);
    assert!(traces.get("a.x").is_none());
    assert!(traces.warned("a"));
}

#[test]
fn null_is_a_gap() {
    let bools = traces(&[
        ("a", Data::Null),
        ("a", Data::Bool(true)),
        ("a", Data::Null),
    ]);
    assert_eq!(bools.series("a").unwrap().kind, NumberKind::Bool);
    assert_values(values(&bools, "a"), &[f64::NAN, 1.0, f64::NAN]);

    let strings = traces(&[("s", Data::Null), ("s", Data::from("on"))]);
    let trace = strings.get("s").unwrap();
    let Samples::Strings(texts) = trace.samples else {
        panic!("strings expected");
    };
    assert_eq!((texts.get(0), texts.get(1)), (None, Some("on")));
}

#[test]
fn a_null_record_is_a_gap_in_all_fields() {
    let imu = record(&[("x", Data::Float(1.0)), ("y", Data::Float(2.0))]);
    let traces = traces(&[("imu", Data::Null), ("imu", imu), ("imu", Data::Null)]);
    // the trace the leading null created turned into the record
    assert!(traces.get("imu").is_none());
    assert_values(values(&traces, "imu.x"), &[f64::NAN, 1.0, f64::NAN]);
    assert_values(values(&traces, "imu.y"), &[f64::NAN, 2.0, f64::NAN]);
}

#[test]
fn a_null_is_no_gap_in_traces_with_a_dot_in_their_names() {
    let imu = record(&[("x", Data::Float(1.0))]);
    let traces = traces(&[
        ("v1.2", Data::Float(1.0)),
        ("imu", imu),
        ("v1", Data::Null),
        ("imu.x", Data::Null),
    ]);
    assert_values(values(&traces, "v1.2"), &[1.0]);
    assert_eq!(*traces.get("v1").unwrap().samples, Samples::Empty(1));
    // `imu.x` is a field of the record, values cannot be sent to it
    assert_values(values(&traces, "imu.x"), &[1.0]);
}

#[test]
fn arrays_keep_their_shape() {
    let matrix = |a: f64| Data::Array(vec![array(&[a, 1.0, 2.0]), array(&[3.0, 4.0, 5.0])]);
    let traces = traces(&[
        ("m", Data::Null),
        ("m", matrix(0.0)),
        ("m", Data::Array(vec![array(&[0.0])])),
        ("m", Data::Array(vec![array(&[0.0, 1.0, 2.0]), Data::Null])),
        ("m", matrix(10.0)),
    ]);
    let trace = traces.get("m").unwrap();
    assert_eq!(trace.samples.type_label(), "float[2×3]");
    assert_eq!(trace.timestamps, [0, 1, 3, 4]);
    assert_values(values(&traces, "m[0][0]"), &[f64::NAN, 0.0, 0.0, 10.0]);
    assert_values(values(&traces, "m[1][2]"), &[f64::NAN, 5.0, f64::NAN, 5.0]);
    assert!(traces.series("m[2][0]").is_none());
    assert!(traces.series("m[0]").is_none());
    assert!(traces.series("m").is_none());
}

#[test]
fn arrays_without_elements_are_kept() {
    let empty = || Data::Array(vec![]);
    let traces = traces(&[
        ("e", Data::Null),
        ("e", empty()),
        ("e", empty()),
        ("m", Data::Array(vec![empty(), empty()])),
    ]);
    let e = traces.get("e").unwrap();
    assert_eq!(e.timestamps.len(), 3);
    assert_eq!(e.samples.type_label(), "null[0]");
    assert_eq!(
        e.samples.format(1, 8, None, DisplayFormat::Default, None),
        "[]"
    );
    assert_eq!(e.value_range(), None);
    let m = traces.get("m").unwrap();
    assert_eq!(m.samples.type_label(), "null[2×0]");
    assert!(traces.series("e[0]").is_none());

    // also when a sample arrives late
    let mut traces = traces;
    traces.append(&proto::Sample {
        timestamp: 0,
        path: "m".to_owned(),
        value: Data::Array(vec![empty(), empty()]),
    });
    assert_eq!(traces.get("m").unwrap().timestamps, [0, 3]);
}

#[test]
fn arrays_hold_numbers_or_bools_only() {
    let traces = traces(&[
        ("a", Data::Array(vec![Data::Uint(1), Data::Null])),
        ("a", Data::Array(vec![Data::Float(0.5), Data::Uint(2)])),
        ("a", Data::Array(vec![Data::Bool(true), Data::Uint(2)])),
        ("a", Data::Array(vec![Data::from("x"), Data::Uint(2)])),
    ]);
    assert_eq!(traces.get("a").unwrap().samples.type_label(), "float[2]");
    assert_values(values(&traces, "a[1]"), &[f64::NAN, 2.0]);
}

#[test]
fn element_paths_round_trip() {
    assert_eq!(element_path("a/b", &[1, 20]), "a/b[1][20]");
    assert_eq!(parse_element_path("a/b[1][20]"), Some(("a/b", vec![1, 20])));
    assert_eq!(parse_element_path("a[1]/b"), None);
    assert_eq!(parse_element_path("a[x]"), None);
    assert_eq!(parse_element_path("a"), None);
}

#[test]
fn series_slices_keep_the_element() {
    let traces = traces(&[
        ("a", array(&[0.0, 1.0])),
        ("a", array(&[2.0, 3.0])),
        ("a", array(&[4.0, 5.0])),
    ]);
    let slice = traces.series("a[1]").unwrap().slice(1..3);
    assert_eq!(slice.timestamps, [1, 2]);
    assert_eq!((slice.value(0), slice.value(1)), (3.0, 5.0));
}

#[test]
fn samples_are_formatted_as_text() {
    let matrix = Data::Array(vec![array(&[0.5, 1.0, 2.0]), array(&[3.0, 4.0, 5.0])]);
    let traces = traces(&[
        ("m", matrix),
        ("s", Data::from("on")),
        ("s", Data::Null),
        ("i", Data::Uint(7)),
        ("f", Data::Float(0.031_414_634_623_640_13)),
    ]);
    let format = |path, index, max_len, digits| {
        let samples = traces.get(path).unwrap().samples;
        samples.format(index, max_len, digits, DisplayFormat::Default, None)
    };
    assert_eq!(format("m", 0, 8, None), "[[0.5, 1, 2],\n [  3, 4, 5]]");
    assert_eq!(format("m", 0, 2, None), "[[0.5, 1, …],\n [  3, 4, …]]");
    assert_eq!(format("m", 0, 1, None), "[[0.5, …],\n …]");
    assert_eq!(format("s", 0, 8, None), "on");
    assert_eq!(format("s", 1, 8, None), "null");
    assert_eq!(format("i", 0, 8, Some(2)), "7");
    assert_eq!(format("f", 0, 8, Some(6)), "0.0314146");
    assert_eq!(format("f", 0, 8, None), "0.03141463462364013");
    let samples = traces.get("m").unwrap().samples;
    assert_eq!(
        samples.array_sample(0).unwrap().get(3),
        Some(Number::Float(3.0))
    );
    assert!(samples.array_sample(1).is_none());
}

#[test]
fn array_elements_are_written_in_their_display_format() {
    let traces = traces(&[("a", Data::Array(vec![Data::Uint(10), Data::Uint(255)]))]);
    let samples = traces.get("a").unwrap().samples;
    assert_eq!(
        samples.format(0, 8, None, DisplayFormat::Hex, None),
        "[0xa, 0xff]"
    );
}

#[test]
fn integers_are_exact() {
    let big = Data::Uint((1 << 53) + 1);
    let traces = traces(&[("a", big.clone()), ("m", Data::Array(vec![big]))]);
    let samples = |path| traces.get(path).unwrap().samples;
    assert_eq!(
        samples("a").format(0, 8, None, DisplayFormat::Default, None),
        "9007199254740993"
    );
    assert_eq!(
        samples("m").format(0, 8, None, DisplayFormat::Default, None),
        "[9007199254740993]"
    );
    let series = traces.series("m[0]").unwrap();
    assert_eq!(series.number(0), Number::Int((1 << 53) + 1));
}

#[test]
fn elements_have_the_unit_of_their_array() {
    let mut traces = Traces::default();
    traces.set_metadata(&proto::TraceMetadata {
        path: "imu.acc".into(),
        unit: "m/s²".into(),
        ..Default::default()
    });
    traces.set_metadata(&proto::TraceMetadata {
        path: "speed".into(),
        unit: "".into(),
        ..Default::default()
    });
    assert_eq!(traces.unit("imu.acc"), Some("m/s²"));
    assert_eq!(traces.unit("imu.acc[2]"), Some("m/s²"));
    assert_eq!(traces.unit("imu.gyro"), None);
    assert_eq!(traces.unit("speed"), None);
}

#[test]
fn arrays_are_formatted_with_a_line_per_row() {
    let format = |value: f64| value.to_string();
    assert_eq!(
        format_array(&[3], &[1.0, 20.0, 3.0], 8, &format),
        "[1, 20, 3]"
    );
    assert_eq!(
        format_array(&[2, 2, 1], &[1.0, 2.0, 3.0, 40.0], 8, &format),
        "[[[ 1],\n  [ 2]],\n\n [[ 3],\n  [40]]]"
    );
    assert_eq!(format_array(&[0], &[], 8, &format), "[]");
}

#[test]
fn metadata_applies_to_fields_and_elements() {
    use proto::{ExpectedRange, TraceMetadata};

    let with_range = |path: &str, min, max| TraceMetadata {
        path: path.into(),
        unit: "m/s²".into(),
        description: "Acceleration".into(),
        range: Some(ExpectedRange { min, max }),
        ..Default::default()
    };
    let mut traces = Traces::default();
    traces.set_metadata(&with_range("imu.acc", -20.0, 20.0));
    traces.set_metadata(&TraceMetadata {
        path: "motor/mode".into(),
        labels: [(0, "Idle".to_owned()), (1, "Run".to_owned())].into(),
        ..Default::default()
    });
    assert_eq!(traces.description("imu.acc[0]"), Some("Acceleration"));
    assert_eq!(traces.expected("imu.acc[2]"), Some((-20.0, 20.0)));
    assert_eq!(traces.unit("imu.acc"), Some("m/s²"));
    assert_eq!(
        traces.labels("motor/mode").map(|labels| labels.len()),
        Some(2)
    );
    assert!(traces.labels("imu").is_none());
    // a range that is not usable is dropped, the rest kept
    for (min, max) in [(5.0, 5.0), (f64::NAN, 1.0)] {
        traces.set_metadata(&with_range("imu.acc", min, max));
        assert_eq!(traces.expected("imu.acc"), None);
        assert_eq!(traces.unit("imu.acc"), Some("m/s²"));
    }
    traces.clear_samples();
    assert_eq!(traces.unit("imu.acc"), Some("m/s²"));
    // metadata with only a path removes the entry
    traces.set_metadata(&TraceMetadata {
        path: "imu.acc".into(),
        ..Default::default()
    });
    assert!(traces.metadata("imu.acc").is_none());
}
