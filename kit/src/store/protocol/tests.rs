use super::*;
use crate::store::flatten::{Flat, Leaf};
use proto::Data;

fn frame(message: &proto::Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).unwrap()
}

fn value_frame(value: Data) -> Vec<u8> {
    frame(&proto::Message::Sample(proto::Sample {
        timestamp: 7,
        path: "imu".to_owned(),
        value,
    }))
}

/// What the parser gave for a message, owned.
#[derive(Debug, PartialEq)]
enum Heard {
    Sample(u64, String, Data),
    Latest(u64, String, Data),
    Samples(String, Vec<u64>, Vec<Data>),
    Log(u64, String, u8, String),
    /// Any other message, as the debug text of what it gave.
    Other(String),
    Unreadable {
        name: Option<String>,
        error: String,
        item: Vec<u8>,
    },
}

fn heard(said: Said<'_>) -> Heard {
    match said {
        Said::Sample {
            timestamp,
            path,
            value,
        } => Heard::Sample(timestamp, path.to_owned(), value.to_data()),
        Said::Latest {
            timestamp,
            path,
            value,
        } => Heard::Latest(timestamp, path.to_owned(), value.to_data()),
        Said::Samples {
            path,
            times,
            values,
        } => Heard::Samples(
            path.to_owned(),
            (0..values.len())
                .map(|row| times.timestamp(row).unwrap())
                .collect(),
            values.iter().map(|value| value.to_data()).collect(),
        ),
        Said::Log {
            timestamp,
            path,
            level,
            message,
        } => Heard::Log(timestamp, path.to_owned(), level, message.to_owned()),
        Said::Unreadable { name, error, item } => Heard::Unreadable {
            name: name.map(str::to_owned),
            error: error.to_owned(),
            item: item.to_vec(),
        },
        other => Heard::Other(format!("{other:?}")),
    }
}

/// What the parser gives for `item`.
fn said_of(item: &[u8], full: bool) -> Vec<Heard> {
    let mut said = vec![];
    Parser::default().item(item, full, |one| said.push(heard(one)));
    said
}

/// Why `item` is unreadable, `None` if it is read.
fn unreadable(item: &[u8]) -> Option<String> {
    match said_of(item, true).pop() {
        Some(Heard::Unreadable { error, .. }) => Some(error),
        _ => None,
    }
}

/// The leaves of `value` with their fields, texts as the text.
fn leaves(value: Data) -> Vec<(String, String)> {
    let bytes = value_frame(value);
    let mut flat = Flat::default();
    let mut texts = String::new();
    let mut given = 0;
    Parser::default().item(&bytes, true, |said| {
        let Said::Sample {
            timestamp: 7,
            path: "imu",
            value,
        } = said
        else {
            panic!("{said:?}");
        };
        given += 1;
        flat.fill(value);
        texts = value.texts().to_owned();
    });
    assert_eq!(given, 1);
    flat.leaves
        .iter()
        .map(|(field, leaf)| {
            let leaf = match leaf {
                Leaf::Text(text) => {
                    format!("{:?}", &texts[text.clone()])
                }
                Leaf::Array { shape, elements } => format!(
                    "{:?} {:?}",
                    &flat.shapes[shape.clone()],
                    &flat.numbers[elements.clone()]
                ),
                leaf => format!("{leaf:?}"),
            };
            (flat.field(field).to_owned(), leaf)
        })
        .collect()
}

