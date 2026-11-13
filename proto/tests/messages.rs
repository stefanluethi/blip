//! The messages of the protocol: their names and fields on the wire.

use proto::{Clock, ExpectedRange, Hello, Message, Sample, TraceMetadata, level};
use serde::Serialize;
use std::collections::BTreeMap;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn encode(message: &Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).unwrap()
}

fn decode(bytes: &[u8]) -> Message {
    serde_cbor_2::from_slice(bytes).unwrap()
}

#[derive(Serialize)]
struct OldMetadata {
    path: &'static str,
    unit: &'static str,
}

/// The names of the messages as older targets sent them.
#[derive(Serialize)]
enum Old {
    Value(Sample),
    ValueMetadata(OldMetadata),
}

fn labels() -> BTreeMap<i64, String> {
    [(-1, "fault"), (0, "off"), (1, "on")]
        .into_iter()
        .map(|(number, label)| (number, label.to_owned()))
        .collect()
}

#[test]
fn samples_and_metadata_have_the_new_names() {
    let metadata = Message::TraceMetadata(TraceMetadata {
        path: "a".to_owned(),
        unit: "%".to_owned(),
        ..Default::default()
    });
    assert_eq!(
        hex(&encode(&metadata)),
        "a16d54726163654d65746164617461a26470617468616164756e69746125"
    );
    let sample = Sample {
        timestamp: 1,
        path: "a".to_owned(),
        value: proto::Data::Uint(2),
    };
    let bytes = encode(&Message::Sample(sample.clone()));
    assert_eq!(hex(&bytes[..8]), "a16653616d706c65");

    let old = serde_cbor_2::to_vec(&Old::Value(sample.clone())).unwrap();
    assert_eq!(decode(&old), Message::Sample(sample));
    let old = serde_cbor_2::to_vec(&Old::ValueMetadata(OldMetadata {
        path: "a",
        unit: "%",
    }))
    .unwrap();
    assert_eq!(decode(&old), metadata);
}

