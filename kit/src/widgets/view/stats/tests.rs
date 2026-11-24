use super::*;

const MS: u64 = 1_000_000;

/// The stats of `values` 1 ms apart over the window from `start` to `end` in ms.
fn stats_of(values: &[f64], (start, end): (u64, u64), linear: bool) -> Option<Stats> {
    let timestamps: Vec<u64> = (0..values.len() as u64).map(|i| i * MS).collect();
    Stats::of(
        &timestamps,
        |i| values[i],
        (start * MS, end * MS),
        Some(linear),
    )
}

fn assert_near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "{actual} is not {expected}"
    );
}

#[test]
fn levels_are_weighted_by_the_time_each_value_holds() {
    // 0 for 1 ms, 10 for 3 ms (the last sample holds for no time)
    let values = [0.0, 10.0, 10.0, 10.0, 10.0];
    let stats = stats_of(&values, (0, 4), false).unwrap();
    assert_eq!((stats.min, stats.max), (0.0, 10.0));
    assert_near(stats.mean, 7.5);
    assert_near(stats.rms, 75.0_f64.sqrt());
    assert_near(stats.std_dev, (75.0 - 7.5 * 7.5_f64).sqrt());
}

#[test]
fn the_sample_before_the_window_holds_into_it() {
    let values = [4.0, 4.0, 8.0];
    // from 0.5 ms to 2 ms: 4 for 1.5 ms, then 8 at the end
    let timestamps = [0, MS, 2 * MS];
    let stats = Stats::of(&timestamps, |i| values[i], (MS / 2, 2 * MS), Some(false)).unwrap();
    assert_eq!((stats.min, stats.max), (4.0, 8.0));
    assert_near(stats.mean, 4.0);
    // a sample after the window counts for nothing
    let stats = Stats::of(&timestamps, |i| values[i], (0, MS + MS / 2), Some(false)).unwrap();
    assert_eq!(stats.max, 4.0);
}

#[test]
fn a_window_without_time_between_samples_averages_their_values() {
    let stats = stats_of(&[2.0, 6.0], (1, 1), false).unwrap();
    assert_eq!((stats.min, stats.max, stats.mean), (6.0, 6.0, 6.0));
    let timestamps = [0, 0, 0];
    let values = [1.0, 2.0, 6.0];
    let stats = Stats::of(&timestamps, |i| values[i], (0, 0), Some(false)).unwrap();
    assert_near(stats.mean, 3.0);
}

#[test]
fn nulls_are_left_out() {
    let values = [1.0, f64::NAN, 3.0, 3.0];
    let stats = stats_of(&values, (0, 3), false).unwrap();
    assert_eq!((stats.min, stats.max), (1.0, 3.0));
    // 1 for 1 ms, the null for 1 ms, 3 for 1 ms
    assert_near(stats.mean, 2.0);
    assert!(stats_of(&[f64::NAN, f64::NAN], (0, 1), false).is_none());
    assert!(stats_of(&[], (0, 1), false).is_none());
}

#[test]
fn values_far_from_zero_keep_a_precise_spread() {
    let values: Vec<f64> = (0..100).map(|i| 1.0e9 + (i % 2) as f64).collect();
    let stats = stats_of(&values, (0, 99), false).unwrap();
    // 50 zeros and 49 ones above 1e9 hold for 1 ms each
    assert_near(stats.std_dev, (49.0_f64 * 50.0).sqrt() / 99.0);
}

/// A square wave 1 ms apart, high for `high` and low for `low` samples, `cycles`
/// times, starting low.
fn square(high: usize, low: usize, cycles: usize) -> Vec<f64> {
    let cycle = [vec![0.0; low], vec![5.0; high]].concat();
    cycle.repeat(cycles)
}

#[test]
fn frequency_and_duty_come_from_the_edges() {
    // period 4 ms, high 1 ms of it
    let values = square(1, 3, 10);
    let stats = stats_of(&values, (0, 39), false).unwrap();
    let (period, duty) = stats.cycles.unwrap();
    assert_near(period, 0.004);
    assert_near(duty, 0.25);
    assert_near(Stat::Frequency.of(&stats).unwrap(), 250.0);
}

