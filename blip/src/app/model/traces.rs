use super::numbers::{DisplayFormat, Number, NumberKind, Numbers};
use proto::Data;
use std::collections::BTreeMap;
use std::ops::Range;

fn data_kind_name(data: &Data) -> &'static str {
    match data {
        Data::Null => "null",
        Data::Uint(_) | Data::Int(_) => "int",
        Data::Float(_) => "float",
        Data::Bool(_) => "bool",
        Data::String(_) => "string",
        Data::Array(_) => "array",
        Data::Record(_) => "record",
    }
}

/// The sample values of a trace, one per timestamp.
#[derive(Debug, PartialEq)]
pub enum Samples {
    /// Only nulls so far, so the kind is not known yet.
    Empty,
    /// Numbers or bools.
    Numbers(Numbers),
    Strings(Vec<Option<String>>),
    /// Arrays of numbers or bools with the same `shape` in every sample. The elements
    /// of each sample follow each other in row-major order. Their kind is unknown
    /// while all elements were null.
    Arrays {
        shape: Vec<usize>,
        elements: Numbers,
    },
}

impl Samples {
    /// Samples of the kind of `data`, starting with `count` nulls.
    fn with_nulls(data: &Data, count: usize) -> Result<Self, String> {
        let mut numbers = Numbers::default();
        Ok(match data {
            Data::String(_) => Self::Strings(vec![None; count]),
            Data::Array(_) => {
                let shape = array_shape(data);
                numbers.push_nulls(count * shape.iter().product::<usize>());
                Self::Arrays {
                    shape,
                    elements: numbers,
                }
            }
            _ if Number::of(data).is_some() => {
                numbers.push_nulls(count);
                Self::Numbers(numbers)
            }
            _ => return Err(format!("a trace cannot hold a {}", data_kind_name(data))),
        })
    }

    fn push(&mut self, data: &Data) -> Result<(), String> {
        match (self, data) {
            (Self::Empty, Data::Null) => {}
            (Self::Numbers(numbers), data) => {
                let number = Number::of(data).ok_or_else(|| {
                    let kind = numbers.kind().map_or("null", NumberKind::name);
                    format!("expected {kind}, got {}", data_kind_name(data))
                })?;
                numbers.extend(&[number])?;
            }
            (Self::Strings(values), Data::Null) => values.push(None),
            (Self::Strings(values), Data::String(value)) => values.push(Some(value.clone())),
            (Self::Arrays { shape, elements }, data) => {
                let mut numbers = Vec::with_capacity(shape.iter().product());
                flatten(data, shape, &mut numbers)?;
                elements.extend(&numbers)?;
            }
            (samples, data) => {
                return Err(format!(
                    "expected {}, got {}",
                    samples.type_label(),
                    data_kind_name(data)
                ));
            }
        }
        Ok(())
    }

    /// Moves the last sample to `index`, after the samples before it.
    fn move_last_to(&mut self, index: usize) {
        match self {
            Self::Empty => {}
            Self::Numbers(numbers) => numbers.move_last_to(index, 1),
            Self::Strings(values) => values[index..].rotate_right(1),
            Self::Arrays { shape, elements } => {
                let len = shape.iter().product();
                elements.move_last_to(index * len, len);
            }
        }
    }

