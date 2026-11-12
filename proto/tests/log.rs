//! Log records: their fields on the wire, and records of older targets that still have a
//! component.

use proto::{LogRecord, Message};
use serde::de::IgnoredAny;
use std::collections::BTreeMap;

/// A log record as targets sent it before records lost their component.
#[derive(serde::Serialize)]
struct OldLogRecord {
    timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'static str>,
    level: u8,
    component: &'static str,
    message: &'static str,
}

#[derive(serde::Serialize)]
enum OldMessage {
    Log(OldLogRecord),
}

fn record(path: &str) -> Message {
    Message::Log(LogRecord {
        timestamp: 3,
        path: path.to_owned(),
        level: 4,
        message: "hot".to_owned(),
    })
}

fn keys(message: &Message) -> Vec<String> {
    let bytes = serde_cbor_2::to_vec(message).unwrap();
    let decoded: BTreeMap<String, BTreeMap<String, IgnoredAny>> =
        serde_cbor_2::from_slice(&bytes).unwrap();
    decoded["Log"].keys().cloned().collect()
}

#[test]
fn log_records_round_trip() {
    for path in ["robot/arm", ""] {
        let bytes = serde_cbor_2::to_vec(&record(path)).unwrap();
        assert_eq!(
            serde_cbor_2::from_slice::<Message>(&bytes).unwrap(),
            record(path)
        );
    }
}

#[test]
fn an_empty_path_is_left_out() {
    assert_eq!(keys(&record("")), ["level", "message", "timestamp"]);
    assert_eq!(
        keys(&record("robot/arm")),
        ["level", "message", "path", "timestamp"]
    );
}

#[test]
fn a_component_of_an_old_record_is_ignored() {
    let old = |path, component| {
        let bytes = serde_cbor_2::to_vec(&OldMessage::Log(OldLogRecord {
            timestamp: 3,
            path,
            level: 4,
            component,
            message: "hot",
        }))
        .unwrap();
        serde_cbor_2::from_slice::<Message>(&bytes).unwrap()
    };
    assert_eq!(old(Some("robot/arm"), "motor"), record("robot/arm"));
    assert_eq!(old(None, "adc"), record(""));
}
