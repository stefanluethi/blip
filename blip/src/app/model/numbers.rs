//! Columns of numbers or bools that keep integers exact: a column holds `i64`, or `u64`
//! once an integer above `i64::MAX` arrives, and switches to `f64` with the first float.

use std::fmt;

/// The kind of the numbers of a trace or of the elements of an array trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberKind {
    Integer,
    Float,
    Bool,
}

impl NumberKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Integer => "int",
            Self::Float => "float",
            Self::Bool => "bool",
        }
    }
}

/// Floats show at most this many significant digits in tables and tooltips.
pub const SHOWN_DIGITS: usize = 4;

/// How the numbers of a trace are written.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum DisplayFormat {
    /// As decimals, bools as `true` and `false`.
    #[default]
    Default,
    /// Integers and whole floats in hexadecimal, e.g. `0x1f`.
    Hex,
    /// Integers and whole floats in binary, e.g. `0b101`.
    Binary,
    /// Numbers with an exponent, e.g. `1.5e3`.
    Scientific,
}

impl DisplayFormat {
    pub const ALL: [DisplayFormat; 4] = [
        DisplayFormat::Default,
        DisplayFormat::Hex,
        DisplayFormat::Binary,
        DisplayFormat::Scientific,
    ];

    /// The formats that apply to numbers of `kind`: hex and binary only to integers,
    /// none to bools.
    pub fn of_kind(kind: NumberKind) -> &'static [DisplayFormat] {
        match kind {
            NumberKind::Integer => &Self::ALL,
            NumberKind::Float => &[DisplayFormat::Default, DisplayFormat::Scientific],
            NumberKind::Bool => &[],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DisplayFormat::Default => "Default",
            DisplayFormat::Hex => "Hex",
            DisplayFormat::Binary => "Binary",
            DisplayFormat::Scientific => "Scientific",
        }
    }
}

/// One sample value of a numeric trace or one element of an array trace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Null,
    Int(i64),
    Uint(u64),
    Float(f64),
    Bool(bool),
}

impl Number {
    pub fn is_null(self) -> bool {
        self == Self::Null
    }

    /// The number as a float for plotting, NaN for null and bools as 0 and 1.
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Null => f64::NAN,
            Self::Int(value) => value as f64,
            Self::Uint(value) => value as f64,
            Self::Float(value) => value,
            Self::Bool(value) => f64::from(u8::from(value)),
        }
    }

    pub fn kind_name(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Int(_) | Self::Uint(_) => "int",
            Self::Float(_) => "float",
            Self::Bool(_) => "bool",
        }
    }

    /// The number written in `format`, a float with at most `digits` significant
    /// digits or all of them if `digits` is `None`. The format does not apply to bools
    /// and nulls, and hex and binary not to floats that are not whole numbers.
    pub fn format(self, digits: Option<usize>, format: DisplayFormat) -> String {
        // a whole float in hex or binary is written as the integer it is
        let whole = match self {
            Self::Float(value) if value.fract() == 0.0 && value.abs() < 2f64.powi(63) => {
                Some(Self::Int(value as i64))
            }
            _ => None,
        };
        match (format, whole.unwrap_or(self)) {
            (_, Self::Null) => "null".to_owned(),
            (_, Self::Bool(value)) => value.to_string(),
            (DisplayFormat::Hex, Self::Int(value)) => {
                format!("{}0x{:x}", sign(value), value.unsigned_abs())
            }
            (DisplayFormat::Hex, Self::Uint(value)) => format!("0x{value:x}"),
            (DisplayFormat::Binary, Self::Int(value)) => {
                format!("{}0b{:b}", sign(value), value.unsigned_abs())
            }
            (DisplayFormat::Binary, Self::Uint(value)) => format!("0b{value:b}"),
            (DisplayFormat::Scientific, _) => match self {
                Self::Int(value) => scientific(value, digits),
                Self::Uint(value) => scientific(value, digits),
                Self::Float(value) if value.is_finite() => scientific(value, digits),
                _ => self.format(digits, DisplayFormat::Default),
            },
            _ => match self {
                Self::Float(value) => match digits {
                    Some(digits) => format_significant(value, digits),
                    None => value.to_string(),
                },
                Self::Int(value) => value.to_string(),
                Self::Uint(value) => value.to_string(),
                _ => unreachable!("null and bools are written above"),
            },
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format(None, DisplayFormat::Default))
    }
}

fn sign(value: i64) -> &'static str {
    if value < 0 { "-" } else { "" }
}

/// `value` with an exponent, with `digits` significant digits or all of them.
fn scientific(value: impl fmt::LowerExp, digits: Option<usize>) -> String {
    let text = match digits {
        Some(digits) => format!("{:.*e}", digits.max(1) - 1, value),
        None => format!("{value:e}"),
    };
    match text.split_once('e') {
        Some((mantissa, exponent)) => format!("{}e{exponent}", trim_zeros(mantissa)),
        None => text,
    }
}