    /// The elements of the `index`th sample of an array trace, in row-major order.
    pub fn array_sample(&self, index: usize) -> Option<Elements<'_>> {
        let Self::Arrays { shape, elements } = self else {
            return None;
        };
        let len = shape.iter().product::<usize>();
        ((index + 1) * len <= elements.len()).then_some(Elements {
            numbers: elements,
            start: index * len,
            len,
        })
    }

    /// The `index`th sample written as text, arrays with at most `max_len` elements per
    /// dimension and a line per row as in [`format_array`], and numbers in `format`,
    /// floats with at most `digits` significant digits or all of them if `digits` is
    /// `None`.
    pub fn format(
        &self,
        index: usize,
        max_len: usize,
        digits: Option<usize>,
        format: DisplayFormat,
    ) -> String {
        match self {
            Self::Empty => "null".to_owned(),
            Self::Numbers(numbers) => numbers.get(index).format(digits, format),
            Self::Strings(values) => values[index].clone().unwrap_or_else(|| "null".to_owned()),
            Self::Arrays { shape, .. } => {
                let elements: Vec<Number> = self
                    .array_sample(index)
                    .map(|elements| elements.iter().collect())
                    .unwrap_or_default();
                format_array(shape, &elements, max_len, &|number: Number| {
                    number.format(digits, format)
                })
            }
        }
    }

    /// The type of the samples, e.g. `float` or `int[2×3]`.
    pub fn type_label(&self) -> String {
        match self {
            Self::Empty => "null".to_owned(),
            Self::Numbers(numbers) => numbers.kind().map_or("null", NumberKind::name).to_owned(),
            Self::Strings(_) => "string".to_owned(),
            Self::Arrays { shape, elements } => {
                let dimensions: Vec<String> = shape.iter().map(usize::to_string).collect();
                format!(
                    "{}[{}]",
                    elements.kind().map_or("null", NumberKind::name),
                    dimensions.join("×")
                )
            }
        }
    }
}

/// The elements of one sample of an array trace.
#[derive(Clone, Copy)]
pub struct Elements<'a> {
    numbers: &'a Numbers,
    start: usize,
    len: usize,
}

impl<'a> Elements<'a> {
    pub fn get(&self, index: usize) -> Option<Number> {
        (index < self.len).then(|| self.numbers.get(self.start + index))
    }

    pub fn iter(&self) -> impl Iterator<Item = Number> + 'a {
        let numbers = self.numbers;
        (self.start..self.start + self.len).map(|index| numbers.get(index))
    }
}

/// The `elements` of an array of `shape` written with at most `max_len` elements per
/// dimension, each written by `format`. A 1D array is written on one line like
/// `[1, 2]`, a larger one with a line per row and the columns aligned:
///
/// ```text
/// [[ 1, 2],
///  [10, 3]]
/// ```
///
/// with an empty line between the 2D arrays of a 3D one, and so on.
fn format_array<T: Copy>(
    shape: &[usize],
    elements: &[T],
    max_len: usize,
    format: &dyn Fn(T) -> String,
) -> String {
    let columns = shape.last().copied().unwrap_or(1);
    let is_shown = |mut flat_index: usize| {
        shape.iter().rev().all(|&len| {
            let index = flat_index % len;
            flat_index /= len;
            index < max_len
        })
    };
    let texts: Vec<Option<String>> = elements
        .iter()
        .enumerate()
        .map(|(flat_index, &value)| is_shown(flat_index).then(|| format(value)))
        .collect();
    let mut widths = vec![0; columns.min(max_len)];
    if shape.len() > 1 {
        for (flat_index, text) in texts.iter().enumerate() {
            if let (Some(text), Some(width)) = (text, widths.get_mut(flat_index % columns)) {
                *width = (*width).max(text.chars().count());
            }
        }
    }
    format_nested(shape, &texts, max_len, &widths, 0)
}

/// The `texts` of the elements of a nested array of `shape` at `depth` written as in
/// [`format_array`], with the elements padded to the `widths` of their columns.
fn format_nested(
    shape: &[usize],
    texts: &[Option<String>],
    max_len: usize,
    widths: &[usize],
    depth: usize,
) -> String {
    let Some((&len, inner)) = shape.split_first() else {
        return texts.first().cloned().flatten().unwrap_or_default();
    };
    let stride = inner.iter().product::<usize>();
    let mut parts: Vec<String> = texts
        .chunks(stride.max(1))
        .take(len.min(max_len))
        .enumerate()
        .map(|(index, chunk)| {
            let text = format_nested(inner, chunk, max_len, widths, depth + 1);
            match widths.get(index).filter(|_| inner.is_empty()) {
                Some(&width) => format!("{text:>width$}"),
                None => text,
            }
        })
        .collect();
    if len > max_len {
        parts.push("…".to_owned());
    }
    let separator = if inner.is_empty() {
        ", ".to_owned()
    } else {
        // as many line breaks as dimensions inside, so blocks of rows stand apart
        format!(",{}{}", "\n".repeat(inner.len()), " ".repeat(depth + 1))
    };
    format!("[{}]", parts.join(&separator))
}

