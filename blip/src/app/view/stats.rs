//! Measurements of the traces of a plot over the shown window, like those of a digital
//! oscilloscope.

use super::time_axis::visible_range;
use super::{Formats, short_names};
use crate::app::model::Model;
use crate::app::model::numbers::{
    DisplayFormat, Number, NumberKind, SHOWN_DIGITS, format_significant,
};
use crate::app::model::traces::Series;
use crate::app::theme;
use crate::app::time::seconds_since;
use std::collections::BTreeMap;

/// How far above and below the mid level a signal must go to count as high or low, as
/// a fraction of its peak-to-peak range, so that noise around the mid level makes no
/// edges.
const HYSTERESIS: f64 = 0.1;

named_enum! {
    /// What is measured of a trace.
    #[derive(
        serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Hash, Debug,
    )]
    pub enum Stat {
        Min => "Min",
        Max => "Max",
        PkPk => "Pk-Pk",
        #[default]
        Mean => "Mean",
        Rms => "RMS",
        StdDev => "Std dev",
        Frequency => "Frequency",
        Period => "Period",
        Duty => "Duty",
    }
}

impl Stat {
    /// The name in the measurements below the plot.
    pub fn short_name(self) -> &'static str {
        match self {
            Stat::Frequency => "Freq",
            stat => stat.name(),
        }
    }

    /// What it measures, for a tooltip.
    pub fn hint(self) -> &'static str {
        match self {
            Stat::Min => "The lowest value in the shown window",
            Stat::Max => "The highest value in the shown window",
            Stat::PkPk => "The highest minus the lowest value in the shown window",
            Stat::Mean => "The average over time, each value held until the next sample",
            Stat::Rms => "The root mean square over time, each value held until the next sample",
            Stat::StdDev => {
                "The standard deviation over time, each value held until the next sample"
            }
            Stat::Frequency => "Full cycles per second, from crossings of the mid level",
            Stat::Period => "The average length of a full cycle, from crossings of the mid level",
            Stat::Duty => "The part of the full cycles above the mid level",
        }
    }

    /// The value of this stat of `stats`, `None` if it has none, like a frequency
    /// without a full cycle.
    pub fn of(self, stats: &Stats) -> Option<f64> {
        match self {
            Stat::Min => Some(stats.min),
            Stat::Max => Some(stats.max),
            Stat::PkPk => Some(stats.max - stats.min),
            Stat::Mean => Some(stats.mean),
            Stat::Rms => Some(stats.rms),
            Stat::StdDev => Some(stats.std_dev),
            Stat::Frequency => stats.cycles.map(|(period, _)| 1.0 / period),
            Stat::Period => stats.cycles.map(|(period, _)| period),
            Stat::Duty => stats.cycles.map(|(_, duty)| duty),
        }
    }

    /// `value` of this stat written for a trace with `unit` whose values are written in
    /// `format`.
    pub fn format(self, value: f64, unit: Option<&str>, format: DisplayFormat) -> String {
        match self {
            Stat::Frequency => with_prefix(value, "Hz"),
            Stat::Period => with_prefix(value, "s"),
            Stat::Duty => format!("{} %", format_significant(value * 100.0, 3)),
            _ => super::with_unit(
                Number::Float(value).format(Some(SHOWN_DIGITS), format),
                unit,
            ),
        }
    }
}

/// `value` in `unit` with an SI prefix that leaves one to three integer digits.
fn with_prefix(value: f64, unit: &str) -> String {
    const PREFIXES: [(f64, &str); 7] = [
        (1.0e9, "G"),
        (1.0e6, "M"),
        (1.0e3, "k"),
        (1.0, ""),
        (1.0e-3, "m"),
        (1.0e-6, "µ"),
        (1.0e-9, "n"),
    ];
    let (scale, prefix) = PREFIXES
        .into_iter()
        .find(|&(scale, _)| value.abs() >= scale)
        .unwrap_or(PREFIXES[PREFIXES.len() - 1]);
    format!(
        "{} {prefix}{unit}",
        format_significant(value / scale, SHOWN_DIGITS)
    )
}

