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
    gzip || serde_cbor_2::Deserializer::from_slice(file)
        .into_iter::<Header>()
        .next()
        .is_some_and(|header| header.is_ok_and(|header| header.format == FORMAT))
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
        let mut items =
            serde_cbor_2::Deserializer::from_slice(&reader.buffer).into_iter::<Header>();
        match items.next() {
            Some(Ok(header)) if header.format == FORMAT => {
                if header.version > VERSION {
                    return Err(format!(
                        "the recording has version {}, this blip reads up to version {VERSION}",
                        header.version
                    ));
                }
            }
            _ => return Err("the file is not a blip recording".to_owned()),
        }
        let header = items.byte_offset();
        reader.buffer.drain(..header);
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
mod tests {
    use super::*;

    fn frame(timestamp: u64) -> Vec<u8> {
        serde_cbor_2::to_vec(&proto::Message::Value(proto::Value {
            timestamp,
            path: "a".to_owned(),
            value: proto::Data::Uint(timestamp),
        }))
        .unwrap()
    }

    fn read(file: &[u8]) -> (Vec<Vec<u8>>, Result<Loaded, String>) {
        let (sourced, loaded) = read_sourced(file);
        (
            sourced.into_iter().map(|(_, frame)| frame).collect(),
            loaded,
        )
    }

    /// The source of each frame and the frame.
    type Sourced = Vec<(String, Vec<u8>)>;

    fn read_sourced(file: &[u8]) -> (Sourced, Result<Loaded, String>) {
        let mut frames = vec![];
        let loaded = read_file(file, |source, frame| {
            frames.push((source.to_owned(), frame.to_vec()));
        });
        (frames, loaded)
    }

    #[test]
    fn recordings_survive_saving_and_loading() {
        let mut recording = Recording::default();
        let sent = [frame(1), frame(2), frame(1 << 40)];
        for frame in &sent {
            recording.push("", frame);
        }
        let file = recording.to_file();
        assert!(is_recording(&file));

        let (frames, loaded) = read(&file);
        assert_eq!(frames, sent);
        assert_eq!(
            loaded,
            Ok(Loaded {
                messages: 3,
                problem: None
            })
        );
    }

    #[test]
    fn long_recordings_are_compressed_while_recording_and_read_whole() {
        let mut recording = Recording::default();
        let mut sent = vec![];
        let mut timestamp = 0;
        // several chunks, the source changing across their ends
        while sent.len() * 30 < 3 * CHUNK {
            let source = ["a", "b"][sent.len() / 50_000 % 2];
            let frame = frame(timestamp);
            recording.push(source, &frame);
            sent.push((source.to_owned(), frame));
            timestamp += 1_000_003;
        }
        // compressed to a fraction while recording
        assert!(recording.members.iter().map(Vec::len).sum::<usize>() * 4 < 3 * CHUNK);
        assert!(recording.pending.len() < CHUNK);
        let file = recording.to_file();
        assert!(is_recording(&file));
        let (frames, loaded) = read_sourced(&file);
        assert_eq!(frames, sent);
        assert_eq!(loaded.unwrap().problem, None);
    }

    #[test]
    fn a_file_read_in_parts_reads_as_whole() {
        let mut recording = Recording::default();
        let mut sent = vec![];
        while sent.len() * 30 < 4 * READ_CHUNK {
            let source = ["a", "b"][sent.len() / 3000 % 2];
            let frame = frame(sent.len() as u64 * 1_000_003);
            recording.push(source, &frame);
            sent.push((source.to_owned(), frame));
        }
        let mut reader = FileReader::new(recording.to_file()).unwrap();
        let mut frames = vec![];
        let mut parts = 0;
        let mut progress = 0.0;
        // no time for more than a few messages at once
        while !reader.read(
            || false,
            |source, frame| frames.push((source.to_owned(), frame.to_vec())),
        ) {
            parts += 1;
            assert!(reader.progress() >= progress && reader.progress() <= 1.0);
            progress = reader.progress();
        }
        assert!(parts > sent.len() / MESSAGES_PER_LOOK / 2);
        assert_eq!(reader.progress(), 1.0);
        assert_eq!(frames, sent);
        assert_eq!(
            reader.loaded(),
            Loaded {
                messages: sent.len(),
                problem: None
            }
        );
    }

