//! The received messages as they arrived, to save them to a file and load them again.
//!
//! A recording file is a CBOR sequence (RFC 8742): a [`Header`] followed by the
//! messages, each the CBOR item of a WebSocket frame, compressed with gzip. Uncompressed
//! files and files of several gzip members, e.g. appended to by the outpost, are read as
//! well.
//!
//! Since version 2, a text string between the messages names the connection the
//! messages after it were received from, the source. Messages before the first name
//! have no source, as all messages of version 1, which are those of one connection.
//!
//! While recording, the messages are compressed as they arrive, a gzip member for
//! each [`CHUNK`] bytes of them, so a recording takes a fraction of the memory of the
//! messages. The chunks are small, so compressing one takes a fraction of a frame and
//! the view does not stutter. Saving writes the members one after another, as they
//! are, which is the file.

use flate2::{Compression, read::MultiGzDecoder, write::GzEncoder};
use std::io::{Cursor, Read, Write};

/// The value of [`Header::format`].
const FORMAT: &str = "blip recording";
/// Version of the format written by [`Recording::to_file`].
const VERSION: u32 = 2;
/// Bytes of messages compressed together into a gzip member while recording, at most
/// about 1 ms of compressing.
const CHUNK: usize = 1 << 16;
/// The level the members are compressed at. Measured with values in 64 KiB chunks,
/// level 2 compresses as fast as level 1 into files 12% smaller, 2% larger than gzip's
/// default level 6, which takes 3.5 times as long.
const LEVEL: u32 = 2;

/// The first item of a recording file.
#[derive(serde::Deserialize, serde::Serialize)]
struct Header {
    /// Always [`FORMAT`], to tell a recording from other CBOR.
    format: String,
    version: u32,
}

/// The frames received since the data was last cleared.
pub struct Recording {
    /// The gzip members of the [`CHUNK`]s compressed so far, the first one starting
    /// with the [`Header`], each in a buffer of its own, so adding one never copies
    /// those before.
    members: Vec<Vec<u8>>,
    /// The frames since the last member, one after another, and the names of their
    /// sources where it changes; the header before the first member.
    pending: Vec<u8>,
    len: usize,
    /// The source of the last frame.
    source: String,
}

impl Default for Recording {
    fn default() -> Self {
        let header = Header {
            format: FORMAT.to_owned(),
            version: VERSION,
        };
        Self {
            members: vec![],
            pending: serde_cbor_2::to_vec(&header).expect("writing to memory"),
            len: 0,
            source: String::new(),
        }
    }
}

/// `bytes` as one gzip member.
fn compress(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(LEVEL));
    encoder
        .write_all(bytes)
        .and_then(|()| encoder.finish())
        .expect("writing to memory does not fail")
}

/// What [`read_file`] found in a recording file.
#[derive(Debug, PartialEq)]
pub struct Loaded {
    pub messages: usize,
    /// Why the file could not be read to its end, e.g. as it was cut off.
    pub problem: Option<String>,
}

impl Recording {
    /// Records `frame` received from `source`, the name of its connection.
    pub fn push(&mut self, source: &str, frame: &[u8]) {
        if source != self.source {
            serde_cbor_2::to_writer(&mut self.pending, &source).expect("writing to memory");
            source.clone_into(&mut self.source);
        }
        self.pending.extend_from_slice(frame);
        self.len += 1;
        if self.pending.len() >= CHUNK {
            self.members.push(compress(&self.pending));
            self.pending.clear();
        }
    }

    /// The number of recorded messages.
    pub fn messages(&self) -> usize {
        self.len
    }

    /// The contents of a recording file of the frames: the members compressed while
    /// recording and the frames since, compressed as one more.
    pub fn to_file(&self) -> Vec<u8> {
        let mut file = self.members.concat();
        if !self.pending.is_empty() {
            file.extend(compress(&self.pending));
        }
        file
    }
}

/// Whether `file` starts like a recording file rather than a layout.
pub fn is_recording(file: &[u8]) -> bool {
    let gzip = file.starts_with(&[0x1f, 0x8b]);
    gzip || read_header(file).is_some()
}

/// The header at the start of `bytes` if its format is [`FORMAT`], and its length in
/// bytes.
fn read_header(bytes: &[u8]) -> Option<(Header, usize)> {
    let mut items = serde_cbor_2::Deserializer::from_slice(bytes).into_iter::<Header>();
    let header = items
        .next()?
        .ok()
        .filter(|header| header.format == FORMAT)?;
    Some((header, items.byte_offset()))
}

/// Bytes decompressed at once while reading a file, about 0.2 ms of inflating.
const READ_CHUNK: usize = 1 << 16;
/// Messages read between looks at the clock, a fraction of a millisecond of them.
const MESSAGES_PER_LOOK: usize = 256;
/// The most bytes a message can take, far more than any target sends. A longer one
/// is damaged, e.g. its length, rather than waiting for the rest of the file.
const MAX_MESSAGE: usize = 64 << 20;

/// The bytes of a recording file as they are read: decompressed or as they are.
enum Source {
    Gzip(MultiGzDecoder<Cursor<Vec<u8>>>),
    Plain(Cursor<Vec<u8>>),
}