/// `value` with at most `digits` significant digits and no trailing zeros, e.g.
/// `0.0123`. Integer digits are kept, large and small values are written with an
/// exponent.
pub fn format_significant(value: f64, digits: usize) -> String {
    if value == 0.0 || !value.is_finite() {
        return value.to_string();
    }
    let digits = digits.max(1);
    let magnitude = value.abs().log10().floor() as i32;
    if magnitude >= digits.max(6) as i32 || magnitude < -4 {
        return scientific(value, Some(digits));
    }
    let decimals = (digits as i32 - 1 - magnitude).max(0) as usize;
    trim_zeros(&format!("{value:.decimals$}")).to_owned()
}

/// `number` without trailing zeros after its decimal point.
fn trim_zeros(number: &str) -> &str {
    if number.contains('.') {
        number.trim_end_matches('0').trim_end_matches('.')
    } else {
        number
    }
}

/// The bits of the NaN that marks a null in a column, unlike the NaN of arithmetic.
const NULL_BITS: u64 = 0x7ff8_0000_0000_0b1b;

/// Rows of a column summarized in one [`Block`], so that plotting many rows reads
/// their blocks instead of each value.
pub const BLOCK: usize = 256;

/// The lowest and highest finite value of some rows, and whether one is not finite,
/// e.g. null, which a plot shows as a gap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    pub min: f64,
    pub max: f64,
    pub gap: bool,
}

impl Extent {
    const NONE: Extent = Extent {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
        gap: false,
    };

    fn add(&mut self, value: f64) {
        if value.is_finite() {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        } else {
            self.gap = true;
        }
    }

    fn merge(&mut self, other: Extent) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.gap |= other.gap;
    }

    /// Whether a finite value was added.
    pub fn has_values(&self) -> bool {
        self.min <= self.max
    }
}

/// The integers of a column kept exactly, once one cannot be kept as a float.
#[derive(Clone, Debug, PartialEq)]
enum Exact {
    None,
    Int(Vec<i64>),
    Uint(Vec<u64>),
}

/// A column of numbers or bools in rows of `lanes` values each, e.g. the elements of
/// an array. Values are kept as floats for plotting, nulls as a marked NaN. Integers
/// beyond 2^53 are kept exactly too: as `i64`, or as `u64` once one is above
/// `i64::MAX`. A column switches to floats with its first float.
#[derive(Clone, Debug, PartialEq)]
pub struct Numbers {
    kind: Option<NumberKind>,
    values: Vec<f64>,
    exact: Exact,
    /// Whether a negative integer arrived, so the integers cannot become unsigned.
    negative: bool,
    lanes: usize,
    /// The extent of all rows per lane.
    extents: Vec<Extent>,
    /// The extent of each [`BLOCK`] of rows, per lane.
    blocks: Vec<Extent>,
}

impl Default for Numbers {
    fn default() -> Self {
        Self::new(1)
    }
}

impl Numbers {
    pub fn new(lanes: usize) -> Self {
        let lanes = lanes.max(1);
        Self {
            kind: None,
            values: vec![],
            exact: Exact::None,
            negative: false,
            lanes,
            extents: vec![Extent::NONE; lanes],
            blocks: vec![],
        }
    }

    /// The number of values, of all lanes.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// The number of values in a row.
    pub fn lanes(&self) -> usize {
        self.lanes
    }

    /// The number of rows.
    pub fn rows(&self) -> usize {
        self.values.len() / self.lanes
    }

    /// The kind of the numbers, or `None` while all are null.
    pub fn kind(&self) -> Option<NumberKind> {
        self.kind
    }

    #[cfg(test)]
    pub fn is_null(&self, index: usize) -> bool {
        self.values
            .get(index)
            .is_some_and(|value| value.to_bits() == NULL_BITS)
    }

    /// The value at `index` as a float for plotting, NaN for null.
    pub fn value(&self, index: usize) -> f64 {
        self.values[index]
    }

    pub fn get(&self, index: usize) -> Number {
        let value = self.values[index];
        if value.to_bits() == NULL_BITS {
            return Number::Null;
        }
        match (self.kind, &self.exact) {
            (None, _) => Number::Null,
            (Some(NumberKind::Float), _) => Number::Float(value),
            (Some(NumberKind::Bool), _) => Number::Bool(value != 0.0),
            (Some(NumberKind::Integer), Exact::None) => Number::Int(value as i64),
            (Some(NumberKind::Integer), Exact::Int(exact)) => Number::Int(exact[index]),
            (Some(NumberKind::Integer), Exact::Uint(exact)) => Number::Uint(exact[index]),
        }
    }

    /// The lowest and highest finite value of all rows in `lane`, `None` if there is
    /// none.
    pub fn range(&self, lane: usize) -> Option<(f64, f64)> {
        let extent = self.extents.get(lane)?;
        extent.has_values().then_some((extent.min, extent.max))
    }

    /// The lowest and highest finite value of all lanes, `None` if there is none.
    pub fn total_range(&self) -> Option<(f64, f64)> {
        let mut total = Extent::NONE;
        for extent in &self.extents {
            total.merge(*extent);
        }
        total.has_values().then_some((total.min, total.max))
    }

