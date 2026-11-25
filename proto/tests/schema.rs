//! The schema of the protocol, `blip.cddl`: what blip-proto, the outpost and sonde
//! write is what it says.

use blip_proto::{
    ActionDescriptor, ActionResult, Clock, Data, ExpectedRange, Hello, Invocation, LogRecord,
    Message, Param, ParamKind, Request, Sample, SampleTimes, Samples, TraceMetadata,
};
use serde::Serialize;
use serde_cbor_2::Deserializer;
use serde_cbor_2::value::Value;

const SCHEMA: &str = include_str!("../blip.cddl");

/// Written by the test "the recording fixture is what sonde writes" of sonde.
const SONDE: &[u8] = include_bytes!("../../sonde/tests/data/recording.blip");

/// Written by `test_the_fixture_is_what_the_outpost_writes` of the outpost.
const OUTPOST: &[u8] = include_bytes!("../../outpost/tests/data/messages.cbor");

/// Whether `bytes` are one item that the rule `rule` of the schema allows.
fn check(rule: &str, bytes: &[u8]) -> Result<(), String> {
    cddl_cat::validate_cbor_bytes(rule, SCHEMA, bytes).map_err(|error| error.to_string())
}

fn encode(value: &impl Serialize) -> Vec<u8> {
    serde_cbor_2::to_vec(value).unwrap()
}

/// The items of the CBOR sequence `bytes`, each as its bytes.
fn items(mut bytes: &[u8]) -> Vec<&[u8]> {
    let mut items = Vec::new();
    while !bytes.is_empty() {
        let mut values = Deserializer::from_slice(bytes).into_iter::<Value>();
        values.next().unwrap().unwrap();
        let (item, rest) = bytes.split_at(values.byte_offset());
        items.push(item);
        bytes = rest;
    }
    items
}

/// Checks that `item` is a message of the schema, encoded as one: a map of definite
/// length with one entry.
#[track_caller]
fn assert_message(item: &[u8]) {
    let shown = serde_cbor_2::from_slice::<Value>(item).unwrap();
    if let Err(error) = check("message", item) {
        panic!("{shown:?} is no message of the schema: {error}");
    }
    assert_eq!(item[0], 0xa1, "{shown:?} is no map of one entry");
}

fn record(fields: &[(&str, Data)]) -> Data {
    Data::Record(
        fields
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
    )
}

fn param(name: &str, kind: ParamKind, default: Option<Data>) -> Param {
    Param {
        name: name.to_owned(),
        kind,
        unit: String::new(),
        default,
    }
}

/// A message of each kind, with and without its optional fields.
fn messages() -> Vec<Message> {
    let sample = |value: Data| {
        Message::Sample(Sample {
            timestamp: 1_000_000,
            path: "a/b".to_owned(),
            value,
        })
    };
    let block = |values: Vec<Data>, times: SampleTimes| {
        Message::Samples(Samples {
            path: "adc".to_owned(),
            values,
            times,
        })
    };
    let action = ActionDescriptor {
        path: "reboot".to_owned(),
        description: String::new(),
        params: Vec::new(),
        readback: None,
        confirm: false,
        enabled: true,
        disabled_reason: None,
    };
    let int = |min, max| ParamKind::Int { min, max };
    let float = |min, max, step| ParamKind::Float { min, max, step };
    vec![
        Message::Hello(Hello {
            version: 1,
            name: Some("rig-7".to_owned()),
            clock: Some(Clock::Boot),
            sender: Some("sonde 0.2.0".to_owned()),
        }),
        Message::Hello(Hello {
            version: 1,
            clock: Some(Clock::Epoch),
            ..Default::default()
        }),
        Message::Hello(Hello::default()),
        sample(Data::Null),
        sample(Data::Uint(u64::MAX)),
        sample(Data::Int(i64::MIN)),
        sample(Data::Float(0.5)),
        sample(Data::Float(0.1)),
        sample(Data::Bool(true)),
        sample(Data::from("running")),
        sample(Data::Array(vec![Data::Uint(1), Data::Float(1.5)])),
        sample(record(&[
            ("acc", Data::Array(vec![Data::Float(9.81)])),
            ("inner", record(&[("valid", Data::Bool(false))])),
        ])),
        block(
            vec![Data::Uint(512), Data::Null],
            SampleTimes::Period {
                timestamp: 4_000_000,
                period: 250_000,
            },
        ),
        block(vec![Data::Float(1.5)], SampleTimes::Each(vec![4_000_000])),
        block(Vec::new(), SampleTimes::Each(Vec::new())),
        Message::TraceMetadata(TraceMetadata {
            path: "motor/speed".to_owned(),
            ..Default::default()
        }),
        Message::TraceMetadata(TraceMetadata {
            path: "motor/state".to_owned(),
            unit: "rpm".to_owned(),
            description: "Drive state".to_owned(),
            labels: [(-1, "fault".to_owned()), (0, "off".to_owned())].into(),
            range: Some(ExpectedRange {
                min: -1.0,
                max: 2.5,
            }),
        }),
        Message::Log(LogRecord {
            timestamp: 0,
            path: String::new(),
            level: 4,
            message: "no path".to_owned(),
        }),
        Message::Log(LogRecord {
            timestamp: 3_000_000,
            path: "robot/arm".to_owned(),
            level: 255,
            message: "started".to_owned(),
        }),
        Message::Actions(Vec::new()),
        Message::Actions(vec![
            action.clone(),
            ActionDescriptor {
                path: "motor/speed".to_owned(),
                description: "Speed setpoint".to_owned(),
                params: vec![
                    param("on", ParamKind::Bool, Some(Data::Bool(true))),
                    param("rpm", int(Some(0), Some(3000)), Some(Data::Uint(1500))),
                    param("offset", int(None, None), None),
                    param("gain", float(Some(0.0), Some(2.5), Some(0.25)), None),
                    param("free", float(None, None, None), None),
                    param("mode", ParamKind::Enum(vec!["idle".to_owned()]), None),
                    Param {
                        unit: "s".to_owned(),
                        ..param("label", ParamKind::String, Some(Data::from("x")))
                    },
                ],
                readback: Some("motor/speed_rpm".to_owned()),
                confirm: true,
                enabled: false,
                disabled_reason: Some("calibrating".to_owned()),
            },
        ]),
        Message::ActionResult(ActionResult {
            id: 1,
            result: Ok(()),
        }),
        Message::ActionResult(ActionResult {
            id: u32::MAX,
            result: Err("unlucky speed".to_owned()),
        }),
    ]
}