fn record(fields: Vec<(&str, Data)>) -> Data {
    Data::Record(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

#[test]
fn values_are_flattened_into_their_leaves() {
    let value = record(vec![
        ("x", Data::Float(0.5)),
        (
            "acc",
            record(vec![("z", Data::Int(-3)), ("ok", Data::Bool(true))]),
        ),
        ("name", Data::from("on")),
        ("gone", Data::Null),
    ]);
    let pairs = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    };
    assert_eq!(
        leaves(value),
        pairs(&[
            (".x", "Number(Float(0.5))"),
            (".acc.z", "Number(Int(-3))"),
            (".acc.ok", "Number(Bool(true))"),
            (".name", "\"on\""),
            (".gone", "Null"),
        ])
    );
    assert_eq!(
        leaves(Data::Uint(u64::MAX)),
        pairs(&[("", "Number(Uint(18446744073709551615))")])
    );
    assert_eq!(
        leaves(Data::Int(i64::MIN)),
        pairs(&[("", "Number(Int(-9223372036854775808))")])
    );
}

#[test]
fn arrays_keep_their_shape() {
    let row = |values: &[f64]| Data::Array(values.iter().map(|&v| Data::Float(v)).collect());
    let matrix = Data::Array(vec![row(&[1.0, 2.0]), Data::Null, row(&[3.0, 4.0])]);
    assert_eq!(
        leaves(matrix)[0].1,
        "[3, 2] [Float(1.0), Float(2.0), Null, Null, Float(3.0), Float(4.0)]"
    );
    assert_eq!(leaves(Data::Array(vec![]))[0].1, "[0] []");
    let ragged = Data::Array(vec![row(&[1.0]), row(&[1.0, 2.0])]);
    assert_eq!(
        leaves(ragged)[0].1,
        "Invalid(\"expected an array of 1 elements, got 2\")"
    );
    let texts = Data::Array(vec![Data::from("a")]);
    assert_eq!(
        leaves(texts)[0].1,
        "Invalid(\"array elements must be numbers or bools, got string\")"
    );
    let null_first = Data::Array(vec![Data::Null, row(&[1.0])]);
    assert!(leaves(null_first)[0].1.starts_with("Invalid"));
    // an invalid array leaves the rest of the record readable
    let value = record(vec![
        ("a", Data::Array(vec![Data::from("a")])),
        ("b", Data::Uint(1)),
    ]);
    assert_eq!(
        leaves(value)[1],
        (".b".to_owned(), "Number(Uint(1))".to_owned())
    );
}

#[test]
fn logs_are_read_and_other_messages_decoded_with_serde() {
    let log = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 3,
        path: "robot/arm".to_owned(),
        level: 4,
        message: "hot".to_owned(),
    }));
    assert_eq!(
        said_of(&log, true),
        [Heard::Log(3, "robot/arm".to_owned(), 4, "hot".to_owned())]
    );
    // without a path, as before logs had one
    let pathless = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 3,
        path: String::new(),
        level: 4,
        message: "hot".to_owned(),
    }));
    assert_eq!(
        said_of(&pathless, true),
        [Heard::Log(3, String::new(), 4, "hot".to_owned())]
    );

    let unit = proto::TraceMetadata {
        path: "rpm".to_owned(),
        unit: "1/min".to_owned(),
        ..Default::default()
    };
    let bytes = frame(&proto::Message::TraceMetadata(unit.clone()));
    let expected = [Heard::Other(format!("{:?}", Said::Metadata(&unit)))];
    assert_eq!(said_of(&bytes, true), expected);
    // while not recording a log is not read, other messages are as before
    assert_eq!(said_of(&log, false), []);
    assert_eq!(said_of(&bytes, false), expected);
    let actions = frame(&proto::Message::Actions(vec![]));
    assert_eq!(said_of(&actions, false), said_of(&actions, true));
    assert_eq!(said_of(&actions, true).len(), 1);
}

#[test]
fn a_component_of_an_old_log_record_is_skipped() {
    #[derive(serde::Serialize)]
    enum OldMessage {
        Log {
            timestamp: u64,
            level: u8,
            component: &'static str,
            message: &'static str,
        },
    }
    let old = serde_cbor_2::to_vec(&OldMessage::Log {
        timestamp: 3,
        level: 4,
        component: "motor",
        message: "hot",
    })
    .unwrap();
    assert_eq!(
        said_of(&old, true),
        [Heard::Log(3, String::new(), 4, "hot".to_owned())]
    );
}

#[test]
fn broken_values_are_refused() {
    // a byte string, a map with an integer key and a duplicate field
    for raw in [
        &[0x41, 0x00][..],
        &[0xa1, 0x01, 0x01],
        &[0xa2, 0x61, b'x', 0x01, 0x61, b'x', 0x02],
    ] {
        let mut bytes = value_frame(Data::Null);
        bytes.truncate(bytes.len() - 1);
        bytes.extend_from_slice(raw);
        assert!(unreadable(&bytes).is_some(), "{raw:?}");
    }
    assert!(unreadable(&[0xa1]).is_some());
    let mut cut = value_frame(Data::Float(1.0));
    cut.pop();
    assert!(unreadable(&cut).is_some());
}

