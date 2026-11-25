//! A small CBOR reader and writer for what blip receives and records: it splits CBOR
//! sequences into their items, reads values without allocating, borrowing strings
//! from the bytes, and writes messages in the shortest form.

use super::value::{Node, ValueRef};
use std::fmt;

/// Why the bytes could not be read, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub at: usize,
    pub what: &'static str,
}

impl Error {
    /// Whether the bytes ended within an item, which more bytes may complete.
    pub fn is_end(&self) -> bool {
        self.what == END
    }
}

/// The [`Error::what`] of bytes that end within an item.
const END: &str = "unexpected end";

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.what, self.at)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The start of a CBOR data item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Head {
    Uint(u64),
    /// A negative integer, `-1 - n`.
    Negative(u64),
    Bytes(Option<u64>),
    Text(Option<u64>),
    Array(Option<u64>),
    Map(Option<u64>),
    Tag(u64),
    Bool(bool),
    Null,
    Undefined,
    Float(f64),
    Break,
    /// Another simple value.
    Simple(u8),
}

/// Reads the items of `bytes` one after another.
#[derive(Clone, Debug, PartialEq)]
pub struct Reader<'b> {
    bytes: &'b [u8],
    pos: usize,
}

impl<'b> Reader<'b> {
    pub fn new(bytes: &'b [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn error<T>(&self, what: &'static str) -> Result<T> {
        Err(Error { at: self.pos, what })
    }

    fn take(&mut self, len: usize) -> Result<&'b [u8]> {
        match self.bytes.get(self.pos..self.pos.saturating_add(len)) {
            Some(bytes) => {
                self.pos += len;
                Ok(bytes)
            }
            None => self.error(END),
        }
    }

    /// Takes the `len` bytes of a string, a length that cannot be in memory, e.g. above
    /// 4 GiB on 32 bit platforms, ending the bytes too.
    fn take_string(&mut self, len: u64) -> Result<&'b [u8]> {
        match usize::try_from(len) {
            Ok(len) => self.take(len),
            Err(_) => self.error(END),
        }
    }

    fn uint(&mut self, len: usize) -> Result<u64> {
        let bytes = self.take(len)?;
        Ok(bytes
            .iter()
            .fold(0, |value, &byte| value << 8 | u64::from(byte)))
    }

    /// The next byte without reading it.
    pub fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// Reads the head of the next item.
    pub fn head(&mut self) -> Result<Head> {
        let initial = self.take(1)?[0];
        let (major, info) = (initial >> 5, initial & 0x1f);
        let argument = match info {
            0..=23 => Some(u64::from(info)),
            24 => Some(self.uint(1)?),
            25 => Some(self.uint(2)?),
            26 => Some(self.uint(4)?),
            27 => Some(self.uint(8)?),
            31 => None,
            _ => return self.error("reserved additional information"),
        };
        let definite = |head: fn(u64) -> Head| match argument {
            Some(value) => Ok(head(value)),
            None => self.error("indefinite length not allowed here"),
        };
        Ok(match major {
            0 => definite(Head::Uint)?,
            1 => definite(Head::Negative)?,
            2 => Head::Bytes(argument),
            3 => Head::Text(argument),
            4 => Head::Array(argument),
            5 => Head::Map(argument),
            6 => definite(Head::Tag)?,
            _ => match (info, argument) {
                (20, _) => Head::Bool(false),
                (21, _) => Head::Bool(true),
                (22, _) => Head::Null,
                (23, _) => Head::Undefined,
                (25, Some(bits)) => Head::Float(f16_to_f64(bits as u16)),
                (26, Some(bits)) => Head::Float(f64::from(f32::from_bits(bits as u32))),
                (27, Some(bits)) => Head::Float(f64::from_bits(bits)),
                (31, None) => Head::Break,
                (_, Some(value)) => Head::Simple(value as u8),
                (_, None) => unreachable!("only info 31 has no argument"),
            },
        })
    }

    /// Reads the text of a text string whose head was read, `len` bytes long.
    pub fn text(&mut self, len: Option<u64>) -> Result<&'b str> {
        let Some(len) = len else {
            return self.error("indefinite text string");
        };
        let start = self.pos;
        let bytes = self.take_string(len)?;
        std::str::from_utf8(bytes).map_err(|_| Error {
            at: start,
            what: "invalid UTF-8",
        })
    }

    /// Reads a text string.
    pub fn str(&mut self) -> Result<&'b str> {
        match self.head()? {
            Head::Text(len) => self.text(len),
            _ => self.error("expected a text string"),
        }
    }

    /// Reads an unsigned integer.
    pub fn u64(&mut self) -> Result<u64> {
        match self.head()? {
            Head::Uint(value) => Ok(value),
            _ => self.error("expected an unsigned integer"),
        }
    }

    /// Skips the rest of an item whose head was read.
    pub fn skip_rest(&mut self, head: Head) -> Result<()> {
        let mut items = match head {
            Head::Bytes(Some(len)) | Head::Text(Some(len)) => {
                self.take_string(len)?;
                return Ok(());
            }
            Head::Array(Some(len)) => Some(len),
            Head::Map(Some(len)) => Some(len.saturating_mul(2)),
            Head::Tag(_) => Some(1),
            Head::Bytes(None) | Head::Text(None) | Head::Array(None) | Head::Map(None) => None,
            Head::Break => return self.error("unexpected break"),
            _ => return Ok(()),
        };
        loop {
            match &mut items {
                Some(0) => return Ok(()),
                Some(left) => *left -= 1,
                None => {
                    if self.peek() == Some(0xff) {
                        self.pos += 1;
                        return Ok(());
                    }
                }
            }
            let head = self.head()?;
            self.skip_rest(head)?;
        }
    }

    /// Skips the next item.
    pub fn skip(&mut self) -> Result<()> {
        let head = self.head()?;
        self.skip_rest(head)
    }
}

