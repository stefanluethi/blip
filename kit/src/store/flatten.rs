//! Flattens the value of a sample into its leaves: the fields of records and their
//! arrays of numbers, the shape the traces hold. The walk reads the items of a
//! [`ValueRef`] once and allocates nothing once the buffers grew, but for a value that
//! is invalid.

use super::numbers::Number;
use super::value::{Item, ValueRef};
use std::ops::Range;

/// The value of a leaf of a value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Leaf {
    Null,
    Number(Number),
    /// The bytes of a text in the texts of the value.
    Text(Range<usize>),
    /// An array of numbers, its shape and its elements in row-major order in
    /// [`Flat::shapes`] and [`Flat::numbers`].
    Array {
        shape: Range<usize>,
        elements: Range<usize>,
    },
    /// A value no trace can hold, and why.
    Invalid(String),
}

/// A value flattened into its leaves, kept to be filled again by the next value.
#[derive(Default, Debug)]
pub(crate) struct Flat {
    /// The leaves with the fields leading to them, e.g. `.acc.z`, empty for a value
    /// that is no record.
    pub(crate) leaves: Vec<(Range<usize>, Leaf)>,
    /// The texts of the fields leading to the leaves.
    pub(crate) fields: String,
    pub(crate) numbers: Vec<Number>,
    pub(crate) shapes: Vec<usize>,
    /// The names of the fields of the records being read, to refuse duplicates.
    keys: Vec<Range<usize>>,
    /// The fields leading to the value being read.
    stack: String,
}

impl Flat {
    fn clear(&mut self) {
        self.leaves.clear();
        self.fields.clear();
        self.numbers.clear();
        self.shapes.clear();
        self.keys.clear();
        self.stack.clear();
    }

    /// Whether the value is a record, which has fields.
    pub(crate) fn is_record(&self) -> bool {
        self.leaves
            .first()
            .is_none_or(|(field, _)| !field.is_empty())
    }

    /// The fields leading to a leaf, e.g. `.acc.z`.
    pub(crate) fn field(&self, field: &Range<usize>) -> &str {
        &self.fields[field.clone()]
    }

    fn leaf(&mut self, leaf: Leaf) {
        let start = self.fields.len();
        self.fields.push_str(&self.stack);
        self.leaves.push((start..self.fields.len(), leaf));
    }

    /// Replaces the leaves with those of `value`. The texts of `Leaf::Text` are ranges
    /// into `value.texts()`. A record with a repeated field name makes the whole value
    /// one invalid leaf.
    pub(crate) fn fill(&mut self, value: ValueRef<'_>) {
        self.clear();
        let items = value.items();
        if items.is_empty() {
            return self.leaf(Leaf::Null);
        }
        if let Err(name) = self.value(items, value.texts(), 0) {
            self.clear();
            self.leaf(Leaf::Invalid(format!("duplicate record field `{name}`")));
        }
    }

    /// Flattens the value at `at` of `items`, returns the index after it. Fails with
    /// the name of a repeated field.
    fn value(&mut self, items: &[Item], texts: &str, at: usize) -> Result<usize, String> {
        let item = items[at];
        let end = at + 1 + item.size();
        match item {
            Item::Record { len, .. } => {
                let (keys, depth) = (self.keys.len(), self.stack.len());
                let mut index = at + 1;
                for _ in 0..len {
                    let Some(&Item::Key { start, len }) = items.get(index) else {
                        break;
                    };
                    let name = &texts[start as usize..(start + len) as usize];
                    if (self.keys[keys..].iter()).any(|key| texts[key.clone()] == *name) {
                        return Err(name.to_owned());
                    }
                    self.keys.push(start as usize..(start + len) as usize);
                    self.stack.push('.');
                    self.stack.push_str(name);
                    index = self.value(items, texts, index + 1)?;
                    self.stack.truncate(depth);
                }
                self.keys.truncate(keys);
            }
            Item::Array { .. } => {
                let (shapes, numbers) = (self.shapes.len(), self.numbers.len());
                let mut dimensions = None;
                let leaf = match self.elements(items, at, 0, shapes, &mut dimensions) {
                    Some(problem) => Leaf::Invalid(problem),
                    None => Leaf::Array {
                        shape: shapes..self.shapes.len(),
                        elements: numbers..self.numbers.len(),
                    },
                };
                self.leaf(leaf);
            }
            Item::Text { start, len } => {
                self.leaf(Leaf::Text(start as usize..(start + len) as usize));
            }
            Item::Null | Item::Key { .. } => self.leaf(Leaf::Null),
            item => match number(item) {
                Some(number) => self.leaf(Leaf::Number(number)),
                None => self.leaf(Leaf::Null),
            },
        }
        Ok(end)
    }

    /// Reads the elements of the array at `head` of `items`, at `depth`, whose shape
    /// starts at `base` in [`Self::shapes`]. `dimensions` is the number of dimensions
    /// once known. Returns why the array cannot be held, if it cannot.
    fn elements(
        &mut self,
        items: &[Item],
        head: usize,
        depth: usize,
        base: usize,
        dimensions: &mut Option<usize>,
    ) -> Option<String> {
        let Item::Array { len, .. } = items[head] else {
            return None;
        };
        let len = len as usize;
        let level = base + depth;
        let mut problem = None;
        if level == self.shapes.len() {
            self.shapes.push(len);
        } else if self.shapes[level] != len {
            problem = Some(format!(
                "expected an array of {} elements, got {len}",
                self.shapes[level]
            ));
        }
        let mut index = head + 1;
        for _ in 0..len {
            // after a problem the rest of the array is skipped by its size
            if problem.is_some() {
                break;
            }
            let (at, item) = (index, items[index]);
            index += 1 + item.size();
            problem = match item {
                Item::Array { .. } if dimensions.is_none_or(|count| depth + 1 < count) => {
                    self.elements(items, at, depth + 1, base, dimensions)
                }
                Item::Null | Item::Key { .. } => {
                    // a null element stands for all elements below it
                    let count = match dimensions {
                        Some(_) => self.shapes[level + 1..].iter().product(),
                        None => {
                            *dimensions = Some(depth + 1);
                            1
                        }
                    };
                    self.numbers
                        .extend(std::iter::repeat_n(Number::Null, count));
                    None
                }
                item => match number(item) {
                    Some(number) if dimensions.is_none_or(|count| count == depth + 1) => {
                        *dimensions = Some(depth + 1);
                        self.numbers.push(number);
                        None
                    }
                    Some(number) => Some(format!("expected an array, got {}", number.kind_name())),
                    None => Some(format!(
                        "array elements must be numbers or bools, got {}",
                        match item {
                            Item::Text { .. } => "string",
                            Item::Record { .. } => "record",
                            _ => "array",
                        }
                    )),
                },
            };
        }
        problem
    }
}

/// The number or bool of `item`, if it is one.
fn number(item: Item) -> Option<Number> {
    Some(match item {
        Item::Uint(value) => Number::Uint(value),
        Item::Int(value) => Number::Int(value),
        Item::Float(value) => Number::Float(value),
        Item::Bool(value) => Number::Bool(value),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