fn block(times: proto::SampleTimes, values: Vec<Data>) -> Vec<u8> {
    frame(&proto::Message::Samples(proto::Samples {
        path: "adc".to_owned(),
        values,
        times,
    }))
}

fn adc() -> Vec<u8> {
    let times = proto::SampleTimes::Period {
        timestamp: 4_000_000,
        period: 250_000,
    };
    block(
        times,
        vec![Data::Uint(512), Data::Uint(513), Data::Uint(511)],
    )
}

#[test]
fn old_and_new_names_decode_alike() {
    #[derive(serde::Serialize)]
    enum Old {
        Value(proto::Sample),
    }
    let sample = proto::Sample {
        timestamp: 7,
        path: "a".to_owned(),
        value: Data::Float(1.5),
    };
    let old = serde_cbor_2::to_vec(&Old::Value(sample.clone())).unwrap();
    let new = frame(&proto::Message::Sample(sample));
    assert_eq!(message_name(&old), Some("Value"));
    assert_eq!(message_name(&new), Some("Sample"));
    let heard_old = said_of(&old, true);
    assert!(matches!(heard_old[..], [Heard::Sample(7, _, _)]));
    assert_eq!(heard_old, said_of(&new, true));
    assert_eq!(leaves_of(&old), leaves_of(&new));
    let log = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 1,
        path: String::new(),
        level: 2,
        message: "hi".to_owned(),
    }));
    // while not recording: the latest value of a sample or block, nothing of a log
    let latest = [Heard::Latest(7, "a".to_owned(), Data::Float(1.5))];
    assert_eq!(said_of(&old, false), latest);
    assert_eq!(said_of(&new, false), latest);
    assert_eq!(
        said_of(&adc(), false),
        [Heard::Latest(4_500_000, "adc".to_owned(), Data::Uint(511))]
    );
    assert_eq!(said_of(&log, false), []);
    let empty = block(
        proto::SampleTimes::Period {
            timestamp: 1,
            period: 1,
        },
        vec![],
    );
    assert_eq!(said_of(&empty, false), []);
    assert_eq!(
        said_of(&empty, true),
        [Heard::Samples("adc".to_owned(), vec![], vec![])]
    );
    let actions = frame(&proto::Message::Actions(vec![]));
    assert_eq!(said_of(&actions, false), said_of(&actions, true));
}

#[test]
fn blocks_are_read_in_one_pass() {
    let bytes = adc();
    let mut parser = Parser::default();
    let mut given = 0;
    parser.item(&bytes, true, |said| {
        let Said::Samples {
            path,
            times,
            values,
        } = said
        else {
            panic!("a block");
        };
        given += 1;
        assert_eq!(path, "adc");
        assert_eq!(
            times,
            Times::Period {
                timestamp: 4_000_000,
                period: 250_000
            }
        );
        let rows: Vec<Data> = values.iter().map(|value| value.to_data()).collect();
        assert_eq!(rows, [Data::Uint(512), Data::Uint(513), Data::Uint(511)]);
    });
    assert_eq!(given, 1);
    let each = block(
        proto::SampleTimes::Each(vec![1, 2, 3]),
        vec![Data::Bool(true), Data::Bool(false), Data::Bool(true)],
    );
    parser.item(&each, true, |said| {
        let Said::Samples { times, values, .. } = said else {
            panic!("a block");
        };
        given += 1;
        assert_eq!(times, Times::Each(&[1, 2, 3]));
        assert_eq!(values.len(), 3);
    });
    assert_eq!(given, 2);
}