/// The value of an IEEE 754 half precision float.
fn f16_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from(bits >> 10 & 0x1f);
    let fraction = f64::from(bits & 0x3ff);
    sign * match exponent {
        0 => fraction * 2f64.powi(-24),
        31 if fraction == 0.0 => f64::INFINITY,
        31 => f64::NAN,
        _ => (1.0 + fraction / 1024.0) * 2f64.powi(exponent - 15),
    }
}

/// The items of the CBOR sequence (RFC 8742) `bytes`, each as its bytes, e.g. the
/// messages of a frame. Ends after the first item that cannot be read, with its error.
#[cfg(test)]
pub fn items(bytes: &[u8]) -> impl Iterator<Item = Result<&[u8]>> {
    let mut reader = Reader::new(bytes);
    let mut failed = false;
    std::iter::from_fn(move || {
        if failed || reader.is_empty() {
            return None;
        }
        let start = reader.position();
        match reader.skip() {
            Ok(()) => Some(Ok(&bytes[start..reader.position()])),
            Err(error) => {
                // the rest cannot be told apart into items
                failed = true;
                Some(Err(error))
            }
        }
    })
}

/// Writes CBOR into a buffer, in the shortest form that reads back as the same value,
/// without allocating beyond the buffer. For what blip records: the bytes it writes for
/// a message are those of serde_cbor_2 for the same message.
pub(crate) struct Writer<'o> {
    out: &'o mut Vec<u8>,
}

impl<'o> Writer<'o> {
    /// Writes after the bytes in `out`.
    pub(crate) fn new(out: &'o mut Vec<u8>) -> Self {
        Self { out }
    }

