//! The values received, as tables: each path that values are sent to is a table with
//! a column per leaf of its values, e.g. the fields of a record, sharing the
//! timestamps of the table. A column is a trace.

use super::decode::{Flat, Leaf};
use super::growing::Growing;
use super::numbers::{DisplayFormat, Extent, Number, NumberKind, Numbers};
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

    /// Appends the value of the last sample again, null if there is none, for a
    /// record that lacks the field of these samples, which keeps its value.
    fn push_previous(&mut self) {
        match self {
            Self::Empty(count) => *count += 1,
            Self::Numbers(numbers)
            | Self::Arrays {
                elements: numbers, ..
            } => {
                numbers.repeat_last_row();
            }
            Self::Strings(strings) => {
                let id = strings.ids.last().copied().unwrap_or(NULL_TEXT);
                strings.ids.push(id);
            }
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
    /// How many samples were received, without the samples of records that lacked
    /// the field, which keep the value before.
    pub received: usize,
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
            received: 0,
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
    /// See [`Trace::received`].
    received: usize,
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
}

impl Traces {
    fn trace(&self, id: usize) -> Trace<'_> {
        let column = &self.columns[id];
        Trace {
            path: &column.path,
            timestamps: &self.tables[column.table].timestamps[..],
            samples: &column.samples,
            fields_start: column.fields_start,
            received: column.received,
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
        column.warned || self.tables[column.table].warned
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
            received: 0,
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
    /// `frame`: a sample to the trace of each of its leaves, a null to the other
    /// traces of the path. A value that does not fit the traces at `path` is dropped,
    /// a field that does not fit is null, a value of which nothing fits is dropped, as
    /// is a value older than [`MAX_LATE`] values of its path.
    pub fn insert(&mut self, path: &str, timestamp: u64, flat: &Flat, frame: &[u8]) {
        let known = self.tables_by_path.get(path).copied();
        if let Some(known) = known {
            let table = &mut self.tables[known];
            let newer =
                table.timestamps.len() - table.timestamps.partition_point(|&t| t <= timestamp);
            if newer > MAX_LATE {
                table.warn(&format!(
                    "it is older than the newest {MAX_LATE} values, e.g. from a clock behind \
                     the others sending to the path"
                ));
                return;
            }
        }
        let Some(table) = self.add_value(known, path, flat, frame) else {
            if known.is_none()
                && let Some(table) = self.tables_by_path.remove(path)
            {
                // the table made for the value holds nothing: its traces go first, as
                // they refer to it, then the table, the last one made
                for id in self.tables[table].columns.clone() {
                    self.remove_column(id);
                }
                self.tables.pop();
            }
            return;
        };
        let table = &mut self.tables[table];
        // searched before the new timestamp is appended, which would leave them unsorted
        let at = table.timestamps.partition_point(|&t| t <= timestamp);
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

    /// Appends the leaves of the value in `flat` to the traces of `table`, or of a new
    /// table at `path`, and nulls to its other traces, without its timestamp. Returns
    /// the table if anything of the value was kept; if not, no trace changed.
    fn add_value(
        &mut self,
        table: Option<usize>,
        path: &str,
        flat: &Flat,
        frame: &[u8],
    ) -> Option<usize> {
        let table = match table {
            Some(table) => table,
            None => {
                if flat.leaves.is_empty() || !flat.is_record() && self.by_path.contains_key(path) {
                    // nothing to keep, or a field of a record has the path
                    return None;
                }
                self.tables.push(Table {
                    path: path.into(),
                    timestamps: Growing::default(),
                    columns: vec![],
                    warned: false,
                });
                self.tables_by_path
                    .insert(path.into(), self.tables.len() - 1);
                self.tables.len() - 1
            }
        };
        let rows = self.tables[table].timestamps.len();
        if flat.is_record() {
            if !self.insert_record(table, flat, frame) {
                return None;
            }
        } else {
            let leaf = &flat.leaves[0].1;
            let record = self.tables[table]
                .columns
                .iter()
                .any(|&id| self.columns[id].fields_start.is_some());
            match (record, leaf) {
                // a missing record leaves a gap in all its fields
                (true, Leaf::Null) => {
                    for &id in &self.tables[table].columns {
                        let column = &mut self.columns[id];
                        column.samples.push_null();
                        column.received += 1;
                    }
                }
                (true, leaf) => {
                    let problem = format!("a record cannot become {}", leaf_kind(leaf));
                    self.tables[table].warn(&problem);
                    return None;
                }
                (false, leaf) => {
                    let id = match self.find_column(table, "", 0) {
                        Some(id) => id,
                        None => match self.add_column(table, "") {
                            Some(id) => id,
                            None => {
                                self.tables[table].warn("a field of a record has the path");
                                return None;
                            }
                        },
                    };
                    if let Err(problem) = self.columns[id].samples.push(leaf, flat, frame) {
                        self.tables[table].warn(&problem);
                        return None;
                    }
                    self.columns[id].received += 1;
                }
            }
        }
        // the fields the record lacks keep their values
        for &id in &self.tables[table].columns {
            let samples = &mut self.columns[id].samples;
            if samples.len() == rows {
                samples.push_previous();
            }
        }
        Some(table)
    }

    /// Appends the leaves of the record in `flat` to the traces of their fields in
    /// `table`. Returns whether any was kept, also as a gap; a leaf that does not fit
    /// its trace is left for a null.
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
        let rows = self.tables[table].timestamps.len();
        let table_path_len = self.tables[table].path.len();
        let mut kept = false;
        for (index, (field, leaf)) in flat.leaves.iter().enumerate() {
            let field = flat.field(field);
            let id = match self.find_column(table, field, index) {
                Some(id) => id,
                None => {
                    let fields = &self.tables[table].columns;
                    let below = |id: &usize| {
                        let other = &self.columns[*id].path[table_path_len..];
                        other.len() > field.len()
                            && other.starts_with(field)
                            && other.as_bytes()[field.len()] == b'.'
                    };
                    if *leaf == Leaf::Null && fields.iter().any(below) {
                        // a missing record leaves a gap in all its fields
                        kept = true;
                        continue;
                    }
                    // a field that only received nulls so far turns out to be a record
                    let above = fields.iter().copied().find(|&id| {
                        let other = &self.columns[id].path[table_path_len..];
                        field.len() > other.len()
                            && field.starts_with(other)
                            && field.as_bytes()[other.len()] == b'.'
                    });
                    if let Some(above) = above {
                        if !matches!(self.columns[above].samples, Samples::Empty(_)) {
                            self.tables[table].warn(&format!("{field} is no record"));
                            continue;
                        }
                        self.remove_column(above);
                    }
                    match self.add_column(table, field) {
                        Some(id) => id,
                        None => {
                            self.tables[table].warn(&format!("another trace is at {field}"));
                            continue;
                        }
                    }
                }
            };
            let column = &mut self.columns[id];
            if column.samples.len() > rows {
                continue;
            }
            match column.samples.push(leaf, flat, frame) {
                Ok(()) => {
                    kept = true;
                    column.received += 1;
                }
                Err(problem) if !column.warned => {
                    column.warned = true;
                    log::warn!(
                        "{}: dropped a sample, {problem} (further ones are dropped silently)",
                        column.path
                    );
                }
                Err(_) => {}
            }
        }
        kept
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
mod tests {
    use super::*;
    use proto::Data;

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
        let mut traces = Traces::default();
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
        // any NaN, also the one of a null
        let bits = |values: &[f64]| {
            values
                .iter()
                .map(|value| {
                    if value.is_nan() {
                        u64::MAX
                    } else {
                        value.to_bits()
                    }
                })
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
        assert_eq!(traces.get("v").unwrap().value_range(), Some((-3.0, 20.0)));
        assert_eq!(traces.get("n").unwrap().value_range(), None);
    }

    #[test]
    fn records_split_into_field_traces() {
        let imu = record(&[
            ("x", Data::Float(0.5)),
            ("acc", record(&[("z", Data::Uint(9))])),
        ]);
        let traces = traces(&[("imu", imu)]);
        let paths: Vec<&str> = traces.iter().map(|trace| trace.path).collect();
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
        let mut traces = Traces::default();
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
    fn fields_a_record_lacks_keep_their_values() {
        let traces = traces(&[
            (
                "m",
                record(&[("speed", Data::Float(1.0)), ("state", Data::from("on"))]),
            ),
            ("m", record(&[("state", Data::from("off"))])),
            ("m", record(&[("speed", Data::Float(3.0))])),
            ("m", Data::Null),
        ]);
        assert_values(values(&traces, "m.speed"), &[1.0, 1.0, 3.0, f64::NAN]);
        let state = traces.get("m.state").unwrap();
        let texts: Vec<String> = (0..4)
            .map(|index| state.samples.format(index, 8, None, DisplayFormat::Default))
            .collect();
        assert_eq!(texts, ["on", "off", "off", "null"]);
        // a missing record is received as a gap in each field
        assert_eq!(traces.get("m.speed").unwrap().received, 3);
        assert_eq!(state.received, 3);
    }

    #[test]
    fn values_far_older_than_the_newest_are_dropped() {
        let mut traces = Traces::default();
        let mut append = |timestamp: u64| {
            traces.append(&proto::Value {
                timestamp,
                path: "a".to_owned(),
                value: Data::Float(timestamp as f64),
            });
        };
        for timestamp in 1..=MAX_LATE as u64 + 10 {
            append(timestamp * 10);
        }
        // late, but not too late
        let newest = (MAX_LATE as u64 + 10) * 10;
        append(newest - 5);
        // too late
        append(5);
        let a = traces.get("a").unwrap();
        assert_eq!(a.timestamps.len(), MAX_LATE + 11);
        assert_eq!(a.timestamps[0], 10);
        assert_eq!(a.timestamps[MAX_LATE + 9], newest - 5);
        assert!(a.timestamps.is_sorted());
        assert!(traces.warned("a"));
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
        assert!(traces.warned("a"));
    }

    #[test]
    fn a_scalar_trace_does_not_turn_into_a_record() {
        let traces = traces(&[
            ("a", Data::Float(1.0)),
            ("a", record(&[("x", Data::Float(2.0))])),
        ]);
        assert!(traces.get("a.x").is_none());
        assert!(traces.warned("a"));
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
        let trace = strings.get("s").unwrap();
        let Samples::Strings(texts) = trace.samples else {
            panic!("strings expected");
        };
        assert_eq!((texts.get(0), texts.get(1)), (None, Some("on")));
    }

    #[test]
    fn a_null_record_is_a_gap_in_all_fields() {
        let imu = record(&[("x", Data::Float(1.0)), ("y", Data::Float(2.0))]);
        let traces = traces(&[("imu", Data::Null), ("imu", imu), ("imu", Data::Null)]);
        // the trace the leading null created turned into the record
        assert!(traces.get("imu").is_none());
        assert_values(values(&traces, "imu.x"), &[f64::NAN, 1.0, f64::NAN]);
        assert_values(values(&traces, "imu.y"), &[f64::NAN, 2.0, f64::NAN]);
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
        assert_eq!(*traces.get("v1").unwrap().samples, Samples::Empty(1));
        // `imu.x` is a field of the record, values cannot be sent to it
        assert_values(values(&traces, "imu.x"), &[1.0]);
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
    fn arrays_without_elements_are_kept() {
        let empty = || Data::Array(vec![]);
        let traces = traces(&[
            ("e", Data::Null),
            ("e", empty()),
            ("e", empty()),
            ("m", Data::Array(vec![empty(), empty()])),
        ]);
        let e = traces.get("e").unwrap();
        assert_eq!(e.timestamps.len(), 3);
        assert_eq!(e.samples.type_label(), "null[0]");
        assert_eq!(e.samples.format(1, 8, None, DisplayFormat::Default), "[]");
        assert_eq!(e.value_range(), None);
        let m = traces.get("m").unwrap();
        assert_eq!(m.samples.type_label(), "null[2×0]");
        assert!(traces.series("e[0]").is_none());

        // also when a sample arrives late
        let mut traces = traces;
        traces.append(&proto::Value {
            timestamp: 0,
            path: "m".to_owned(),
            value: Data::Array(vec![empty(), empty()]),
        });
        assert_eq!(traces.get("m").unwrap().timestamps, [0, 3]);
    }

    #[test]
    fn values_of_which_nothing_fits_add_no_sample() {
        let mut traces = traces(&[
            ("a", record(&[("b", record(&[("c", Data::Uint(1))]))])),
            ("x", record(&[("y", Data::Float(1.0))])),
        ]);
        fn append(traces: &mut Traces, timestamp: u64, path: &str, value: Data) {
            traces.append(&proto::Value {
                timestamp,
                path: path.to_owned(),
                value,
            });
        }
        // the field of a record has the path `a.b.c`
        append(&mut traces, 10, "a.b", record(&[("c", Data::Uint(2))]));
        // the first value of a new path, of which no trace can hold anything
        append(&mut traces, 10, "t", Data::Array(vec![Data::from("a")]));
        append(
            &mut traces,
            10,
            "r",
            record(&[("s", Data::Array(vec![Data::from("a")]))]),
        );
        assert!(traces.get("t").is_none() && traces.get("r.s").is_none());
        // a string does not fit the float field
        append(&mut traces, 11, "x", record(&[("y", Data::from("text"))]));
        assert!(traces.get("a.b").is_none());
        assert_eq!(traces.get("a.b.c").unwrap().timestamps, [0]);
        assert_eq!(traces.get("x.y").unwrap().timestamps, [1]);
        assert_eq!(traces.tables().count(), 2);
        assert_eq!(traces.time_span(), Some((0, 1)));
        // a value of which a field fits is kept, the other field keeps its value
        let both = record(&[("y", Data::from("text")), ("z", Data::Float(2.0))]);
        append(&mut traces, 12, "x", both);
        assert_values(values(&traces, "x.y"), &[1.0, 1.0]);
        assert_values(values(&traces, "x.z"), &[f64::NAN, 2.0]);
        // only the samples received count
        assert_eq!(traces.get("x.y").unwrap().received, 1);
        assert_eq!(traces.get("x.z").unwrap().received, 1);
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
            let samples = traces.get(path).unwrap().samples;
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
        let samples = traces.get("m").unwrap().samples;
        assert_eq!(
            samples.array_sample(0).unwrap().get(3),
            Some(Number::Float(3.0))
        );
        assert!(samples.array_sample(1).is_none());
    }

    #[test]
    fn array_elements_are_written_in_their_display_format() {
        let traces = traces(&[("a", Data::Array(vec![Data::Uint(10), Data::Uint(255)]))]);
        let samples = traces.get("a").unwrap().samples;
        assert_eq!(
            samples.format(0, 8, None, DisplayFormat::Hex),
            "[0xa, 0xff]"
        );
    }

    #[test]
    fn integers_are_exact() {
        let big = Data::Uint((1 << 53) + 1);
        let traces = traces(&[("a", big.clone()), ("m", Data::Array(vec![big]))]);
        let samples = |path| traces.get(path).unwrap().samples;
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
        let mut traces = Traces::default();
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
