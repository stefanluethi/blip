//! Searching ascending timestamps, in nanoseconds.

use std::ops::Range;

/// Index of the first item at or after `t`.
pub fn lower_bound_by<T>(items: &[T], t: u64, time: impl Fn(&T) -> u64) -> usize {
    // items are ascending
    items.partition_point(|item| time(item) < t)
}

/// Index of the first item after `t`.
pub fn upper_bound_by<T>(items: &[T], t: u64, time: impl Fn(&T) -> u64) -> usize {
    // items are ascending
    items.partition_point(|item| time(item) <= t)
}

/// The items from `start` up to `end`, without `end`. Empty if `end <= start`.
pub fn between_by<T>(
    items: &[T],
    (start, end): (u64, u64),
    time: impl Fn(&T) -> u64,
) -> Range<usize> {
    let first = lower_bound_by(items, start, &time);
    first..first + lower_bound_by(&items[first..], end, &time)
}

/// Index of the first timestamp at or after `t`.
pub fn lower_bound(timestamps: &[u64], t: u64) -> usize {
    lower_bound_by(timestamps, t, |&timestamp| timestamp)
}

/// Index of the first timestamp after `t`.
pub fn upper_bound(timestamps: &[u64], t: u64) -> usize {
    upper_bound_by(timestamps, t, |&timestamp| timestamp)
}

/// Index of the timestamp closest to `t`, all in nanoseconds.
pub fn closest_index(timestamps: &[u64], t: u64) -> Option<usize> {
    let next = lower_bound(timestamps, t);
    let candidates = [next.checked_sub(1), Some(next)];
    candidates
        .into_iter()
        .flatten()
        .filter(|&index| index < timestamps.len())
        .min_by_key(|&index| timestamps[index].abs_diff(t))
}

/// Index of the newest timestamp at or before `t`, i.e. of the sample that is current
/// at `t`, all in nanoseconds.
pub fn index_at(timestamps: &[u64], t: u64) -> Option<usize> {
    upper_bound(timestamps, t).checked_sub(1)
}

/// Indices of the timestamps within `[start, end]`, all in nanoseconds.
pub fn within(timestamps: &[u64], (start, end): (u64, u64)) -> Range<usize> {
    let first = lower_bound(timestamps, start);
    let last = upper_bound(timestamps, end);
    first..last.max(first)
}

#[cfg(test)]
mod tests;
