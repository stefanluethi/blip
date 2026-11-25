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
    let a = cbor(&proto::Message::Sample(proto::Sample {
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

fn written(write: impl FnOnce(&mut Writer<'_>)) -> Vec<u8> {
    let mut out = vec![];
    write(&mut Writer::new(&mut out));
    out
}

#[test]
fn numbers_are_written_in_their_shortest_exact_form() {
    let uints: [(u64, &[u8]); 10] = [
        (0, &[0x00]),
        (23, &[0x17]),
        (24, &[0x18, 0x18]),
        (255, &[0x18, 0xff]),
        (256, &[0x19, 0x01, 0x00]),
        (65535, &[0x19, 0xff, 0xff]),
        (65536, &[0x1a, 0x00, 0x01, 0x00, 0x00]),
        (4294967295, &[0x1a, 0xff, 0xff, 0xff, 0xff]),
        (4294967296, &[0x1b, 0, 0, 0, 1, 0, 0, 0, 0]),
        (
            u64::MAX,
            &[0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ),
    ];
    for (value, bytes) in uints {
        let out = written(|w| w.uint(value));
        assert_eq!(out, bytes, "{value}");
        assert_eq!(Reader::new(&out).head().unwrap(), Head::Uint(value));
    }

    let ints: [(i64, &[u8]); 7] = [
        (-1, &[0x20]),
        (-24, &[0x37]),
        (-25, &[0x38, 0x18]),
        (-256, &[0x38, 0xff]),
        (-257, &[0x39, 0x01, 0x00]),
        (
            i64::MIN,
            &[0x3b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ),
        (5, &[0x05]),
    ];
    for (value, bytes) in ints {
        let out = written(|w| w.int(value));
        assert_eq!(out, bytes, "{value}");
        let head = Reader::new(&out).head().unwrap();
        match head {
            Head::Uint(v) => assert_eq!(i64::try_from(v).unwrap(), value),
            Head::Negative(n) => assert_eq!(-1 - i128::from(n), i128::from(value)),
            other => panic!("{other:?}"),
        }
    }

    let floats: [(f64, &[u8]); 27] = [
        (0.0, &[0xf9, 0x00, 0x00]),
        (-0.0, &[0xf9, 0x80, 0x00]),
        (0.5, &[0xf9, 0x38, 0x00]),
        (1.5, &[0xf9, 0x3e, 0x00]),
        (21.0, &[0xf9, 0x4d, 0x40]),
        (65504.0, &[0xf9, 0x7b, 0xff]),
        (2f64.powi(-24), &[0xf9, 0x00, 0x01]),
        (2f64.powi(-14), &[0xf9, 0x04, 0x00]),
        (f64::INFINITY, &[0xf9, 0x7c, 0x00]),
        (f64::NEG_INFINITY, &[0xf9, 0xfc, 0x00]),
        (f64::NAN, &[0xf9, 0x7e, 0x00]),
        (1234.5, &[0xfa, 0x44, 0x9a, 0x50, 0x00]),
        (65520.0, &[0xfa, 0x47, 0x7f, 0xf0, 0x00]),
        (100000.0, &[0xfa, 0x47, 0xc3, 0x50, 0x00]),
        (16777216.0, &[0xfa, 0x4b, 0x80, 0x00, 0x00]),
        (2f64.powi(-25), &[0xfa, 0x33, 0x00, 0x00, 0x00]),
        (3.4028234663852886e38, &[0xfa, 0x7f, 0x7f, 0xff, 0xff]),
        (0.1, &[0xfb, 0x3f, 0xb9, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9a]),
        (
            1.0 / 3.0,
            &[0xfb, 0x3f, 0xd5, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55],
        ),
        (
            16777217.0,
            &[0xfb, 0x41, 0x70, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00],
        ),
        (
            1e300,
            &[0xfb, 0x7e, 0x37, 0xe4, 0x3c, 0x88, 0x00, 0x75, 0x9c],
        ),
        (1.0, &[0xf9, 0x3c, 0x00]),
        (-2.0, &[0xf9, 0xc0, 0x00]),
        (6.103515625e-5, &[0xf9, 0x04, 0x00]),
        (5.960464477539063e-8, &[0xf9, 0x00, 0x01]),
        (-1234.5, &[0xfa, 0xc4, 0x9a, 0x50, 0x00]),
        (
            -0.1,
            &[0xfb, 0xbf, 0xb9, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9a],
        ),
    ];
    for (value, bytes) in floats {
        let out = written(|w| w.float(value));
        assert_eq!(out, bytes, "{value}");
        let Head::Float(back) = Reader::new(&out).head().unwrap() else {
            panic!("{value}");
        };
        if value.is_nan() {
            assert!(back.is_nan());
        } else {
            assert_eq!(back.to_bits(), value.to_bits(), "{value}");
        }
    }
}

#[test]
fn every_half_precision_float_is_written_as_one() {
    for bits in 0..=0xffffu16 {
        if bits >> 10 & 0x1f == 31 && bits & 0x3ff != 0 {
            continue;
        }
        let out = written(|w| w.float(f16_to_f64(bits)));
        assert_eq!(out, [0xf9, (bits >> 8) as u8, bits as u8], "{bits:#06x}");
    }
}

#[test]
fn floats_read_back_as_the_same_value() {
    let mut x: u64 = 1;
    for _ in 0..10_000 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let value = f64::from_bits(x);
        let out = written(|w| w.float(value));
        let Head::Float(back) = Reader::new(&out).head().unwrap() else {
            panic!("{x:#x}");
        };
        if value.is_nan() {
            assert!(back.is_nan());
        } else {
            assert_eq!(back.to_bits(), x, "{x:#x}");
        }
    }
}