    /// The extent of `rows` in `lane`, read from the blocks they cover in whole.
    pub fn extent(&self, lane: usize, rows: std::ops::Range<usize>) -> Extent {
        let mut extent = Extent::NONE;
        let mut row = rows.start;
        while row < rows.end {
            if row % BLOCK == 0 && row + BLOCK <= rows.end {
                extent.merge(self.blocks[row / BLOCK * self.lanes + lane]);
                row += BLOCK;
            } else {
                extent.add(self.values[row * self.lanes + lane]);
                row += 1;
            }
        }
        extent
    }

    /// Appends `rows` rows of nulls.
    pub fn push_nulls(&mut self, rows: usize) {
        for _ in 0..rows {
            let start = self.values.len();
            self.values
                .extend(std::iter::repeat_n(f64::from_bits(NULL_BITS), self.lanes));
            match &mut self.exact {
                Exact::None => {}
                Exact::Int(exact) => exact.resize(start + self.lanes, 0),
                Exact::Uint(exact) => exact.resize(start + self.lanes, 0),
            }
            self.add_to_blocks(self.rows() - 1);
        }
    }

    /// Appends a row of `numbers`, one per lane, or nothing if one does not mix with
    /// the others or with the numbers so far.
    pub fn push_row(&mut self, numbers: &[Number]) -> Result<(), String> {
        debug_assert_eq!(numbers.len(), self.lanes);
        let (mut kind, mut negative) = (self.kind, self.negative);
        let mut unsigned = matches!(self.exact, Exact::Uint(_));
        let mut beyond_float = false;
        for &number in numbers {
            let name = |kind: Option<NumberKind>| kind.map_or("null", NumberKind::name);
            kind = match (kind, number) {
                (_, Number::Null) => kind,
                (None | Some(NumberKind::Bool), Number::Bool(_)) => Some(NumberKind::Bool),
                (Some(NumberKind::Bool), _) | (Some(_), Number::Bool(_)) => {
                    return Err(format!(
                        "expected {}, got {}",
                        name(kind),
                        number.kind_name()
                    ));
                }
                (_, Number::Float(_)) | (Some(NumberKind::Float), _) => Some(NumberKind::Float),
                (_, Number::Int(value)) => {
                    if unsigned {
                        return Err(format!(
                            "{number} does not fit in 64 bits with the other integers of the trace"
                        ));
                    }
                    negative = true;
                    beyond_float |= value.unsigned_abs() > 1 << 53;
                    Some(NumberKind::Integer)
                }
                (_, Number::Uint(value)) => {
                    if value > i64::MAX as u64 {
                        if negative {
                            return Err(format!(
                                "{number} does not fit in 64 bits with the other integers of the trace"
                            ));
                        }
                        unsigned = true;
                    }
                    beyond_float |= value > 1 << 53;
                    Some(NumberKind::Integer)
                }
            };
        }
        self.kind = kind;
        self.negative = negative;
        if kind != Some(NumberKind::Integer) {
            self.exact = Exact::None;
        } else if unsigned && !matches!(self.exact, Exact::Uint(_)) {
            let exact = match &self.exact {
                Exact::Int(exact) => exact.iter().map(|&value| value as u64).collect(),
                _ => self.values.iter().map(|&value| value as u64).collect(),
            };
            self.exact = Exact::Uint(exact);
        } else if beyond_float && matches!(self.exact, Exact::None) {
            self.exact = Exact::Int(self.values.iter().map(|&value| value as i64).collect());
        }

        for &number in numbers {
            self.values.push(match number {
                Number::Null => f64::from_bits(NULL_BITS),
                number => number.as_f64(),
            });
            match &mut self.exact {
                Exact::None => {}
                Exact::Int(exact) => exact.push(match number {
                    Number::Int(value) => value,
                    Number::Uint(value) => value as i64,
                    _ => 0,
                }),
                Exact::Uint(exact) => exact.push(match number {
                    Number::Uint(value) => value,
                    _ => 0,
                }),
            }
        }
        self.add_to_blocks(self.rows() - 1);
        Ok(())
    }

    /// Moves the last row to `row`, after the rows before it, e.g. a sample that
    /// arrived late.
    pub fn move_last_row_to(&mut self, row: usize) {
        let start = row * self.lanes;
        self.values[start..].rotate_right(self.lanes);
        match &mut self.exact {
            Exact::None => {}
            Exact::Int(exact) => exact[start..].rotate_right(self.lanes),
            Exact::Uint(exact) => exact[start..].rotate_right(self.lanes),
        }
        let first_block = row / BLOCK;
        self.blocks.truncate(first_block * self.lanes);
        for row in first_block * BLOCK..self.rows() {
            self.add_to_blocks(row);
        }
    }

    /// Adds the values of `row`, the last row or the next one of the blocks, to the
    /// extents.
    fn add_to_blocks(&mut self, row: usize) {
        if self.blocks.len() <= row / BLOCK * self.lanes {
            self.blocks
                .extend(std::iter::repeat_n(Extent::NONE, self.lanes));
        }
        let first = row / BLOCK * self.lanes;
        for lane in 0..self.lanes {
            let value = self.values[row * self.lanes + lane];
            self.blocks[first + lane].add(value);
            self.extents[lane].add(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
