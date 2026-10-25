//! Decodes the messages blip receives. Values and log records, which arrive all the
//! time, are read in one pass without allocating: a value is flattened into its leaves,
//! the fields of records and their arrays of numbers. The other messages are decoded
//! with serde.

use super::cbor::{self, Head, Reader};
use super::numbers::Number;
use std::ops::Range;

/// A message, with the strings borrowed from its bytes. The value of a
/// [`Message::Value`] is in the [`Flat`] passed to [`decode`].
#[derive(Debug, PartialEq)]
pub enum Message<'b> {
    Value {
        timestamp: u64,
        path: &'b str,
    },
    Log {
        timestamp: u64,
        level: u8,
        component: &'b str,
        message: &'b str,
    },
    Other(proto::Message),
}

/// The value of a leaf of a value.
#[derive(Clone, Debug, PartialEq)]
pub enum Leaf {
    Null,
    Number(Number),
    /// The bytes of a text in the message.
    Text(Range<usize>),
    /// An array of numbers, its shape and its elements in row-major order in
    /// [`Flat::shapes`] and [`Flat::numbers`].
    Array {
        shape: Range<usize>,
        elements: Range<usize>,
    },
    /// A value no trace can hold, and why.
    Invalid(String),
}

/// A value flattened into its leaves, kept to be filled again by the next value.
#[derive(Default, Debug)]
pub struct Flat {
    /// The leaves with the fields leading to them, e.g. `.acc.z`, empty for a value
    /// that is no record.
    pub leaves: Vec<(Range<usize>, Leaf)>,
    /// The texts of the fields leading to the leaves.
    pub fields: String,
    pub numbers: Vec<Number>,
    pub shapes: Vec<usize>,
    /// The fields of the records being read, to refuse duplicates.
    keys: Vec<Range<usize>>,
    /// The fields leading to the value being read.
    stack: String,
}

impl Flat {
    fn clear(&mut self) {
        self.leaves.clear();
        self.fields.clear();
        self.numbers.clear();
        self.shapes.clear();
        self.keys.clear();
        self.stack.clear();
    }

    /// Whether the value is a record, which has fields.
    pub fn is_record(&self) -> bool {
        self.leaves
            .first()
            .is_none_or(|(field, _)| !field.is_empty())
    }

    /// The fields leading to a leaf, e.g. `.acc.z`.
    pub fn field(&self, field: &Range<usize>) -> &str {
        &self.fields[field.clone()]
    }

    fn leaf(&mut self, leaf: Leaf) {
        let start = self.fields.len();
        self.fields.push_str(&self.stack);
        self.leaves.push((start..self.fields.len(), leaf));
    }

    /// Reads a value at `reader` into leaves.
    fn value(&mut self, reader: &mut Reader<'_>, frame: &[u8]) -> cbor::Result<()> {
        let head = reader.head()?;
        match head {
            Head::Map(len) => {
                let Some(len) = len else {
                    return Err(error(reader, "indefinite map"));
                };
                let (keys, depth) = (self.keys.len(), self.stack.len());
                for _ in 0..len {
                    let name = reader.str()?;
                    // the bytes of the name, which end where the reader is
                    let name_range = reader.position() - name.len()..reader.position();
                    if self.keys[keys..]
                        .iter()
                        .any(|key| frame[key.clone()] == *name.as_bytes())
                    {
                        return Err(error(reader, "duplicate record field"));
                    }
                    self.keys.push(name_range);
                    self.stack.push('.');
                    self.stack.push_str(name);
                    self.value(reader, frame)?;
                    self.stack.truncate(depth);
                }
                self.keys.truncate(keys);
            }
            Head::Array(Some(len)) => {
                let (shapes, numbers) = (self.shapes.len(), self.numbers.len());
                let mut dimensions = None;
                let leaf = match self.elements(reader, len, 0, shapes, &mut dimensions)? {
                    Some(problem) => Leaf::Invalid(problem),
                    None => Leaf::Array {
                        shape: shapes..self.shapes.len(),
                        elements: numbers..self.numbers.len(),
                    },
                };
                self.leaf(leaf);
            }
            Head::Text(len) => {
                let text = reader.text(len)?;
                let end = reader.position();
                self.leaf(Leaf::Text(end - text.len()..end));
            }
            Head::Null | Head::Undefined => self.leaf(Leaf::Null),
            head => match number(head) {
                Some(number) => self.leaf(Leaf::Number(number)),
                None => return Err(error(reader, "a value cannot be of this CBOR type")),
            },
        }
        Ok(())
    }