/// The dimensions of the nested arrays `data`, following the first elements.
fn array_shape(mut data: &Data) -> Vec<usize> {
    let mut shape = vec![];
    while let Data::Array(elements) = data {
        shape.push(elements.len());
        match elements.first() {
            Some(first) => data = first,
            None => break,
        }
    }
    shape
}

/// Appends the elements of the array `data` of `shape` to `numbers` in row-major
/// order.
fn flatten(data: &Data, shape: &[usize], numbers: &mut Vec<Number>) -> Result<(), String> {
    match (shape.split_first(), data) {
        (_, Data::Null) => {
            numbers.extend(std::iter::repeat_n(Number::Null, shape.iter().product()))
        }
        (Some((&len, inner)), Data::Array(elements)) => {
            if elements.len() != len {
                return Err(format!(
                    "expected an array of {len} elements, got {}",
                    elements.len()
                ));
            }
            for element in elements {
                flatten(element, inner, numbers)?;
            }
        }
        (Some(_), data) => return Err(format!("expected an array, got {}", data_kind_name(data))),
        (None, data) => numbers.push(Number::of(data).ok_or_else(|| {
            format!(
                "array elements must be numbers or bools, got {}",
                data_kind_name(data)
            )
        })?),
    }
    Ok(())
}

pub struct Trace {
    pub path: String,
    /// Ascending: a sample older than the newest, e.g. of another outpost sending the
    /// same path, is put in its place, after those with the same timestamp.
    pub timestamps: Vec<u64>,
    pub samples: Samples,
    /// For a field of a record, the index of the `.` in `path` before the first field
    /// name, e.g. 3 for `imu.acc.z`.
    pub fields_start: Option<usize>,
    /// Whether a dropped sample was reported, so it is only reported once.
    warned: bool,
    /// The lowest and highest finite value of all samples, one pair per element of an
    /// array trace, `None` where all were null so far.
    ranges: Vec<Option<(f64, f64)>>,
}

impl Trace {
    pub fn new(path: String, fields_start: Option<usize>) -> Self {
        Self {
            path,
            fields_start,
            timestamps: vec![],
            samples: Samples::Empty,
            warned: false,
            ranges: vec![],
        }
    }

    /// Adds a sample in the order of the timestamps, unless its kind does not match the
    /// previous samples.
    fn push(&mut self, timestamp: u64, data: &Data) -> Result<(), String> {
        if self.samples == Samples::Empty && *data != Data::Null {
            self.samples = Samples::with_nulls(data, self.timestamps.len())?;
        }
        self.samples.push(data)?;
        self.extend_ranges(self.timestamps.len());
        self.timestamps.push(timestamp);
        let at = self.timestamps.partition_point(|&t| t <= timestamp);
        if at < self.timestamps.len() {
            self.timestamps[at..].rotate_right(1);
            self.samples.move_last_to(at);
        }
        Ok(())
    }

    /// Widens [`Trace::ranges`] to the values of the `index`th sample.
    fn extend_ranges(&mut self, index: usize) {
        fn widen(range: &mut Option<(f64, f64)>, number: Number) {
            let value = number.as_f64();
            if value.is_finite() {
                *range = Some(range.map_or((value, value), |(low, high)| {
                    (low.min(value), high.max(value))
                }));
            }
        }
        match &self.samples {
            Samples::Numbers(numbers) => {
                self.ranges.resize(1, None);
                widen(&mut self.ranges[0], numbers.get(index));
            }
            Samples::Arrays { shape, .. } => {
                self.ranges.resize(shape.iter().product(), None);
                if let Some(elements) = self.samples.array_sample(index) {
                    for (range, number) in self.ranges.iter_mut().zip(elements.iter()) {
                        widen(range, number);
                    }
                }
            }
            Samples::Empty | Samples::Strings(_) => {}
        }
    }

    /// The numbers of a numeric trace.
    fn series(&self) -> Option<Series<'_>> {
        let Samples::Numbers(numbers) = &self.samples else {
            return None;
        };
        Some(Series {
            timestamps: &self.timestamps,
            numbers,
            stride: 1,
            offset: 0,
            kind: numbers.kind().unwrap_or(NumberKind::Float),
            range: self.ranges.first().copied().flatten(),
        })
    }
}

