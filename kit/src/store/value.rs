//! The value of a sample as a tape: the items of the protocol's data model in one buffer
//! and their texts in another, so a value that is set again allocates nothing.

use proto::Data;

/// One item of a tape. An array or record head is followed by its contents, a `Key`
/// by the value of its field. `size` counts the items that belong to a head, so a slice
/// that starts at a head is a whole value wherever it lies. Offsets count from the start
/// of the texts of the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Item {
    Null,
    Bool(bool),
    Uint(u64),
    Int(i64),
    Float(f64),
    /// A text: `len` bytes at `start` of the texts.
    Text {
        start: u32,
        len: u32,
    },
    /// An array of `len` elements, whose items are the `size` items after it.
    Array {
        len: u32,
        size: u32,
    },
    /// A record of `len` fields, each a `Key` and its value: the `size` items after it.
    Record {
        len: u32,
        size: u32,
    },
    /// The name of the field whose value follows.
    Key {
        start: u32,
        len: u32,
    },
}

impl Item {
    /// The number of items that follow this one and belong to it.
    pub(crate) fn size(self) -> usize {
        match self {
            Self::Array { size, .. } | Self::Record { size, .. } => size as usize,
            _ => 0,
        }
    }
}

/// A value of the protocol's data model (null, integer, float, bool, text, array,
/// record), as a target sends it, in two buffers it reuses: set again, it allocates
/// nothing once they grew.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Value {
    items: Vec<Item>,
    texts: String,
}

impl Value {
    /// Replaces the value with `value`. A value that is never set is null.
    pub fn set(&mut self, value: impl AsValue) {
        self.write_with(|to| to.value(value));
    }

    /// Clears the value, then lets `write` write it with a writer that takes the items
    /// as they come, as the parser does.
    pub(crate) fn write_with<R>(&mut self, write: impl FnOnce(&mut Writer<'_>) -> R) -> R {
        self.items.clear();
        self.texts.clear();
        write(&mut Writer::new(&mut self.items, &mut self.texts, 0, false))
    }

    /// The texts of the value, which its text items and keys point into.
    pub(crate) fn texts(&self) -> &str {
        &self.texts
    }

    /// Replaces the value with a copy of `value`: two slice copies, no allocation once
    /// the buffers grew. An empty value stays empty, i.e. null.
    pub(crate) fn copy_from(&mut self, value: ValueRef<'_>) {
        self.items.clear();
        self.items.extend_from_slice(value.items);
        self.texts.clear();
        self.texts.push_str(value.texts);
    }

    /// The value, borrowed to read it.
    pub fn as_ref(&self) -> ValueRef<'_> {
        ValueRef {
            items: &self.items,
            texts: &self.texts,
        }
    }
}

impl From<&Data> for Value {
    fn from(data: &Data) -> Self {
        let mut value = Self::default();
        value.set(data);
        value
    }
}

/// One value, borrowed from a [`Value`]: read it with [`ValueRef::node`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRef<'a> {
    items: &'a [Item],
    texts: &'a str,
}

impl<'a> ValueRef<'a> {
    /// The texts of the value.
    pub(crate) fn texts(self) -> &'a str {
        self.texts
    }

    /// The items of the value.
    pub(crate) fn items(self) -> &'a [Item] {
        self.items
    }

    /// What the value is, to read it. An empty value is null.
    pub fn node(self) -> Node<'a> {
        let Some(&head) = self.items.first() else {
            return Node::Null;
        };
        let rest = self.items.get(1..).unwrap_or_default();
        match head {
            Item::Null | Item::Key { .. } => Node::Null,
            Item::Bool(value) => Node::Bool(value),
            Item::Uint(value) => Node::Uint(value),
            Item::Int(value) => Node::Int(value),
            Item::Float(value) => Node::Float(value),
            Item::Text { start, len } => Node::Text(self.text(start, len)),
            Item::Array { len, .. } => Node::Array(Elements {
                rest,
                texts: self.texts,
                left: len as usize,
            }),
            Item::Record { len, .. } => Node::Record(Fields {
                rest,
                texts: self.texts,
                left: len as usize,
            }),
        }
    }

    /// The value as a protocol value. This allocates; to read a value, use
    /// [`ValueRef::node`].
    pub fn to_data(self) -> Data {
        match self.node() {
            Node::Null => Data::Null,
            Node::Bool(value) => Data::Bool(value),
            Node::Uint(value) => Data::Uint(value),
            Node::Int(value) => Data::Int(value),
            Node::Float(value) => Data::Float(value),
            Node::Text(value) => Data::String(value.to_owned()),
            Node::Array(elements) => Data::Array(elements.map(ValueRef::to_data).collect()),
            Node::Record(fields) => Data::Record(
                fields
                    .map(|(name, value)| (name.to_owned(), value.to_data()))
                    .collect(),
            ),
        }
    }

    fn text(self, start: u32, len: u32) -> &'a str {
        value_text(self.texts, start, len)
    }
}