/// A stat of a trace shown below a plot.
#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Debug)]
pub struct Measurement {
    pub trace: String,
    pub stat: Stat,
}

/// The traces a plot shows: their paths, names in the legend and indices, which pick
/// their colours.
pub type ShownTraces<'a> = [(&'a str, &'a str, usize)];

impl Measurement {
    /// A measurement to add after `measurements`: of the trace of the last one if it is
    /// still among `paths`, else of the first of `paths`, and of the next stat it has
    /// none of yet.
    fn next(measurements: &[Measurement], paths: &[&str]) -> Option<Self> {
        let last = measurements.last();
        let trace = last
            .map(|last| last.trace.as_str())
            .filter(|trace| paths.contains(trace))
            .or(paths.first().copied())?;
        let after = last.map_or(Stat::default() as usize, |last| last.stat as usize + 1);
        let stat = (0..Stat::ALL.len())
            .map(|offset| Stat::ALL[(after + offset) % Stat::ALL.len()])
            .find(|&stat| {
                !measurements
                    .iter()
                    .any(|other| other.trace == trace && other.stat == stat)
            })
            .unwrap_or_default();
        Some(Self {
            trace: trace.to_owned(),
            stat,
        })
    }
}

/// Shows a section of the properties to choose the stats shown below a plot of the
/// traces at `paths`, one per row, if there are traces or stats. Stats of traces no
/// longer shown are kept, greyed.
pub fn properties_ui(ui: &mut egui::Ui, measurements: &mut Vec<Measurement>, paths: &[&str]) {
    if paths.is_empty() && measurements.is_empty() {
        return;
    }
    let names = short_names(paths.iter().copied());
    let name_of = |path: &str| {
        paths
            .iter()
            .position(|&shown| shown == path)
            .map(|index| names[index])
    };
    super::properties_section(ui, "Stats", |ui| {
        let mut removed = None;
        for (index, measurement) in measurements.iter_mut().enumerate() {
            let name = name_of(&measurement.trace);
            let mut text = egui::RichText::new(
                name.unwrap_or_else(|| short_names([measurement.trace.as_str()])[0]),
            );
            if name.is_none() {
                text = text.color(theme::tokens(ui.visuals()).text_faint);
            }
            let hint = if name.is_some() {
                measurement.trace.clone()
            } else {
                format!("{} is not shown in the plot", measurement.trace)
            };
            egui::ComboBox::from_id_salt(("stat trace", index))
                .selected_text(text)
                .width(super::PROPERTY_LABEL_WIDTH)
                .show_ui(ui, |ui| {
                    for (&path, &name) in paths.iter().zip(&names) {
                        ui.selectable_value(&mut measurement.trace, path.to_owned(), name)
                            .on_hover_text(path);
                    }
                })
                .response
                .on_hover_text(hint);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("stat", index))
                    .selected_text(measurement.stat.name())
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for stat in Stat::ALL {
                            ui.selectable_value(&mut measurement.stat, stat, stat.name())
                                .on_hover_text(stat.hint());
                        }
                    })
                    .response
                    .on_hover_text(measurement.stat.hint());
                let remove = ui
                    .add(theme::icon_button(egui_phosphor::regular::MINUS))
                    .on_hover_text("Remove this stat");
                if remove.clicked() {
                    removed = Some(index);
                }
            });
            ui.end_row();
        }
        if let Some(index) = removed {
            measurements.remove(index);
        }
        let next = Measurement::next(measurements, paths);
        let add = egui::Button::new(format!("{} Add stat", egui_phosphor::regular::PLUS));
        let response = ui
            .add_enabled(next.is_some(), theme::hover_frame(add))
            .on_disabled_hover_text("Add traces to the plot first");
        if response.clicked()
            && let Some(next) = next
        {
            measurements.push(next);
        }
        ui.end_row();
    });
}

/// Whether the curve of `series` in a plot with `interpolation` runs straight between
/// samples, see [`Stats::of_series`].
pub fn is_linear(series: &Series<'_>, interpolation: super::plot::Interpolation) -> bool {
    interpolation == super::plot::Interpolation::Linear && series.kind != NumberKind::Bool
}