#[test]
fn a_block_is_read_only_up_to_its_last_value_while_not_recording() {
    // {"Samples": {"path": "a", "values": [h'00', 5], "timestamps": [1, 2]}}
    let mut bytes = vec![0xa1, 0x67];
    bytes.extend(b"Samples");
    bytes.extend([0xa3, 0x64]);
    bytes.extend(b"path");
    bytes.extend([0x61, b'a', 0x66]);
    bytes.extend(b"values");
    bytes.extend([0x82, 0x41, 0x00, 0x05, 0x6a]);
    bytes.extend(b"timestamps");
    bytes.extend([0x82, 0x01, 0x02]);
    assert!(matches!(
        said_of(&bytes, true).as_slice(),
        [Heard::Unreadable { .. }]
    ));
    assert_eq!(
        said_of(&bytes, false),
        [Heard::Latest(2, "a".to_owned(), Data::Uint(5))]
    );
    let empty = block(
        proto::SampleTimes::Period {
            timestamp: 1,
            period: 1,
        },
        vec![],
    );
    assert_eq!(said_of(&empty, false), []);
    assert_eq!(
        said_of(&empty, true),
        [Heard::Samples("adc".to_owned(), vec![], vec![])]
    );
}

#[test]
fn malformed_blocks_are_refused() {
    use proto::SampleTimes::Each;
    let values = |len: usize| vec![Data::Uint(1); len];
    let both = {
        #[derive(serde::Serialize)]
        struct Fields {
            path: &'static str,
            values: Vec<u8>,
            timestamp: u64,
            period: u64,
            timestamps: Vec<u64>,
        }
        #[derive(serde::Serialize)]
        enum Wire {
            Samples(Fields),
        }
        serde_cbor_2::to_vec(&Wire::Samples(Fields {
            path: "a",
            values: vec![1],
            timestamp: 1,
            period: 1,
            timestamps: vec![1],
        }))
        .unwrap()
    };
    let lacks = |keys: &str| {
        // a block with only some fields, written by hand
        let mut bytes = vec![0xa1, 0x67];
        bytes.extend(b"Samples");
        let fields: Vec<(&str, Vec<u8>)> = vec![
            ("path", vec![0x61, b'a']),
            ("values", vec![0x81, 0x01]),
            ("timestamp", vec![0x01]),
        ];
        let chosen: Vec<_> = fields.iter().filter(|(k, _)| keys.contains(k)).collect();
        bytes.push(0xa0 | chosen.len() as u8);
        for (key, value) in chosen {
            bytes.push(0x60 | key.len() as u8);
            bytes.extend(key.as_bytes());
            bytes.extend(value);
        }
        bytes
    };
    let max = u64::MAX;
    let cases = [
        (both, "both a period and timestamps"),
        (lacks("path values"), "neither a period nor timestamps"),
        (lacks("path values timestamp"), "a timestamp but no period"),
        (lacks("values timestamp"), "lacks its path"),
        (lacks("path timestamp"), "lacks its values"),
        (
            block(Each(vec![1, 2, 3]), values(2)),
            "2 values but 3 timestamps",
        ),
        (
            block(
                proto::SampleTimes::Period {
                    timestamp: max - 5,
                    period: 10,
                },
                values(2),
            ),
            "beyond 2^64 ns",
        ),
    ];
    for (bytes, text) in cases {
        let error = unreadable(&bytes).expect(text);
        assert!(error.contains(text), "{error} for {text}");
    }
    let one = block(
        proto::SampleTimes::Period {
            timestamp: max - 5,
            period: 10,
        },
        values(1),
    );
    assert!(unreadable(&one).is_none());
    // a duplicate field of the second value fails the block before any sample
    let mut bytes = vec![0xa1, 0x67];
    bytes.extend(b"Samples");
    bytes.extend([0xa4, 0x64]);
    bytes.extend(b"path");
    bytes.extend([0x61, b'a', 0x66]);
    bytes.extend(b"values");
    // 1, {"x": 1, "x": 2}
    bytes.extend([0x82, 0x01, 0xa2, 0x61, b'x', 0x01, 0x61, b'x', 0x02, 0x69]);
    bytes.extend(b"timestamp");
    bytes.extend([0x01, 0x66]);
    bytes.extend(b"period");
    bytes.push(0x01);
    let error = unreadable(&bytes).unwrap();
    assert!(error.contains("duplicate record field"), "{error}");
}

