use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The value of a trace sample, encoded as a plain CBOR data item without any type
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
    /// The elements of an array, with the same shape in every sample of a trace.
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
mod tests;
