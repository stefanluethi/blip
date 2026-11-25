//! Parses the messages blip receives, see [`Parser`]. Samples, blocks of samples and log
//! records, which arrive all the time, are read without allocating once the buffers
//! grew: a value is read into a tape, see [`Value`]. The other messages are decoded
//! with serde.

use super::cbor::{self, Head, Reader, Writer as CborWriter};
use super::ingest::Times;
use super::said::Said;
use super::value::{AsValue, RowsBuf, Value, Writer};
use std::ops::Range;

/// Reads the messages of a source, one CBOR item at a time, and gives what it made of
/// them. Owns the buffers it reads values into, which it reuses from message to message.
#[derive(Default)]
pub(crate) struct Parser {
    /// The value of the sample, or the row of a block, being read.
    value: Value,
    /// The values of the block being read.
    rows: RowsBuf,
    /// The timestamps of the block being read, when it lists them.
    times: Vec<u64>,
    /// The field names of the records being read, to refuse duplicates.
    keys: Vec<Range<usize>>,
}

impl Parser {
    /// Parses the message `item` and calls `said` with what it made of it: once, or not
    /// at all for a message that gives nothing. A message it cannot read is given as
    /// [`Said::Unreadable`] with its bytes.
    ///
    /// With `full` false (not recording) a sample is given as [`Said::Latest`], a block
    /// is read up to its last value and given as one [`Said::Latest`], a log record is
    /// not read.
    pub(crate) fn item(&mut self, item: &[u8], full: bool, mut said: impl FnMut(Said<'_>)) {
        if let Err(error) = self.parse(item, full, &mut said) {
            said(Said::Unreadable {
                name: message_name(item),
                error: &error,
                item,
            });
        }
    }

    /// Parses each message of `frame`, a CBOR sequence received from `source`, as
    /// [`Self::item`] does, and returns how many it split off. Bytes that are not a whole
    /// CBOR item end the frame: they are logged and not given.
    pub(crate) fn frame(
        &mut self,
        source: &str,
        frame: &[u8],
        full: bool,
        mut said: impl FnMut(Said<'_>),
    ) -> usize {
        let mut items = 0;
        let mut at = 0;
        while at < frame.len() {
            // one scan for the messages read here: the reader ends at the item's end
            let mut reader = Reader::new(&frame[at..]);
            if let Some(Ok(())) = self.read_item(&mut reader, full, &mut said) {
                at += reader.position();
                items += 1;
                continue;
            }
            // the rest: find the end of the item, then handle its bytes as `item` does
            let mut reader = Reader::new(&frame[at..]);
            if let Err(error) = reader.skip() {
                log::warn!("error reading a message of {source}: {error}");
                break;
            }
            let end = at + reader.position();
            items += 1;
            self.item(&frame[at..end], full, &mut said);
            at = end;
        }
        items
    }

    /// Gives what the application hands over as the message `message`, as
    /// [`Self::item`] does for its bytes, without a trip through CBOR.
    pub(crate) fn message(
        &mut self,
        message: &proto::Message,
        full: bool,
        mut said: impl FnMut(Said<'_>),
    ) {
        match message {
            proto::Message::Sample(s) => self.sample(s.timestamp, &s.path, &s.value, full, said),
            proto::Message::Samples(b) => {
                self.samples(&b.path, Times::from(&b.times), &b.values, full, said);
            }
            proto::Message::Log(l) => {
                self.log(l.timestamp, &l.path, l.level, &l.message, full, said)
            }
            proto::Message::TraceMetadata(metadata) => said(Said::Metadata(metadata)),
            proto::Message::Actions(actions) => said(Said::Actions(actions)),
            proto::Message::ActionResult(result) => said(Said::ActionResult(result)),
            proto::Message::Hello(hello) => said(Said::Hello(hello)),
        }
    }

    /// Gives a sample the application hands over: written into the value buffer, no
    /// CBOR. With `full` false it is given as [`Said::Latest`].
    pub(crate) fn sample(
        &mut self,
        timestamp: u64,
        path: &str,
        value: impl AsValue,
        full: bool,
        mut said: impl FnMut(Said<'_>),
    ) {
        self.value.set(value);
        said(Said::sample(full, timestamp, path, self.value.as_ref()));
    }

    /// Gives a block the application hands over. Rows are taken while `times` has a
    /// timestamp for them: values beyond are not consumed, timestamps beyond the values
    /// are cut, each with a warning. With `full` false the last value is kept and given
    /// as [`Said::Latest`] with the time of the last row.
    pub(crate) fn samples<V: AsValue>(
        &mut self,
        path: &str,
        times: Times<'_>,
        values: impl IntoIterator<Item = V>,
        full: bool,
        mut said: impl FnMut(Said<'_>),
    ) {
        let Self { value, rows, .. } = self;
        rows.clear();
        let mut values = values.into_iter();
        let mut taken = 0;
        let mut rest = false;
        loop {
            if times.timestamp(taken).is_none() {
                // look one value further, to warn, but do not drain the iterator
                rest = values.next().is_some();
                break;
            }
            let Some(v) = values.next() else {
                break;
            };
            if full {
                rows.push_with(|to| to.value(v));
            } else {
                value.set(v);
            }
            taken += 1;
        }
        let times = match times {
            Times::Each(list) if rest || list.len() > taken => {
                log::warn!(
                    "{path}: a block of {taken} values does not fit its {} timestamps, the rest is dropped",
                    list.len()
                );
                Times::Each(&list[..taken.min(list.len())])
            }
            Times::Period { .. } if rest => {
                log::warn!(
                    "{path}: the timestamps of a block go beyond 2^64 ns, the rest is dropped"
                );
                times
            }
            times => times,
        };
        give_block(path, times, taken, full, rows, value, &mut said);
    }

    /// Gives a log record the application hands over, only with `full`: not recording,
    /// logs are dropped.
    pub(crate) fn log(
        &mut self,
        timestamp: u64,
        path: &str,
        level: u8,
        message: &str,
        full: bool,
        mut said: impl FnMut(Said<'_>),
    ) {
        if full {
            said(Said::Log {
                timestamp,
                path,
                level,
                message,
            });
        }
    }

    fn parse(
        &mut self,
        item: &[u8],
        full: bool,
        said: &mut impl FnMut(Said<'_>),
    ) -> Result<(), String> {
        if let Some(result) = self.read_item(&mut Reader::new(item), full, said) {
            return result;
        }
        let message = serde_cbor_2::from_slice(item).map_err(|error| error.to_string())?;
        // serde also reads encodings that the readers above do not, e.g. an enum as an
        // array; the model takes samples and logs from those readers only
        if let proto::Message::Sample(_) | proto::Message::Samples(_) | proto::Message::Log(_) =
            message
        {
            return Err("a sample or log that is not a map of one entry".to_owned());
        }
        self.message(&message, full, said);
        Ok(())
    }

    /// Reads the item at `reader` if it is a message read here, a sample, a block of
    /// samples or a log record: `None` for any other. The reader then ends at the end of
    /// the item, when it is `Some(Ok(..))`. `said` is called only then.
    fn read_item<'b>(
        &mut self,
        reader: &mut Reader<'b>,
        full: bool,
        said: &mut impl FnMut(Said<'_>),
    ) -> Option<Result<(), String>> {
        let name = (|| -> cbor::Result<Option<&str>> {
            if reader.head()? != Head::Map(Some(1)) {
                return Ok(None);
            }
            reader.str().map(Some)
        })();
        let name = match name {
            Ok(Some(name)) => name,
            Ok(None) => return None,
            Err(error) => return Some(Err(error.to_string())),
        };
        Some(match name {
            "Value" | "Sample" => self
                .read_sample(reader, full, said)
                .map_err(|error| error.to_string()),
            "Samples" => self.read_samples(reader, full, said),
            "Log" => read_log(reader, full, said).map_err(|error| error.to_string()),
            _ => return None,
        })
    }

    fn read_sample<'b>(
        &mut self,
        reader: &mut Reader<'b>,
        full: bool,
        said: &mut impl FnMut(Said<'_>),
    ) -> cbor::Result<()> {
        let Self { value, keys, .. } = self;
        keys.clear();
        let (mut timestamp, mut path, mut seen) = (None, None, None);
        fields(reader, |reader, name| {
            match name {
                "timestamp" => set(&mut timestamp, reader.u64()?, reader)?,
                "path" => set(&mut path, reader.str()?, reader)?,
                "value" => {
                    // before reading it: a second value would add to the first
                    set(&mut seen, (), reader)?;
                    value.write_with(|to| read_value(reader, to, keys))?;
                }
                _ => reader.skip()?,
            }
            Ok(())
        })?;
        let (Some(timestamp), Some(path), Some(())) = (timestamp, path, seen) else {
            return Err(error(reader, "a Sample lacks a field"));
        };
        said(Said::sample(full, timestamp, path, value.as_ref()));
        Ok(())
    }

    /// Reads a block of samples and checks its times. With `full` false only its last
    /// value is read, into `self.value`, the others are skipped.
    fn read_samples<'b>(
        &mut self,
        reader: &mut Reader<'b>,
        full: bool,
        said: &mut impl FnMut(Said<'_>),
    ) -> Result<(), String> {
        let Self {
            value,
            rows,
            times,
            keys,
        } = self;
        keys.clear();
        rows.clear();
        times.clear();
        let (mut path, mut len, mut timestamp, mut period, mut listed) =
            (None, None, None, None, None);
        fields(reader, |reader, name| {
            match name {
                "path" => set(&mut path, reader.str()?, reader)?,
                "values" => {
                    let Head::Array(Some(count)) = reader.head()? else {
                        return Err(error(reader, "expected an array of values"));
                    };
                    let count =
                        usize::try_from(count).map_err(|_| error(reader, "unexpected end"))?;
                    set(&mut len, count, reader)?;
                    for index in 0..count {
                        if full {
                            rows.push_with(|to| read_value(reader, to, keys))?;
                        } else if index + 1 == count {
                            value.write_with(|to| read_value(reader, to, keys))?;
                        } else {
                            reader.skip()?;
                        }
                    }
                }
                "timestamp" => set(&mut timestamp, reader.u64()?, reader)?,
                "period" => set(&mut period, reader.u64()?, reader)?,
                "timestamps" => {
                    let Head::Array(Some(count)) = reader.head()? else {
                        return Err(error(reader, "expected an array of timestamps"));
                    };
                    set(&mut listed, (), reader)?;
                    for _ in 0..count {
                        times.push(reader.u64()?);
                    }
                }
                _ => reader.skip()?,
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        let path = path.ok_or("a Samples lacks its path")?;
        let len = len.ok_or("a Samples lacks its values")?;
        let times = block_times(timestamp, period, listed.map(|()| &times[..]), len)?;
        give_block(path, times, len, full, rows, value, said);
        Ok(())
    }
}

/// Gives a block of `len` values, read into `rows`. With `full` false only its last
/// value was read, into `last`: it is given as [`Said::Latest`] at the time of its row.
fn give_block(
    path: &str,
    times: Times<'_>,
    len: usize,
    full: bool,
    rows: &RowsBuf,
    last: &Value,
    said: &mut impl FnMut(Said<'_>),
) {
    if full {
        said(Said::Samples {
            path,
            times,
            values: rows.rows(),
        });
    } else if let Some(timestamp) = len.checked_sub(1).and_then(|row| times.timestamp(row)) {
        said(Said::Latest {
            timestamp,
            path,
            value: last.as_ref(),
        });
    }
}

/// The times of a block of `len` values from the fields of a `Samples` message, or why
/// they do not make one.
fn block_times<'t>(
    timestamp: Option<u64>,
    period: Option<u64>,
    timestamps: Option<&'t [u64]>,
    len: usize,
) -> Result<Times<'t>, String> {
    let times = match (timestamp, period, timestamps) {
        (None, None, Some(list)) => Times::Each(list),
        (_, _, Some(_)) => return Err("a Samples has both a period and timestamps".into()),
        (Some(timestamp), Some(period), None) => Times::Period { timestamp, period },
        (Some(_), None, None) => return Err("a Samples has a timestamp but no period".into()),
        (None, Some(_), None) => return Err("a Samples has a period but no timestamp".into()),
        (None, None, None) => {
            return Err("a Samples has neither a period nor timestamps".into());
        }
    };
    match times {
        Times::Each(list) if list.len() != len => Err(format!(
            "a Samples has {len} values but {} timestamps",
            list.len()
        )),
        Times::Period { .. } if len > 0 && times.timestamp(len - 1).is_none() => {
            Err("the timestamps of a Samples go beyond 2^64 ns".into())
        }
        _ => Ok(times),
    }
}

/// Reads a value at `reader` into `to`. `keys` holds the field names of the records
/// being read, to refuse duplicates.
fn read_value(
    reader: &mut Reader<'_>,
    to: &mut Writer<'_>,
    keys: &mut Vec<Range<usize>>,
) -> cbor::Result<()> {
    let too_large = |reader: &Reader<'_>| error(reader, "a value too large");
    match reader.head()? {
        Head::Map(len) => {
            let Some(len) = len else {
                return Err(error(reader, "indefinite map"));
            };
            let len = u32::try_from(len).map_err(|_| too_large(reader))?;
            let at = to.open_record(len);
            let base = keys.len();
            for _ in 0..len {
                let name = reader.str()?;
                if u32::try_from(to.texts().len() + name.len()).is_err() {
                    return Err(too_large(reader));
                }
                if keys[base..]
                    .iter()
                    .any(|key| to.texts()[key.clone()] == *name)
                {
                    return Err(error(reader, "duplicate record field"));
                }
                keys.push(to.key(name));
                read_value(reader, to, keys)?;
            }
            keys.truncate(base);
            to.close(at);
        }
        Head::Array(Some(len)) => {
            let len = u32::try_from(len).map_err(|_| too_large(reader))?;
            let at = to.open_array(len);
            for _ in 0..len {
                read_value(reader, to, keys)?;
            }
            to.close(at);
        }
        Head::Text(len) => {
            let text = reader.text(len)?;
            if u32::try_from(to.texts().len() + text.len()).is_err() {
                return Err(too_large(reader));
            }
            to.text(text);
        }
        Head::Null | Head::Undefined => to.null(),
        Head::Uint(value) => to.uint(value),
        Head::Negative(value) => match i64::try_from(value) {
            Ok(value) => to.int(-1 - value),
            Err(_) => return Err(error(reader, "a value cannot be of this CBOR type")),
        },
        Head::Float(value) => to.float(value),
        Head::Bool(value) => to.bool(value),
        _ => return Err(error(reader, "a value cannot be of this CBOR type")),
    }
    Ok(())
}

fn error(reader: &Reader<'_>, what: &'static str) -> cbor::Error {
    cbor::Error {
        at: reader.position(),
        what,
    }
}

/// The name of the message in `item`, e.g. "Sample", read from its start only.
pub(crate) fn message_name(item: &[u8]) -> Option<&str> {
    let mut reader = Reader::new(item);
    if reader.head().ok()? != Head::Map(Some(1)) {
        return None;
    }
    reader.str().ok()
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

/// Sets the field `slot` of a message, which may hold it once only, as with serde.
fn set<T>(slot: &mut Option<T>, value: T, reader: &Reader<'_>) -> cbor::Result<()> {
    if slot.replace(value).is_some() {
        return Err(error(reader, "a repeated field"));
    }
    Ok(())
}

fn read_log<'b>(
    reader: &mut Reader<'b>,
    full: bool,
    said: &mut impl FnMut(Said<'_>),
) -> cbor::Result<()> {
    if !full {
        // not read, but skipped: the reader ends at the end of the item
        return reader.skip();
    }
    let (mut timestamp, mut path, mut level, mut message) = (None, None, None, None);
    fields(reader, |reader, name| {
        match name {
            "timestamp" => set(&mut timestamp, reader.u64()?, reader)?,
            "path" => set(&mut path, reader.str()?, reader)?,
            "level" => {
                let value = reader.u64()?;
                let value = u8::try_from(value).map_err(|_| error(reader, "level above 255"))?;
                set(&mut level, value, reader)?;
            }
            "message" => set(&mut message, reader.str()?, reader)?,
            _ => reader.skip()?,
        }
        Ok(())
    })?;
    let (Some(timestamp), Some(level), Some(message)) = (timestamp, level, message) else {
        return Err(error(reader, "a Log lacks a field"));
    };
    said(Said::Log {
        timestamp,
        path: path.unwrap_or(""),
        level,
        message,
    });
    Ok(())
}

/// Writes `said` as the CBOR item the outpost would send for it, in the shortest form, and
/// returns whether it did. `Latest` and `Unreadable` are no message to write: nothing is
/// written for them. Writes without allocating but for the messages that are seldom: those
/// that go through serde.
pub(crate) fn encode(said: &Said<'_>, out: &mut Vec<u8>) -> bool {
    match said {
        Said::Hello(hello) => entry(out, "Hello", *hello),
        Said::Metadata(metadata) => entry(out, "TraceMetadata", *metadata),
        Said::Actions(actions) => entry(out, "Actions", *actions),
        Said::ActionResult(result) => entry(out, "ActionResult", *result),
        Said::Sample {
            timestamp,
            path,
            value,
        } => {
            let mut to = CborWriter::new(out);
            to.map(1);
            to.text("Sample");
            to.map(3);
            to.text("timestamp");
            to.uint(*timestamp);
            to.text("path");
            to.text(path);
            to.text("value");
            to.value(*value);
        }
        Said::Samples {
            path,
            times,
            values,
        } => {
            let mut to = CborWriter::new(out);
            to.map(1);
            to.text("Samples");
            to.map(if matches!(times, Times::Period { .. }) {
                4
            } else {
                3
            });
            to.text("path");
            to.text(path);
            to.text("values");
            to.array(values.len());
            for value in values.iter() {
                to.value(value);
            }
            match times {
                Times::Period { timestamp, period } => {
                    to.text("timestamp");
                    to.uint(*timestamp);
                    to.text("period");
                    to.uint(*period);
                }
                Times::Each(list) => {
                    to.text("timestamps");
                    to.array(list.len());
                    for timestamp in *list {
                        to.uint(*timestamp);
                    }
                }
            }
        }
        Said::Log {
            timestamp,
            path,
            level,
            message,
        } => {
            let mut to = CborWriter::new(out);
            to.map(1);
            to.text("Log");
            to.map(if path.is_empty() { 3 } else { 4 });
            to.text("timestamp");
            to.uint(*timestamp);
            if !path.is_empty() {
                to.text("path");
                to.text(path);
            }
            to.text("level");
            to.uint(u64::from(*level));
            to.text("message");
            to.text(message);
        }
        Said::Latest { .. } | Said::Unreadable { .. } => return false,
    }
    true
}

/// Writes the map of one entry `name: value`, with serde.
fn entry<T: serde::Serialize + ?Sized>(out: &mut Vec<u8>, name: &str, value: &T) {
    use serde::Serializer as _;
    use serde::ser::SerializeMap as _;
    let mut serializer = serde_cbor_2::Serializer::new(out);
    let mut map = (&mut serializer)
        .serialize_map(Some(1))
        .expect("writing to memory");
    map.serialize_entry(name, value).expect("writing to memory");
    map.end().expect("writing to memory");
}

#[cfg(test)]
mod tests;