/// The message `name` with `fields` as they are, CBOR values by name.
fn raw(name: &str, fields: &[(&str, &[u8])]) -> Vec<u8> {
    let text = |text: &str| [&[0x60 | text.len() as u8][..], text.as_bytes()].concat();
    let mut bytes = [vec![0xa1], text(name), vec![0xa0 | fields.len() as u8]].concat();
    for (field, value) in fields {
        bytes.extend(text(field));
        bytes.extend_from_slice(value);
    }
    bytes
}

#[test]
fn repeated_fields_are_refused() {
    let (path, one) = (&[0x61, b'a'][..], &[0x01][..]);
    let values = &[0x81, 0x01][..];
    let message = &[0x61, b'm'][..];
    let sample = [("timestamp", one), ("path", path), ("value", one)];
    let block = [
        ("path", path),
        ("values", values),
        ("timestamp", one),
        ("period", one),
    ];
    let log = [("timestamp", one), ("level", one), ("message", message)];
    for (name, fields) in [("Sample", &sample[..]), ("Samples", &block), ("Log", &log)] {
        assert!(unreadable(&raw(name, fields)).is_none(), "{name}");
        for &repeated in fields {
            let fields = [fields, &[repeated]].concat();
            let bytes = raw(name, &fields);
            let refused = unreadable(&bytes).is_some();
            assert!(refused, "{name} with two {}", repeated.0);
        }
    }
}

#[test]
fn samples_and_logs_in_another_encoding_are_refused() {
    let log = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 7,
        path: String::new(),
        level: 4,
        message: "hot".to_owned(),
    }));
    for mut bytes in [value_frame(Data::Uint(1)), adc(), log] {
        assert!(unreadable(&bytes).is_none());
        // an array of the name and the content, which serde reads as the enum too
        assert_eq!(bytes[0], 0xa1);
        bytes[0] = 0x82;
        assert!(serde_cbor_2::from_slice::<proto::Message>(&bytes).is_ok());
        assert!(unreadable(&bytes).is_some());
    }
}

#[test]
fn samples_are_read_only_up_to_their_latest_value_while_not_recording() {
    #[derive(serde::Serialize)]
    enum Old {
        Value(proto::Sample),
    }
    let sample = proto::Sample {
        timestamp: 7,
        path: "imu".to_owned(),
        value: Data::Float(1.5),
    };
    let latest = [Heard::Latest(7, "imu".to_owned(), Data::Float(1.5))];
    let new = frame(&proto::Message::Sample(sample.clone()));
    assert_eq!(said_of(&new, false), latest);
    let old = serde_cbor_2::to_vec(&Old::Value(sample)).unwrap();
    assert_eq!(said_of(&old, false), latest);
    let log = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 1,
        path: String::new(),
        level: 2,
        message: "hi".to_owned(),
    }));
    assert_eq!(said_of(&log, false), []);
    let hello = frame(&proto::Message::Hello(proto::Hello::default()));
    assert_eq!(said_of(&hello, true).len(), 1);
    assert_eq!(said_of(&hello, false), said_of(&hello, true));
    // the values before the last are skipped, whatever they hold: a byte string
    let skipped = [raw(
        "Samples",
        &[
            ("path", &[0x61, b'a'][..]),
            ("values", &[0x82, 0x41, 0x00, 0x05]),
            ("timestamp", &[0x01]),
            ("period", &[0x02]),
        ],
    )];
    assert_eq!(
        said_of(&skipped[0], false),
        [Heard::Latest(3, "a".to_owned(), Data::Uint(5))]
    );
    assert!(unreadable(&skipped[0]).is_some());
}

