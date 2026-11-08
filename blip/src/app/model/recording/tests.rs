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
