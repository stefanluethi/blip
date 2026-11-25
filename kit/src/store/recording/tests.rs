use super::*;

fn frame(timestamp: u64) -> Vec<u8> {
    serde_cbor_2::to_vec(&proto::Message::Sample(proto::Sample {
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
    let mut recording = Recorder::default();
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
            problem: None,
            notices: vec![],
        })
    );
}

#[test]
fn long_recordings_are_compressed_while_recording_and_read_whole() {
    let mut recording = Recorder::default();
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
    let mut recording = Recorder::default();
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
    while !reader.read_raw(
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
            problem: None,
            notices: vec![],
        }
    );
}

#[test]
fn the_sources_of_the_frames_are_recorded() {
    let mut recording = Recorder::default();
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
    let mut recording = Recorder::default();
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
    let mut recording = Recorder::default();
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
    while !reader.read_raw(|| true, |_, _| frames += 1) {}
    assert_eq!(frames, 1);
    let loaded = reader.loaded();
    assert!(loaded.problem.unwrap().contains("cannot be read"));
    assert!(reader.progress() == 1.0 && !reader.ended);
}

/// The recording of the Python outpost: `gzip.compress(header() + frames, mtime=0)` of
/// `blip_outpost.recording`, with the frames of the metadata of `speed`, the value
/// 2^53 + 1 and a log record.
fn outpost_file() -> Vec<u8> {
    let hex = "1f8b080000000000020a4dca411282300c4661ca35dc7020ddba0fed4fc94cd366dad0fb801e14\
               71a36ffbbe63594a15b23c27d6a9c2971a38c7d8511b97ec767952daf0805120a32328d98aa640\
               085b66f35565c7d7bcd858d08c44dd1f43bfe66d98862bb7fb7b89ef1f1d91d09146f645b46464\
               231f3faf51c4bcf2095e1f6207a0000000";
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn reads_recordings_of_the_python_outpost() {
    let file = outpost_file();
    let (frames, loaded) = read(&file);
    assert_eq!(loaded.unwrap().problem, None);
    let messages: Vec<proto::Message> = frames
        .iter()
        .map(|frame| serde_cbor_2::from_slice(frame).unwrap())
        .collect();
    assert_eq!(
        messages[1],
        proto::Message::Sample(proto::Sample {
            timestamp: 1,
            path: "speed".to_owned(),
            value: proto::Data::Uint((1 << 53) + 1),
        })
    );
    assert!(
        matches!(&messages[0], proto::Message::TraceMetadata(metadata) if metadata.unit == "rpm")
    );
    // a record of the time logs had a component, which is ignored
    assert_eq!(
        messages[2],
        proto::Message::Log(proto::LogRecord {
            timestamp: 2,
            path: String::new(),
            level: 2,
            message: "hi".to_owned(),
        })
    );
}

#[test]
fn recordings_are_written_as_version_3() {
    let file = Recorder::default().to_file();
    let mut plain = vec![];
    MultiGzDecoder::new(Cursor::new(file))
        .read_to_end(&mut plain)
        .unwrap();
    assert_eq!(read_header(&plain).unwrap().0.version, 3);
    let mut recording = Recorder::default();
    recording.push("a", &frame(1));
    let (frames, loaded) = read_sourced(&recording.to_file());
    assert_eq!(loaded.unwrap().problem, None);
    assert_eq!(frames, [("a".to_owned(), frame(1))]);
}

#[test]
fn recordings_of_versions_1_to_3_are_read() {
    let encode = |message: proto::Message| serde_cbor_2::to_vec(&message).unwrap();
    let metadata = encode(proto::Message::TraceMetadata(proto::TraceMetadata {
        path: "a".to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    }));
    let hello = encode(proto::Message::Hello(proto::Hello {
        version: 1,
        ..Default::default()
    }));
    let block = encode(proto::Message::Samples(proto::Samples {
        path: "b".to_owned(),
        values: vec![proto::Data::Uint(1), proto::Data::Uint(2)],
        times: proto::SampleTimes::Each(vec![1, 2]),
    }));
    // the version 1 names of a sample and of the metadata
    #[derive(serde::Serialize)]
    enum Old {
        Value(proto::Sample),
        ValueMetadata { path: String, unit: String },
    }
    let old = |message: Old| serde_cbor_2::to_vec(&message).unwrap();
    let old_value = old(Old::Value(proto::Sample {
        timestamp: 1,
        path: "a".to_owned(),
        value: proto::Data::Uint(1),
    }));
    let old_metadata = old(Old::ValueMetadata {
        path: "a".to_owned(),
        unit: "rpm".to_owned(),
    });
    let text = serde_cbor_2::to_vec(&"x").unwrap();
    let file = |version: u32, parts: &[&[u8]]| {
        let mut file = serde_cbor_2::to_vec(&Header {
            format: FORMAT.to_owned(),
            version,
        })
        .unwrap();
        parts.iter().for_each(|part| file.extend(*part));
        file
    };
    let files = [
        (1, file(1, &[&old_value, &old_metadata]), 0),
        (2, file(2, &[&text, &old_value, &old_metadata]), 0),
        (
            3,
            file(3, &[&text, &hello, &frame(1), &block, &metadata]),
            2,
        ),
    ];
    for (version, file, in_b) in files {
        let mut model = crate::store::MemoryStore::default();
        let mut reader = FileReader::new(file).unwrap();
        while !reader.read(|| true, &mut model) {}
        assert_eq!(reader.loaded().problem, None, "version {version}");
        assert_eq!(model.traces.get("a").unwrap().timestamps.len(), 1);
        assert_eq!(model.traces.unit("a"), Some("rpm"));
        let in_b_now = model
            .traces
            .get("b")
            .map_or(0, |trace| trace.timestamps.len());
        assert_eq!(in_b_now, in_b, "version {version}");
    }
    assert_eq!(VERSION + 1, 4);
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

#[test]
fn read_header_finds_the_header() {
    let header = |format: &str| {
        serde_cbor_2::to_vec(&Header {
            format: format.to_owned(),
            version: 2,
        })
        .unwrap()
    };
    let mut bytes = header(FORMAT);
    let (found, len) = read_header(&bytes).unwrap();
    assert_eq!((found.version, len), (2, bytes.len()));
    bytes.push(0x01);
    assert_eq!(read_header(&bytes).unwrap().1, bytes.len() - 1);
    assert!(read_header(&header("other")).is_none());
    assert!(read_header(&[0xff]).is_none());
    assert!(read_header(&[]).is_none());
}

/// FNV-1a, 64 bits.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

/// What a store shows of a file: the traces, units, logs and actions, a line each, and
/// the number of messages read.
fn digest_of(file: &[u8]) -> (usize, Vec<String>) {
    use crate::store::numbers::DisplayFormat;
    let mut store = crate::store::MemoryStore::default();
    let mut reader = FileReader::new(file.to_vec()).unwrap();
    while !reader.read(|| true, &mut store) {}
    let messages = reader.loaded().messages;
    let mut lines = vec![];
    for trace in store.traces.iter() {
        let (path, n) = (trace.path, trace.timestamps.len());
        if n == 0 {
            lines.push(format!("{path} 0"));
        } else {
            let last = trace
                .samples
                .format(n - 1, 4, None, DisplayFormat::Default, None);
            lines.push(format!(
                "{path} {n} {} {} {last}",
                trace.timestamps[0],
                trace.timestamps[n - 1]
            ));
        }
        if let Some(unit) = store.traces.unit(path) {
            lines.push(format!("unit {path} {unit}"));
        }
    }
    lines.push(format!("logs {}", store.logs.entries().count()));
    if let Some(entry) = store.logs.entries().last() {
        lines.push(format!("last log {} {}", entry.time, entry.message));
    }
    for source in store.actions.sources() {
        let paths: Vec<&str> = store
            .actions
            .paths()
            .filter(|path| store.actions.announced(source, path).is_some())
            .collect();
        lines.push(format!("actions {source:?} {}", paths.join(",")));
    }
    lines.push(format!(
        "origin {:?} base {:?}",
        store.origin(),
        store.time_base()
    ));
    (messages, lines)
}

#[test]
fn kept_recordings_show_the_same_data() {
    let demo = include_bytes!("../../../../blip/tests/data/demo.blip").to_vec();
    let sonde = include_bytes!("../../../../sonde/tests/data/recording.blip").to_vec();
    // the demo without its version 1 header, as a file of version 2 with a source
    let mut plain = vec![];
    MultiGzDecoder::new(Cursor::new(demo.clone()))
        .read_to_end(&mut plain)
        .unwrap();
    let (_, header_len) = read_header(&plain).unwrap();
    let mut v2 = serde_cbor_2::to_vec(&Header {
        format: FORMAT.to_owned(),
        version: 2,
    })
    .unwrap();
    v2.extend(serde_cbor_2::to_vec(&"bench").unwrap());
    v2.extend(&plain[header_len..]);
    let cases = [
        ("demo", demo, 9341, 63, 0x755a_72fb_4606_06f8),
        ("v2", v2, 9341, 63, 0x326e_c437_23e5_7236),
        ("sonde", sonde, 17, 17, 0x1434_7cd8_c16d_7e95),
        ("outpost", outpost_file(), 3, 5, 0xa42c_2bce_4800_5b56),
    ];
    for (name, file, messages, lines, hash) in cases {
        let (read, digest) = digest_of(&file);
        let text = digest.join("\n");
        assert_eq!(
            (read, text.split('\n').count(), fnv1a(text.as_bytes())),
            (messages, lines, hash),
            "{name}:\n{text}"
        );
    }
}

/// A `Sample { 1, "a", value }` with the value as a double precision float, 42 bytes.
fn sample_f64(value: f64) -> Vec<u8> {
    let mut bytes = vec![0xa1, 0x66];
    bytes.extend_from_slice(b"Sample");
    bytes.extend_from_slice(&[0xa3, 0x69]);
    bytes.extend_from_slice(b"timestamp");
    bytes.extend_from_slice(&[0x01, 0x64]);
    bytes.extend_from_slice(b"path");
    bytes.extend_from_slice(&[0x61, 0x61, 0x65]);
    bytes.extend_from_slice(b"value");
    bytes.push(0xfb);
    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
    bytes
}

/// A block with a period and `values` as double precision floats, as a hand-written
/// encoder writes it, but for the integers in their shortest form.
fn block_f64(values: &[f64]) -> Vec<u8> {
    let mut bytes = vec![0xa1, 0x67];
    bytes.extend_from_slice(b"Samples");
    bytes.extend_from_slice(&[0xa4, 0x64]);
    bytes.extend_from_slice(b"path");
    bytes.push(0x67);
    bytes.extend_from_slice(b"adc/ch0");
    bytes.push(0x66);
    bytes.extend_from_slice(b"values");
    bytes.extend_from_slice(&[0x98, values.len() as u8]);
    for value in values {
        bytes.push(0xfb);
        bytes.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    bytes.push(0x69);
    bytes.extend_from_slice(b"timestamp");
    bytes.push(0x1a);
    bytes.extend_from_slice(&1_000_000_000u32.to_be_bytes());
    bytes.push(0x66);
    bytes.extend_from_slice(b"period");
    bytes.push(0x1a);
    bytes.extend_from_slice(&1_000_000u32.to_be_bytes());
    bytes
}

/// What recording `item` as `source` and reading the file back gives.
fn recorded(item: &[u8]) -> Vec<u8> {
    let mut recording = Recorder::default();
    crate::store::protocol::Parser::default().item(item, true, |said| {
        recording.record("", &said, true);
    });
    let (mut frames, loaded) = read(&recording.to_file());
    assert_eq!(loaded.unwrap().messages, 1);
    assert_eq!(frames.len(), 1);
    frames.pop().unwrap()
}

/// The floats of the sample or block in `item`, as bits.
fn float_bits(item: &[u8]) -> Vec<u64> {
    let mut bits = vec![];
    crate::store::protocol::Parser::default().item(item, true, |said| {
        let floats = |value: crate::store::value::ValueRef<'_>| match value.to_data() {
            proto::Data::Float(v) => v.to_bits(),
            other => panic!("{other:?}"),
        };
        match said {
            Said::Sample { value, .. } => bits.push(floats(value)),
            Said::Samples { values, .. } => bits.extend(values.iter().map(floats)),
            other => panic!("{other:?}"),
        }
    });
    bits
}

#[test]
fn a_recording_is_not_longer_than_the_messages_it_was_made_from() {
    // the half precision float of 21.0 in the shortest form
    let mut shortest = sample_f64(21.0);
    shortest.truncate(33);
    shortest.extend_from_slice(&[0xf9, 0x4d, 0x40]);
    assert_eq!(recorded(&sample_f64(21.0)), shortest);
    assert_eq!(shortest.len(), 36);

    for (value, len) in [
        (21.0, 36),
        (1234.5, 38),
        (0.1, 42),
        (f64::INFINITY, 36),
        (-0.0, 36),
        (f64::NAN, 36),
    ] {
        let item = sample_f64(value);
        assert_eq!(item.len(), 42);
        let out = recorded(&item);
        assert_eq!(out.len(), len, "{value}");
        let (before, after) = (float_bits(&item)[0], float_bits(&out)[0]);
        if value.is_nan() {
            assert!(f64::from_bits(after).is_nan());
        } else {
            assert_eq!(before, after, "{value}");
        }
    }

    // the value 5 written as eight bytes
    let mut five = sample_f64(0.0);
    let at = five.len() - 9;
    five.truncate(at);
    five.extend_from_slice(&[0x1b, 0, 0, 0, 0, 0, 0, 0, 5]);
    assert_eq!(five.len(), 42);
    let out = recorded(&five);
    assert_eq!(out.len(), 34);
    assert_eq!(&out[out.len() - 1..], &[0x05]);

    // a block of floats does not grow
    let halves: Vec<f64> = (0..100).map(|k| f64::from(k) * 0.5).collect();
    let singles: Vec<f64> = (0..100).map(|k| 1234.5 + f64::from(k)).collect();
    let doubles: Vec<f64> = (0..100).map(|k| 0.1 * f64::from(k) + 0.1).collect();
    for (values, len) in [(halves, 359), (singles, 559), (doubles, 863)] {
        let item = block_f64(&values);
        assert_eq!(item.len(), 959);
        let out = recorded(&item);
        assert_eq!(out.len(), len);
        assert_eq!(float_bits(&item), float_bits(&out));
    }

    // what serde wrote is recorded as it is
    let serde_sample = serde_cbor_2::to_vec(&proto::Message::Sample(proto::Sample {
        timestamp: 1,
        path: "a".to_owned(),
        value: proto::Data::Float(0.1),
    }))
    .unwrap();
    let serde_block = serde_cbor_2::to_vec(&proto::Message::Samples(proto::Samples {
        path: "adc/ch0".to_owned(),
        values: (0..100)
            .map(|k| proto::Data::Float(f64::from(k) * 0.5))
            .collect(),
        times: proto::SampleTimes::Period {
            timestamp: 1_000_000_000,
            period: 1_000_000,
        },
    }))
    .unwrap();
    for item in [serde_sample, serde_block] {
        assert_eq!(recorded(&item), item);
    }
}

#[test]
fn unreadable_messages_are_recorded_as_they_are() {
    let mut future = vec![0xa1, 0x66];
    future.extend_from_slice(b"Future");
    future.push(0x01);
    let mut recording = Recorder::default();
    let unreadable = |item| Said::Unreadable {
        name: Some("Future"),
        error: "unknown",
        item,
    };
    recording.record("a", &unreadable(&future), true);
    // a text string would name a source
    recording.record("a", &unreadable(&[0x61, b'x']), true);
    // not recording: nothing
    recording.record("a", &unreadable(&future), false);
    let (frames, loaded) = read_sourced(&recording.to_file());
    assert_eq!(frames, [("a".to_owned(), future)]);
    assert_eq!(loaded.unwrap().messages, 1);
    assert_eq!(recording.messages(), 1);
}

/// The serde bytes of `message`.
fn bytes_of(message: &proto::Message) -> Vec<u8> {
    serde_cbor_2::to_vec(message).unwrap()
}

/// Opens `file` into a store and returns the recorder and what was found.
fn opened(file: &[u8]) -> (Recorder, Loaded) {
    let mut store = crate::store::MemoryStore::default();
    let mut reader = FileReader::new(file.to_vec()).unwrap();
    while !reader.read(|| true, &mut store) {}
    reader.finish()
}

#[test]
fn an_opened_recording_is_saved_as_it_was_opened() {
    let metadata = bytes_of(&proto::Message::TraceMetadata(proto::TraceMetadata {
        path: "a".to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    }));
    #[derive(serde::Serialize)]
    enum Old {
        Value(proto::Sample),
        ValueMetadata { path: String, unit: String },
    }
    let old = |message: Old| serde_cbor_2::to_vec(&message).unwrap();
    let old_value = old(Old::Value(proto::Sample {
        timestamp: 1,
        path: "a".to_owned(),
        value: proto::Data::Uint(1),
    }));
    let old_metadata = old(Old::ValueMetadata {
        path: "a".to_owned(),
        unit: "rpm".to_owned(),
    });
    let text = serde_cbor_2::to_vec(&"x").unwrap();
    let file = |version: u32, parts: &[&[u8]]| {
        let mut file = serde_cbor_2::to_vec(&Header {
            format: FORMAT.to_owned(),
            version,
        })
        .unwrap();
        parts.iter().for_each(|part| file.extend(*part));
        file
    };
    let mut raw = vec![0xa1, 0x66];
    raw.extend(b"Sample");
    raw.push(0xa4);
    raw.extend(serde_cbor_2::to_vec(&"timestamp").unwrap());
    raw.push(1);
    raw.extend(serde_cbor_2::to_vec(&"path").unwrap());
    raw.extend(serde_cbor_2::to_vec(&"a").unwrap());
    raw.extend(serde_cbor_2::to_vec(&"value").unwrap());
    raw.push(1);
    raw.extend(serde_cbor_2::to_vec(&"quality").unwrap());
    raw.push(2);
    let mut future_map = vec![0xa1, 0x66];
    future_map.extend(b"Future");
    future_map.push(0xa0);
    let files = [
        file(1, &[&old_value, &old_metadata]),
        compress(&file(2, &[&text, &old_value, &old_metadata])),
        file(3, &[&text, &raw, &future_map, &metadata]),
        include_bytes!("../../../../blip/tests/data/demo.blip").to_vec(),
        include_bytes!("../../../../sonde/tests/data/recording.blip").to_vec(),
        outpost_file(),
    ];
    for (n, file) in files.iter().enumerate() {
        let (recorder, loaded) = opened(file);
        assert_eq!(recorder.to_file(), *file, "file {n}");
        assert_eq!(recorder.messages(), loaded.messages, "file {n}");
        assert!(loaded.messages > 0, "file {n}");
    }
}

#[test]
fn a_recording_after_opening_starts_with_the_descriptions_of_the_file() {
    let sonde = include_bytes!("../../../../sonde/tests/data/recording.blip").to_vec();
    // the descriptions of the file in the order they first arrived, each the latest
    let mut expected: Vec<(String, proto::Message)> = vec![];
    read_file(&sonde, |source, frame| {
        let message: proto::Message = serde_cbor_2::from_slice(frame).unwrap();
        let slot = |known: &(String, proto::Message)| {
            known.0 == source
                && match (&known.1, &message) {
                    (proto::Message::Hello(_), proto::Message::Hello(_))
                    | (proto::Message::Actions(_), proto::Message::Actions(_)) => true,
                    (proto::Message::TraceMetadata(a), proto::Message::TraceMetadata(b)) => {
                        a.path == b.path
                    }
                    _ => false,
                }
        };
        if !matches!(
            message,
            proto::Message::Hello(_)
                | proto::Message::Actions(_)
                | proto::Message::TraceMetadata(_)
        ) {
            return;
        }
        match expected.iter_mut().find(|known| slot(known)) {
            Some(known) => known.1 = message,
            None => expected.push((source.to_owned(), message)),
        }
    })
    .unwrap();
    let (mut recorder, _) = opened(&sonde);
    recorder.restart();
    let (read, loaded) = read_sourced(&recorder.to_file());
    assert_eq!(loaded.unwrap().messages, 4);
    assert_eq!(recorder.messages(), 4);
    assert_eq!(expected.len(), 4);
    let expected: Vec<_> = expected
        .iter()
        .map(|(source, message)| (source.clone(), bytes_of(message)))
        .collect();
    assert_eq!(read, expected);
    assert!(read.iter().all(|(source, _)| source.is_empty()));
}

#[test]
fn a_recording_that_starts_later_begins_with_the_latest_descriptions() {
    let hello = |name: &str| {
        proto::Message::Hello(proto::Hello {
            version: 1,
            name: Some(name.to_owned()),
            ..Default::default()
        })
    };
    let metadata = |unit: &str| {
        proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "rpm".to_owned(),
            unit: unit.to_owned(),
            ..Default::default()
        })
    };
    let actions = proto::Message::Actions(vec![proto::ActionDescriptor {
        path: "set".to_owned(),
        description: String::new(),
        params: vec![],
        readback: None,
        confirm: false,
        enabled: true,
        disabled_reason: None,
    }]);
    let sample = proto::Message::Sample(proto::Sample {
        timestamp: 1,
        path: "rpm".to_owned(),
        value: proto::Data::Uint(1),
    });
    let mut recorder = Recorder::default();
    let mut parser = Parser::default();
    let mut feed = |recorder: &mut Recorder, source: &str, message: &proto::Message| {
        parser.item(&bytes_of(message), true, |said| {
            recorder.record(source, &said, false);
        });
    };
    feed(&mut recorder, "a", &hello("one"));
    feed(&mut recorder, "a", &metadata("rpm"));
    feed(&mut recorder, "b", &actions);
    feed(&mut recorder, "a", &sample);
    feed(&mut recorder, "a", &metadata("1/min"));
    feed(&mut recorder, "a", &hello("two"));
    assert_eq!(recorder.messages(), 0);
    recorder.restart();
    let (read, loaded) = read_sourced(&recorder.to_file());
    assert_eq!(loaded.unwrap().messages, 3);
    assert_eq!(
        read,
        vec![
            ("a".to_owned(), bytes_of(&hello("two"))),
            ("a".to_owned(), bytes_of(&metadata("1/min"))),
            ("b".to_owned(), bytes_of(&actions)),
        ]
    );
}
