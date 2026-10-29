//! A small CBOR reader for what blip receives: it splits CBOR sequences into their
//! items and reads values without allocating, borrowing strings from the bytes.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn cbor(value: &impl serde::Serialize) -> Vec<u8> {
        serde_cbor_2::to_vec(value).unwrap()
    }

    #[test]
    fn heads_of_all_kinds_are_read() {
        let read = |bytes: &[u8]| Reader::new(bytes).head().unwrap();
        assert_eq!(read(&cbor(&5u8)), Head::Uint(5));
        assert_eq!(read(&cbor(&u64::MAX)), Head::Uint(u64::MAX));
        assert_eq!(read(&cbor(&-5i8)), Head::Negative(4));
        assert_eq!(read(&cbor(&1.5f64)), Head::Float(1.5));
        assert_eq!(read(&cbor(&0.1f32)), Head::Float(f64::from(0.1f32)));
        assert_eq!(read(&cbor(&0.1f64)), Head::Float(0.1));
        assert_eq!(read(&[0xf9, 0xc4, 0x00]), Head::Float(-4.0));
        assert_eq!(read(&[0xf9, 0x00, 0x01]), Head::Float(2f64.powi(-24)));
        assert_eq!(read(&cbor(&true)), Head::Bool(true));
        assert_eq!(read(&[0xf6]), Head::Null);
        assert_eq!(read(&cbor(&"ab")), Head::Text(Some(2)));
        assert_eq!(read(&[0x9f]), Head::Array(None));
        assert!(Reader::new(&[0x1c]).head().is_err());
    }

    #[test]
    fn strings_are_borrowed() {
        let bytes = cbor(&"motor/rpm");
        assert_eq!(Reader::new(&bytes).str().unwrap(), "motor/rpm");
        assert!(Reader::new(&[0x62, 0xff, 0xfe]).str().is_err());
        assert!(Reader::new(&[0x65, b'a']).str().is_err());
        // a length beyond the bytes, also beyond 32 bits
        let huge = [0x7b, 0, 0, 0, 1, 0, 0, 0, 5, b'a'];
        assert!(Reader::new(&huge).str().is_err());
        assert!(Reader::new(&huge).skip().is_err());
    }

    #[test]
    fn a_sequence_is_split_into_its_items() {
        let a = cbor(&proto::Message::Value(proto::Value {
            timestamp: 1,
            path: "cpu".to_owned(),
            value: proto::Data::Array(vec![proto::Data::Float(1.5), proto::Data::Null]),
        }));
        let b = cbor(&"source");
        // indefinite lengths, as other encoders may write them
        let c = [0x9f, 0x01, 0x7f, 0x61, b'x', 0xff, 0xff];
        let all = [&a[..], &b, &c].concat();
        let items: Vec<&[u8]> = items(&all).collect::<Result<_>>().unwrap();
        assert_eq!(items, [&a[..], &b[..], &c[..]]);
        assert_eq!(super::items(&[]).count(), 0);

        // the items end with the first that cannot be read
        let cut = &all[..all.len() - 1];
        let read: Vec<_> = super::items(cut).collect();
        assert_eq!(read.len(), 3);
        assert!(read[2].is_err());
        let broken = [&a[..], &[0xff], &b[..]].concat();
        let read: Vec<_> = super::items(&broken).collect();
        assert_eq!(read.len(), 2);
        assert!(read[1].is_err());
    }
}