/// Shows `measurements` of the `shown` traces of a plot over the shown `window`, in
/// nanoseconds, side by side and wrapping, each in the colour of its trace. Those of
/// traces not shown are left out. `linear` tells for a series whether its curve runs
/// straight between samples.
pub fn measurements_ui(
    ui: &mut egui::Ui,
    measurements: &[Measurement],
    shown: &ShownTraces<'_>,
    (model, formats): (&Model, &Formats),
    window: Option<(u64, u64)>,
    linear: impl Fn(&Series<'_>) -> bool,
) {
    // the stats of each trace, computed once for all its measurements
    let mut stats: BTreeMap<&str, Option<Stats>> = BTreeMap::new();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        for measurement in measurements {
            let Some(&(path, name, index)) =
                shown.iter().find(|(path, _, _)| *path == measurement.trace)
            else {
                continue;
            };
            let trace_stats = *stats.entry(path).or_insert_with(|| {
                let series = model.traces.series(path)?;
                Stats::of_series(&series, window?, linear(&series))
            });
            let value = trace_stats.and_then(|stats| measurement.stat.of(&stats));
            let format = formats.of(path);
            let text = value.map(|value| {
                measurement
                    .stat
                    .format(value, model.traces.unit(path), format)
            });
            let color = theme::trace_color(ui.visuals(), index);
            let label = format!("{}({name})", measurement.stat.short_name());
            chip_ui(ui, measurement, color, &label, text.as_deref(), format)
                .on_hover_text(format!("{path}\n{}", measurement.stat.hint()));
        }
    });
}

/// The room around the contents of a measurement and between them, in points.
const CHIP_MARGIN: egui::Vec2 = egui::vec2(6.0, 2.0);
const CHIP_SPACING: f32 = 6.0;
const DOT_SIZE: f32 = 8.0;

/// Shows a measurement as a dot in `color`, its `label` and its `value`, a dash without
/// one, in a slot as wide as the widest value it showed since it or its `format` last
/// changed, so that the measurements after it keep their place while it changes. It
/// goes on the next line as a whole if it does not fit.
fn chip_ui(
    ui: &mut egui::Ui,
    measurement: &Measurement,
    color: egui::Color32,
    label: &str,
    value: Option<&str>,
    format: DisplayFormat,
) -> egui::Response {
    let tokens = theme::tokens(ui.visuals());
    let galley = |ui: &egui::Ui, text: egui::RichText| {
        egui::WidgetText::from(text).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
    };
    let label = galley(
        ui,
        egui::RichText::new(label).color(ui.visuals().weak_text_color()),
    );
    let value = galley(
        ui,
        match value {
            Some(value) => egui::RichText::new(value).strong(),
            None => egui::RichText::new("—").color(tokens.text_faint),
        },
    );
    let slot = format!("{} {:?} {format:?}", label.text(), measurement.stat);
    let id = ui
        .id()
        .with(("stat width", &measurement.trace, measurement.stat));
    let value_width = ui.data_mut(|data| {
        let (last_slot, width) = data.get_temp_mut_or_default::<(String, f32)>(id);
        if *last_slot != slot {
            *last_slot = slot;
            *width = 0.0;
        }
        *width = width.max(value.size().x);
        *width
    });
    let inner = egui::vec2(
        DOT_SIZE + CHIP_SPACING + label.size().x + CHIP_SPACING + value_width,
        label.size().y.max(value.size().y),
    );
    let size = inner + 2.0 * CHIP_MARGIN;
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        let rect = egui::Rect::from_min_size(ui.max_rect().min, size);
        ui.painter()
            .rect_filled(rect, theme::CONTROL_RADIUS, tokens.control);
        let content = rect.shrink2(CHIP_MARGIN);
        let dot = egui::pos2(content.left() + DOT_SIZE / 2.0, content.center().y);
        ui.painter().circle_filled(dot, DOT_SIZE / 2.0, color);
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(
                content.left() + DOT_SIZE + CHIP_SPACING,
                content.center().y - label.size().y / 2.0,
            ),
            label.size(),
        );
        let value_rect = egui::Rect::from_min_size(
            egui::pos2(
                content.right() - value.size().x,
                content.center().y - value.size().y / 2.0,
            ),
            value.size(),
        );
        ui.put(label_rect, egui::Label::new(label));
        ui.put(value_rect, egui::Label::new(value));
        ui.allocate_rect(rect, egui::Sense::hover())
    })
    .inner
}