    #[test]
    fn the_sources_of_the_frames_are_recorded() {
        let mut recording = Recording::default();
        recording.push("", &frame(0));
        recording.push("a", &frame(1));
        recording.push("a", &frame(2));
        recording.push("b", &frame(3));
        recording.push("", &frame(4));
        assert_eq!(recording.messages(), 5);
        let (frames, loaded) = read_sourced(&recording.to_file());
        let sources: Vec<&str> = frames.iter().map(|(source, _)| source.as_str()).collect();
        assert_eq!(sources, ["", "a", "a", "b", ""]);
        assert_eq!(frames[3].1, frame(3));
        assert_eq!(loaded.unwrap().messages, 5);
    }

    #[test]
    fn uncompressed_and_appended_files_are_read() {
        let mut file = serde_cbor_2::to_vec(&Header {
            format: FORMAT.to_owned(),
            version: 1,
        })
        .unwrap();
        file.extend(frame(1));
        assert!(is_recording(&file));
        assert_eq!(read(&file).0, [frame(1)]);

        // a header in a gzip member and each frame in one of its own, as the outpost
        // appends them
        let gzip = |bytes: &[u8]| {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(bytes).unwrap();
            encoder.finish().unwrap()
        };
        let mut members = gzip(&file);
        members.extend(gzip(&frame(2)));
        assert_eq!(read(&members).0, [frame(1), frame(2)]);
    }

    #[test]
    fn a_cut_off_file_loads_up_to_the_cut() {
        let mut recording = Recording::default();
        for timestamp in 0..1000 {
            recording.push("", &frame(timestamp));
        }
        let file = recording.to_file();
        let (frames, loaded) = read(&file[..file.len() / 2]);
        let loaded = loaded.unwrap();
        assert!(loaded.problem.is_some());
        assert!(loaded.messages > 0 && loaded.messages < 1000);
        assert_eq!(frames.len(), loaded.messages);
        assert_eq!(frames[0], frame(0));
    }

    #[test]
    fn a_damaged_length_ends_the_file_without_reading_all_of_it() {
        let mut recording = Recording::default();
        recording.push("", &frame(1));
        // a byte string claiming 2^40 bytes
        recording.push("", &[0x5b, 0, 0, 1, 0, 0, 0, 0, 0]);
        let mut bytes = recording.to_file();
        // followed by more than the longest message of other members
        let filler = compress(&vec![0; READ_CHUNK]);
        for _ in 0..MAX_MESSAGE / READ_CHUNK + 64 {
            bytes.extend(&filler);
        }
        let mut reader = FileReader::new(bytes).unwrap();
        let mut frames = 0;
        while !reader.read(|| true, |_, _| frames += 1) {}
        assert_eq!(frames, 1);
        let loaded = reader.loaded();
        assert!(loaded.problem.unwrap().contains("cannot be read"));
        assert!(reader.progress() == 1.0 && !reader.ended);
    }

    #[test]
    fn reads_recordings_of_the_python_outpost() {
        // gzip.compress(header() + frames, mtime=0) of blip_outpost.recording, with the
        // frames of the metadata of `speed`, the value 2^53 + 1 and a log record
        let hex = "1f8b080000000000020a4dca411282300c4661ca35dc7020ddba0fed4fc94cd366dad0fb801e14\
                   71a36ffbbe63594a15b23c27d6a9c2971a38c7d8511b97ec767952daf0805120a32328d98aa640\
                   085b66f35565c7d7bcd858d08c44dd1f43bfe66d98862bb7fb7b89ef1f1d91d09146f645b46464\
                   231f3faf51c4bcf2095e1f6207a0000000";
        let file: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let (frames, loaded) = read(&file);
        assert_eq!(loaded.unwrap().problem, None);
        let messages: Vec<proto::Message> = frames
            .iter()
            .map(|frame| serde_cbor_2::from_slice(frame).unwrap())
            .collect();
        assert_eq!(
            messages[1],
            proto::Message::Value(proto::Value {
                timestamp: 1,
                path: "speed".to_owned(),
                value: proto::Data::Uint((1 << 53) + 1),
            })
        );
        assert!(
            matches!(&messages[0], proto::Message::ValueMetadata(metadata) if metadata.unit == "rpm")
        );
        assert!(matches!(&messages[2], proto::Message::Log(record) if record.message == "hi"));
    }

    #[test]
    fn other_files_are_no_recordings() {
        assert!(!is_recording(b"{\"version\": 3}"));
        assert!(read(b"{\"version\": 3}").1.is_err());
        assert!(read(&frame(1)).1.is_err());
        let newer = serde_cbor_2::to_vec(&Header {
            format: FORMAT.to_owned(),
            version: VERSION + 1,
        })
        .unwrap();
        assert!(read(&newer).1.unwrap_err().contains("version"));
    }
}