#[test]
fn blip_proto_writes_what_the_schema_says() {
    for message in messages() {
        assert_message(&encode(&message));
    }
    for args in [record(&[]), record(&[("rpm", Data::Uint(1500))])] {
        let request = encode(&Request::Invoke(Invocation {
            id: 7,
            path: "motor/speed".to_owned(),
            args,
        }));
        check("request", &request).unwrap();
        assert_eq!(request[0], 0xa1);
    }
}

#[test]
fn sonde_writes_what_the_schema_says() {
    let items = items(SONDE);
    check("recording-header", items[0]).unwrap();
    assert!(items.len() > 10);
    for item in &items[1..] {
        assert_message(item);
        check("recording-item", item).unwrap();
    }
}

#[test]
fn the_outpost_writes_what_the_schema_says() {
    let items = items(OUTPOST);
    assert!(items.len() > 10);
    for item in items {
        assert_message(item);
    }
}

#[test]
fn the_old_names_are_legacy_messages() {
    #[derive(Serialize)]
    struct OldMetadata {
        path: &'static str,
        unit: &'static str,
    }
    #[derive(Serialize)]
    enum Old {
        Value(Sample),
        ValueMetadata(OldMetadata),
    }
    let sample = Sample {
        timestamp: 1,
        path: "a".to_owned(),
        value: Data::Uint(2),
    };
    let metadata = OldMetadata {
        path: "a",
        unit: "%",
    };
    for old in [Old::Value(sample), Old::ValueMetadata(metadata)] {
        let bytes = encode(&old);
        check("legacy-message", &bytes).unwrap();
        check("recording-item", &bytes).unwrap();
        check("message", &bytes).unwrap_err();
    }
    check("recording-item", &encode(&"ws://127.0.0.1:3000/ws")).unwrap();
}

#[test]
fn the_schema_refuses_what_is_no_message() {
    use serde_cbor_2::value::to_value;
    use std::collections::BTreeMap;

    type Map = BTreeMap<&'static str, Value>;
    let int = |n: u64| Value::Integer(n.into());
    let text = |text: &str| Value::Text(text.to_owned());
    let message =
        |name: &'static str, fields: Map| encode(&Map::from([(name, to_value(fields).unwrap())]));
    let sample = |sample_value: Value| {
        message(
            "Sample",
            Map::from([
                ("timestamp", int(1)),
                ("path", text("a")),
                ("value", sample_value),
            ]),
        )
    };

    // the pieces above make messages of the schema
    check("message", &sample(int(1))).unwrap();

    let refused = [
        // a byte string is no value, nor is a map with a key that is no text
        sample(Value::Bytes(vec![1, 2])),
        sample(Value::Map([(int(1), int(2))].into())),
        // a misspelt and a missing field
        message(
            "Sample",
            Map::from([
                ("timestamp", int(1)),
                ("path", text("a")),
                ("valeu", int(1)),
            ]),
        ),
        message(
            "Sample",
            Map::from([("timestamp", int(1)), ("value", int(1))]),
        ),
        // a negative timestamp
        message(
            "Sample",
            Map::from([
                ("timestamp", Value::Integer(-1)),
                ("path", text("a")),
                ("value", int(1)),
            ]),
        ),
        // a block with both forms of times, and with neither
        message(
            "Samples",
            Map::from([
                ("path", text("a")),
                ("values", Value::Array(vec![int(1)])),
                ("timestamp", int(1)),
                ("period", int(1)),
                ("timestamps", Value::Array(vec![int(1)])),
            ]),
        ),
        message(
            "Samples",
            Map::from([("path", text("a")), ("values", Value::Array(Vec::new()))]),
        ),
        // a clock and a message the protocol does not have
        message(
            "Hello",
            Map::from([("version", int(1)), ("clock", text("gps"))]),
        ),
        message("Sampel", Map::new()),
        // a level beyond a byte
        message(
            "Log",
            Map::from([
                ("timestamp", int(1)),
                ("level", int(256)),
                ("message", text("m")),
            ]),
        ),
        // two messages in one map
        encode(&Map::from([
            (
                "ActionResult",
                to_value(Map::from([("id", int(1))])).unwrap(),
            ),
            ("Actions", Value::Array(Vec::new())),
        ])),
    ];
    for bytes in refused {
        let shown = serde_cbor_2::from_slice::<Value>(&bytes).unwrap();
        assert!(check("message", &bytes).is_err(), "{shown:?}");
    }
}