/// The stats of the samples of a trace in a window of time.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub rms: f64,
    pub std_dev: f64,
    /// The average length of the full cycles in seconds and the part of them that is
    /// high, `None` without a full cycle.
    pub cycles: Option<(f64, f64)>,
}

impl Stats {
    /// The stats of `series` within `window`, in nanoseconds. Each value holds until the
    /// next sample, also the one before the window into it. The last sample holds for no
    /// time, as what follows it is unknown. Edges are found where the curve crosses the
    /// mid level, between samples if it is `linear`, else at the sample after. `None`
    /// without values.
    pub fn of_series(series: &Series<'_>, window: (u64, u64), linear: bool) -> Option<Self> {
        let series = series.slice(visible_range(series.timestamps, Some(window)));
        Self::of(
            series.timestamps,
            |index| series.value(index),
            window,
            linear,
        )
    }

    /// The stats of the samples at `timestamps` with `value` at their indices, NaN for
    /// nulls, as [`Stats::of_series`] takes them.
    fn of(
        timestamps: &[u64],
        value: impl Fn(usize) -> f64,
        (start, end): (u64, u64),
        linear: bool,
    ) -> Option<Self> {
        // times in seconds from the start of the window
        let seconds = |timestamp: u64| seconds_since(start, timestamp);
        let length = seconds(end);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        // sums over the time each value holds, and over the values for windows without
        // time between their samples, of the values minus the first one, which keeps
        // the sum of squares precise for values far from zero
        let mut shift = None;
        let mut timed = Sums::default();
        let mut counted = Sums::default();
        for (index, &timestamp) in timestamps.iter().enumerate() {
            let value = value(index);
            if !value.is_finite() {
                continue;
            }
            let next = timestamps.get(index + 1).copied().unwrap_or(timestamp);
            let held = seconds(next).min(length) - seconds(timestamp).max(0.0);
            let inside = (start..=end).contains(&timestamp);
            if held <= 0.0 && !inside {
                continue;
            }
            min = min.min(value);
            max = max.max(value);
            let shifted = value - *shift.get_or_insert(value);
            timed.add(shifted, held.max(0.0));
            counted.add(shifted, 1.0);
        }
        let shift = shift?;
        let sums = if timed.weight > 0.0 { timed } else { counted };
        let mean = sums.mean();
        let variance = (sums.mean_square() - mean * mean).max(0.0);
        let mean = shift + mean;
        let cycles = (max > min)
            .then(|| {
                cycles(
                    timestamps,
                    &value,
                    (min + max) / 2.0,
                    (max - min) * HYSTERESIS,
                    start,
                    length,
                    linear,
                )
            })
            .flatten();
        Some(Self {
            min,
            max,
            mean,
            rms: (variance + mean * mean).sqrt(),
            std_dev: variance.sqrt(),
            cycles,
        })
    }
}

/// Sums of weighted values.
#[derive(Clone, Copy, Default)]
struct Sums {
    weight: f64,
    values: f64,
    squares: f64,
}

impl Sums {
    fn add(&mut self, value: f64, weight: f64) {
        self.weight += weight;
        self.values += value * weight;
        self.squares += value * value * weight;
    }

    fn mean(&self) -> f64 {
        self.values / self.weight
    }

    fn mean_square(&self) -> f64 {
        self.squares / self.weight
    }
}

