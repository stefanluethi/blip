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
        /// Empty if the record has none.
        path: &'b str,
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
        // more elements than fit in memory cannot be in the message
        let len = usize::try_from(len).map_err(|_| error(reader, "unexpected end"))?;
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

/// The path of the value in `frame` if it is a [`Message::Value`], read without
/// decoding the value.
pub fn value_path(frame: &[u8]) -> Option<&str> {
    let mut reader = Reader::new(frame);
    if reader.head().ok()? != Head::Map(Some(1)) || reader.str().ok()? != "Value" {
        return None;
    }
    let Head::Map(Some(len)) = reader.head().ok()? else {
        return None;
    };
    for _ in 0..len {
        match reader.str().ok()? {
            "path" => return reader.str().ok(),
            _ => reader.skip().ok()?,
        }
    }
    None
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
    let (mut timestamp, mut path, mut level, mut component, mut message) =
        (None, "", None, "", None);
    fields(reader, |reader, name| {
        match name {
            "timestamp" => timestamp = Some(reader.u64()?),
            "path" => path = reader.str()?,
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
            path,
            level,
            component,
            message,
        }),
        _ => Err(error(reader, "a Log lacks a field")),
    }
}

#[cfg(test)]
mod tests;