    /// Reads `len` elements of an array at `depth`, whose shape starts at `base` in
    /// [`Self::shapes`]. `dimensions` is the number of dimensions once known. Returns
    /// why the array cannot be held, if it cannot.
    fn elements(
        &mut self,
        reader: &mut Reader<'_>,
        len: u64,
        depth: usize,
        base: usize,
        dimensions: &mut Option<usize>,
    ) -> cbor::Result<Option<String>> {
        let level = base + depth;
        let len = len as usize;
        let mut problem = None;
        if level == self.shapes.len() {
            self.shapes.push(len);
        } else if self.shapes[level] != len {
            problem = Some(format!(
                "expected an array of {} elements, got {len}",
                self.shapes[level]
            ));
        }
        for index in 0..len {
            if problem.is_some() {
                for _ in index..len {
                    reader.skip()?;
                }
                break;
            }
            let head = reader.head()?;
            problem = match head {
                Head::Array(Some(inner)) if dimensions.is_none_or(|count| depth + 1 < count) => {
                    self.elements(reader, inner, depth + 1, base, dimensions)?
                }
                Head::Null | Head::Undefined => {
                    // a null element stands for all elements below it
                    let count = match dimensions {
                        Some(_) => self.shapes[level + 1..].iter().product(),
                        None => {
                            *dimensions = Some(depth + 1);
                            1
                        }
                    };
                    self.numbers
                        .extend(std::iter::repeat_n(Number::Null, count));
                    None
                }
                head => match number(head) {
                    Some(number) if dimensions.is_none_or(|count| count == depth + 1) => {
                        *dimensions = Some(depth + 1);
                        self.numbers.push(number);
                        None
                    }
                    Some(number) => Some(format!("expected an array, got {}", number.kind_name())),
                    None => {
                        reader.skip_rest(head)?;
                        Some(format!(
                            "array elements must be numbers or bools, got {}",
                            kind_name(head)
                        ))
                    }
                },
            };
        }
        Ok(problem)
    }
}

fn error(reader: &Reader<'_>, what: &'static str) -> cbor::Error {
    cbor::Error {
        at: reader.position(),
        what,
    }
}

/// The number, bool or null of `head`, if it is one.
fn number(head: Head) -> Option<Number> {
    Some(match head {
        Head::Uint(value) => Number::Uint(value),
        Head::Negative(value) => Number::Int(-1 - i64::try_from(value).ok()?),
        Head::Float(value) => Number::Float(value),
        Head::Bool(value) => Number::Bool(value),
        _ => return None,
    })
}

/// The kind of value of `head`, as named in warnings.
fn kind_name(head: Head) -> &'static str {
    match head {
        Head::Uint(_) | Head::Negative(_) => "int",
        Head::Float(_) => "float",
        Head::Bool(_) => "bool",
        Head::Text(_) => "string",
        Head::Array(_) => "array",
        Head::Map(_) => "record",
        _ => "an unsupported CBOR item",
    }
}

/// Whether `frame` is a value or a log record, read from its start only.
pub fn is_sample(frame: &[u8]) -> bool {
    let mut reader = Reader::new(frame);
    reader.head() == Ok(Head::Map(Some(1))) && matches!(reader.str(), Ok("Value" | "Log"))
}

/// Decodes the message `frame`, the value of a [`Message::Value`] into `flat`.
pub fn decode<'b>(frame: &'b [u8], flat: &mut Flat) -> Result<Message<'b>, String> {
    let mut reader = Reader::new(frame);
    let fast = (|| -> cbor::Result<Option<Message<'b>>> {
        if reader.head()? != Head::Map(Some(1)) {
            return Ok(None);
        }
        Ok(match reader.str()? {
            "Value" => Some(value(&mut reader, frame, flat)?),
            "Log" => Some(log(&mut reader)?),
            _ => None,
        })
    })();
    match fast {
        Ok(Some(message)) => Ok(message),
        Ok(None) => serde_cbor_2::from_slice(frame)
            .map(Message::Other)
            .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

/// The fields of a map: calls `field` with each name until it returns an error.
fn fields<'b>(
    reader: &mut Reader<'b>,
    mut field: impl FnMut(&mut Reader<'b>, &'b str) -> cbor::Result<()>,
) -> cbor::Result<()> {
    let Head::Map(Some(len)) = reader.head()? else {
        return Err(error(reader, "expected a map"));
    };
    for _ in 0..len {
        let name = reader.str()?;
        field(reader, name)?;
    }
    Ok(())
}

fn value<'b>(
    reader: &mut Reader<'b>,
    frame: &'b [u8],
    flat: &mut Flat,
) -> cbor::Result<Message<'b>> {
    flat.clear();
    let (mut timestamp, mut path, mut has_value) = (None, None, false);
    fields(reader, |reader, name| {
        match name {
            "timestamp" => timestamp = Some(reader.u64()?),
            "path" => path = Some(reader.str()?),
            "value" => {
                flat.value(reader, frame)?;
                has_value = true;
            }
            _ => reader.skip()?,
        }
        Ok(())
    })?;
    match (timestamp, path, has_value) {
        (Some(timestamp), Some(path), true) => Ok(Message::Value { timestamp, path }),
        _ => Err(error(reader, "a Value lacks a field")),
    }
}

fn log<'b>(reader: &mut Reader<'b>) -> cbor::Result<Message<'b>> {
    let (mut timestamp, mut level, mut component, mut message) = (None, None, "", None);
    fields(reader, |reader, name| {
        match name {
            "timestamp" => timestamp = Some(reader.u64()?),
            "level" => {
                let value = reader.u64()?;
                level = Some(u8::try_from(value).map_err(|_| error(reader, "level above 255"))?);
            }
            "component" => component = reader.str()?,
            "message" => message = Some(reader.str()?),
            _ => reader.skip()?,
        }
        Ok(())
    })?;
    match (timestamp, level, message) {
        (Some(timestamp), Some(level), Some(message)) => Ok(Message::Log {
            timestamp,
            level,
            component,
            message,
        }),
        _ => Err(error(reader, "a Log lacks a field")),
    }
}

#[cfg(test)]
mod tests {
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
            level: 4,
            component: "motor".to_owned(),
            message: "hot".to_owned(),
        }));
        let mut flat = Flat::default();
        assert_eq!(
            decode(&log, &mut flat).unwrap(),
            Message::Log {
                timestamp: 3,
                level: 4,
                component: "motor",
                message: "hot"
            }
        );
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
}