    /// The head of major type `major` with `value`, in as few bytes as it takes.
    fn head(&mut self, major: u8, value: u64) {
        let major = major << 5;
        match value {
            0..=23 => self.out.push(major | value as u8),
            24..=0xff => self.out.extend_from_slice(&[major | 24, value as u8]),
            0x100..=0xffff => {
                self.out.push(major | 25);
                self.out.extend_from_slice(&(value as u16).to_be_bytes());
            }
            0x1_0000..=0xffff_ffff => {
                self.out.push(major | 26);
                self.out.extend_from_slice(&(value as u32).to_be_bytes());
            }
            _ => {
                self.out.push(major | 27);
                self.out.extend_from_slice(&value.to_be_bytes());
            }
        }
    }

    pub(crate) fn uint(&mut self, v: u64) {
        self.head(0, v);
    }

    /// An integer, as an unsigned one if it is not negative.
    pub(crate) fn int(&mut self, v: i64) {
        match u64::try_from(v) {
            Ok(v) => self.head(0, v),
            Err(_) => self.head(1, !(v as u64)),
        }
    }

    /// A float in half, single or double precision, the first that holds it exactly.
    /// A NaN is written as the half precision one.
    pub(crate) fn float(&mut self, v: f64) {
        if v.is_nan() {
            self.out.extend_from_slice(&[0xf9, 0x7e, 0x00]);
        } else if let Some(half) = half(v) {
            self.out.push(0xf9);
            self.out.extend_from_slice(&half.to_be_bytes());
        } else if f64::from(v as f32).to_bits() == v.to_bits() {
            self.out.push(0xfa);
            self.out
                .extend_from_slice(&(v as f32).to_bits().to_be_bytes());
        } else {
            self.out.push(0xfb);
            self.out.extend_from_slice(&v.to_bits().to_be_bytes());
        }
    }

    pub(crate) fn bool(&mut self, v: bool) {
        self.out.push(if v { 0xf5 } else { 0xf4 });
    }

    pub(crate) fn null(&mut self) {
        self.out.push(0xf6);
    }

    pub(crate) fn text(&mut self, v: &str) {
        self.head(3, v.len() as u64);
        self.out.extend_from_slice(v.as_bytes());
    }

    /// The head of an array of `len` elements, which the caller writes next.
    pub(crate) fn array(&mut self, len: usize) {
        self.head(4, len as u64);
    }

    /// The head of a map of `len` entries, which the caller writes next, key and value.
    pub(crate) fn map(&mut self, len: usize) {
        self.head(5, len as u64);
    }

    /// A value of the tape: a record is a map, an array an array.
    pub(crate) fn value(&mut self, value: ValueRef<'_>) {
        match value.node() {
            Node::Null => self.null(),
            Node::Bool(v) => self.bool(v),
            Node::Uint(v) => self.uint(v),
            Node::Int(v) => self.int(v),
            Node::Float(v) => self.float(v),
            Node::Text(v) => self.text(v),
            Node::Array(elements) => {
                self.array(elements.size_hint().0);
                for element in elements {
                    self.value(element);
                }
            }
            Node::Record(fields) => {
                self.map(fields.size_hint().0);
                for (name, element) in fields {
                    self.text(name);
                    self.value(element);
                }
            }
        }
    }
}

/// The bits of `value` as a half precision float, if it is exactly one. A NaN is not.
fn half(value: f64) -> Option<u16> {
    let bits = value.to_bits();
    let sign = (bits >> 48) as u16 & 0x8000;
    if value == 0.0 {
        return Some(sign);
    }
    if value.is_infinite() {
        return Some(sign | 0x7c00);
    }
    if value.abs() < 2f64.powi(-14) {
        // a subnormal half: a whole multiple of 2^-24
        let m = value.abs() * 2f64.powi(24);
        return (m.fract() == 0.0 && m < 1024.0).then_some(sign | m as u16);
    }
    let exponent = (bits >> 52 & 0x7ff) as i32 - 1023;
    let fraction = bits & ((1 << 52) - 1);
    ((-14..=15).contains(&exponent) && fraction & ((1 << 42) - 1) == 0)
        .then(|| sign | ((exponent + 15) as u16) << 10 | (fraction >> 42) as u16)
}

#[cfg(test)]
mod tests;