impl Source {
    /// The bytes of the file read so far.
    fn position(&self) -> u64 {
        match self {
            Source::Gzip(decoder) => decoder.get_ref().position(),
            Source::Plain(cursor) => cursor.position(),
        }
    }

    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Source::Gzip(decoder) => decoder.read(buffer),
            Source::Plain(cursor) => cursor.read(buffer),
        }
    }
}

/// Reads a recording file a part at a time, so that a long one is read across frames
/// and the view stays responsive, decompressing only the part being read.
pub struct FileReader {
    source: Source,
    len: usize,
    /// The decompressed bytes from the item being read on.
    buffer: Vec<u8>,
    /// Whether all bytes are decompressed, or as many as could be.
    ended: bool,
    /// Whether all messages are read.
    done: bool,
    /// The source of the messages being read.
    name: String,
    messages: usize,
    problem: Option<String>,
}

impl FileReader {
    /// Starts reading the recording `file`, after its header, or tells why it is none.
    pub fn new(file: Vec<u8>) -> Result<Self, String> {
        let len = file.len();
        let source = if file.starts_with(&[0x1f, 0x8b]) {
            Source::Gzip(MultiGzDecoder::new(Cursor::new(file)))
        } else {
            Source::Plain(Cursor::new(file))
        };
        let mut reader = Self {
            source,
            len,
            buffer: vec![],
            ended: false,
            done: false,
            name: String::new(),
            messages: 0,
            problem: None,
        };
        // the header is a few bytes at the start
        reader.fill(READ_CHUNK);
        let Some((header, len)) = read_header(&reader.buffer) else {
            return Err("the file is not a blip recording".to_owned());
        };
        if header.version > VERSION {
            return Err(format!(
                "the recording has version {}, this blip reads up to version {VERSION}",
                header.version
            ));
        }
        reader.buffer.drain(..len);
        Ok(reader)
    }

    /// Decompresses bytes until the buffer holds `wanted` or all are.
    fn fill(&mut self, wanted: usize) {
        while !self.ended && self.buffer.len() < wanted {
            let start = self.buffer.len();
            self.buffer.resize(start + READ_CHUNK, 0);
            let read = self.source.read(&mut self.buffer[start..]);
            self.buffer.truncate(start + *read.as_ref().unwrap_or(&0));
            match read {
                Ok(0) => self.ended = true,
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => {
                    // the bytes read so far are kept
                    self.problem = Some(format!("the file is cut off or damaged: {error}"));
                    self.ended = true;
                }
            }
        }
    }

    /// Calls `receive` with the source and each frame of the file in the order they
    /// were received, while `more` tells there is time for more, which it is asked
    /// every few messages. Returns whether all are read.
    pub fn read(
        &mut self,
        mut more: impl FnMut() -> bool,
        mut receive: impl FnMut(&str, &[u8]),
    ) -> bool {
        while !self.done {
            let mut reader = super::cbor::Reader::new(&self.buffer);
            // the bytes of the items read
            let mut consumed = 0;
            let mut read = 0;
            let mut out_of_time = false;
            while !reader.is_empty() {
                if let Err(error) = reader.skip() {
                    // the item may go on in the bytes not decompressed yet
                    let partial = self.buffer.len() - consumed;
                    if !error.is_end() || self.ended || partial > MAX_MESSAGE {
                        self.problem
                            .get_or_insert_with(|| format!("a message cannot be read: {error}"));
                        self.done = true;
                    }
                    break;
                }
                let item = &self.buffer[consumed..reader.position()];
                consumed = reader.position();
                // messages are maps, a text string names the source
                if let Ok(name) = super::cbor::Reader::new(item).str() {
                    name.clone_into(&mut self.name);
                } else {
                    receive(&self.name, item);
                    self.messages += 1;
                    read += 1;
                    if read % MESSAGES_PER_LOOK == 0 && !more() {
                        out_of_time = true;
                        break;
                    }
                }
            }
            self.buffer.drain(..consumed);
            if self.ended && self.buffer.is_empty() {
                self.done = true;
            }
            if self.done || out_of_time {
                break;
            }
            // more than a chunk, and twice the bytes of a long item, so that one is
            // read again only so often, but little more than the longest message
            let wanted = (self.buffer.len() + READ_CHUNK)
                .max(2 * self.buffer.len())
                .min(MAX_MESSAGE + READ_CHUNK);
            self.fill(wanted);
            if !more() {
                break;
            }
        }
        self.done
    }

    /// How much of the file is read, from 0 to 1.
    pub fn progress(&self) -> f32 {
        if self.done || self.len == 0 {
            1.0
        } else {
            self.source.position() as f32 / self.len as f32
        }
    }

    /// What was found in the file, all of it once [`Self::read`] returned true.
    pub fn loaded(&self) -> Loaded {
        Loaded {
            messages: self.messages,
            problem: self.problem.clone(),
        }
    }
}

/// Calls `receive` with the source and each frame of the recording `file` in the order
/// they were received, all at once.
#[cfg(test)]
pub fn read_file(file: &[u8], mut receive: impl FnMut(&str, &[u8])) -> Result<Loaded, String> {
    let mut reader = FileReader::new(file.to_vec())?;
    while !reader.read(|| true, &mut receive) {}
    Ok(reader.loaded())
}

#[cfg(test)]
mod tests;