/// The average length of the full cycles of the samples at `timestamps` with `value` at
/// their indices, and the part of them that is high, `None` without a full cycle. An
/// edge is where the curve crossed `mid`, last before it went past `mid` by
/// `hysteresis`, between `start` and `length` seconds after it.
fn cycles(
    timestamps: &[u64],
    value: impl Fn(usize) -> f64,
    mid: f64,
    hysteresis: f64,
    start: u64,
    length: f64,
    linear: bool,
) -> Option<(f64, f64)> {
    let mut edges = Edges::default();
    let mut high: Option<bool> = None;
    let mut crossed = None;
    let mut previous: Option<(f64, f64)> = None;
    for (index, &timestamp) in timestamps.iter().enumerate() {
        let value = value(index);
        if !value.is_finite() {
            // a gap ends what came before
            (high, crossed, previous) = (None, None, None);
            edges.end_run();
            continue;
        }
        let t = seconds_since(start, timestamp);
        if let Some((previous_t, previous_value)) = previous
            && (previous_value < mid) != (value < mid)
        {
            crossed = Some(if linear {
                previous_t + (mid - previous_value) / (value - previous_value) * (t - previous_t)
            } else {
                t
            });
        }
        let now_high = if value >= mid + hysteresis {
            Some(true)
        } else if value <= mid - hysteresis {
            Some(false)
        } else {
            high
        };
        if let (Some(_), Some(rising), Some(at)) = (high, now_high, crossed)
            && high != now_high
            && (0.0..=length).contains(&at)
        {
            edges.add(at, rising);
        }
        high = now_high;
        previous = Some((t, value));
    }
    edges.cycles()
}

/// The edges of a signal, summed up as they come, in runs between gaps.
#[derive(Default)]
struct Edges {
    /// The run under way.
    run: Option<Run>,
    /// How many full cycles the runs before had.
    full: usize,
    /// How long they were.
    length: f64,
    /// How long they were high.
    high: f64,
}

/// Edges without a gap between them, which rise and fall in turn.
struct Run {
    count: usize,
    first: f64,
    last: f64,
    /// The time of the last edge that ended a full cycle.
    cycle_end: f64,
    /// How long the full cycles were high.
    high: f64,
    /// How long the cycle under way was high so far.
    high_so_far: f64,
}

impl Edges {
    fn add(&mut self, at: f64, rising: bool) {
        let Some(run) = &mut self.run else {
            self.run = Some(Run {
                count: 1,
                first: at,
                last: at,
                cycle_end: at,
                high: 0.0,
                high_so_far: 0.0,
            });
            return;
        };
        run.count += 1;
        if !rising {
            // high since the edge before
            run.high_so_far += at - run.last;
        }
        run.last = at;
        // every other edge ends a full cycle
        if run.count % 2 == 1 {
            run.high += run.high_so_far;
            run.high_so_far = 0.0;
            run.cycle_end = at;
        }
    }

    /// Ends the run under way, at a gap.
    fn end_run(&mut self) {
        if let Some(run) = self.run.take() {
            self.full += (run.count - 1) / 2;
            self.length += run.cycle_end - run.first;
            self.high += run.high;
        }
    }

    /// The average length of the full cycles and the part of them that was high.
    fn cycles(mut self) -> Option<(f64, f64)> {
        self.end_run();
        (self.full > 0 && self.length > 0.0)
            .then(|| (self.length / self.full as f64, self.high / self.length))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    /// The stats of `values` 1 ms apart over the window from `start` to `end` in ms.
    fn stats_of(values: &[f64], (start, end): (u64, u64), linear: bool) -> Option<Stats> {
        let timestamps: Vec<u64> = (0..values.len() as u64).map(|i| i * MS).collect();
        Stats::of(&timestamps, |i| values[i], (start * MS, end * MS), linear)
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
        let stats = Stats::of(&timestamps, |i| values[i], (MS / 2, 2 * MS), false).unwrap();
        assert_eq!((stats.min, stats.max), (4.0, 8.0));
        assert_near(stats.mean, 4.0);
        // a sample after the window counts for nothing
        let stats = Stats::of(&timestamps, |i| values[i], (0, MS + MS / 2), false).unwrap();
        assert_eq!(stats.max, 4.0);
    }

    #[test]
    fn a_window_without_time_between_samples_averages_their_values() {
        let stats = stats_of(&[2.0, 6.0], (1, 1), false).unwrap();
        assert_eq!((stats.min, stats.max, stats.mean), (6.0, 6.0, 6.0));
        let timestamps = [0, 0, 0];
        let values = [1.0, 2.0, 6.0];
        let stats = Stats::of(&timestamps, |i| values[i], (0, 0), false).unwrap();
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
}