/// The numbers of a numeric trace or of one element of an array trace.
#[derive(Clone, Copy)]
pub struct Series<'a> {
    pub timestamps: &'a [u64],
    numbers: &'a Numbers,
    /// Distance between the values of two samples in `numbers`.
    stride: usize,
    /// Index of the value of the first sample in `numbers`.
    offset: usize,
    pub kind: NumberKind,
    /// The lowest and highest finite value of all samples, also of those outside a
    /// [`Series::slice`], `None` if there is none.
    pub range: Option<(f64, f64)>,
}

impl Series<'_> {
    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    /// The value of the `index`th sample as a float for plotting, NaN for null.
    pub fn value(&self, index: usize) -> f64 {
        self.number(index).as_f64()
    }

    /// The value of the `index`th sample.
    pub fn number(&self, index: usize) -> Number {
        self.numbers.get(index * self.stride + self.offset)
    }

    /// The samples within `range`.
    pub fn slice(&self, range: Range<usize>) -> Self {
        Self {
            timestamps: &self.timestamps[range.clone()],
            offset: self.offset + range.start * self.stride,
            ..*self
        }
    }
}

/// The path of an element of an array trace, e.g. `adc[1][0]`.
pub fn element_path(path: &str, indices: &[usize]) -> String {
    indices
        .iter()
        .fold(path.to_owned(), |path, index| format!("{path}[{index}]"))
}

/// The trace path and the indices of an element path written by [`element_path`].
fn parse_element_path(path: &str) -> Option<(&str, Vec<usize>)> {
    let segment_start = path.rfind('/').map_or(0, |slash| slash + 1);
    let open = segment_start + path[segment_start..].find('[')?;
    let indices = path[open..].strip_prefix('[')?.strip_suffix(']')?;
    let indices = indices
        .split("][")
        .map(|index| index.parse().ok())
        .collect::<Option<_>>()?;
    Some((&path[..open], indices))
}

pub struct Traces {
    /// Sorted, so the traces of a record follow each other.
    map: BTreeMap<String, Trace>,
    /// The units of the traces by their paths, as received in their metadata, also
    /// for traces without samples yet.
    units: BTreeMap<String, String>,
}

impl Default for Traces {
    fn default() -> Self {
        Self::new()
    }
}

impl Traces {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            units: BTreeMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Trace> {
        self.map.values()
    }

    /// The number of traces.
    pub fn count(&self) -> usize {
        self.map.len()
    }

    /// Sets the unit of the trace at `path`, an empty `unit` for none.
    pub fn set_unit(&mut self, path: &str, unit: &str) {
        if unit.is_empty() {
            self.units.remove(path);
        } else if self.units.get(path).is_none_or(|known| known != unit) {
            self.units.insert(path.to_owned(), unit.to_owned());
        }
    }

    /// The unit of the trace at `path`, or of the array trace of an element written by
    /// [`element_path`].
    pub fn unit(&self, path: &str) -> Option<&str> {
        let unit = match self.units.get(path) {
            Some(unit) => Some(unit),
            None => self.units.get(parse_element_path(path)?.0),
        };
        unit.map(String::as_str)
    }

    pub fn get(&self, path: &str) -> Option<&Trace> {
        self.map.get(path)
    }

