use super::*;

fn encode(data: &Data) -> Vec<u8> {
    serde_cbor_2::to_vec(data).unwrap()
}

fn decode(bytes: &[u8]) -> Result<Data, serde_cbor_2::Error> {
    serde_cbor_2::from_slice(bytes)
}

#[test]
fn scalars_are_plain_cbor_items() {
    assert_eq!(encode(&Data::Null), [0xf6]);
    assert_eq!(encode(&Data::Uint(5)), [0x05]);
    assert_eq!(encode(&Data::Int(-5)), [0x24]);
    assert_eq!(encode(&Data::Bool(true)), [0xf5]);
    assert_eq!(encode(&Data::from("ab")), [0x62, b'a', b'b']);
    // floats take the shortest width that keeps the value
    assert_eq!(encode(&Data::Float(1.5)), [0xf9, 0x3e, 0x00]);
    assert_eq!(encode(&Data::Float(0.1)).len(), 9);
}

#[test]
fn composites_are_plain_arrays_and_maps() {
    let record = Data::Record(vec![
        ("x".to_owned(), Data::Uint(1)),
        (
            "y".to_owned(),
            Data::Array(vec![Data::Bool(false), Data::Null]),
        ),
    ]);
    assert_eq!(
        encode(&record),
        [0xa2, 0x61, b'x', 0x01, 0x61, b'y', 0x82, 0xf4, 0xf6]
    );
}

#[test]
fn round_trips() {
    let values = [
        Data::Null,
        Data::Uint(u64::MAX),
        Data::Int(i64::MIN),
        Data::Float(-0.1),
        Data::Bool(false),
        Data::from("text"),
        Data::Array(vec![Data::Array(vec![Data::Float(1.0), Data::Float(2.0)])]),
        Data::Record(vec![
            ("b".to_owned(), Data::Uint(2)),
            (
                "a".to_owned(),
                Data::Record(vec![("c".to_owned(), Data::Null)]),
            ),
        ]),
    ];
    for value in values {
        assert_eq!(decode(&encode(&value)).unwrap(), value);
    }
}

#[test]
fn integers_are_unsigned_unless_negative() {
    assert_eq!(decode(&encode(&Data::Int(7))).unwrap(), Data::Uint(7));
    assert_eq!(decode(&encode(&Data::Int(-7))).unwrap(), Data::Int(-7));
}

#[test]
fn rejects_unsupported_items() {
    // -2^64, below i64::MIN
    assert!(decode(&[0x3b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).is_err());
    // byte string
    assert!(decode(&[0x41, 0x00]).is_err());
    // map with an integer key
    assert!(decode(&[0xa1, 0x01, 0x01]).is_err());
    // duplicate field
    assert!(decode(&[0xa2, 0x61, b'x', 0x01, 0x61, b'x', 0x02]).is_err());
}

#[test]
fn value_message_is_a_plain_cbor_map() {
    use serde_cbor_2::Value as Cbor;
    let map = |entries: Vec<(&str, Cbor)>| {
        Cbor::Map(
            entries
                .into_iter()
                .map(|(key, value)| (Cbor::Text(key.to_owned()), value))
                .collect(),
        )
    };
    let message = crate::Message::Sample(crate::Sample {
        timestamp: 1,
        path: "imu".to_owned(),
        value: Data::Record(vec![("x".to_owned(), Data::Float(0.5))]),
    });
    let bytes = serde_cbor_2::to_vec(&message).unwrap();
    let expected = map(vec![(
        "Sample",
        map(vec![
            ("timestamp", Cbor::Integer(1)),
            ("path", Cbor::Text("imu".to_owned())),
            ("value", map(vec![("x", Cbor::Float(0.5))])),
        ]),
    )]);
    assert_eq!(serde_cbor_2::from_slice::<Cbor>(&bytes).unwrap(), expected);
    assert_eq!(
        serde_cbor_2::from_slice::<crate::Message>(&bytes).unwrap(),
        message
    );
}

#[test]
fn decodes_values_of_the_python_outpost() {
    let value = |hex: &str| {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        match serde_cbor_2::from_slice(&bytes).unwrap() {
            crate::Message::Sample(value) => value.value,
            message => panic!("not a value: {message:?}"),
        }
    };
    // blip_outpost.proto.Message.create_value(...).encode_cbor() of cbor2 5.x
    let motor = "a16556616c7565a36974696d657374616d70016470617468656d6f746f726576616c7565a3\
                 6973706565645f72706d1905dc67656e61626c6564f56573746174656772756e6e696e67";
    assert_eq!(
        value(motor),
        Data::Record(vec![
            ("speed_rpm".to_owned(), Data::Uint(1500)),
            ("enabled".to_owned(), Data::Bool(true)),
            ("state".to_owned(), Data::from("running")),
        ])
    );
    let imu = "a16556616c7565a36974696d657374616d7002647061746863696d756576616c7565a16361\
               636383fb3fe0000000000000fbbff0000000000000fb40239eb851eb851f";
    assert_eq!(
        value(imu),
        Data::Record(vec![(
            "acc".to_owned(),
            Data::Array(vec![Data::Float(0.5), Data::Float(-1.0), Data::Float(9.81)])
        )])
    );
    let temperature = "a16556616c7565a36974696d657374616d700364706174687273656e736f722f74656d\
                       70657261747572656576616c7565f6";
    assert_eq!(value(temperature), Data::Null);
}