#[test]
fn unreadable_messages_keep_their_bytes() {
    let mut both = vec![0xa1, 0x67];
    both.extend(b"Samples");
    both.extend([0xa5, 0x64]);
    both.extend(b"path");
    both.push(0x63);
    both.extend(b"bad");
    both.push(0x66);
    both.extend(b"values");
    both.extend([0x81, 0x01, 0x69]);
    both.extend(b"timestamp");
    both.extend([0x01, 0x66]);
    both.extend(b"period");
    both.extend([0x01, 0x6a]);
    both.extend(b"timestamps");
    both.extend([0x81, 0x01]);
    let future = [
        0xa1, 0x66, b'F', b'u', b't', b'u', b'r', b'e', 0xa1, 0x61, b'x', 0x01,
    ];
    let text = [0x61, b'x'];
    let mut bytes_sample = value_frame(Data::Null);
    bytes_sample.truncate(bytes_sample.len() - 1);
    bytes_sample.extend([0x41, 0x00]);
    for (item, name) in [
        (&both[..], Some("Samples")),
        (&future[..], Some("Future")),
        (&text[..], None),
        (&bytes_sample[..], Some("Sample")),
    ] {
        for full in [true, false] {
            let heard = said_of(item, full);
            let [
                Heard::Unreadable {
                    name: got,
                    item: kept,
                    ..
                },
            ] = &heard[..]
            else {
                panic!("{heard:?} for {item:?}");
            };
            assert_eq!(got.as_deref(), name);
            assert_eq!(kept, item);
        }
    }
}

/// The leaves of the value of the sample in `item`, as pairs.
fn leaves_of(item: &[u8]) -> Vec<(String, Leaf)> {
    let mut flat = Flat::default();
    Parser::default().item(item, true, |said| {
        if let Said::Sample { value, .. } = said {
            flat.fill(value);
        }
    });
    flat.leaves
        .iter()
        .map(|(field, leaf)| (flat.field(field).to_owned(), leaf.clone()))
        .collect()
}

fn data_record(fields: &[(&str, Data)]) -> Data {
    record(fields.to_vec())
}

fn sample_message(path: &str, value: Data) -> proto::Message {
    proto::Message::Sample(proto::Sample {
        timestamp: 20,
        path: path.to_owned(),
        value,
    })
}

