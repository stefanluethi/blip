//! The messages sonde, the C++ tracing library, encodes, as in a recording it wrote.

use proto::{
    Clock, Data, ExpectedRange, Hello, LogRecord, Message, Sample, SampleTimes, Samples,
    TraceMetadata,
};
use serde_cbor_2::Deserializer;

/// Written by the test "the recording fixture is what sonde writes" of sonde.
const RECORDING: &[u8] = include_bytes!("../../sonde/tests/data/recording.blip");

#[derive(serde::Deserialize, Debug, PartialEq)]
struct Header {
    format: String,
    version: u32,
}

fn value(timestamp: u64, path: &str, value: Data) -> Message {
    Message::Sample(Sample {
        timestamp,
        path: path.to_owned(),
        value,
    })
}

fn log(timestamp: u64, path: &str, level: u8, message: &str) -> Message {
    Message::Log(LogRecord {
        timestamp,
        path: path.to_owned(),
        level,
        message: message.to_owned(),
    })
}

fn record(fields: Vec<(&str, Data)>) -> Data {
    Data::Record(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

/// The items of the recording after its header, each with its bytes.
fn messages() -> Vec<(Message, &'static [u8])> {
    let mut header = Deserializer::from_slice(RECORDING).into_iter::<Header>();
    assert_eq!(
        header.next().unwrap().unwrap(),
        Header {
            format: "blip recording".to_owned(),
            version: 3,
        }
    );
    let mut rest = &RECORDING[header.byte_offset()..];
    let mut messages = Vec::new();
    while !rest.is_empty() {
        let mut items = Deserializer::from_slice(rest).into_iter::<Message>();
        let message = items.next().unwrap().unwrap();
        let (bytes, after) = rest.split_at(items.byte_offset());
        messages.push((message, bytes));
        rest = after;
    }
    messages
}

#[test]
fn decodes_a_recording_of_sonde() {
    let ms = 1_000_000;
    let expected = vec![
        Message::Hello(Hello {
            version: 1,
            name: Some("bench".to_owned()),
            clock: Some(Clock::Boot),
            sender: Some("sonde 0.2.0".to_owned()),
        }),
        Message::TraceMetadata(TraceMetadata {
            path: "motor/speed".to_owned(),
            unit: "rpm".to_owned(),
            ..Default::default()
        }),
        value(ms, "motor/speed", Data::Uint(1500)),
        value(2 * ms, "cpu_load", Data::Float(12.5)),
        value(2 * ms, "sensor/temperature", Data::Null),
        value(
            2 * ms,
            "imu",
            record(vec![
                (
                    "acc",
                    Data::Array(vec![
                        Data::Float(0.5),
                        Data::Float(-1.0),
                        Data::Float(f64::from(9.81_f32)),
                    ]),
                ),
                ("valid", Data::Bool(true)),
            ]),
        ),
        value(
            3 * ms,
            "motor",
            record(vec![
                ("speed_rpm", Data::Uint(1500)),
                ("enabled", Data::Bool(true)),
                ("state", Data::from("running")),
            ]),
        ),
        value(3 * ms, "offset", Data::Int(-42)),
        value(3 * ms, "ratio", Data::Float(0.1)),
        log(3 * ms, "robot/arm", 2, "started"),
        log(3 * ms, "", 3, "no path"),
        log(3 * ms, "", 4, "overrun 3"),
        value(
            3 * ms,
            "adc/samples",
            Data::Array(vec![
                Data::Uint(0),
                Data::Uint(1023),
                Data::Uint(2048),
                Data::Uint(4095),
            ]),
        ),
        Message::TraceMetadata(TraceMetadata {
            path: "motor/state".to_owned(),
            description: "Drive state".to_owned(),
            labels: [(-1, "fault"), (0, "off"), (1, "on")]
                .into_iter()
                .map(|(number, label)| (number, label.to_owned()))
                .collect(),
            ..Default::default()
        }),
        Message::TraceMetadata(TraceMetadata {
            path: "motor/current".to_owned(),
            unit: "A".to_owned(),
            range: Some(ExpectedRange {
                min: -2.5,
                max: 10.0,
            }),
            ..Default::default()
        }),
        Message::Samples(Samples {
            path: "adc/raw".to_owned(),
            values: vec![Data::Uint(512), Data::Uint(513), Data::Uint(511)],
            times: SampleTimes::Period {
                timestamp: 4 * ms,
                period: 250_000,
            },
        }),
        Message::Samples(Samples {
            path: "adc/peak".to_owned(),
            values: vec![Data::Float(1.5), Data::Null],
            times: SampleTimes::Each(vec![4 * ms, 4 * ms + 500_000]),
        }),
    ];
    let decoded: Vec<Message> = messages().into_iter().map(|(message, _)| message).collect();
    assert_eq!(decoded, expected);
}

#[test]
fn sonde_encodes_as_serde_does() {
    for (message, bytes) in messages() {
        assert_eq!(
            serde_cbor_2::to_vec(&message).unwrap(),
            bytes,
            "{message:?}"
        );
    }
}
