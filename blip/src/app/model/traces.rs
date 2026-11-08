//! The values received, as tables: each path that values are sent to is a table with
//! a column per leaf of its values, e.g. the fields of a record, sharing the
//! timestamps of the table. A column is a trace.

use super::decode::{Flat, Leaf};
use super::growing::Growing;
use super::numbers::{DisplayFormat, Extent, Number, NumberKind, Numbers};
use super::sorted::upper_bound;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

/// A value older than this many values of its path is dropped: putting it in its place
/// moves all newer ones, which takes about 1 ms for this many, but a minute for a
/// stream of values from a clock minutes behind, e.g. of a second outpost.
const MAX_LATE: usize = 1 << 16;

/// The value of a string trace that is null.
const NULL_TEXT: u32 = u32::MAX;

/// The texts of a string trace, each different text kept once.
#[derive(Debug, Default, PartialEq)]
pub struct Strings {
    /// The index of the text of each sample in `texts`, [`NULL_TEXT`] for null.
    ids: Growing<u32>,
    texts: Vec<Box<str>>,
    known: HashMap<Box<str>, u32>,
}

impl Strings {
    fn with_nulls(count: usize) -> Self {
        Self {
            ids: vec![NULL_TEXT; count].into(),
            ..Self::default()
        }
    }

    fn push(&mut self, text: Option<&str>) {
        let id = match text {
            None => NULL_TEXT,
            Some(text) => match self.known.get(text) {
                Some(&id) => id,
                None => {
                    let id = self.texts.len() as u32;
                    self.texts.push(text.into());
                    self.known.insert(text.into(), id);
                    id
                }
            },
        };
        self.ids.push(id);
    }

    /// The text of the `index`th sample, `None` for null.
    pub fn get(&self, index: usize) -> Option<&str> {
        self.texts.get(self.ids[index] as usize).map(|text| &**text)
    }

    /// Whether the `a`th and the `b`th sample are the same text or both null.
    pub fn same(&self, a: usize, b: usize) -> bool {
        self.ids[a] == self.ids[b]
    }
}

/// The samples of a trace.
#[derive(Debug, PartialEq)]
pub enum Samples {
    /// This many nulls so far, so the kind is not known yet.
    Empty(usize),
    /// Numbers or bools.
    Numbers(Numbers),
    Strings(Strings),
    /// Arrays of numbers or bools with the same `shape` in every sample, the elements
    /// of a sample in a row of `elements`. Their kind is unknown while all elements
    /// were null.
    Arrays {
        shape: Vec<usize>,
        elements: Numbers,
    },
}

/// The kind of `leaf` as named in warnings.
fn leaf_kind(leaf: &Leaf) -> &'static str {
    match leaf {
        Leaf::Null => "null",
        Leaf::Number(number) => number.kind_name(),
        Leaf::Text(_) => "string",
        Leaf::Array { .. } => "array",
        Leaf::Invalid(_) => "invalid value",
    }
}

impl Samples {
    /// The number of samples.
    pub fn len(&self) -> usize {
        match self {
            Self::Empty(count) => *count,
            Self::Numbers(numbers) => numbers.rows(),
            Self::Strings(strings) => strings.ids.len(),
            Self::Arrays { elements, .. } => elements.rows(),
        }
    }

    fn push_null(&mut self) {
        match self {
            Self::Empty(count) => *count += 1,
            Self::Numbers(numbers) => numbers.push_nulls(1),
            Self::Strings(strings) => strings.push(None),
            Self::Arrays { elements, .. } => elements.push_nulls(1),
        }
    }

    /// What decides what views can show of the samples: their variant and the kind
    /// of their numbers.
    fn kind(&self) -> (u8, Option<NumberKind>) {
        match self {
            Self::Empty(_) => (0, None),
            Self::Numbers(numbers) => (1, numbers.kind()),
            Self::Strings(_) => (2, None),
            Self::Arrays { elements, .. } => (3, elements.kind()),
        }
    }

