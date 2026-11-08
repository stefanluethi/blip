use super::*;

const TS: [u64; 4] = [10, 20, 20, 30];

#[test]
fn bounds_put_equal_times_inside_or_outside() {
    assert_eq!(lower_bound(&TS, 20), 1);
    assert_eq!(upper_bound(&TS, 20), 3);
    assert_eq!(lower_bound(&TS, 5), 0);
    assert_eq!(upper_bound(&TS, 5), 0);
    assert_eq!(lower_bound(&TS, 35), 4);
    assert_eq!(upper_bound(&TS, 35), 4);
}

#[test]
fn between_leaves_out_the_end_within_keeps_it() {
    let time = |&t: &u64| t;
    assert_eq!(between_by(&TS, (20, 30), time), 1..3);
    assert_eq!(within(&TS, (20, 30)), 1..4);
    assert_eq!(between_by(&TS, (10, 10), time), 0..0);
    assert_eq!(within(&TS, (20, 20)), 1..3);
    assert_eq!(between_by(&TS, (30, 20), time), 3..3);
    assert_eq!(within(&TS, (30, 20)), 3..3);
    assert_eq!(within(&TS, (21, 29)), 3..3);
    assert_eq!(between_by(&TS, (0, 100), time), 0..4);
}

#[test]
fn upper_bound_by_reads_the_key() {
    let items = [(1, 'a'), (2, 'b'), (2, 'c'), (5, 'd')];
    assert_eq!(upper_bound_by(&items, 2, |item| item.0), 3);
    assert_eq!(lower_bound_by(&items, 2, |item| item.0), 1);
}

#[test]
fn closest_index_picks_the_closest_timestamp() {
    let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
    assert_eq!(closest_index(&timestamps, 2_400_000), Some(2));
    assert_eq!(closest_index(&timestamps, 2_600_000), Some(3));
    assert_eq!(closest_index(&timestamps, 1_000_000_000), Some(9));
    assert!(closest_index(&[], 1).is_none());
}

#[test]
fn index_at_picks_the_current_sample() {
    let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
    assert_eq!(index_at(&timestamps, 2_600_000), Some(2));
    assert_eq!(index_at(&timestamps, 3_000_000), Some(3));
    assert_eq!(index_at(&timestamps, 1_000_000_000), Some(9));
    assert!(index_at(&timestamps[1..], 0).is_none());
}