/// Where a row of [`Rows`] ends in the items and the texts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct End {
    item: u32,
    text: u32,
}

/// The values of a block, one after another, borrowed: read them with [`Rows::get`] or
/// [`Rows::iter`].
#[derive(Clone, Copy, Debug)]
pub struct Rows<'a> {
    items: &'a [Item],
    texts: &'a str,
    ends: &'a [End],
}

impl<'a> Rows<'a> {
    /// The number of values.
    pub fn len(self) -> usize {
        self.ends.len()
    }

    /// Whether there are no values.
    pub fn is_empty(self) -> bool {
        self.ends.is_empty()
    }

    /// The value `row`, `None` past the last.
    pub fn get(self, row: usize) -> Option<ValueRef<'a>> {
        let end = *self.ends.get(row)?;
        let start = match row.checked_sub(1) {
            Some(before) => self.ends[before],
            None => End { item: 0, text: 0 },
        };
        Some(ValueRef {
            items: &self.items[start.item as usize..end.item as usize],
            texts: &self.texts[start.text as usize..end.text as usize],
        })
    }

    /// The values in order.
    pub fn iter(self) -> impl Iterator<Item = ValueRef<'a>> + 'a {
        (0..self.len()).filter_map(move |row| self.get(row))
    }

    /// The last value, `None` if there are none.
    pub fn last(self) -> Option<ValueRef<'a>> {
        self.get(self.len().checked_sub(1)?)
    }
}

/// The owned buffers behind [`Rows`], reused from block to block.
#[derive(Default, Debug)]
pub(crate) struct RowsBuf {
    items: Vec<Item>,
    texts: String,
    ends: Vec<End>,
}

impl RowsBuf {
    /// Removes the rows, keeping the buffers.
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.texts.clear();
        self.ends.clear();
    }

    /// Lets `write` write the next row straight into the buffers, with a writer that takes
    /// the items as they come, as the parser does. Gives what `write` gives. A row that
    /// would take the buffers past 4 GiB is not added. If `write` fails the row is
    /// partial: the caller drops the rows then.
    pub(crate) fn push_with<R>(&mut self, write: impl FnOnce(&mut Writer<'_>) -> R) -> R {
        let (items, texts) = (self.items.len(), self.texts.len());
        let mut writer = Writer::new(&mut self.items, &mut self.texts, texts, false);
        let result = write(&mut writer);
        match (
            u32::try_from(self.items.len()),
            u32::try_from(self.texts.len()),
        ) {
            (Ok(item), Ok(text)) => self.ends.push(End { item, text }),
            _ => {
                self.items.truncate(items);
                self.texts.truncate(texts);
            }
        }
        result
    }

    /// The rows, borrowed.
    pub(crate) fn rows(&self) -> Rows<'_> {
        Rows {
            items: &self.items,
            texts: &self.texts,
            ends: &self.ends,
        }
    }
}