    /// Appends `leaf` of `flat` decoded from `frame`, unless its kind does not match.
    fn push(&mut self, leaf: &Leaf, flat: &Flat, frame: &[u8]) -> Result<(), String> {
        if let (Self::Empty(count), leaf) = (&*self, leaf) {
            let count = *count;
            *self = match leaf {
                Leaf::Null => Self::Empty(count),
                Leaf::Number(_) => Self::Numbers(Numbers::default()),
                Leaf::Text(_) => Self::Strings(Strings::default()),
                Leaf::Array { shape, .. } => {
                    let shape = flat.shapes[shape.clone()].to_vec();
                    Self::Arrays {
                        elements: Numbers::new(shape.iter().product()),
                        shape,
                    }
                }
                Leaf::Invalid(problem) => return Err(problem.clone()),
            };
            match self {
                Self::Empty(_) => {}
                Self::Numbers(numbers)
                | Self::Arrays {
                    elements: numbers, ..
                } => {
                    numbers.push_nulls(count);
                }
                Self::Strings(strings) => *strings = Strings::with_nulls(count),
            }
        }
        match (self, leaf) {
            (samples, Leaf::Null) => samples.push_null(),
            (Self::Numbers(numbers), Leaf::Number(number)) => numbers.push_row(&[*number])?,
            (Self::Strings(strings), Leaf::Text(text)) => {
                let text = std::str::from_utf8(&frame[text.clone()]).expect("checked when read");
                strings.push(Some(text));
            }
            (
                Self::Arrays { shape, elements },
                Leaf::Array {
                    shape: got,
                    elements: values,
                },
            ) => {
                let got = &flat.shapes[got.clone()];
                if got != shape.as_slice() {
                    return Err(format!("expected an array of shape {shape:?}, got {got:?}"));
                }
                elements.push_row(&flat.numbers[values.clone()])?;
            }
            (_, Leaf::Invalid(problem)) => return Err(problem.clone()),
            (samples, leaf) => {
                return Err(format!(
                    "expected {}, got {}",
                    samples.type_label(),
                    leaf_kind(leaf)
                ));
            }
        }
        Ok(())
    }

    /// Moves the last sample to `index`, after the samples before it.
    fn move_last_to(&mut self, index: usize) {
        match self {
            Self::Empty(_) => {}
            Self::Numbers(numbers)
            | Self::Arrays {
                elements: numbers, ..
            } => {
                numbers.move_last_row_to(index);
            }
            Self::Strings(strings) => strings.ids.rotate_tail(index, 1),
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
            Self::Empty(_) => "null".to_owned(),
            Self::Numbers(numbers) => numbers.get(index).format(digits, format),
            Self::Strings(strings) => strings.get(index).unwrap_or("null").to_owned(),
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
            Self::Empty(_) => "null".to_owned(),
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

/// A trace as views see it: its samples with the timestamps of its table.
#[derive(Clone, Copy)]
pub struct Trace<'a> {
    pub path: &'a str,
    /// Ascending: a sample older than the newest, e.g. of another outpost sending the
    /// same path, is put in its place, after those with the same timestamp.
    pub timestamps: &'a [u64],
    pub samples: &'a Samples,
    /// For a field of a record, the index of the `.` in `path` before the first field
    /// name, e.g. 3 for `imu.acc.z`.
    pub fields_start: Option<usize>,
}

impl<'a> Trace<'a> {
    /// The lowest and highest finite value of all samples, of all elements of an array
    /// trace, `None` if there is none.
    pub fn value_range(&self) -> Option<(f64, f64)> {
        match self.samples {
            Samples::Numbers(numbers)
            | Samples::Arrays {
                elements: numbers, ..
            } => numbers.total_range(),
            _ => None,
        }
    }

    /// The numbers of a numeric trace.
    fn series(self) -> Option<Series<'a>> {
        let Samples::Numbers(numbers) = self.samples else {
            return None;
        };
        Some(Series::new(self.timestamps, numbers, 0))
    }
}

#[cfg(test)]
impl Trace<'static> {
    /// A trace without timestamps, kept for the rest of a test.
    pub fn leaked(path: &str, fields_start: Option<usize>, samples: Samples) -> Self {
        Self {
            path: Box::leak(path.into()),
            timestamps: &[],
            samples: Box::leak(Box::new(samples)),
            fields_start,
        }
    }
}

