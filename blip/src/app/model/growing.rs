//! A vector that grows without stopping to copy all its items at once.

use std::ops::Deref;

/// Vectors of this many items or more grow by [`Growing`] copying, smaller ones as
/// usual, which copies so few at once that it takes no time.
const MIN_CAPACITY: usize = 1 << 16;
/// Items copied to the larger buffer for each item pushed, which starts when the
/// smaller one has room for an eighth of its items left, so it is full when all are
/// copied, and the larger buffer is kept beside it only for the last eighth.
const COPIED_PER_ITEM: usize = 8;

/// A vector of `Copy` items, e.g. the samples of a trace, that grows without pausing:
/// a plain vector doubles its buffer when it is full and copies all items at once,
/// about 35 ms for 8 million timestamps. Once this one is seven eighths full, it makes
/// the buffer twice as large and copies eight items over with each item pushed, so the
/// larger buffer holds all items by the time the smaller one is full. It reads as a
/// slice, of the smaller buffer while copying.
#[derive(Clone, Debug)]
pub struct Growing<T> {
    items: Vec<T>,
    /// The larger buffer while the items are copied to it, and how many are copied.
    next: Option<(Vec<T>, usize)>,
    /// Up to how many items can be pushed as to a plain vector, without copying or
    /// starting to.
    room: usize,
}

impl<T> Default for Growing<T> {
    fn default() -> Self {
        Vec::new().into()
    }
}

impl<T: PartialEq> PartialEq for Growing<T> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl<T> Deref for Growing<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items
    }
}

impl<T> From<Vec<T>> for Growing<T> {
    fn from(items: Vec<T>) -> Self {
        Self {
            items,
            next: None,
            room: 0,
        }
    }
}

impl<T> FromIterator<T> for Growing<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        Vec::from_iter(items).into()
    }
}

/// Frees `items`, a large buffer apart from the frame on native platforms, where
/// returning it to the system takes as long as copying it.
fn release<T: Send + 'static>(items: Vec<T>) {
    #[cfg(not(target_arch = "wasm32"))]
    if items.capacity() >= MIN_CAPACITY {
        std::thread::spawn(move || drop(items));
        return;
    }
    drop(items);
}

impl<T: Copy + Send + 'static> Growing<T> {
    pub fn push(&mut self, item: T) {
        if self.items.len() >= self.room {
            self.make_room(1);
        }
        self.items.push(item);
    }

    /// Appends `count` copies of `item`.
    pub fn extend_repeat(&mut self, item: T, count: usize) {
        if self.items.len() + count > self.room {
            self.make_room(count);
        }
        self.items.extend(std::iter::repeat_n(item, count));
    }

    /// Rotates the items from `start` on right by `by`, e.g. to move the last item to
    /// `start`.
    pub fn rotate_tail(&mut self, start: usize, by: usize) {
        self.items[start..].rotate_right(by);
        if let Some((next, copied)) = &mut self.next {
            // the moved items are copied again
            *copied = (*copied).min(start);
            next.truncate(*copied);
        }
    }

    /// Copies on to the larger buffer for `additional` items to be pushed, switches to
    /// it when the smaller one has no room for them, and starts copying once seven
    /// eighths full.
    fn make_room(&mut self, additional: usize) {
        if let Some((next, copied)) = &mut self.next {
            let end = (*copied + COPIED_PER_ITEM * additional).min(self.items.len());
            next.extend_from_slice(&self.items[*copied..end]);
            *copied = end;
        }
        if self.items.len() + additional > self.items.capacity()
            && let Some((mut next, copied)) = self.next.take()
        {
            // none or a few left to copy, unless many items are pushed at once
            next.extend_from_slice(&self.items[copied..]);
            release(std::mem::replace(&mut self.items, next));
        }
        // as usual for small vectors, or more items than double the capacity
        self.items.reserve(additional);
        let capacity = self.items.capacity();
        let filled = self.items.len() + additional;
        if self.next.is_none() && capacity >= MIN_CAPACITY && filled > capacity - capacity / 8 {
            self.next = Some((Vec::with_capacity(capacity * 2), 0));
        }
        self.room = match (&self.next, capacity < MIN_CAPACITY) {
            // each item pushed copies some
            (Some(_), _) => 0,
            (None, true) => capacity,
            (None, false) => capacity - capacity / 8,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_are_kept_in_order_while_growing() {
        let mut growing = Growing::default();
        let mut plain = vec![];
        for item in 0..(MIN_CAPACITY * 9) as u64 {
            match item % 1000 {
                0 => {
                    growing.extend_repeat(item, 3);
                    plain.extend([item; 3]);
                }
                _ => {
                    growing.push(item);
                    plain.push(item);
                }
            }
            if item % 7919 == 0 {
                // a late item moved into place, also into what was copied already
                let start = plain.len() / 3;
                growing.rotate_tail(start, 1);
                plain[start..].rotate_right(1);
            }
            assert_eq!(growing.len(), plain.len());
        }
        assert_eq!(&*growing, plain.as_slice());
    }

    #[test]
    fn the_larger_buffer_is_filled_before_switching() {
        let mut growing = Growing::from(Vec::with_capacity(MIN_CAPACITY));
        for item in 0..MIN_CAPACITY as u32 {
            growing.push(item);
        }
        // the copying started seven eighths full and is all but done when full
        let (next, copied) = growing.next.as_ref().unwrap();
        assert!(*copied + COPIED_PER_ITEM >= MIN_CAPACITY, "{copied}");
        assert_eq!(next.len(), *copied);
        growing.push(0);
        assert!(growing.items.capacity() >= 2 * MIN_CAPACITY);
        assert_eq!(growing.len(), MIN_CAPACITY + 1);
        // and starts again only seven eighths full
        assert!(growing.next.is_none());
    }
}