/// What a value is: a scalar, a text, an array or a record.
#[derive(Clone, Copy, Debug)]
pub enum Node<'a> {
    /// No value.
    Null,
    /// A boolean.
    Bool(bool),
    /// A non-negative integer.
    Uint(u64),
    /// A negative integer.
    Int(i64),
    /// A floating point number.
    Float(f64),
    /// A text.
    Text(&'a str),
    /// The elements of an array.
    Array(Elements<'a>),
    /// The fields of a record, in their order.
    Record(Fields<'a>),
}

/// The elements of an array, each a [`ValueRef`].
#[derive(Clone, Copy, Debug)]
pub struct Elements<'a> {
    rest: &'a [Item],
    texts: &'a str,
    left: usize,
}

impl<'a> Iterator for Elements<'a> {
    type Item = ValueRef<'a>;

    fn next(&mut self) -> Option<ValueRef<'a>> {
        if self.left == 0 {
            return None;
        }
        let (items, rest) = split_value(self.rest)?;
        self.rest = rest;
        self.left -= 1;
        Some(ValueRef {
            items,
            texts: self.texts,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

/// The fields of a record: the name and the [`ValueRef`] of each.
#[derive(Clone, Copy, Debug)]
pub struct Fields<'a> {
    rest: &'a [Item],
    texts: &'a str,
    left: usize,
}

impl<'a> Iterator for Fields<'a> {
    type Item = (&'a str, ValueRef<'a>);

    fn next(&mut self) -> Option<(&'a str, ValueRef<'a>)> {
        if self.left == 0 {
            return None;
        }
        let Some((&Item::Key { start, len }, after)) = self.rest.split_first() else {
            return None;
        };
        let (items, rest) = split_value(after)?;
        self.rest = rest;
        self.left -= 1;
        let value = ValueRef {
            items,
            texts: self.texts,
        };
        Some((value_text(self.texts, start, len), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

fn value_text(texts: &str, start: u32, len: u32) -> &str {
    let start = start as usize;
    texts.get(start..start + len as usize).unwrap_or("")
}

/// Splits the first whole value off `items`.
fn split_value(items: &[Item]) -> Option<(&[Item], &[Item])> {
    let n = 1 + items.first()?.size();
    (n <= items.len()).then(|| items.split_at(n))
}

/// What can be the value of a sample: written into a [`Value`] without building a
/// [`proto::Data`]. `write` writes exactly one value: none is null,
/// and if it writes more, the first is kept and the others are ignored.
pub trait AsValue {
    /// Writes the value to `to`.
    fn write(&self, to: &mut Writer<'_>);
}

/// Writes one value, or the elements of an array, to a tape.
pub struct Writer<'a> {
    items: &'a mut Vec<Item>,
    texts: &'a mut String,
    /// Where the texts of the value start in `texts`.
    base: usize,
    /// The values written so far.
    count: u32,
    /// Whether the writer takes one value only and ignores the rest.
    single: bool,
}

impl<'a> Writer<'a> {
    /// A writer after the items and texts there are, whose texts start at `base`.
    fn new(items: &'a mut Vec<Item>, texts: &'a mut String, base: usize, single: bool) -> Self {
        Self {
            items,
            texts,
            base,
            count: 0,
            single,
        }
    }
}

impl Writer<'_> {
    /// A writer of the contents of a value, on the same tape.
    fn inner(&mut self, single: bool) -> Writer<'_> {
        Writer::new(&mut *self.items, &mut *self.texts, self.base, single)
    }

    /// Whether the next value is taken: not if a single writer has its value.
    fn open(&self) -> bool {
        !(self.single && self.count >= 1)
    }

    fn push(&mut self, item: Item) {
        if self.open() {
            self.items.push(item);
            self.count += 1;
        }
    }

    /// The offset of the next text in the texts of the value, if `len` more bytes fit.
    fn text_start(&self, len: usize) -> Option<u32> {
        let start = self.texts.len() - self.base;
        u32::try_from(start + len).ok()?;
        u32::try_from(start).ok()
    }

    /// Writes null.
    pub fn null(&mut self) {
        self.push(Item::Null);
    }

    /// Writes a boolean.
    pub fn bool(&mut self, value: bool) {
        self.push(Item::Bool(value));
    }

    /// Writes a non-negative integer.
    pub fn uint(&mut self, value: u64) {
        self.push(Item::Uint(value));
    }

    /// Writes an integer. Non-negative ones are written as [`Writer::uint`], as the
    /// protocol has one kind of integer.
    pub fn int(&mut self, value: i64) {
        match u64::try_from(value) {
            Ok(value) => self.uint(value),
            Err(_) => self.push(Item::Int(value)),
        }
    }

    /// Writes a floating point number.
    pub fn float(&mut self, value: f64) {
        self.push(Item::Float(value));
    }

    /// Writes a text. A text that would take the texts of the value past 4 GiB is
    /// written as null.
    pub fn text(&mut self, value: &str) {
        if !self.open() {
            return;
        }
        match self.text_start(value.len()) {
            Some(start) => {
                self.texts.push_str(value);
                self.push(Item::Text {
                    start,
                    len: value.len() as u32,
                });
            }
            None => self.null(),
        }
    }

    /// Writes `value` as exactly one value: null if it writes none, the first if it
    /// writes more.
    pub fn value(&mut self, value: impl AsValue) {
        if !self.open() {
            return;
        }
        let mut one = self.inner(true);
        value.write(&mut one);
        if one.count == 0 {
            one.null();
        }
        self.count += 1;
    }

    /// Writes an array of the values `elements` writes, each an element.
    pub fn array(&mut self, elements: impl FnOnce(&mut Writer<'_>)) {
        if !self.open() {
            return;
        }
        let head = self.items.len();
        self.items.push(Item::Array { len: 0, size: 0 });
        let mut inner = self.inner(false);
        elements(&mut inner);
        let len = inner.count;
        let size = (self.items.len() - head - 1) as u32;
        self.items[head] = Item::Array { len, size };
        self.count += 1;
    }

    /// Writes a record of the fields `fields` writes.
    pub fn record(&mut self, fields: impl FnOnce(&mut FieldWriter<'_>)) {
        if !self.open() {
            return;
        }
        let head = self.items.len();
        self.items.push(Item::Record { len: 0, size: 0 });
        let mut inner = FieldWriter {
            writer: self.inner(false),
        };
        fields(&mut inner);
        let len = inner.writer.count;
        let size = (self.items.len() - head - 1) as u32;
        self.items[head] = Item::Record { len, size };
        self.count += 1;
    }

    /// Opens an array of `len` elements: pushes its head and gives its index, to
    /// close it with [`Writer::close`] after its elements.
    pub(crate) fn open_array(&mut self, len: u32) -> usize {
        self.items.push(Item::Array { len, size: 0 });
        self.items.len() - 1
    }

    /// Opens a record of `len` fields, see [`Writer::open_array`].
    pub(crate) fn open_record(&mut self, len: u32) -> usize {
        self.items.push(Item::Record { len, size: 0 });
        self.items.len() - 1
    }

    /// Closes the array or record opened at `at`: sets its size, counts a value.
    pub(crate) fn close(&mut self, at: usize) {
        let size = (self.items.len() - at - 1) as u32;
        match &mut self.items[at] {
            Item::Array { size: slot, .. } | Item::Record { size: slot, .. } => *slot = size,
            _ => {}
        }
        self.count += 1;
    }

    /// Writes the name of the next field of a record, gives its range in the texts
    /// buffer. A name past 4 GiB is the caller's to refuse beforehand.
    pub(crate) fn key(&mut self, name: &str) -> std::ops::Range<usize> {
        let at = self.texts.len();
        let start = self.text_start(name.len()).unwrap_or(0);
        self.texts.push_str(name);
        self.items.push(Item::Key {
            start,
            len: name.len() as u32,
        });
        at..self.texts.len()
    }

    /// The whole texts buffer.
    pub(crate) fn texts(&self) -> &str {
        self.texts
    }

    /// Writes a copy of `value`, with its text offsets moved to this tape. An empty
    /// value is written as null.
    pub(crate) fn copy(&mut self, value: ValueRef<'_>) {
        if !self.open() {
            return;
        }
        if value.items.is_empty() {
            return self.null();
        }
        let Some(shift) = self.text_start(value.texts.len()) else {
            return self.null();
        };
        self.texts.push_str(value.texts);
        self.items
            .extend(value.items.iter().map(|item| match *item {
                Item::Text { start, len } => Item::Text {
                    start: start + shift,
                    len,
                },
                Item::Key { start, len } => Item::Key {
                    start: start + shift,
                    len,
                },
                other => other,
            }));
        self.count += 1;
    }
}

/// Writes the fields of a record: the name and the value of each.
pub struct FieldWriter<'a> {
    writer: Writer<'a>,
}

impl FieldWriter<'_> {
    /// Writes a field. Names are not checked: a record with a repeated name is stored as
    /// written and the store drops it as invalid.
    pub fn field(&mut self, name: &str, value: impl AsValue) {
        let Some(start) = self.writer.text_start(name.len()) else {
            return;
        };
        self.writer.texts.push_str(name);
        self.writer.items.push(Item::Key {
            start,
            len: name.len() as u32,
        });
        self.writer.value(value);
    }
}

struct Rec<F>(F);

impl<F: Fn(&mut FieldWriter<'_>)> AsValue for Rec<F> {
    fn write(&self, to: &mut Writer<'_>) {
        to.record(|fields| (self.0)(fields));
    }
}

/// A record value built from its fields, e.g.
/// `record(|r| { r.field("t", 21.5); r.field("mode", "run"); })`.
pub fn record<F: Fn(&mut FieldWriter<'_>)>(fields: F) -> impl AsValue {
    Rec(fields)
}

macro_rules! as_value {
    ($method:ident as $cast:ty: $($type:ty),*) => {$(
        impl AsValue for $type {
            fn write(&self, to: &mut Writer<'_>) {
                to.$method(*self as $cast);
            }
        }
    )*};
}

as_value!(float as f64: f64, f32);
as_value!(int as i64: i64, i32, i16, i8);
as_value!(uint as u64: u64, u32, u16, u8);
as_value!(bool as bool: bool);

impl AsValue for str {
    fn write(&self, to: &mut Writer<'_>) {
        to.text(self);
    }
}

impl AsValue for String {
    fn write(&self, to: &mut Writer<'_>) {
        to.text(self);
    }
}

impl<T: AsValue> AsValue for Option<T> {
    fn write(&self, to: &mut Writer<'_>) {
        match self {
            Some(value) => to.value(value),
            None => to.null(),
        }
    }
}

impl<T: AsValue> AsValue for [T] {
    fn write(&self, to: &mut Writer<'_>) {
        to.array(|elements| {
            for element in self {
                elements.value(element);
            }
        });
    }
}

impl<T: AsValue, const N: usize> AsValue for [T; N] {
    fn write(&self, to: &mut Writer<'_>) {
        self.as_slice().write(to);
    }
}

impl<T: AsValue> AsValue for Vec<T> {
    fn write(&self, to: &mut Writer<'_>) {
        self.as_slice().write(to);
    }
}

impl AsValue for Data {
    fn write(&self, to: &mut Writer<'_>) {
        match self {
            Self::Null => to.null(),
            Self::Uint(value) => to.uint(*value),
            Self::Int(value) => to.int(*value),
            Self::Float(value) => to.float(*value),
            Self::Bool(value) => to.bool(*value),
            Self::String(value) => to.text(value),
            Self::Array(values) => values.as_slice().write(to),
            Self::Record(fields) => to.record(|record| {
                for (name, value) in fields {
                    record.field(name, value);
                }
            }),
        }
    }
}

impl AsValue for Value {
    fn write(&self, to: &mut Writer<'_>) {
        to.copy(self.as_ref());
    }
}

impl AsValue for ValueRef<'_> {
    fn write(&self, to: &mut Writer<'_>) {
        to.copy(*self);
    }
}

impl<T: AsValue + ?Sized> AsValue for &T {
    fn write(&self, to: &mut Writer<'_>) {
        (**self).write(to);
    }
}

#[cfg(test)]
mod tests;