    /// The numbers shown for `path`: a numeric trace, or an element of an array
    /// trace written by [`element_path`].
    pub fn series(&self, path: &str) -> Option<Series<'_>> {
        if let Some(trace) = self.map.get(path) {
            return trace.series();
        }
        let (path, indices) = parse_element_path(path)?;
        let trace = self.map.get(path)?;
        let Samples::Arrays { shape, elements } = &trace.samples else {
            return None;
        };
        if indices.len() != shape.len()
            || indices.iter().zip(shape).any(|(index, len)| index >= len)
        {
            return None;
        }
        let offset = indices
            .iter()
            .zip(shape)
            .fold(0, |offset, (index, len)| offset * len + index);
        Some(Series {
            timestamps: &trace.timestamps,
            numbers: elements,
            stride: shape.iter().product(),
            offset,
            kind: elements.kind().unwrap_or(NumberKind::Float),
            range: trace.ranges.get(offset).copied().flatten(),
        })
    }

    pub fn append(&mut self, value: &proto::Value) {
        self.insert(&value.path, None, value.timestamp, &value.value);
    }

    /// Appends `data` to the trace at `path`, or to the traces of its fields if it is
    /// a record. The fields of a record are joined to its path with `.`, e.g. `imu.acc`.
    /// `fields_start` is the [`Trace::fields_start`] of a new trace.
    fn insert(&mut self, path: &str, fields_start: Option<usize>, timestamp: u64, data: &Data) {
        match data {
            Data::Record(fields)
                if self
                    .map
                    .get(path)
                    .is_none_or(|trace| trace.samples == Samples::Empty) =>
            {
                // a trace that only received nulls so far turns out to be a record
                self.map.remove(path);
                let fields_start = fields_start.unwrap_or(path.len());
                for (name, value) in fields {
                    self.insert(
                        &format!("{path}.{name}"),
                        Some(fields_start),
                        timestamp,
                        value,
                    );
                }
            }
            Data::Null
                if !self.map.contains_key(path) && self.field_paths(path).next().is_some() =>
            {
                // a missing record leaves a gap in all its fields
                let fields: Vec<String> = self.field_paths(path).map(str::to_owned).collect();
                for field in fields {
                    self.insert(&field, None, timestamp, data);
                }
            }
            _ => {
                let trace = self
                    .map
                    .entry(path.to_owned())
                    .or_insert_with(|| Trace::new(path.to_owned(), fields_start));
                if let Err(error) = trace.push(timestamp, data)
                    && !trace.warned
                {
                    trace.warned = true;
                    log::warn!(
                        "{path}: dropped a sample, {error} (further ones are dropped silently)"
                    );
                }
            }
        }
    }

    /// The paths of the fields of the record at `path`, also of nested records, but not
    /// of traces whose names only contain a `.`, e.g. `v1.2`.
    pub fn field_paths(&self, path: &str) -> impl Iterator<Item = &str> {
        let prefix = format!("{path}.");
        self.map
            .range(prefix.clone()..)
            .take_while(move |(field, _)| field.starts_with(&prefix))
            .filter(move |(_, trace)| trace.fields_start.is_some_and(|start| start <= path.len()))
            .map(|(field, _)| field.as_str())
    }

    /// Timestamps of the first and the newest value of all traces.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        self.map
            .values()
            .filter_map(|trace| Some((*trace.timestamps.first()?, *trace.timestamps.last()?)))
            .reduce(|(first_a, last_a), (first_b, last_b)| {
                (first_a.min(first_b), last_a.max(last_b))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(fields: &[(&str, Data)]) -> Data {
        Data::Record(
            fields
                .iter()
                .map(|(name, value)| (name.to_string(), value.clone()))
                .collect(),
        )
    }

    fn array(elements: &[f64]) -> Data {
        Data::Array(elements.iter().copied().map(Data::Float).collect())
    }

    fn traces(samples: &[(&str, Data)]) -> Traces {
        let mut traces = Traces::new();
        for (timestamp, (path, value)) in samples.iter().enumerate() {
            traces.append(&proto::Value {
                timestamp: timestamp as u64,
                path: path.to_string(),
                value: value.clone(),
            });
        }
        traces
    }

    fn values(traces: &Traces, path: &str) -> Vec<f64> {
        let series = traces.series(path).unwrap();
        (0..series.len()).map(|index| series.value(index)).collect()
    }

    /// Compares the bit patterns, so NaN equals NaN.
    fn assert_values(actual: Vec<f64>, expected: &[f64]) {
        let bits = |values: &[f64]| {
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&actual), bits(expected));
    }

    #[test]
    fn series_know_the_range_of_all_their_values() {
        let traces = traces(&[
            ("a", Data::Null),
            ("a", Data::Float(2.0)),
            ("a", Data::Float(-1.0)),
            ("a", Data::Float(f64::NAN)),
            ("a", Data::Int(5)),
            ("v", array(&[1.0, 10.0])),
            ("v", array(&[-3.0, 20.0])),
            ("n", Data::Null),
        ]);
        let a = traces.series("a").unwrap();
        assert_eq!(a.range, Some((-1.0, 5.0)));
        // also in a slice of the samples
        assert_eq!(a.slice(0..1).range, Some((-1.0, 5.0)));
        assert_eq!(traces.series("v[0]").unwrap().range, Some((-3.0, 1.0)));
        assert_eq!(traces.series("v[1]").unwrap().range, Some((10.0, 20.0)));
        assert!(traces.series("n").is_none_or(|n| n.range.is_none()));
    }

    #[test]
    fn records_split_into_field_traces() {
        let imu = record(&[
            ("x", Data::Float(0.5)),
            ("acc", record(&[("z", Data::Uint(9))])),
        ]);
        let traces = traces(&[("imu", imu)]);
        let paths: Vec<&str> = traces.iter().map(|trace| trace.path.as_str()).collect();
        assert_eq!(paths, ["imu.acc.z", "imu.x"]);
        assert_values(values(&traces, "imu.x"), &[0.5]);
        assert_eq!(
            traces.series("imu.acc.z").unwrap().kind,
            NumberKind::Integer
        );
    }

    #[test]
    fn fields_start_marks_where_record_fields_begin() {
        let imu = record(&[("acc", record(&[("z", Data::Uint(9))]))]);
        let traces = traces(&[("sensors/imu", imu), ("v1.2", Data::Float(1.0))]);
        let fields_start = |path| traces.get(path).unwrap().fields_start;
        assert_eq!(fields_start("sensors/imu.acc.z"), Some(11));
        // a dot in a path that is not a record is part of the name
        assert_eq!(fields_start("v1.2"), None);
    }

    #[test]
    fn late_samples_are_put_in_the_order_of_their_timestamps() {
        let mut traces = Traces::new();
        let mut push = |timestamp, path: &str, data: Data| {
            traces.append(&proto::Value {
                timestamp,
                path: path.to_owned(),
                value: data,
            });
        };
        for (timestamp, value) in [(10, 1.0), (30, 3.0), (20, 2.0), (5, 0.5), (30, 4.0)] {
            push(timestamp, "n", Data::Float(value));
            push(timestamp, "s", Data::String(value.to_string()));
            push(timestamp, "a", array(&[value, -value]));
        }
        push(25, "n", Data::Null);

        let n = traces.get("n").unwrap();
        assert_eq!(n.timestamps, [5, 10, 20, 25, 30, 30]);
        assert_values(values(&traces, "n"), &[0.5, 1.0, 2.0, f64::NAN, 3.0, 4.0]);
        let Samples::Numbers(numbers) = &n.samples else {
            panic!("numbers expected");
        };
        assert!(numbers.is_null(3) && !numbers.is_null(4));
        let s = traces.get("s").unwrap();
        let texts: Vec<String> = (0..5)
            .map(|index| s.samples.format(index, 4, None, DisplayFormat::Default))
            .collect();
        assert_eq!(texts, ["0.5", "1", "2", "3", "4"]);
        let a = traces.get("a").unwrap();
        let second: Vec<Number> = (0..5)
            .map(|index| a.samples.array_sample(index).unwrap().get(1).unwrap())
            .collect();
        assert_eq!(second, [-0.5, -1.0, -2.0, -3.0, -4.0].map(Number::Float));
    }

    #[test]
    fn integer_traces_switch_to_float() {
        let traces = traces(&[
            ("a", Data::Uint(0)),
            ("a", Data::Int(-1)),
            ("a", Data::Float(0.5)),
        ]);
        let series = traces.series("a").unwrap();
        assert_eq!(series.kind, NumberKind::Float);
        assert_values(values(&traces, "a"), &[0.0, -1.0, 0.5]);
    }

    #[test]
    fn samples_of_another_kind_are_dropped() {
        let traces = traces(&[
            ("a", Data::Float(1.0)),
            ("a", Data::from("text")),
            ("a", Data::Bool(true)),
            ("a", Data::Float(2.0)),
        ]);
        assert_values(values(&traces, "a"), &[1.0, 2.0]);
        assert_eq!(traces.get("a").unwrap().timestamps, [0, 3]);
        assert!(traces.get("a").unwrap().warned);
    }

    #[test]
    fn a_scalar_trace_does_not_turn_into_a_record() {
        let traces = traces(&[
            ("a", Data::Float(1.0)),
            ("a", record(&[("x", Data::Float(2.0))])),
        ]);
        assert!(traces.get("a.x").is_none());
        assert!(traces.get("a").unwrap().warned);
    }

    #[test]
    fn null_is_a_gap() {
        let bools = traces(&[
            ("a", Data::Null),
            ("a", Data::Bool(true)),
            ("a", Data::Null),
        ]);
        assert_eq!(bools.series("a").unwrap().kind, NumberKind::Bool);
        assert_values(values(&bools, "a"), &[f64::NAN, 1.0, f64::NAN]);

        let strings = traces(&[("s", Data::Null), ("s", Data::from("on"))]);
        assert_eq!(
            strings.get("s").unwrap().samples,
            Samples::Strings(vec![None, Some("on".to_owned())])
        );
    }

    #[test]
    fn a_null_record_is_a_gap_in_all_fields() {
        let imu = record(&[("x", Data::Float(1.0)), ("y", Data::Float(2.0))]);
        let traces = traces(&[("imu", Data::Null), ("imu", imu), ("imu", Data::Null)]);
        // the leading null is dropped with the trace it created
        assert!(traces.get("imu").is_none());
        assert_values(values(&traces, "imu.x"), &[1.0, f64::NAN]);
        assert_values(values(&traces, "imu.y"), &[2.0, f64::NAN]);
    }

    #[test]
    fn a_null_is_no_gap_in_traces_with_a_dot_in_their_names() {
        let imu = record(&[("x", Data::Float(1.0))]);
        let traces = traces(&[
            ("v1.2", Data::Float(1.0)),
            ("imu", imu),
            ("v1", Data::Null),
            ("imu.x", Data::Null),
        ]);
        assert_values(values(&traces, "v1.2"), &[1.0]);
        assert_eq!(traces.get("v1").unwrap().samples, Samples::Empty);
        // `imu.x` is a field, but not a record
        assert_values(values(&traces, "imu.x"), &[1.0, f64::NAN]);
    }

    #[test]
    fn arrays_keep_their_shape() {
        let matrix = |a: f64| Data::Array(vec![array(&[a, 1.0, 2.0]), array(&[3.0, 4.0, 5.0])]);
        let traces = traces(&[
            ("m", Data::Null),
            ("m", matrix(0.0)),
            ("m", Data::Array(vec![array(&[0.0])])),
            ("m", Data::Array(vec![array(&[0.0, 1.0, 2.0]), Data::Null])),
            ("m", matrix(10.0)),
        ]);
        let trace = traces.get("m").unwrap();
        assert_eq!(trace.samples.type_label(), "float[2×3]");
        assert_eq!(trace.timestamps, [0, 1, 3, 4]);
        assert_values(values(&traces, "m[0][0]"), &[f64::NAN, 0.0, 0.0, 10.0]);
        assert_values(values(&traces, "m[1][2]"), &[f64::NAN, 5.0, f64::NAN, 5.0]);
        assert!(traces.series("m[2][0]").is_none());
        assert!(traces.series("m[0]").is_none());
        assert!(traces.series("m").is_none());
    }

    #[test]
    fn arrays_hold_numbers_or_bools_only() {
        let traces = traces(&[
            ("a", Data::Array(vec![Data::Uint(1), Data::Null])),
            ("a", Data::Array(vec![Data::Float(0.5), Data::Uint(2)])),
            ("a", Data::Array(vec![Data::Bool(true), Data::Uint(2)])),
            ("a", Data::Array(vec![Data::from("x"), Data::Uint(2)])),
        ]);
        assert_eq!(traces.get("a").unwrap().samples.type_label(), "float[2]");
        assert_values(values(&traces, "a[1]"), &[f64::NAN, 2.0]);
    }

    #[test]
    fn element_paths_round_trip() {
        assert_eq!(element_path("a/b", &[1, 20]), "a/b[1][20]");
        assert_eq!(parse_element_path("a/b[1][20]"), Some(("a/b", vec![1, 20])));
        assert_eq!(parse_element_path("a[1]/b"), None);
        assert_eq!(parse_element_path("a[x]"), None);
        assert_eq!(parse_element_path("a"), None);
    }

    #[test]
    fn series_slices_keep_the_element() {
        let traces = traces(&[
            ("a", array(&[0.0, 1.0])),
            ("a", array(&[2.0, 3.0])),
            ("a", array(&[4.0, 5.0])),
        ]);
        let slice = traces.series("a[1]").unwrap().slice(1..3);
        assert_eq!(slice.timestamps, [1, 2]);
        assert_eq!((slice.value(0), slice.value(1)), (3.0, 5.0));
    }

    #[test]
    fn samples_are_formatted_as_text() {
        let matrix = Data::Array(vec![array(&[0.5, 1.0, 2.0]), array(&[3.0, 4.0, 5.0])]);
        let traces = traces(&[
            ("m", matrix),
            ("s", Data::from("on")),
            ("s", Data::Null),
            ("i", Data::Uint(7)),
            ("f", Data::Float(0.031_414_634_623_640_13)),
        ]);
        let format = |path, index, max_len, digits| {
            let samples = &traces.get(path).unwrap().samples;
            samples.format(index, max_len, digits, DisplayFormat::Default)
        };
        assert_eq!(format("m", 0, 8, None), "[[0.5, 1, 2],\n [  3, 4, 5]]");
        assert_eq!(format("m", 0, 2, None), "[[0.5, 1, …],\n [  3, 4, …]]");
        assert_eq!(format("m", 0, 1, None), "[[0.5, …],\n …]");
        assert_eq!(format("s", 0, 8, None), "on");
        assert_eq!(format("s", 1, 8, None), "null");
        assert_eq!(format("i", 0, 8, Some(2)), "7");
        assert_eq!(format("f", 0, 8, Some(6)), "0.0314146");
        assert_eq!(format("f", 0, 8, None), "0.03141463462364013");
        let samples = &traces.get("m").unwrap().samples;
        assert_eq!(
            samples.array_sample(0).unwrap().get(3),
            Some(Number::Float(3.0))
        );
        assert!(samples.array_sample(1).is_none());
    }

    #[test]
    fn array_elements_are_written_in_their_display_format() {
        let traces = traces(&[("a", Data::Array(vec![Data::Uint(10), Data::Uint(255)]))]);
        let samples = &traces.get("a").unwrap().samples;
        assert_eq!(
            samples.format(0, 8, None, DisplayFormat::Hex),
            "[0xa, 0xff]"
        );
    }

    #[test]
    fn integers_are_exact() {
        let big = Data::Uint((1 << 53) + 1);
        let traces = traces(&[("a", big.clone()), ("m", Data::Array(vec![big]))]);
        let samples = |path| &traces.get(path).unwrap().samples;
        assert_eq!(
            samples("a").format(0, 8, None, DisplayFormat::Default),
            "9007199254740993"
        );
        assert_eq!(
            samples("m").format(0, 8, None, DisplayFormat::Default),
            "[9007199254740993]"
        );
        let series = traces.series("m[0]").unwrap();
        assert_eq!(series.number(0), Number::Int((1 << 53) + 1));
    }

    #[test]
    fn elements_have_the_unit_of_their_array() {
        let mut traces = Traces::new();
        traces.set_unit("imu.acc", "m/s²");
        traces.set_unit("speed", "");
        assert_eq!(traces.unit("imu.acc"), Some("m/s²"));
        assert_eq!(traces.unit("imu.acc[2]"), Some("m/s²"));
        assert_eq!(traces.unit("imu.gyro"), None);
        assert_eq!(traces.unit("speed"), None);
    }

    #[test]
    fn arrays_are_formatted_with_a_line_per_row() {
        let format = |value: f64| value.to_string();
        assert_eq!(
            format_array(&[3], &[1.0, 20.0, 3.0], 8, &format),
            "[1, 20, 3]"
        );
        assert_eq!(
            format_array(&[2, 2, 1], &[1.0, 2.0, 3.0, 40.0], 8, &format),
            "[[[ 1],\n  [ 2]],\n\n [[ 3],\n  [40]]]"
        );
        assert_eq!(format_array(&[0], &[], 8, &format), "[]");
    }
}