#[test]
fn the_writer_writes_what_serde_writes() {
    use proto::{ActionDescriptor, Param, ParamKind};
    let matrix = Data::Array(vec![
        Data::Array(vec![Data::Float(1.0), Data::Float(2.0)]),
        Data::Null,
        Data::Array(vec![Data::Float(3.0), Data::Float(4.0)]),
    ]);
    let mut messages = vec![
        proto::Message::Hello(proto::Hello {
            version: 1,
            name: Some("rig".to_owned()),
            clock: Some(proto::Clock::Boot),
            sender: Some("sonde 1.2".to_owned()),
        }),
        proto::Message::Hello(proto::Hello {
            version: 1,
            name: None,
            clock: None,
            sender: None,
        }),
        proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "motor/rpm".to_owned(),
            unit: "rpm".to_owned(),
            description: "Speed".to_owned(),
            labels: [(0, "off".to_owned()), (1, "on".to_owned())].into(),
            range: Some(proto::ExpectedRange {
                min: 0.0,
                max: 100.0,
            }),
        }),
        proto::Message::Actions(vec![
            ActionDescriptor {
                path: "reset".to_owned(),
                description: String::new(),
                params: vec![Param {
                    name: "rpm".to_owned(),
                    kind: ParamKind::Float {
                        min: Some(0.0),
                        max: None,
                        step: None,
                    },
                    unit: "rpm".to_owned(),
                    default: Some(Data::Float(1.0)),
                }],
                readback: Some("motor/rpm".to_owned()),
                confirm: false,
                enabled: true,
                disabled_reason: None,
            },
            ActionDescriptor {
                path: "stop".to_owned(),
                description: String::new(),
                params: vec![],
                readback: None,
                confirm: true,
                enabled: false,
                disabled_reason: Some("calibrating".to_owned()),
            },
        ]),
        proto::Message::ActionResult(proto::ActionResult {
            id: 0,
            result: Ok(()),
        }),
        proto::Message::ActionResult(proto::ActionResult {
            id: 1,
            result: Err("jammed".to_owned()),
        }),
        proto::Message::Log(proto::LogRecord {
            timestamp: 10,
            path: "motor".to_owned(),
            level: proto::level::WARN,
            message: "hot".to_owned(),
        }),
        proto::Message::Log(proto::LogRecord {
            timestamp: 11,
            path: String::new(),
            level: proto::level::INFO,
            message: "up".to_owned(),
        }),
        sample_message("null", Data::Null),
        sample_message("uint", Data::Uint(7)),
        sample_message("int", Data::Int(-7)),
        sample_message("float", Data::Float(0.1)),
        sample_message("bool", Data::Bool(true)),
        sample_message("text", Data::String("on".to_owned())),
        sample_message(
            "array",
            Data::Array(vec![Data::Float(1.0), Data::Null, Data::Float(3.0)]),
        ),
        sample_message("matrix", matrix),
        sample_message(
            "record",
            data_record(&[
                ("t", Data::Float(21.5)),
                (
                    "acc",
                    data_record(&[("x", Data::Float(0.5)), ("y", Data::Int(-1))]),
                ),
                ("mode", Data::String("run".to_owned())),
            ]),
        ),
        sample_message(
            "strings",
            Data::Array(vec![
                Data::String("a".to_owned()),
                Data::String("b".to_owned()),
            ]),
        ),
        sample_message(
            "records",
            Data::Array(vec![
                data_record(&[("a", Data::Uint(1))]),
                data_record(&[("a", Data::Uint(2))]),
            ]),
        ),
        proto::Message::Samples(proto::Samples {
            path: "adc".to_owned(),
            values: vec![Data::Uint(1), Data::Uint(2), Data::Uint(3)],
            times: proto::SampleTimes::Period {
                timestamp: 40,
                period: 5,
            },
        }),
        proto::Message::Samples(proto::Samples {
            path: "flags".to_owned(),
            values: vec![Data::Bool(true), Data::Bool(false)],
            times: proto::SampleTimes::Each(vec![50, 52]),
        }),
    ];
    for value in [
        0.0,
        -0.0,
        0.5,
        1.5,
        21.0,
        65504.0,
        2f64.powi(-24),
        2f64.powi(-14),
        f64::INFINITY,
        f64::NEG_INFINITY,
        1234.5,
        65520.0,
        100000.0,
        16777216.0,
        2f64.powi(-25),
        3.4028234663852886e38,
        0.1,
        1.0 / 3.0,
        16777217.0,
        1e300,
        f64::NAN,
    ] {
        messages.push(proto::Message::Sample(proto::Sample {
            timestamp: 1,
            path: "a".to_owned(),
            value: Data::Float(value),
        }));
    }
    for message in &messages {
        let bytes = frame(message);
        let mut out = vec![];
        let mut said_count = 0;
        Parser::default().item(&bytes, true, |said| {
            said_count += 1;
            assert!(encode(&said, &mut out), "{said:?}");
        });
        assert_eq!(said_count, 1, "{message:?}");
        assert_eq!(out, bytes, "{message:?}");
    }

    // what is no message to write
    let mut out = vec![];
    let bytes = value_frame(Data::Uint(1));
    Parser::default().item(&bytes, false, |said| assert!(!encode(&said, &mut out)));
    Parser::default().item(&[0xa1], true, |said| assert!(!encode(&said, &mut out)));
    assert!(out.is_empty());
}

/// What `frame` gave, and how many messages it split off.
fn said_of_frame(bytes: &[u8]) -> (usize, Vec<Heard>) {
    let mut said = vec![];
    let items = Parser::default().frame("a", bytes, true, |one| said.push(heard(one)));
    (items, said)
}

#[test]
fn a_frame_gives_its_messages_in_order() {
    let log = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 3,
        path: String::new(),
        level: 2,
        message: "hi".to_owned(),
    }));
    let sample = frame(&proto::Message::Sample(proto::Sample {
        timestamp: 4,
        path: "a".to_owned(),
        value: Data::Uint(1),
    }));
    let heard_log = Heard::Log(3, String::new(), 2, "hi".to_owned());
    let heard_sample = Heard::Sample(4, "a".to_owned(), Data::Uint(1));

    let both = [log.clone(), sample.clone()].concat();
    assert_eq!(said_of_frame(&both), (2, vec![heard_log, heard_sample]));

    // cut off in its last message: the messages before it
    let cut = &both[..both.len() - 1];
    assert_eq!(said_of_frame(cut).0, 1);
    assert_eq!(said_of_frame(cut).1.len(), 1);
    assert!(matches!(said_of_frame(cut).1[0], Heard::Log(..)));

    // a byte that starts no item ends the frame
    let broken = [log, vec![0xff], sample].concat();
    let (items, said) = said_of_frame(&broken);
    assert_eq!(items, 1);
    assert!(matches!(said[..], [Heard::Log(..)]));
}

