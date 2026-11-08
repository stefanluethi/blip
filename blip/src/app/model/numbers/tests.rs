use super::*;

#[test]
fn values_of_a_range_get_the_same_decimals_and_room() {
    assert_eq!(fixed_decimals((-0.2, 9.9), 4), Some(3));
    assert_eq!(fixed_decimals((0.0, 2400.0), 4), Some(0));
    assert_eq!(fixed_decimals((0.0, 0.0), 4), Some(0));
    assert_eq!(fixed_decimals((0.0, 1.0e9), 4), None);
    assert_eq!(fixed_decimals((0.0, 1.0e-6), 4), None);
    assert_eq!(widest_fixed((-0.2, 9.9), 3), "-8.888");
    assert_eq!(widest_fixed((0.0, 2400.0), 0), "8888");
    assert_eq!(Number::Float(0.08026).format_fixed(3), "0.080");
    assert_eq!(Number::Float(-0.14).format_fixed(3), "-0.140");
    assert_eq!(Number::Int(-3).format_fixed(3), "-3");
    assert_eq!(Number::Float(-0.00003).format_fixed(4), "0.0000");
    assert_eq!(Number::Float(-0.00006).format_fixed(4), "-0.0001");
}

fn numbers(values: &[Number]) -> Result<Numbers, String> {
    let mut numbers = Numbers::default();
    for &value in values {
        numbers.push_row(&[value])?;
    }
    Ok(numbers)
}

fn all(numbers: &Numbers) -> Vec<Number> {
    (0..numbers.len()).map(|index| numbers.get(index)).collect()
}

#[test]
fn integers_stay_exact() {
    let big: i64 = (1 << 53) + 1;
    let column = numbers(&[Number::Uint(big as u64), Number::Int(-big)]).unwrap();
    assert_eq!(column.kind(), Some(NumberKind::Integer));
    assert_eq!(all(&column), [Number::Int(big), Number::Int(-big)]);
}

#[test]
fn integers_above_i64_become_unsigned() {
    let column = numbers(&[Number::Null, Number::Uint(1), Number::Uint(u64::MAX)]).unwrap();
    assert_eq!(
        all(&column),
        [Number::Null, Number::Uint(1), Number::Uint(u64::MAX)]
    );
    let mut column = column;
    assert!(column.push_row(&[Number::Int(-1)]).is_err());

    let mut signed = numbers(&[Number::Int(-1)]).unwrap();
    assert!(signed.push_row(&[Number::Uint(u64::MAX)]).is_err());
    assert_eq!(all(&signed), [Number::Int(-1)]);
}

#[test]
fn integers_switch_to_float() {
    let column = numbers(&[Number::Uint(0), Number::Int(-1), Number::Float(0.5)]).unwrap();
    assert_eq!(column.kind(), Some(NumberKind::Float));
    assert_eq!(
        all(&column),
        [Number::Float(0.0), Number::Float(-1.0), Number::Float(0.5)]
    );
}

#[test]
fn a_rejected_number_leaves_the_column_unchanged() {
    let mut column = Numbers::new(2);
    column
        .push_row(&[Number::Uint(1), Number::Uint(2)])
        .unwrap();
    let before = column.clone();
    assert!(
        column
            .push_row(&[Number::Float(0.5), Number::Bool(true)])
            .is_err()
    );
    assert_eq!(column, before);
}

#[test]
fn nulls_are_kept_in_every_kind() {
    let mut column = Numbers::default();
    column.push_nulls(70);
    assert_eq!(column.kind(), None);
    column.push_row(&[Number::Bool(true)]).unwrap();
    column.push_row(&[Number::Null]).unwrap();
    assert_eq!(column.kind(), Some(NumberKind::Bool));
    assert!(column.is_null(69));
    assert_eq!(column.get(70), Number::Bool(true));
    assert!(column.is_null(71));
    assert!(column.get(71).as_f64().is_nan());
    assert_eq!(column.get(70).as_f64(), 1.0);
}

