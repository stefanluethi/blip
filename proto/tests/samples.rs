//! Blocks of samples: their two forms on the wire and the checks when decoding.

use proto::{Data, Message, Sample, SampleTimes, Samples};
use serde::Serialize;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn round_trip(samples: Samples) -> String {
    let message = Message::Samples(samples);
    let bytes = serde_cbor_2::to_vec(&message).unwrap();
    assert_eq!(
        serde_cbor_2::from_slice::<Message>(&bytes).unwrap(),
        message
    );
    hex(&bytes)
}

#[test]
fn blocks_round_trip_in_both_forms() {
    let period = Samples {
        path: "adc".to_owned(),
        values: vec![Data::Uint(512), Data::Uint(513), Data::Uint(511)],
        times: SampleTimes::Period {
            timestamp: 4_000_000,
            period: 250_000,
        },
    };
    assert_eq!(
        round_trip(period),
        "a16753616d706c6573a46470617468636164636676616c756573831902001902011901ff6974696d657374616d701a003d090066706572696f641a0003d090"
    );
    let each = Samples {
        path: "peak".to_owned(),
        values: vec![Data::Float(1.5), Data::Null],
        times: SampleTimes::Each(vec![4_000_000, 4_500_000]),
    };
    assert_eq!(
        round_trip(each),
        "a16753616d706c6573a36470617468647065616b6676616c75657382f93e00f66a74696d657374616d7073821a003d09001a0044aa20"
    );
    round_trip(Samples {
        path: "a".to_owned(),
        values: vec![],
        times: SampleTimes::Period {
            timestamp: 1,
            period: 2,
        },
    });
    round_trip(Samples {
        path: "a".to_owned(),
        values: vec![],
        times: SampleTimes::Each(vec![]),
    });
}

/// The fields of a block as a sender may write them, any left out.
#[derive(Serialize, Default)]
struct Fields {
    path: &'static str,
    values: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    period: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamps: Option<Vec<u64>>,
}

#[derive(Serialize)]
enum Wire {
    Samples(Fields),
}

fn error_of(fields: Fields) -> String {
    let bytes = serde_cbor_2::to_vec(&Wire::Samples(fields)).unwrap();
    serde_cbor_2::from_slice::<Message>(&bytes)
        .unwrap_err()
        .to_string()
}

fn fields() -> Fields {
    Fields {
        path: "a",
        values: vec![1, 2],
        ..Default::default()
    }
}

#[test]
fn malformed_blocks_are_refused() {
    let both = Fields {
        timestamp: Some(1),
        period: Some(2),
        timestamps: Some(vec![1, 2]),
        ..fields()
    };
    assert!(error_of(both).contains("both a period and timestamps"));
    assert!(error_of(fields()).contains("neither a period nor timestamps"));
    let timestamp = Fields {
        timestamp: Some(1),
        ..fields()
    };
    assert!(error_of(timestamp).contains("a timestamp but no period"));
    let period = Fields {
        period: Some(1),
        ..fields()
    };
    assert!(error_of(period).contains("a period but no timestamp"));
    let lengths = Fields {
        timestamps: Some(vec![1, 2, 3]),
        ..fields()
    };
    assert!(error_of(lengths).contains("2 values but 3 timestamps"));
    let beyond = Fields {
        timestamp: Some(u64::MAX - 5),
        period: Some(10),
        ..fields()
    };
    assert!(error_of(beyond).contains("beyond 2^64 ns"));

    let one = Fields {
        timestamp: Some(u64::MAX - 5),
        period: Some(10),
        values: vec![1],
        ..fields()
    };
    let bytes = serde_cbor_2::to_vec(&Wire::Samples(one)).unwrap();
    assert!(serde_cbor_2::from_slice::<Message>(&bytes).is_ok());
}

#[test]
fn times_of_the_samples() {
    let times = SampleTimes::Period {
        timestamp: 1000,
        period: 10,
    };
    assert_eq!(times.timestamp(0), Some(1000));
    assert_eq!(times.timestamp(2), Some(1020));
    let block = Samples {
        path: "a".to_owned(),
        values: vec![Data::Uint(1), Data::Uint(2), Data::Uint(3)],
        times,
    };
    let sample = |timestamp, value| Sample {
        timestamp,
        path: "a".to_owned(),
        value: Data::Uint(value),
    };
    assert_eq!(
        block.to_samples(),
        vec![sample(1000, 1), sample(1010, 2), sample(1020, 3)]
    );
    let each = SampleTimes::Each(vec![5, 7]);
    assert_eq!(each.timestamp(1), Some(7));
    assert_eq!(each.timestamp(2), None);
}