#[test]
fn linear_edges_are_where_the_line_crosses_the_mid_level() {
    // rising from 0 to 4 over 1 ms crosses 2 halfway
    let values = [0.0, 4.0, 4.0, 0.0, 0.0, 4.0, 4.0, 0.0];
    let (period, duty) = stats_of(&values, (0, 7), true).unwrap().cycles.unwrap();
    assert_near(period, 0.004);
    assert_near(duty, 0.5);
    let (period, duty) = stats_of(&values, (0, 7), false).unwrap().cycles.unwrap();
    assert_near(period, 0.004);
    assert_near(duty, 0.5);
}

#[test]
fn a_cycle_needs_three_edges() {
    // rising and falling once is no full cycle
    let values = [0.0, 5.0, 5.0, 0.0, 0.0];
    assert_eq!(stats_of(&values, (0, 4), false).unwrap().cycles, None);
    // falling, rising and falling again is one, low for 2 of 3 ms
    let values = [5.0, 0.0, 0.0, 5.0, 0.0];
    let (period, duty) = stats_of(&values, (0, 4), false).unwrap().cycles.unwrap();
    assert_near(period, 0.003);
    assert_near(duty, 1.0 / 3.0);
    assert_eq!(
        stats_of(&[3.0, 3.0, 3.0], (0, 2), false).unwrap().cycles,
        None
    );
}

#[test]
fn noise_around_the_mid_level_makes_no_edges() {
    let values = [0.0, 10.0, 5.2, 4.8, 5.1, 4.9, 10.0, 0.0, 10.0, 0.0];
    let (period, _) = stats_of(&values, (0, 9), false).unwrap().cycles.unwrap();
    // the edges rise at 1 ms, fall at 7 ms (the last crossing before going low) and
    // rise at 8 ms: the noise in between counts not
    assert_near(period, 0.007);
}

#[test]
fn edges_outside_the_window_are_left_out() {
    let values = square(2, 2, 10);
    let (period, duty) = stats_of(&values, (5, 30), false).unwrap().cycles.unwrap();
    assert_near(period, 0.004);
    assert_near(duty, 0.5);
}

#[test]
fn a_gap_ends_a_cycle() {
    let values = [0.0, 5.0, f64::NAN, 0.0, 5.0, 0.0];
    // the rise before the gap and the rise and fall after it make no full cycle
    assert_eq!(stats_of(&values, (0, 5), false).unwrap().cycles, None);
    // full cycles on both sides of a gap count
    let values = [square(2, 2, 3), vec![f64::NAN], square(1, 1, 3)].concat();
    let (period, duty) = stats_of(&values, (0, 18), false).unwrap().cycles.unwrap();
    // 2 cycles of 4 ms before (from 2 to 10 ms), 2 of 2 ms after (14 to 18 ms)
    assert_near(period, 0.012 / 4.0);
    assert_near(duty, 0.5);
}

#[test]
fn cycles_are_found_only_when_asked() {
    let values = square(2, 2, 4);
    let timestamps: Vec<u64> = (0..values.len() as u64).map(|i| i * MS).collect();
    let window = (0, (values.len() as u64 - 1) * MS);
    let stats = Stats::of(&timestamps, |i| values[i], window, None).unwrap();
    assert_eq!(stats.cycles, None);
    assert_eq!((stats.min, stats.max), (0.0, 5.0));
    assert!(
        stats_of(&values, (0, values.len() as u64 - 1), false)
            .unwrap()
            .cycles
            .is_some()
    );
}

#[test]
fn the_next_stat_starts_anew_for_another_trace() {
    let measurement = |trace: &str, stat| Measurement {
        trace: trace.to_owned(),
        stat,
    };
    let stats = [measurement("a", Stat::Mean), measurement("a", Stat::Rms)];
    let next = Measurement::next(&stats, &["a", "b"]).unwrap();
    assert_eq!(next, measurement("a", Stat::StdDev));
    // "a" is no longer shown
    let next = Measurement::next(&stats, &["b"]).unwrap();
    assert_eq!(next, measurement("b", Stat::Mean));
}

#[test]
fn stats_are_written_in_their_units() {
    let format = DisplayFormat::Default;
    assert_eq!(
        Stat::Frequency.format(20_000.0, Some("V"), format),
        "20 kHz"
    );
    assert_eq!(Stat::Period.format(0.000_05, None, format), "50 µs");
    assert_eq!(Stat::Period.format(2.5, None, format), "2.5 s");
    assert_eq!(Stat::Duty.format(0.25, Some("V"), format), "25 %");
    assert_eq!(Stat::Mean.format(12.0234, Some("V"), format), "12.02 V");
    assert_eq!(Stat::Max.format(255.0, None, DisplayFormat::Hex), "0xff");
}
