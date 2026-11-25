use super::*;
use crate::store::protocol::Parser;
use crate::store::said::Said;
use crate::store::value::{Value, record};
use proto::Data;

fn row(values: &[f64]) -> Data {
    Data::Array(values.iter().map(|&v| Data::Float(v)).collect())
}

fn fields(fields: Vec<(&str, Data)>) -> Data {
    Data::Record(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

/// The values of `values_are_flattened_into_their_leaves` and `arrays_keep_their_shape`
/// in `protocol/tests.rs`.
fn cases() -> Vec<Data> {
    vec![
        fields(vec![
            ("x", Data::Float(0.5)),
            (
                "acc",
                fields(vec![("z", Data::Int(-3)), ("ok", Data::Bool(true))]),
            ),
            ("name", Data::from("on")),
            ("gone", Data::Null),
        ]),
        Data::Uint(u64::MAX),
        Data::Int(i64::MIN),
        Data::Array(vec![row(&[1.0, 2.0]), Data::Null, row(&[3.0, 4.0])]),
        Data::Array(vec![]),
        Data::Array(vec![row(&[1.0]), row(&[1.0, 2.0])]),
        Data::Array(vec![Data::from("a")]),
        Data::Array(vec![Data::Null, row(&[1.0])]),
        fields(vec![
            ("a", Data::Array(vec![Data::from("a")])),
            ("b", Data::Uint(1)),
        ]),
        Data::Array(vec![fields(vec![("a", Data::Uint(1))])]),
        Data::Array(vec![row(&[1.0]), Data::Array(vec![row(&[1.0])])]),
    ]
}

#[test]
fn typed_values_flatten_as_parsed_ones() {
    for data in cases() {
        let bytes = serde_cbor_2::to_vec(&proto::Message::Sample(proto::Sample {
            timestamp: 1,
            path: "p".to_owned(),
            value: data.clone(),
        }))
        .unwrap();
        let (mut from_bytes, mut from_data) = (Flat::default(), Flat::default());
        let mut parsed = 0;
        Parser::default().item(&bytes, true, |said| {
            assert!(matches!(said, Said::Sample { .. }));
            if let Said::Sample { value, .. } = said {
                from_bytes.fill(value);
            }
            parsed += 1;
        });
        assert_eq!(parsed, 1);
        from_data.fill(Value::from(&data).as_ref());
        assert_eq!(from_data.leaves, from_bytes.leaves, "{data:?}");
        assert_eq!(from_data.fields, from_bytes.fields, "{data:?}");
        assert_eq!(from_data.numbers, from_bytes.numbers, "{data:?}");
        assert_eq!(from_data.shapes, from_bytes.shapes, "{data:?}");
    }
}

#[test]
fn a_record_with_a_repeated_field_is_invalid() {
    let mut value = Value::default();
    value.set(record(|r| {
        r.field("x", 1u64);
        r.field("x", 2u64);
    }));
    let mut flat = Flat::default();
    flat.fill(value.as_ref());
    assert_eq!(
        flat.leaves,
        [(0..0, Leaf::Invalid("duplicate record field `x`".to_owned()))]
    );
    assert!(!flat.is_record());
}

#[test]
fn texts_are_ranges_into_the_texts_of_the_value() {
    let mut value = Value::default();
    value.set(record(|r| {
        r.field("a", "x");
        r.field("b", "yz");
    }));
    assert_eq!(value.texts(), "axbyz");
    let mut flat = Flat::default();
    flat.fill(value.as_ref());
    let leaves: Vec<_> = (flat.leaves.iter())
        .map(|(field, leaf)| (flat.field(field), leaf.clone()))
        .collect();
    assert_eq!(leaves, [(".a", Leaf::Text(1..2)), (".b", Leaf::Text(3..5))]);
}
