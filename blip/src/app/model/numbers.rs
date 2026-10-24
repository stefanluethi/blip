//! Columns of numbers or bools that keep integers exact: a column holds `i64`, or `u64`
//! once an integer above `i64::MAX` arrives, and switches to `f64` with the first float.

use proto::Data;
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
    /// The number in `data`, or `None` if it is no number, bool or null. Non-negative
    /// integers are [`Number::Uint`], so [`Number::Int`] is negative.
    pub fn of(data: &Data) -> Option<Self> {
        Some(match *data {
            Data::Null => Self::Null,
            Data::Uint(value) => Self::Uint(value),
            Data::Int(value) => match u64::try_from(value) {
                Ok(value) => Self::Uint(value),
                Err(_) => Self::Int(value),
            },
            Data::Float(value) => Self::Float(value),
            Data::Bool(value) => Self::Bool(value),
            _ => return None,
        })
    }

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

    fn kind_name(self) -> &'static str {
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

/// How the values of a [`Numbers`] are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Storage {
    /// Only nulls so far.
    Null,
    Int,
    Uint,
    Float,
    Bool,
}

impl Storage {
    fn name(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Int | Self::Uint => "int",
            Self::Float => "float",
            Self::Bool => "bool",
        }
    }

    /// The storage that holds the values stored so far and `number`, given whether a
    /// negative integer arrived. Integers and floats mix, so a producer may send `0` in
    /// a float trace.
    fn with(self, number: Number, negative: bool) -> Result<Self, String> {
        use Number as N;
        let fits_i64 = |value: u64| i64::try_from(value).is_ok();
        Ok(match (self, number) {
            (_, N::Null) => self,
            (Self::Null | Self::Int, N::Int(_)) => Self::Int,
            (Self::Null | Self::Int, N::Uint(value)) if fits_i64(value) => Self::Int,
            (Self::Null | Self::Int, N::Uint(_)) if !negative => Self::Uint,
            (Self::Uint, N::Uint(_)) => Self::Uint,
            (Self::Int | Self::Uint, N::Int(_) | N::Uint(_)) => {
                return Err(format!(
                    "{number} does not fit in 64 bits with the other integers of the trace"
                ));
            }
            (Self::Null | Self::Int | Self::Uint | Self::Float, N::Float(_))
            | (Self::Float, N::Int(_) | N::Uint(_)) => Self::Float,
            (Self::Null | Self::Bool, N::Bool(_)) => Self::Bool,
            _ => {
                return Err(format!(
                    "expected {}, got {}",
                    self.name(),
                    number.kind_name()
                ));
            }
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Values {
    #[default]
    Null,
    Int(Vec<i64>),
    Uint(Vec<u64>),
    Float(Vec<f64>),
    Bool(Vec<bool>),
}

impl Values {
    fn storage(&self) -> Storage {
        match self {
            Self::Null => Storage::Null,
            Self::Int(_) => Storage::Int,
            Self::Uint(_) => Storage::Uint,
            Self::Float(_) => Storage::Float,
            Self::Bool(_) => Storage::Bool,
        }
    }

    /// Appends `number`, which the storage holds, any value for null.
    fn push(&mut self, number: Number) {
        match self {
            Self::Null => {}
            Self::Int(values) => values.push(match number {
                Number::Int(value) => value,
                Number::Uint(value) => value as i64,
                _ => 0,
            }),
            Self::Uint(values) => values.push(match number {
                Number::Uint(value) => value,
                _ => 0,
            }),
            Self::Float(values) => values.push(number.as_f64()),
            Self::Bool(values) => values.push(number == Number::Bool(true)),
        }
    }

    /// The values in `storage`, which holds them all, starting with `len` values for
    /// nulls if there are none yet.
    fn convert(self, storage: Storage, len: usize) -> Self {
        match (self, storage) {
            (values, storage) if values.storage() == storage => values,
            (Self::Null, Storage::Int) => Self::Int(vec![0; len]),
            (Self::Null, Storage::Uint) => Self::Uint(vec![0; len]),
            (Self::Null, Storage::Float) => Self::Float(vec![f64::NAN; len]),
            (Self::Null, Storage::Bool) => Self::Bool(vec![false; len]),
            // only non-negative integers are converted to unsigned
            (Self::Int(values), Storage::Uint) => {
                Self::Uint(values.into_iter().map(|value| value as u64).collect())
            }
            (Self::Int(values), Storage::Float) => {
                Self::Float(values.into_iter().map(|value| value as f64).collect())
            }
            (Self::Uint(values), Storage::Float) => {
                Self::Float(values.into_iter().map(|value| value as f64).collect())
            }
            (values, storage) => {
                debug_assert!(false, "{:?} cannot become {storage:?}", values.storage());
                values
            }
        }
    }
}

/// A column of numbers of one kind, or of bools, with nulls in between.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Numbers {
    values: Values,
    /// A bit per value, set for nulls, up to the last null.
    nulls: Vec<u64>,
    len: usize,
    /// Whether a negative integer arrived, so the integers cannot become unsigned.
    negative: bool,
}

impl Numbers {
    pub fn len(&self) -> usize {
        self.len
    }

    /// The kind of the numbers, or `None` while all are null.
    pub fn kind(&self) -> Option<NumberKind> {
        match self.values {
            Values::Null => None,
            Values::Int(_) | Values::Uint(_) => Some(NumberKind::Integer),
            Values::Float(_) => Some(NumberKind::Float),
            Values::Bool(_) => Some(NumberKind::Bool),
        }
    }

    pub fn is_null(&self, index: usize) -> bool {
        self.nulls
            .get(index / 64)
            .is_some_and(|bits| bits >> (index % 64) & 1 == 1)
    }

    pub fn get(&self, index: usize) -> Number {
        if self.is_null(index) {
            return Number::Null;
        }
        match &self.values {
            Values::Null => Number::Null,
            Values::Int(values) => Number::Int(values[index]),
            Values::Uint(values) => Number::Uint(values[index]),
            Values::Float(values) => Number::Float(values[index]),
            Values::Bool(values) => Number::Bool(values[index]),
        }
    }

    /// Appends `count` nulls.
    pub fn push_nulls(&mut self, count: usize) {
        for _ in 0..count {
            self.push_unchecked(Number::Null);
        }
    }

    /// Appends all of `numbers`, or none if one does not mix with the others or with
    /// the numbers so far.
    pub fn extend(&mut self, numbers: &[Number]) -> Result<(), String> {
        let mut storage = self.values.storage();
        let mut negative = self.negative;
        for &number in numbers {
            storage = storage.with(number, negative)?;
            negative |= matches!(number, Number::Int(value) if value < 0);
        }
        self.values = std::mem::take(&mut self.values).convert(storage, self.len);
        self.negative = negative;
        for &number in numbers {
            self.push_unchecked(number);
        }
        Ok(())
    }

    /// Moves the last `count` numbers to `start`, after the numbers before it, e.g. the
    /// elements of a sample that arrived late.
    pub fn move_last_to(&mut self, start: usize, count: usize) {
        match &mut self.values {
            Values::Null => {}
            Values::Int(values) => values[start..].rotate_right(count),
            Values::Uint(values) => values[start..].rotate_right(count),
            Values::Float(values) => values[start..].rotate_right(count),
            Values::Bool(values) => values[start..].rotate_right(count),
        }
        let mut nulls: Vec<bool> = (start..self.len).map(|index| self.is_null(index)).collect();
        nulls.rotate_right(count);
        for (index, null) in (start..).zip(nulls) {
            self.set_null(index, null);
        }
    }

    /// Marks the number at `index` as null or not.
    fn set_null(&mut self, index: usize, null: bool) {
        let word = index / 64;
        if self.nulls.len() <= word {
            if !null {
                return;
            }
            self.nulls.resize(word + 1, 0);
        }
        let bit = 1 << (index % 64);
        if null {
            self.nulls[word] |= bit;
        } else {
            self.nulls[word] &= !bit;
        }
    }

    /// Appends `number`, which the storage holds.
    fn push_unchecked(&mut self, number: Number) {
        let index = self.len;
        self.values.push(number);
        self.len += 1;
        if number.is_null() {
            self.set_null(index, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(values: &[Number]) -> Result<Numbers, String> {
        let mut numbers = Numbers::default();
        for &value in values {
            numbers.extend(&[value])?;
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
        assert!(column.extend(&[Number::Int(-1)]).is_err());

        let mut signed = numbers(&[Number::Int(-1)]).unwrap();
        assert!(signed.extend(&[Number::Uint(u64::MAX)]).is_err());
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
        let mut column = numbers(&[Number::Uint(1)]).unwrap();
        let before = column.clone();
        assert!(
            column
                .extend(&[Number::Float(0.5), Number::Bool(true)])
                .is_err()
        );
        assert_eq!(column, before);
    }

    #[test]
    fn nulls_are_kept_in_every_kind() {
        let mut column = Numbers::default();
        column.push_nulls(70);
        assert_eq!(column.kind(), None);
        column.extend(&[Number::Bool(true), Number::Null]).unwrap();
        assert_eq!(column.kind(), Some(NumberKind::Bool));
        assert!(column.is_null(69));
        assert_eq!(column.get(70), Number::Bool(true));
        assert!(column.is_null(71));
        assert!(column.get(71).as_f64().is_nan());
        assert_eq!(column.get(70).as_f64(), 1.0);
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