/// The numbers of a numeric trace or of one element of an array trace.
#[derive(Clone, Copy)]
pub struct Series<'a> {
    pub timestamps: &'a [u64],
    numbers: &'a Numbers,
    /// The element of the rows of `numbers`.
    lane: usize,
    /// The row of the first sample.
    first: usize,
    pub kind: NumberKind,
    /// The lowest and highest finite value of all samples, also of those outside a
    /// [`Series::slice`], `None` if there is none.
    pub range: Option<(f64, f64)>,
}

impl<'a> Series<'a> {
    fn new(timestamps: &'a [u64], numbers: &'a Numbers, lane: usize) -> Self {
        Self {
            timestamps,
            numbers,
            lane,
            first: 0,
            kind: numbers.kind().unwrap_or(NumberKind::Float),
            range: numbers.range(lane),
        }
    }

    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    /// The value of the `index`th sample as a float for plotting, NaN for null.
    pub fn value(&self, index: usize) -> f64 {
        self.numbers
            .value((self.first + index) * self.numbers.lanes() + self.lane)
    }

    /// The value of the `index`th sample.
    pub fn number(&self, index: usize) -> Number {
        self.numbers
            .get((self.first + index) * self.numbers.lanes() + self.lane)
    }

    /// The lowest and highest finite value of the samples in `range`, and whether one
    /// of them is a gap, read from blocks of samples where it can.
    pub fn extent(&self, range: Range<usize>) -> Extent {
        self.numbers
            .extent(self.lane, self.first + range.start..self.first + range.end)
    }

    /// The samples within `range`.
    pub fn slice(&self, range: Range<usize>) -> Self {
        Self {
            timestamps: &self.timestamps[range.clone()],
            first: self.first + range.start,
            ..*self
        }
    }
}

/// A column of a table: the samples of a trace.
struct Column {
    path: Box<str>,
    fields_start: Option<usize>,
    table: usize,
    samples: Samples,
    warned: bool,
    /// Whether the column was removed, as it only held nulls of what turned out to be a
    /// record.
    removed: bool,
}

/// The values sent to one path, a column per leaf.
struct Table {
    path: Box<str>,
    timestamps: Growing<u64>,
    columns: Vec<usize>,
    /// Whether a value was dropped as it did not fit the columns.
    warned: bool,
    /// Whether a value was dropped as it was too late, warned apart from the rest, as a
    /// clock behind the others is a problem of its own.
    warned_late: bool,
}

impl Table {
    fn warn(&mut self, problem: &str) {
        if !self.warned {
            self.warned = true;
            log::warn!(
                "{}: dropped a value, {problem} (further ones are dropped silently)",
                self.path
            );
        }
    }
}

#[derive(Default)]
pub struct Traces {
    columns: Vec<Column>,
    tables: Vec<Table>,
    by_path: HashMap<Box<str>, usize>,
    tables_by_path: HashMap<Box<str>, usize>,
    /// The columns that were not removed, by their paths, so the fields of a record
    /// follow each other.
    sorted: Vec<usize>,
    /// The units of the traces by their paths, as received in their metadata, also
    /// for traces without samples yet.
    units: BTreeMap<String, String>,
    /// Timestamps of the first and the newest value.
    span: Option<(u64, u64)>,
    /// Changes when a trace is added or removed.
    generation: u64,
    /// Changes when the kind of a trace changes, see [`Self::kinds`].
    kinds: u64,
}