/// A sample as a CBOR map by hand: `entries` are its fields, in this order.
fn sample_by_hand(entries: Vec<(&str, serde_cbor_2::Value)>) -> Vec<u8> {
    use serde_cbor_2::Value;
    let fields = entries
        .into_iter()
        .map(|(name, value)| (Value::Text(name.to_owned()), value))
        .collect();
    let message = [(Value::Text("Sample".to_owned()), Value::Map(fields))].into();
    serde_cbor_2::to_vec(&Value::Map(message)).unwrap()
}

/// The messages of the frame test: every kind, readable here, by serde or not at all.
fn mixed_messages() -> Vec<Vec<u8>> {
    use serde_cbor_2::Value;
    let sample = |value| {
        frame(&proto::Message::Sample(proto::Sample {
            timestamp: 1,
            path: "a".to_owned(),
            value,
        }))
    };
    let log = |path: &str| {
        frame(&proto::Message::Log(proto::LogRecord {
            timestamp: 9,
            path: path.to_owned(),
            level: proto::level::INFO,
            message: "hi".to_owned(),
        }))
    };
    vec![
        sample(Data::Float(1.5)),
        sample(Data::Record(vec![
            ("t".to_owned(), Data::String("x".to_owned())),
            ("v".to_owned(), Data::Array(vec![Data::Uint(1), Data::Null])),
        ])),
        block(
            proto::SampleTimes::Period {
                timestamp: 10,
                period: 5,
            },
            vec![Data::Uint(1), Data::Uint(2), Data::Uint(3)],
        ),
        block(
            proto::SampleTimes::Each(vec![3, 4]),
            vec![Data::Float(0.5), Data::Float(0.25)],
        ),
        log("motor"),
        log(""),
        frame(&proto::Message::Hello(proto::Hello {
            version: 1,
            name: Some("rig".to_owned()),
            clock: None,
            sender: None,
        })),
        frame(&proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "a".to_owned(),
            unit: "V".to_owned(),
            description: String::new(),
            labels: Default::default(),
            range: None,
        })),
        frame(&proto::Message::Actions(vec![])),
        frame(&proto::Message::ActionResult(proto::ActionResult {
            id: 3,
            result: Err("no".to_owned()),
        })),
        serde_cbor_2::to_vec(&Value::Map(
            [(Value::Text("Future".to_owned()), Value::Integer(1))].into(),
        ))
        .unwrap(),
        sample_by_hand(vec![
            ("extra", Value::Array(vec![Value::Integer(1), Value::Null])),
            ("value", Value::Integer(5)),
            ("path", Value::Text("a".to_owned())),
            ("timestamp", Value::Integer(2)),
        ]),
        sample_by_hand(vec![
            ("timestamp", Value::Integer(2)),
            ("path", Value::Text("a".to_owned())),
            ("value", Value::Bytes(vec![1, 2, 3])),
        ]),
    ]
}

#[test]
fn a_frame_is_read_as_its_items_are() {
    let messages = mixed_messages();
    for full in [true, false] {
        let one_by_one = |messages: &[Vec<u8>]| -> Vec<Heard> {
            let mut parser = Parser::default();
            let mut said = vec![];
            for message in messages {
                parser.item(message, full, |one| said.push(heard(one)));
            }
            said
        };
        let in_frame = |bytes: &[u8]| {
            let mut said = vec![];
            let items = Parser::default().frame("a", bytes, full, |one| said.push(heard(one)));
            (items, said)
        };
        let whole = messages.concat();
        assert_eq!(
            in_frame(&whole),
            (messages.len(), one_by_one(&messages)),
            "full {full}"
        );
        assert!(
            one_by_one(&messages)
                .iter()
                .any(|h| matches!(h, Heard::Unreadable { .. })),
        );

        // cut one byte short: the messages before the last
        let cut = &whole[..whole.len() - 1];
        let before = &messages[..messages.len() - 1];
        assert_eq!(
            in_frame(cut),
            (before.len(), one_by_one(before)),
            "cut, full {full}"
        );
    }
}
