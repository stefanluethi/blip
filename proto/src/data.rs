use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The value of a stream sample, encoded as a plain CBOR data item without any type
/// tag, so each variant maps to one CBOR major type.
///
/// Integers are a single kind on the wire, their sign decides the CBOR major type:
/// decoding yields [`Data::Uint`] for non-negative and [`Data::Int`] for negative
/// integers.
#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    /// No value, e.g. a sample that could not be measured.
    Null,
    Uint(u64),
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    /// The elements of an array, with the same shape in every sample of a stream.
    Array(Vec<Data>),
    /// The fields of a record in their order on the wire.
    Record(Vec<(String, Data)>),
}

impl From<u64> for Data {
    fn from(value: u64) -> Self {
        Self::Uint(value)
    }
}

impl From<i64> for Data {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for Data {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for Data {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Data {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl Serialize for Data {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_none(),
            Self::Uint(value) => serializer.serialize_u64(*value),
            Self::Int(value) => serializer.serialize_i64(*value),
            Self::Float(value) => serializer.serialize_f64(*value),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(elements) => {
                let mut seq = serializer.serialize_seq(Some(elements.len()))?;
                for element in elements {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            Self::Record(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (name, value) in fields {
                    map.serialize_entry(name, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Data {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(DataVisitor)
    }
}

struct DataVisitor;

impl<'de> Visitor<'de> for DataVisitor {
    type Value = Data;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("null, an integer, a float, a bool, a string, an array or a record")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Data, E> {
        Ok(Data::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Data, E> {
        Ok(Data::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Data, D::Error> {
        Data::deserialize(deserializer)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Data, E> {
        Ok(Data::Bool(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Data, E> {
        Ok(Data::Uint(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Data, E> {
        Ok(u64::try_from(value).map_or(Data::Int(value), Data::Uint))
    }

    fn visit_i128<E: de::Error>(self, value: i128) -> Result<Data, E> {
        // CBOR negative integers reach down to -2^64
        match i64::try_from(value) {
            Ok(value) => self.visit_i64(value),
            Err(_) => Err(E::invalid_value(
                de::Unexpected::Other("integer below i64::MIN"),
                &self,
            )),
        }
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Data, E> {
        Ok(Data::Float(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Data, E> {
        Ok(Data::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Data, E> {
        Ok(Data::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Data, A::Error> {
        let mut elements = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(element) = seq.next_element()? {
            elements.push(element);
        }
        Ok(Data::Array(elements))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Data, A::Error> {
        let mut fields: Vec<(String, Data)> = Vec::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((name, value)) = map.next_entry::<String, Data>()? {
            if fields.iter().any(|(existing, _)| *existing == name) {
                return Err(de::Error::custom(format_args!(
                    "duplicate record field `{name}`"
                )));
            }
            fields.push((name, value));
        }
        Ok(Data::Record(fields))
    }
}

#[cfg(test)]
mod tests {
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
        let message = crate::Message::Value(crate::Value {
            timestamp: 1,
            path: "imu".to_owned(),
            value: Data::Record(vec![("x".to_owned(), Data::Float(0.5))]),
        });
        let bytes = serde_cbor_2::to_vec(&message).unwrap();
        let expected = map(vec![(
            "Value",
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
    fn decodes_values_of_the_python_bridge() {
        let value = |hex: &str| {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            match serde_cbor_2::from_slice(&bytes).unwrap() {
                crate::Message::Value(value) => value.value,
                message => panic!("not a value: {message:?}"),
            }
        };
        // bridge.proto.Message.create_value(...).encode_cbor() of cbor2 5.x
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
}