impl Traces {
    fn trace(&self, id: usize) -> Trace<'_> {
        let column = &self.columns[id];
        Trace {
            path: &column.path,
            timestamps: &self.tables[column.table].timestamps[..],
            samples: &column.samples,
            fields_start: column.fields_start,
        }
    }

    /// The traces, sorted by path.
    pub fn iter(&self) -> impl Iterator<Item = Trace<'_>> {
        self.sorted.iter().map(|&id| self.trace(id))
    }

    /// The paths values were sent to with the timestamps of their values, e.g. a
    /// record once.
    pub fn tables(&self) -> impl Iterator<Item = (&str, &[u64])> {
        self.tables
            .iter()
            .map(|table| (&*table.path, &table.timestamps[..]))
    }

    /// Changes whenever a trace is added or removed.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Changes whenever the kind of a trace changes, e.g. with its first value or the
    /// first float of an integer trace, which changes what views can show of it.
    pub fn kinds(&self) -> u64 {
        self.kinds
    }

    /// Appends `leaf` to the samples of the column `id`, counting a change of their
    /// kind.
    fn push_leaf(
        &mut self,
        id: usize,
        leaf: &Leaf,
        flat: &Flat,
        frame: &[u8],
    ) -> Result<(), String> {
        let samples = &mut self.columns[id].samples;
        let kind = samples.kind();
        let pushed = samples.push(leaf, flat, frame);
        if samples.kind() != kind {
            self.kinds += 1;
        }
        pushed
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

    pub fn get(&self, path: &str) -> Option<Trace<'_>> {
        self.by_path.get(path).map(|&id| self.trace(id))
    }

    /// The numbers shown for `path`: a numeric trace, or an element of an array
    /// trace written by [`element_path`].
    pub fn series(&self, path: &str) -> Option<Series<'_>> {
        if let Some(trace) = self.get(path) {
            return trace.series();
        }
        let (path, indices) = parse_element_path(path)?;
        let trace = self.get(path)?;
        let Samples::Arrays { shape, elements } = trace.samples else {
            return None;
        };
        if indices.len() != shape.len()
            || indices.iter().zip(shape).any(|(index, len)| index >= len)
        {
            return None;
        }
        let lane = indices
            .iter()
            .zip(shape)
            .fold(0, |offset, (index, len)| offset * len + index);
        Some(Series::new(trace.timestamps, elements, lane))
    }

    /// The paths of the fields of the record at `path`, also of nested records, but not
    /// of traces whose names only contain a `.`, e.g. `v1.2`.
    pub fn field_paths(&self, path: &str) -> impl Iterator<Item = &str> {
        let prefix = format!("{path}.");
        let start = self
            .sorted
            .partition_point(|&id| *self.columns[id].path < *prefix);
        self.sorted[start..]
            .iter()
            .map(|&id| &self.columns[id])
            .take_while(move |column| column.path.starts_with(&prefix))
            .filter(move |column| column.fields_start.is_some_and(|start| start <= path.len()))
            .map(|column| &*column.path)
    }

    /// Timestamps of the first and the newest value of all traces.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        self.span
    }

    /// Whether a sample at `path` was dropped as its kind did not match, for tests.
    #[cfg(test)]
    pub fn warned(&self, path: &str) -> bool {
        let column = &self.columns[self.by_path[path]];
        let table = &self.tables[column.table];
        column.warned || table.warned || table.warned_late
    }

    /// Appends `value` as if received, for tests.
    #[cfg(test)]
    pub fn append(&mut self, value: &proto::Value) {
        let frame = serde_cbor_2::to_vec(&proto::Message::Value(value.clone())).unwrap();
        let mut flat = Flat::default();
        let message = super::decode::decode(&frame, &mut flat).unwrap();
        let super::decode::Message::Value { timestamp, path } = message else {
            unreachable!("a value was encoded");
        };
        self.insert(path, timestamp, &flat, &frame);
    }

    /// Adds a column for the leaf `field` of `table`, e.g. `.acc`, with nulls for the
    /// values so far, unless a trace of another table has its path.
    fn add_column(&mut self, table: usize, field: &str) -> Option<usize> {
        let path: Box<str> = format!("{}{field}", self.tables[table].path).into();
        if self.by_path.contains_key(&path) {
            return None;
        }
        let id = self.columns.len();
        let at = self
            .sorted
            .partition_point(|&other| self.columns[other].path < path);
        self.sorted.insert(at, id);
        self.by_path.insert(path.clone(), id);
        let table_path_len = self.tables[table].path.len();
        self.columns.push(Column {
            path,
            fields_start: (!field.is_empty()).then_some(table_path_len),
            table,
            samples: Samples::Empty(self.tables[table].timestamps.len()),
            warned: false,
            removed: false,
        });
        self.tables[table].columns.push(id);
        self.generation += 1;
        Some(id)
    }

    /// Removes the column `id`, which only held nulls.
    fn remove_column(&mut self, id: usize) {
        self.columns[id].removed = true;
        self.by_path.remove(&self.columns[id].path);
        self.sorted.retain(|&other| other != id);
        let table = self.columns[id].table;
        self.tables[table].columns.retain(|&other| other != id);
        self.generation += 1;
    }

    /// The column of `table` with the leaf `field`, looked for first at `hint`, the
    /// index among the columns of the table where the fields usually are.
    fn find_column(&self, table: usize, field: &str, hint: usize) -> Option<usize> {
        let table = &self.tables[table];
        let start = table.path.len();
        let is_field = |&id: &usize| self.columns[id].path[start..] == *field;
        match table.columns.get(hint) {
            Some(&id) if is_field(&id) => Some(id),
            _ => table.columns.iter().copied().find(is_field),
        }
    }

    /// Appends the value at `path` with `timestamp`, flattened into `flat` from
    /// `frame`: a sample to the trace of each of its leaves. The first record at a path
    /// sets its fields: a record with other fields is dropped, a field whose value does
    /// not fit its trace is null. A value that does not fit the trace at `path` is
    /// dropped, as is a value older than [`MAX_LATE`] values of its path.
    pub fn insert(&mut self, path: &str, timestamp: u64, flat: &Flat, frame: &[u8]) {
        let table = match self.tables_by_path.get(path) {
            Some(&table) => {
                let table_ref = &mut self.tables[table];
                let timestamps = &table_ref.timestamps;
                let newer = timestamps.len() - upper_bound(timestamps, timestamp);
                if newer > MAX_LATE {
                    if !table_ref.warned_late {
                        table_ref.warned_late = true;
                        log::warn!(
                            "{}: dropped a value older than the newest {MAX_LATE}, e.g. from \
                             a clock behind the others sending to the path (further ones are \
                             dropped silently)",
                            table_ref.path
                        );
                    }
                    return;
                }
                table
            }
            // nothing to keep, or another trace has a path of the value
            None if flat.leaves.is_empty() || self.is_taken(path, flat) => return,
            None => {
                self.tables.push(Table {
                    path: path.into(),
                    timestamps: Growing::default(),
                    columns: vec![],
                    warned: false,
                    warned_late: false,
                });
                self.tables_by_path
                    .insert(path.into(), self.tables.len() - 1);
                self.tables.len() - 1
            }
        };
        let rows = self.tables[table].timestamps.len();
        let kept = if flat.is_record() {
            self.insert_record(table, flat, frame)
        } else {
            self.insert_scalar(table, flat, frame)
        };
        if !kept {
            return;
        }
        // the traces without a sample, e.g. of a missing record or a field that did not
        // fit
        for &id in &self.tables[table].columns {
            let samples = &mut self.columns[id].samples;
            if samples.len() == rows {
                samples.push_null();
            }
        }

        let table = &mut self.tables[table];
        // searched before the new timestamp is appended, which would leave them unsorted
        let at = upper_bound(&table.timestamps, timestamp);
        table.timestamps.push(timestamp);
        if at + 1 < table.timestamps.len() {
            table.timestamps.rotate_tail(at, 1);
            for &id in &table.columns {
                self.columns[id].samples.move_last_to(at);
            }
        }
        self.span = Some(self.span.map_or((timestamp, timestamp), |(first, last)| {
            (first.min(timestamp), last.max(timestamp))
        }));
    }

    /// Whether another trace has the path of the value in `flat` at `path`, or of one
    /// of its fields.
    fn is_taken(&self, path: &str, flat: &Flat) -> bool {
        match flat.is_record() {
            true => (flat.leaves.iter()).any(|(field, _)| {
                self.by_path
                    .contains_key(&*format!("{path}{}", flat.field(field)))
            }),
            false => self.by_path.contains_key(path),
        }
    }

    /// Appends the value in `flat`, which is no record, to `table`. Returns whether it
    /// did: a null of a record leaves a gap in all its fields, other values do not fit
    /// a record.
    fn insert_scalar(&mut self, table: usize, flat: &Flat, frame: &[u8]) -> bool {
        let leaf = &flat.leaves[0].1;
        let record = self.tables[table]
            .columns
            .iter()
            .any(|&id| self.columns[id].fields_start.is_some());
        match (record, leaf) {
            (true, Leaf::Null) => true,
            (true, leaf) => {
                let problem = format!("a record cannot become {}", leaf_kind(leaf));
                self.tables[table].warn(&problem);
                false
            }
            (false, leaf) => {
                let Some(id) = self
                    .find_column(table, "", 0)
                    .or_else(|| self.add_column(table, ""))
                else {
                    self.tables[table].warn("a field of a record has the path");
                    return false;
                };
                match self.push_leaf(id, leaf, flat, frame) {
                    Ok(()) => true,
                    Err(problem) => {
                        self.tables[table].warn(&problem);
                        false
                    }
                }
            }
        }
    }

    /// Appends the leaves of the record in `flat` to the traces of their fields in
    /// `table`, which the first record at its path sets. Returns whether it did, i.e.
    /// the record has the fields of the first; a leaf that does not fit its trace is
    /// left for a null.
    fn insert_record(&mut self, table: usize, flat: &Flat, frame: &[u8]) -> bool {
        // a trace that only received nulls so far turns out to be a record
        if let Some(id) = self.find_column(table, "", 0) {
            if !matches!(self.columns[id].samples, Samples::Empty(_)) {
                let kind = self.columns[id].samples.type_label();
                self.tables[table].warn(&format!("a {kind} trace cannot become a record"));
                return false;
            }
            self.remove_column(id);
        }
        let known = &self.tables[table].columns;
        if known.is_empty() {
            // the first record sets the fields, unless another trace has a path of them
            let table_path = self.tables[table].path.clone();
            if self.is_taken(&table_path, flat) {
                self.tables[table].warn("another trace has the path of a field");
                return false;
            }
            for (field, _) in &flat.leaves {
                self.add_column(table, flat.field(field));
            }
        } else {
            let same = flat.leaves.len() == known.len()
                && (flat.leaves.iter().enumerate()).all(|(index, (field, _))| {
                    self.find_column(table, flat.field(field), index).is_some()
                });
            if !same {
                self.tables[table].warn("its fields are not those of the first record at the path");
                return false;
            }
        }
        for (index, (field, leaf)) in flat.leaves.iter().enumerate() {
            let id = (self.find_column(table, flat.field(field), index))
                .expect("the record has the fields of the table");
            if let Err(problem) = self.push_leaf(id, leaf, flat, frame) {
                let column = &mut self.columns[id];
                if !column.warned {
                    column.warned = true;
                    log::warn!(
                        "{}: dropped a sample, {problem} (further ones are dropped silently)",
                        column.path
                    );
                }
            }
        }
        true
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

#[cfg(test)]
mod tests;