#[test]
fn trace_metadata_round_trips() {
    let full = Message::TraceMetadata(TraceMetadata {
        path: "motor/state".to_owned(),
        unit: String::new(),
        description: "Drive state".to_owned(),
        labels: labels(),
        range: Some(ExpectedRange {
            min: -1.0,
            max: 1.0,
        }),
    });
    assert_eq!(decode(&encode(&full)), full);

    #[derive(Serialize)]
    enum PathOnly {
        TraceMetadata { path: &'static str },
    }
    let bytes = serde_cbor_2::to_vec(&PathOnly::TraceMetadata { path: "a" }).unwrap();
    assert_eq!(
        decode(&bytes),
        Message::TraceMetadata(TraceMetadata {
            path: "a".to_owned(),
            ..Default::default()
        })
    );

    let current = Message::TraceMetadata(TraceMetadata {
        path: "motor/current".to_owned(),
        unit: "A".to_owned(),
        range: Some(ExpectedRange {
            min: -2.5,
            max: 10.0,
        }),
        ..Default::default()
    });
    let bytes = encode(&current);
    assert_eq!(bytes.len(), 63);
    assert_eq!(
        hex(&bytes),
        "a16d54726163654d65746164617461a364706174686d6d6f746f722f63757272656e7464756e697461416572616e6765a2636d696ef9c100636d6178f94900"
    );
    assert_eq!(decode(&bytes), current);

    let state = Message::TraceMetadata(TraceMetadata {
        path: "motor/state".to_owned(),
        description: "Drive state".to_owned(),
        labels: labels(),
        ..Default::default()
    });
    let bytes = encode(&state);
    assert_eq!(bytes.len(), 79);
    assert_eq!(decode(&bytes), state);
}

#[test]
fn integer_range_ends_widen() {
    #[derive(Serialize)]
    struct Range {
        min: u8,
        max: u8,
    }
    #[derive(Serialize)]
    enum Wire {
        TraceMetadata { path: &'static str, range: Range },
    }
    let bytes = serde_cbor_2::to_vec(&Wire::TraceMetadata {
        path: "a",
        range: Range { min: 0, max: 5 },
    })
    .unwrap();
    let Message::TraceMetadata(metadata) = decode(&bytes) else {
        panic!("not metadata");
    };
    assert_eq!(metadata.range, Some(ExpectedRange { min: 0.0, max: 5.0 }));
}

#[test]
fn a_label_key_out_of_range_fails() {
    #[derive(Serialize)]
    struct Metadata {
        path: &'static str,
        #[serde(rename = "enum")]
        labels: BTreeMap<u64, &'static str>,
    }
    #[derive(Serialize)]
    enum Wire {
        TraceMetadata(Metadata),
    }
    let bytes = serde_cbor_2::to_vec(&Wire::TraceMetadata(Metadata {
        path: "a",
        labels: BTreeMap::from([(u64::MAX, "big")]),
    }))
    .unwrap();
    assert!(serde_cbor_2::from_slice::<Message>(&bytes).is_err());
}

#[test]
fn hello_round_trips() {
    let hello = Hello {
        version: 1,
        name: Some("rig".to_owned()),
        clock: Some(Clock::Boot),
        sender: Some("sonde 1.2".to_owned()),
    };
    let bytes = encode(&Message::Hello(hello.clone()));
    assert_eq!(
        hex(&bytes),
        "a16548656c6c6fa46776657273696f6e01646e616d656372696765636c6f636b64626f6f746673656e64657269736f6e646520312e32"
    );
    assert_eq!(decode(&bytes), Message::Hello(hello.clone()));

    let epoch = Hello {
        clock: Some(Clock::Epoch),
        ..hello
    };
    assert_eq!(
        decode(&encode(&Message::Hello(epoch.clone()))),
        Message::Hello(epoch)
    );

    let bare = Hello {
        version: 1,
        ..Default::default()
    };
    let bytes = encode(&Message::Hello(bare.clone()));
    assert_eq!(hex(&bytes), "a16548656c6c6fa16776657273696f6e01");
    assert_eq!(decode(&bytes), Message::Hello(bare));
}

#[test]
fn a_hello_with_sorted_keys_decodes() {
    #[derive(Serialize)]
    struct Sorted {
        clock: &'static str,
        name: &'static str,
        sender: &'static str,
        version: u32,
    }
    #[derive(Serialize)]
    enum Wire {
        Hello(Sorted),
    }
    let bytes = serde_cbor_2::to_vec(&Wire::Hello(Sorted {
        clock: "epoch",
        name: "rig",
        sender: "sonde 1.2",
        version: 1,
    }))
    .unwrap();
    assert_eq!(
        decode(&bytes),
        Message::Hello(Hello {
            version: 1,
            name: Some("rig".to_owned()),
            clock: Some(Clock::Epoch),
            sender: Some("sonde 1.2".to_owned()),
        })
    );
}

#[test]
fn an_unknown_clock_is_none_and_a_missing_version_fails() {
    #[derive(Serialize)]
    struct Newer {
        clock: &'static str,
        version: u32,
    }
    #[derive(Serialize)]
    enum Wire {
        Hello(Newer),
    }
    let bytes = serde_cbor_2::to_vec(&Wire::Hello(Newer {
        clock: "gps",
        version: 2,
    }))
    .unwrap();
    assert_eq!(
        decode(&bytes),
        Message::Hello(Hello {
            version: 2,
            ..Default::default()
        })
    );

    #[derive(Serialize)]
    struct NoVersion {
        name: &'static str,
    }
    #[derive(Serialize)]
    enum Missing {
        Hello(NoVersion),
    }
    let bytes = serde_cbor_2::to_vec(&Missing::Hello(NoVersion { name: "rig" })).unwrap();
    assert!(serde_cbor_2::from_slice::<Message>(&bytes).is_err());
}

#[test]
fn the_version_and_levels() {
    assert_eq!(proto::VERSION, 1);
    assert_eq!(
        [
            level::TRACE,
            level::DEBUG,
            level::INFO,
            level::WARN,
            level::ERROR
        ],
        [0, 1, 2, 3, 4]
    );
}