#[test]
fn extents_of_rows_are_read_from_blocks() {
    let mut column = Numbers::new(2);
    let rows = 3 * BLOCK + 10;
    for row in 0..rows {
        let value = row as f64;
        let second = if row == 700 {
            Number::Null
        } else {
            Number::Float(-value)
        };
        column.push_row(&[Number::Float(value), second]).unwrap();
    }
    fn scanned(column: &Numbers, lane: usize, range: std::ops::Range<usize>) -> Extent {
        let mut extent = Extent::NONE;
        for row in range {
            extent.add(column.value(row * 2 + lane));
        }
        extent
    }
    for range in [0..rows, 5..600, 256..512, 700..701, 3..4, 0..0] {
        for lane in 0..2 {
            assert_eq!(
                column.extent(lane, range.clone()),
                scanned(&column, lane, range.clone())
            );
        }
    }
    assert!(column.extent(1, 0..rows).gap);
    assert_eq!(column.range(0), Some((0.0, (rows - 1) as f64)));
    assert_eq!(
        column.total_range(),
        Some((-((rows - 1) as f64), (rows - 1) as f64))
    );

    // a row moved into place updates the blocks
    column
        .push_row(&[Number::Float(-5.0), Number::Float(1.0e6)])
        .unwrap();
    column.move_last_row_to(1);
    assert_eq!(column.get(2), Number::Float(-5.0));
    assert_eq!(column.extent(0, 0..BLOCK).min, -5.0);
    assert_eq!(
        column.extent(1, BLOCK..2 * BLOCK),
        scanned(&column, 1, BLOCK..2 * BLOCK)
    );
}

#[test]
fn a_target_nan_is_no_null() {
    let column = numbers(&[Number::Float(f64::NAN), Number::Null]).unwrap();
    assert!(!column.is_null(0) && column.get(0).as_f64().is_nan());
    assert!(column.is_null(1));
}

#[test]
fn numbers_are_written_in_their_display_format() {
    use DisplayFormat::{Binary, Default, Hex, Scientific};
    assert_eq!(Number::Uint(31).format(None, Hex), "0x1f");
    assert_eq!(Number::Int(-5).format(None, Binary), "-0b101");
    assert_eq!(
        Number::Uint(u64::MAX).format(None, Hex),
        "0xffffffffffffffff"
    );
    assert_eq!(Number::Uint(u64::MAX).to_string(), "18446744073709551615");
    assert_eq!(Number::Null.format(None, Hex), "null");
    assert_eq!(Number::Uint(1500).format(None, Scientific), "1.5e3");
    assert_eq!(Number::Uint(7).format(Some(2), Default), "7");
    assert_eq!(Number::Float(16.0).format(None, Hex), "0x10");
    assert_eq!(Number::Float(0.5).format(Some(3), Hex), "0.5");
    assert_eq!(
        Number::Float(0.000_123_45).format(Some(3), Scientific),
        "1.23e-4"
    );
    assert_eq!(Number::Float(0.5).to_string(), "0.5");
    assert_eq!(Number::Bool(true).format(None, Binary), "true");
}

#[test]
fn floats_are_shortened_to_significant_digits() {
    assert_eq!(format_significant(22.024_154_775, 6), "22.0242");
    assert_eq!(format_significant(25.1234, 3), "25.1");
    assert_eq!(format_significant(0.012_345, 3), "0.0123");
    assert_eq!(format_significant(1234.4, 3), "1234");
    assert_eq!(format_significant(-60.0, 3), "-60");
    assert_eq!(format_significant(9.996, 3), "10");
    assert_eq!(format_significant(1.0e7, 3), "1e7");
    assert_eq!(format_significant(1.234_5e-6, 3), "1.23e-6");
    assert_eq!(format_significant(0.0, 3), "0");
}
