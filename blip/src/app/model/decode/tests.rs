use super::*;
use proto::Data;

fn frame(message: &proto::Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).unwrap()
}

fn value_frame(value: Data) -> Vec<u8> {
    frame(&proto::Message::Value(proto::Value {
        timestamp: 7,
        path: "imu".to_owned(),
        value,
    }))
}

/// The leaves of `value` with their fields, texts as the text.
fn leaves(value: Data) -> Vec<(String, String)> {
    let bytes = value_frame(value);
    let mut flat = Flat::default();
    let message = decode(&bytes, &mut flat).unwrap();
    assert_eq!(
        message,
        Message::Value {
            timestamp: 7,
            path: "imu"
        }
    );
    flat.leaves
        .iter()
        .map(|(field, leaf)| {
            let leaf = match leaf {
                Leaf::Text(text) => {
                    format!("{:?}", std::str::from_utf8(&bytes[text.clone()]).unwrap())
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
    let mut flat = Flat::default();
    assert_eq!(
        decode(&log, &mut flat).unwrap(),
        Message::Log {
            timestamp: 3,
            path: "robot/arm",
            level: 4,
            message: "hot"
        }
    );
    // without a path, as before logs had one
    let pathless = frame(&proto::Message::Log(proto::LogRecord {
        timestamp: 3,
        path: String::new(),
        level: 4,
        message: "hot".to_owned(),
    }));
    assert!(matches!(
        decode(&pathless, &mut flat).unwrap(),
        Message::Log { path: "", .. }
    ));

    let unit = proto::Message::ValueMetadata(proto::ValueMetadata {
        path: "rpm".to_owned(),
        unit: "1/min".to_owned(),
    });
    assert!(is_sample(&log) && !is_sample(&frame(&unit)) && !is_sample(&[0xa1]));
    assert_eq!(
        decode(&frame(&unit), &mut flat).unwrap(),
        Message::Other(unit)
    );
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
        decode(&old, &mut Flat::default()).unwrap(),
        Message::Log {
            timestamp: 3,
            path: "",
            level: 4,
            message: "hot"
        }
    );
}

#[test]
fn broken_values_are_refused() {
    let mut flat = Flat::default();
    // a byte string, a map with an integer key and a duplicate field
    for value in [
        &[0x41, 0x00][..],
        &[0xa1, 0x01, 0x01],
        &[0xa2, 0x61, b'x', 0x01, 0x61, b'x', 0x02],
    ] {
        let mut bytes = value_frame(Data::Null);
        bytes.truncate(bytes.len() - 1);
        bytes.extend_from_slice(value);
        assert!(decode(&bytes, &mut flat).is_err(), "{value:?}");
    }
    assert!(decode(&[0xa1], &mut flat).is_err());
    let mut cut = value_frame(Data::Float(1.0));
    cut.pop();
    assert!(decode(&cut, &mut flat).is_err());
}
