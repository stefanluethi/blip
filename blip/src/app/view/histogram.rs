//! A histogram: how the values of the checked traces spread over the shown window,
//! counted in bins shared by all traces and drawn over each other in their colours.

use super::Selectable;
use super::time_axis::within;
use super::{Formats, Plots, PropertiesContext, ViewContext, ViewId, Viewable, short_names};
use crate::app::model::Model;
use crate::app::model::numbers::{
    DisplayFormat, Number, NumberKind, SHOWN_DIGITS, format_significant,
};
use crate::app::model::traces::{Samples, Series, Trace};
use crate::app::theme;
use crate::app::time::TimeRange;
use std::collections::BTreeSet;

/// Automatic bins number about the square root of the samples, at least this many…
const MIN_AUTO_BINS: f64 = 8.0;
/// …and at most this many.
const MAX_AUTO_BINS: f64 = 64.0;
/// How far past a step between round widths the ideal width of automatic bins goes
/// before their width changes, as a part of it.
const WIDTH_HYSTERESIS: f64 = 0.2;
/// Automatic bins are at least this many points wide.
const MIN_BIN_POINTS: f64 = 4.0;
/// More bins than this are made wider, so that a tiny width does not stall blip.
const MAX_BINS: usize = 4096;
/// The room on the value axis on each side of the binned values, as a part of their
/// range, as egui_plot fits the y axis of a plot.
const VALUE_MARGIN: f64 = 0.05;
/// The value axis around a single value, in bin widths on each side.
const SINGLE_VALUE_ROOM: f64 = 2.5;
/// Where the logarithmic counts axis starts: at least this many decades below the share
/// of one sample, so a bin of one sample shows as a short bar.
const LOG_ROOM: f64 = 0.5;
/// The significant digits of the shares in the tooltip.
const SHARE_DIGITS: usize = 3;
/// The most of the counts axis the room beyond the bars takes, for the markers, the
/// counts outside the bins and the legend.
const MAX_ROOM: f64 = 2.0 / 3.0;
/// The room above the tallest bin, as a part of the counts axis up to it.
const HEADROOM: f64 = 0.08;
/// The counts axis shrinks only once the tallest bin in view is below this part of its
/// top, so that it does not jump while live.
const SHRINK_BELOW: f64 = 0.7;
/// The width of the outlines of the bars in points.
const OUTLINE_WIDTH: f32 = 1.5;
/// The height of a row of the marker strip, one per trace, in points.
const MARKER_ROW: f32 = 10.0;
/// The room between the edge of the plot and the marker strip, and between the strip
/// and the bars, in points.
const MARKER_INSET: f32 = 3.0;
const MARKER_GAP: f32 = 6.0;
/// The radius of the dot at the mean and the half length of the ends of a whisker.
const MARKER_RADIUS: f32 = 3.0;
/// The room between the edges of the plot and the texts and the legend on it.
const OVERLAY_INSET: f32 = 6.0;
/// The legend dots, as in the legends of plots.
const DOT_SIZE: f32 = 8.0;
/// The height of a row of the legend in its box, and of the unit below its rows.
const LEGEND_ROW: f32 = 18.0;
const UNIT_ROW: f32 = 14.0;
/// The room around the rows of the legend in its box.
const LEGEND_MARGIN: egui::Vec2 = egui::vec2(8.0, 3.0);
/// The room between a name and its dot in the legend, and between the traces of a
/// wrapped legend.
const LEGEND_DOT_GAP: f32 = 6.0;
const LEGEND_ITEM_GAP: f32 = 14.0;
/// The least width left for a name in the legend of the title row, else it ends in "…".
const MIN_NAME_WIDTH: f32 = 16.0;
/// How far apart in points the labels of the value axis of an upright histogram are at
/// least, and from where they show in full.
const VALUE_LABEL_SPACING: std::ops::RangeInclusive<f32> = 45.0..=60.0;
/// The same for the value axis of a sideways histogram that follows no plot.
const SIDEWAYS_LABEL_SPACING: std::ops::RangeInclusive<f32> = 30.0..=40.0;
/// The least width of the value axis of a sideways histogram, as of the y axis of a
/// plot, so that both line up.
const VALUE_AXIS_THICKNESS: f32 = 50.0;

/// The hint of a histogram without traces.
const CHECK_TRACES: &str = "Check the traces to show\nin the tree on the left";
/// The hint of a histogram without samples in the shown window.
const NO_SAMPLES: &str = "No samples in the shown window";

named_enum! {
    /// How the values are split into bins.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum BinMode {
        /// About the square root of the samples, of a round width.
        #[default]
        Auto => "Auto",
        /// A fixed number of bins over the values.
        Number => "Number",
        /// Bins of a fixed width.
        Width => "Width",
    }
}

named_enum! {
    /// The scale of the counts axis.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum CountScale {
        #[default]
        Linear => "Linear",
        Log => "Log",
    }
}

named_enum! {
    /// Which way the bars grow.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum Orientation {
        /// The values along the horizontal axis, the bars growing up.
        #[default]
        Upright => "Upright",
        /// The values along the vertical axis, the bars growing to the right, e.g.
        /// beside a plot of the same traces.
        Sideways => "Sideways",
    }
}

named_enum! {
    /// The side of the value axis of a sideways histogram.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum AxisSide {
        /// As the y axis of a plot.
        #[default]
        Left => "Left",
        Right => "Right",
    }
}

named_enum! {
    /// What is marked of the binned samples of each trace.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum Markers {
        #[default]
        Off => "Off",
        Mean => "Mean",
        /// The mean and a whisker over one standard deviation on each side.
        MeanSigma => "Mean ±σ",
    }
}

/// The checked traces counted in bins of their values over the shown window.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Histogram {
    /// Paths of the shown traces.
    pub traces: BTreeSet<String>,
    pub bins: BinMode,
    /// The number of bins with [`BinMode::Number`].
    pub bin_count: usize,
    /// The width of the bins with [`BinMode::Width`], in the unit of the traces.
    pub bin_width: f64,
    /// The lowest and the highest value binned, or `None` to bin all shown values.
    pub range: Option<(f64, f64)>,
    pub counts: CountScale,
    pub orientation: Orientation,
    pub axis_side: AxisSide,
    /// The plot linked to, whose traces the histogram shows and whose y range its value
    /// axis shows, in either orientation. The layout copies the traces, see
    /// `Layout::take_linked_traces`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<ViewId>,
    pub markers: Markers,
    /// The values panned or zoomed to instead of the fitted ones, until a double click.
    #[serde(skip)]
    value_view: Option<(f64, f64)>,
    /// The share in percent at the top of the bars, kept while the tallest bin shrinks
    /// a little, see [`fit_top`].
    #[serde(skip)]
    share_top: Option<f64>,
    /// What the last frame showed, for the properties, boxed as views are kept by
    /// value.
    #[serde(skip)]
    last: Option<Box<Last>>,
    /// The bin under the pointer in the last frame.
    #[serde(skip)]
    hovered: Option<usize>,
}

impl Default for Histogram {
    fn default() -> Self {
        Self {
            traces: BTreeSet::new(),
            bins: BinMode::default(),
            bin_count: 20,
            bin_width: 1.0,
            range: None,
            counts: CountScale::default(),
            orientation: Orientation::default(),
            axis_side: AxisSide::default(),
            link: None,
            markers: Markers::default(),
            value_view: None,
            share_top: None,
            last: None,
            hovered: None,
        }
    }
}

/// What a histogram showed in a frame.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Last {
    /// The lowest and the highest value of the samples.
    data: (f64, f64),
    /// The range of the value axis.
    values: (f64, f64),
    bins: Bins,
    /// The plot without its axes, and the part of it the bars can reach.
    frame: egui::Rect,
    bars: egui::Rect,
}

/// Bins of equal width, the first starting at `start`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bins {
    pub start: f64,
    pub width: f64,
    pub count: usize,
}

impl Bins {
    /// The lower edge of the `index`th bin, the upper edge of the last one at `count`.
    pub fn edge(&self, index: usize) -> f64 {
        self.start + index as f64 * self.width
    }

    fn center(&self, index: usize) -> f64 {
        self.start + (index as f64 + 0.5) * self.width
    }

    /// The bin of `value`, the first or the last one for values beyond them, so that a
    /// value at the end of a fixed range or just outside by rounding is in the last.
    fn index(&self, value: f64) -> usize {
        let index = ((value - self.start) / self.width).floor().max(0.0) as usize;
        index.min(self.count - 1)
    }

    /// The bin at `value` on the value axis, `None` beyond the bins.
    fn at(&self, value: f64) -> Option<usize> {
        (self.start..self.edge(self.count))
            .contains(&value)
            .then(|| self.index(value))
    }
}

/// The 1, 2 or 5 times a power of ten nearest to `value`.
fn nice_near(value: f64) -> f64 {
    let power = 10f64.powf(value.log10().floor());
    let mantissa = value / power;
    let step = match mantissa {
        m if m < 1.5 => 1.0,
        m if m < 3.5 => 2.0,
        m if m < 7.5 => 5.0,
        _ => 10.0,
    };
    step * power
}

/// The next 1, 2 or 5 times a power of ten after `value`, itself one.
fn nice_above(value: f64) -> f64 {
    let power = 10f64.powf(value.log10().floor());
    let mantissa = (value / power).round();
    let step = match mantissa {
        m if m < 2.0 => 2.0,
        m if m < 5.0 => 5.0,
        _ => 10.0,
    };
    step * power
}

/// The width of automatic bins for `samples` samples of values `span` apart: about the
/// square root of the samples, at least [`MIN_AUTO_BINS`] and at most
/// [`MAX_AUTO_BINS`], rounded to 1, 2 or 5 times a power of ten, at least `min_width`,
/// and at least 1 for `integers`. The `previous` width is kept while the ideal width
/// is within [`WIDTH_HYSTERESIS`] of rounding to it, so that it does not flip between
/// two widths while live.
fn auto_width(
    span: f64,
    samples: usize,
    min_width: f64,
    integers: bool,
    previous: Option<f64>,
) -> f64 {
    let target = (samples as f64).sqrt().clamp(MIN_AUTO_BINS, MAX_AUTO_BINS);
    let ideal = span / target;
    let least = if integers {
        min_width.max(1.0)
    } else {
        min_width
    };
    let enough = |width: f64| width >= least * (1.0 - 1.0e-9);
    let kept = previous.filter(|&width| {
        let round = (nice_near(width) - width).abs() <= width * 1.0e-9;
        let near = nice_near(ideal / (1.0 + WIDTH_HYSTERESIS)) <= width * (1.0 + 1.0e-9)
            && width <= nice_near(ideal * (1.0 + WIDTH_HYSTERESIS)) * (1.0 + 1.0e-9);
        round && near && enough(width)
    });
    if let Some(width) = kept {
        return width;
    }
    let mut width = nice_near(ideal);
    while !enough(width) && width.is_finite() {
        width = nice_above(width);
    }
    width
}

/// Bins of `width` from a multiple of it at or below `low`, as many as it takes to hold
/// `high`, each holding the values from its lower edge to before its upper one. With
/// `closed`, the last bin ends at or after `high` and holds it. With `centred`, for a
/// whole width over integers, the edges lie half a unit below the multiples, so that no
/// integer is on an edge and each bin holds `width` integers, e.g. 4.5 to 5.5 for 5.
fn cover((low, high): (f64, f64), width: f64, closed: bool, centred: bool) -> Bins {
    let offset = if centred { 0.5 } else { 0.0 };
    let start = ((low + offset) / width).floor() * width - offset;
    let bins = (high - start) / width;
    let count = if closed {
        (bins - 1.0e-9).ceil()
    } else {
        (bins + 1.0e-9).floor() + 1.0
    };
    Bins {
        start,
        width,
        count: (count as usize).max(1),
    }
}

/// The width of one bin centred on `value`, of a histogram whose values all are
/// `value`: 1 for integers, else 1, 2 or 5 times a power of ten near 1 % of it.
fn single_width(value: f64, integers: bool) -> f64 {
    if integers || value == 0.0 {
        1.0
    } else {
        nice_near(value.abs() * 0.01)
    }
}

impl Histogram {
    /// The bins for the values from `low` to `high`, of which there are about
    /// `samples` per trace, binned all if `fixed`, else as they are. Automatic bins are
    /// at least `min_width` wide, and those of `integers` at least 1 and, if of a whole
    /// width, with no integer on an edge.
    pub fn make_bins(
        &self,
        (low, high): (f64, f64),
        fixed: bool,
        samples: usize,
        min_width: f64,
        integers: bool,
    ) -> Bins {
        let width = (self.bin_width.is_finite() && self.bin_width > 0.0).then_some(self.bin_width);
        if high <= low {
            // one bar, centred on the value
            let width = match (self.bins, width) {
                (BinMode::Width, Some(width)) => width,
                _ => single_width(low, integers),
            };
            return Bins {
                start: low - width / 2.0,
                width,
                count: 1,
            };
        }
        let span = high - low;
        let least = span / MAX_BINS as f64;
        // bins of a whole width over integers have no integer on an edge
        let centred = |width: f64| integers && width >= 1.0 && width.fract() == 0.0;
        match self.bins {
            BinMode::Number => {
                let count = self.bin_count.clamp(1, MAX_BINS);
                Bins {
                    start: low,
                    width: span / count as f64,
                    count,
                }
            }
            BinMode::Width => {
                let width = width.unwrap_or(1.0);
                let width = if width < least {
                    nice_above(nice_near(least))
                } else {
                    width
                };
                cover((low, high), width, fixed, centred(width))
            }
            BinMode::Auto => {
                // the width of the last frame, to keep it while it is about right
                let previous = self.last.as_ref().map(|last| last.bins.width);
                let width = auto_width(span, samples, min_width.max(least), integers, previous);
                cover((low, high), width, fixed, centred(width))
            }
        }
    }

    /// Whether the histogram follows a plot of `plots`: while it is linked to one.
    pub fn following(&self, plots: &Plots) -> bool {
        self.link.and_then(|id| plots.get(id)).is_some()
    }

    /// The y range of the plot the histogram follows, if it shows one.
    fn followed(&self, plots: &Plots) -> Option<(f64, f64)> {
        if !self.following(plots) {
            return None;
        }
        plots
            .get(self.link?)?
            .y_range
            .filter(|(low, high)| low < high)
    }

    /// The shown paths with their names in the legend and their indices, which pick
    /// their colors.
    fn selected_path_iter(&self) -> impl Iterator<Item = (&str, &str, usize)> {
        let names = short_names(self.traces.iter().map(String::as_str));
        self.traces
            .iter()
            .zip(names)
            .enumerate()
            .map(|(index, (path, name))| (path.as_str(), name, index))
    }

    /// The shown traces that were received, with their samples in `window`, in
    /// nanoseconds, or all of them without one.
    fn shown_traces<'a>(&self, model: &'a Model, window: Option<(u64, u64)>) -> Vec<Shown<'a>> {
        self.selected_path_iter()
            .filter_map(|(path, name, index)| {
                let series = model.traces.series(path)?;
                let series = match window {
                    Some(window) => series.slice(within(series.timestamps, window)),
                    None => series,
                };
                Some(Shown {
                    path: path.to_owned(),
                    name: name.to_owned(),
                    index,
                    series,
                })
            })
            .collect()
    }

    /// Counts the samples of `shown` whose values span `data`, over the range `fixed`
    /// if it is set, else over all of them, in bins for a value axis `length` points
    /// long.
    fn count(
        &self,
        shown: &[Shown<'_>],
        data: (f64, f64),
        fixed: Option<(f64, f64)>,
        length: f32,
    ) -> Counted {
        let (low, high) = fixed.unwrap_or(data);
        let samples = shown.iter().map(|trace| trace.series.len()).max();
        // automatic bins are at least a few points wide on the fitted value axis
        let view_span = match fixed {
            Some(_) => high - low,
            None => (high - low) * (1.0 + 2.0 * VALUE_MARGIN),
        };
        let min_width = MIN_BIN_POINTS * view_span / f64::from(length.max(1.0));
        let integers = shown
            .iter()
            .all(|trace| trace.series.kind == NumberKind::Integer);
        let bins = self.make_bins(
            (low, high),
            fixed.is_some(),
            samples.unwrap_or(0),
            min_width,
            integers,
        );
        let traces = shown
            .iter()
            .map(|trace| count(values(&trace.series), &bins, fixed))
            .collect();
        Counted {
            bins,
            traces,
            single: fixed.is_none() && data.1 <= data.0,
        }
    }

    /// Counts the samples of the shown traces of `model` in the shown window of `time`,
    /// following a plot of `plots` or over the fixed range if it is set, in bins for a
    /// value axis `length` points long.
    fn counting<'a>(
        &self,
        model: &'a Model,
        time: &TimeRange,
        plots: &Plots,
        length: f32,
    ) -> Counting<'a> {
        let window = time
            .range()
            .map(|(start, end)| (time.timestamp(start), time.timestamp(end)));
        let shown = self.shown_traces(model, window);
        let following = self.following(plots);
        let followed = self.followed(plots);
        // the own range is off while following, also before the plot shows a range
        let fixed = match following {
            true => followed,
            false => self.range.filter(|(low, high)| low < high),
        };
        let data = data_range(&shown).filter(|_| window.is_some());
        let counted = data.map(|data| self.count(&shown, data, fixed, length));
        // without bins, the infinite samples are still counted at their ends
        let outside = match &counted {
            Some(counted) => counted.outside(),
            None => (shown.iter())
                .map(|trace| infinite(values(&trace.series)))
                .fold((0, 0), |(below, above), (b, a)| (below + b, above + a)),
        };
        Counting {
            shown,
            data,
            following,
            followed,
            fixed,
            counted,
            outside,
        }
    }

    /// Whether the plot can be panned and zoomed along the axes, x and y.
    fn movable(&self, following: bool) -> [bool; 2] {
        match self.orientation {
            Orientation::Upright => [!following, false],
            Orientation::Sideways => [false, !following],
        }
    }

    /// The point at `value` and `count` on the axes of the plot.
    fn point(&self, value: f64, count: f64) -> [f64; 2] {
        match self.orientation {
            Orientation::Upright => [value, count],
            Orientation::Sideways => [count, value],
        }
    }
}

/// What a histogram counts in a frame, see [`Histogram::counting`].
struct Counting<'a> {
    shown: Vec<Shown<'a>>,
    /// The lowest and the highest value of the shown samples, `None` without one.
    data: Option<(f64, f64)>,
    /// Whether a plot is followed, see [`Histogram::following`].
    following: bool,
    /// The y range of the plot followed.
    followed: Option<(f64, f64)>,
    /// The range binned, of the plot followed or fixed in the properties.
    fixed: Option<(f64, f64)>,
    counted: Option<Counted>,
    /// The samples below and above the bins, or the infinite ones without bins.
    outside: (u64, u64),
}

/// A checked trace with its samples in the shown window.
struct Shown<'a> {
    path: String,
    name: String,
    /// Picks its colour.
    index: usize,
    series: Series<'a>,
}

/// The values of the samples of `series`, NaN for nulls.
fn values<'a>(series: &'a Series<'a>) -> impl Iterator<Item = f64> + 'a {
    (0..series.len()).map(|index| series.value(index))
}

/// The lowest and the highest value of the samples of `shown`, `None` without values.
fn data_range(shown: &[Shown<'_>]) -> Option<(f64, f64)> {
    shown
        .iter()
        .map(|trace| trace.series.extent(0..trace.series.len()))
        .filter(|extent| extent.has_values())
        .map(|extent| (extent.min, extent.max))
        .reduce(|(low_a, high_a), (low_b, high_b)| (low_a.min(low_b), high_a.max(high_b)))
}

/// The samples of `values` at minus and plus infinity.
fn infinite(values: impl Iterator<Item = f64>) -> (u64, u64) {
    values
        .filter(|value| value.is_infinite())
        .fold((0, 0), |(below, above), value| match value > 0.0 {
            true => (below, above + 1),
            false => (below + 1, above),
        })
}

/// The samples of a trace counted in bins.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Counts {
    /// The samples in each bin.
    pub bins: Vec<u64>,
    /// The samples below and above a fixed range, which are not binned.
    pub below: u64,
    pub above: u64,
    /// The mean and the population standard deviation of the binned samples, each
    /// counted once, `None` without one.
    pub mean: Option<f64>,
    pub std_dev: Option<f64>,
}

impl Counts {
    /// All samples counted, binned or below or above the bins.
    pub fn total(&self) -> u64 {
        self.bins.iter().sum::<u64>() + self.below + self.above
    }

    /// The share of `count` samples in percent of all samples counted.
    pub fn share(&self, count: u64) -> f64 {
        match self.total() {
            0 => 0.0,
            total => 100.0 * count as f64 / total as f64,
        }
    }

    /// The share of each bin in percent.
    pub fn shares(&self) -> Vec<f64> {
        self.bins.iter().map(|&count| self.share(count)).collect()
    }
}

/// Counts `values` in `bins`, without NaN, which marks a null. Values outside `fixed`
/// and infinite ones are counted below or above instead.
pub fn count(values: impl Iterator<Item = f64>, bins: &Bins, fixed: Option<(f64, f64)>) -> Counts {
    let mut counts = Counts {
        bins: vec![0; bins.count],
        ..Counts::default()
    };
    // the sums of the values minus the first one, which keeps the sum of squares
    // precise for values far from zero
    let mut shift = None;
    let (mut binned, mut sum, mut squares) = (0u64, 0.0, 0.0);
    for value in values {
        if value.is_nan() {
            continue;
        }
        if value.is_infinite() {
            match value > 0.0 {
                true => counts.above += 1,
                false => counts.below += 1,
            }
            continue;
        }
        if let Some((low, high)) = fixed {
            if value < low {
                counts.below += 1;
                continue;
            }
            if value > high {
                counts.above += 1;
                continue;
            }
        }
        counts.bins[bins.index(value)] += 1;
        let shifted = value - *shift.get_or_insert(value);
        binned += 1;
        sum += shifted;
        squares += shifted * shifted;
    }
    if let Some(shift) = shift {
        let n = binned as f64;
        let mean = sum / n;
        counts.mean = Some(shift + mean);
        counts.std_dev = Some((squares / n - mean * mean).max(0.0).sqrt());
    }
    counts
}

/// What a histogram counted in a frame.
struct Counted {
    bins: Bins,
    /// The counts of each shown trace.
    traces: Vec<Counts>,
    /// Whether all values are the same, in one bin centred on them.
    single: bool,
}

impl Counted {
    /// The samples of all traces below and above the bins.
    fn outside(&self) -> (u64, u64) {
        let below = self.traces.iter().map(|counts| counts.below).sum();
        let above = self.traces.iter().map(|counts| counts.above).sum();
        (below, above)
    }

    /// The share in percent of one sample of the trace with the most samples, the
    /// smallest share a bin that is not empty can have.
    fn one_sample(&self) -> f64 {
        let most = self.traces.iter().map(Counts::total).max().unwrap_or(0);
        100.0 / most.max(1) as f64
    }

    /// The largest share in percent of a trace in one bin within `values` on the value
    /// axis, at least that of one sample.
    fn tallest_in(&self, (low, high): (f64, f64)) -> f64 {
        let bins = &self.bins;
        let in_view =
            (0..bins.count).filter(|&index| bins.edge(index) < high && bins.edge(index + 1) > low);
        in_view
            .flat_map(|index| {
                (self.traces.iter()).map(move |counts| counts.share(counts.bins[index]))
            })
            .fold(self.one_sample(), f64::max)
    }
}

/// The share at the top of the bars after `previous` with `tallest` the share of the
/// tallest bin in view: it grows at once and shrinks only once the tallest bin is below
/// [`SHRINK_BELOW`] of it.
pub fn fit_top(previous: Option<f64>, tallest: f64) -> f64 {
    match previous {
        Some(top) if tallest <= top && tallest >= top * SHRINK_BELOW => top,
        _ => tallest,
    }
}

/// The counts axis, which shows the share of each trace's samples in a bin in percent:
/// where the bars start and where the tallest reaches, and the top of the axis beyond
/// them, with the room for the marker strip.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CountAxis {
    pub scale: CountScale,
    /// Where the bars start.
    pub floor: f64,
    /// Above the tallest bar, which reaches `top`.
    pub bars_top: f64,
    /// The end of the axis.
    pub axis_top: f64,
}

impl CountAxis {
    /// The counts axis `length` points long of bars up to `top` percent, with `strip`
    /// points left at its end for the markers, on which one sample is `one` percent.
    /// The logarithmic axis starts at least [`LOG_ROOM`] below one sample, rounded down
    /// to half a decade, so that it moves seldom while live.
    pub fn new(scale: CountScale, top: f64, one: f64, length: f32, strip: f32) -> Self {
        let floor = match scale {
            CountScale::Linear => 0.0,
            CountScale::Log => ((one.log10() - LOG_ROOM) * 2.0).floor() / 2.0,
        };
        let top = match scale {
            CountScale::Linear => top,
            CountScale::Log => top.log10(),
        };
        let bars_top = top + HEADROOM * (top - floor);
        let length = f64::from(length.max(1.0));
        let bars_length = (length - f64::from(strip)).max(length * (1.0 - MAX_ROOM));
        Self {
            scale,
            floor,
            bars_top,
            axis_top: floor + (bars_top - floor) * length / bars_length,
        }
    }

    /// Where a bin of a `share` of the samples in percent reaches on the axis, `None`
    /// for an empty bin, which shows nothing.
    pub fn position(&self, share: f64) -> Option<f64> {
        (share > 0.0).then(|| match self.scale {
            CountScale::Linear => share,
            CountScale::Log => share.log10(),
        })
    }

    /// The marks of the axis, `horizontal` or not: a round step of percent apart, or a
    /// mark at each power of ten above the floor, up to the bars, without those whose
    /// labels would run past the end of the axis.
    fn marks(self, horizontal: bool) -> impl Fn(egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
        move |input| {
            let marks = match self.scale {
                CountScale::Linear => {
                    // labels need more room than the least spacing of grid lines
                    let step = nice_at_least(input.base_step_size * 6.0);
                    (0..=(self.bars_top / step).floor() as i64)
                        .map(|multiple| egui_plot::GridMark {
                            value: multiple as f64 * step,
                            step_size: step,
                        })
                        .collect()
                }
                CountScale::Log => (self.floor.floor() as i64 + 1..=self.bars_top.floor() as i64)
                    .map(|decade| egui_plot::GridMark {
                        value: decade as f64,
                        step_size: 1.0,
                    })
                    .collect::<Vec<_>>(),
            };
            let fits = |mark: &egui_plot::GridMark| {
                let text = self.label(mark.value, mark.step_size);
                text.is_empty() || label_fits(mark.value, &text, &input, horizontal)
            };
            marks.into_iter().filter(fits).collect()
        }
    }

    /// The text of a mark of the axis at `value`, multiples of `step` apart, none at the
    /// floor, where the end label of the value axis is.
    fn label(self, value: f64, step: f64) -> String {
        match self.scale {
            CountScale::Linear if value <= self.floor => String::new(),
            CountScale::Linear => {
                format!("{} %", write_value(value, step, DisplayFormat::Default))
            }
            CountScale::Log => decade_label(value.round() as i32),
        }
    }
}

/// The least room between grid lines in points, egui_plot's default, by which it
/// gives the smallest step in a [`egui_plot::GridInput`].
const GRID_SPACING: f64 = 8.0;
/// About the width of a character of the labels of the axes, and their height, in
/// points, to keep them within the ends of the axes.
const LABEL_CHAR_WIDTH: f64 = 7.5;
const LABEL_HEIGHT: f64 = 16.0;

/// Whether the label `text` of a mark at `value` of an axis with `input` stays within the
/// ends of the axis: half its width along a horizontal axis, half its height along a
/// vertical one.
fn label_fits(value: f64, text: &str, input: &egui_plot::GridInput, horizontal: bool) -> bool {
    let per_point = input.base_step_size / GRID_SPACING;
    let half = if horizontal {
        text.chars().count() as f64 * LABEL_CHAR_WIDTH / 2.0
    } else {
        LABEL_HEIGHT / 2.0
    } * per_point;
    let (low, high) = input.bounds;
    value - half >= low && value + half <= high
}

/// Marks a value axis whose values are written in `format`: the labels at the smallest
/// round step, 1, 2 or 5 times a power of ten, at least `spacing` points apart, the
/// powers of ten above it stronger, and grid lines between them at a step that divides
/// it, so that steps of 2 and 5 never show together. A label that would run past an
/// end of the axis, horizontal or not, is left out.
fn round_marks(
    format: DisplayFormat,
    spacing: f32,
    horizontal: bool,
) -> impl Fn(egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
    move |input| {
        let (low, high) = input.bounds;
        let per_point = input.base_step_size / GRID_SPACING;
        if !(per_point.is_finite() && per_point > 0.0 && high > low) {
            return vec![];
        }
        let label = nice_at_least(per_point * f64::from(spacing));
        let mantissa = (label / 10f64.powf(label.log10().floor())).round();
        // the grid lines between the labels, at a step that divides theirs
        let minor = [
            if mantissa == 2.0 {
                label / 4.0
            } else {
                label / 5.0
            },
            if mantissa == 5.0 { label } else { label / 2.0 },
            label,
        ]
        .into_iter()
        .find(|&step| step >= input.base_step_size * (1.0 - 1.0e-9))
        .unwrap_or(label);
        let (first, last) = ((low / minor).ceil(), (high / minor).floor());
        if !(last - first).is_finite() || last - first >= 10_000.0 {
            return vec![];
        }
        let multiple = |value: f64, step: f64| {
            let ratio = value / step;
            (ratio - ratio.round()).abs() < 1.0e-6
        };
        (first as i64..=last as i64)
            .map(|index| {
                let value = index as f64 * minor;
                let mut step_size = minor;
                if multiple(value, label) {
                    step_size = label;
                    // a power of ten above the labels is stronger
                    let mut power = 10f64.powf(label.log10().floor() + 1.0);
                    while power <= label * 1000.0 && multiple(value, power) {
                        step_size = power;
                        power *= 10.0;
                    }
                    let text = write_value(value, step_size, format);
                    if !label_fits(value, &text, &input, horizontal) {
                        step_size = minor;
                    }
                }
                egui_plot::GridMark { value, step_size }
            })
            .collect()
    }
}

/// The smallest 1, 2 or 5 times a power of ten at least `value`.
fn nice_at_least(value: f64) -> f64 {
    let near = nice_near(value);
    if near < value * (1.0 - 1.0e-9) {
        nice_above(near)
    } else {
        near
    }
}

/// The share in percent of the power of ten `decade`, e.g. "0.01 %", "1 %", "100 %".
pub fn decade_label(decade: i32) -> String {
    format!("{} %", format_significant(10f64.powi(decade), 1))
}

/// A share in percent, as in the tooltip, e.g. "12.5 %".
pub fn write_share(share: f64) -> String {
    format!("{} %", format_significant(share, SHARE_DIGITS))
}

/// The decimals that write the multiples of `step` exactly, of a few beyond those of its
/// magnitude.
fn decimals(step: f64) -> usize {
    let magnitude = (-step.abs().log10().floor()).max(0.0) as usize;
    (magnitude..=magnitude + 2)
        .find(|&decimals| {
            let scaled = step * 10f64.powi(decimals as i32);
            (scaled - scaled.round()).abs() < 1.0e-6 * scaled.abs().max(1.0)
        })
        .unwrap_or(magnitude + 2)
}

/// `value` on the value axis or at the edge of a bin, multiples of `step` apart, in
/// `format`.
pub fn write_value(value: f64, step: f64, format: DisplayFormat) -> String {
    match format {
        DisplayFormat::Default => Number::Float(value).format_fixed(decimals(step)),
        format => Number::Float(value).format(Some(SHOWN_DIGITS), format),
    }
}

/// The format of the traces at `paths` if all have the same, else the default one.
fn common_format<'a>(formats: &Formats, paths: impl IntoIterator<Item = &'a str>) -> DisplayFormat {
    let mut formats = paths.into_iter().map(|path| formats.of(path));
    let first = formats.next().unwrap_or_default();
    if formats.all(|format| format == first) {
        first
    } else {
        DisplayFormat::Default
    }
}

/// Shows a dot in `color` as in the legends.
fn dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (dot, _) = ui.allocate_exact_size(egui::Vec2::splat(DOT_SIZE), egui::Sense::hover());
    ui.painter()
        .circle_filled(dot.center(), DOT_SIZE / 2.0, color);
}

impl super::ViewInfo for Histogram {
    const NAME: &'static str = "Histogram";
    const ICON: &'static str = egui_phosphor::regular::CHART_BAR;

    fn selectable(trace: Trace<'_>) -> Selectable {
        const NO_BOOLS: &str = "Histograms count numbers, not bools";
        match trace.samples {
            Samples::Strings(_) => Selectable::No("Histograms count numbers".into()),
            Samples::Numbers(numbers) if numbers.kind() == Some(NumberKind::Bool) => {
                Selectable::No(NO_BOOLS.into())
            }
            Samples::Arrays { elements, .. } if elements.kind() == Some(NumberKind::Bool) => {
                Selectable::No(NO_BOOLS.into())
            }
            Samples::Arrays { shape, .. } => super::elements(shape),
            _ => Selectable::Trace,
        }
    }
}

impl Viewable for Histogram {
    fn title_ui(&self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let unit = super::common_unit(context.model, self.traces.iter().map(String::as_str));
        let weak = ui.visuals().weak_text_color();
        // the unit keeps its place at the end, the names are cut off with "…" before it
        let unit_width = unit.map_or(0.0, |unit| {
            text_size(ui, unit, egui::TextStyle::Body).x + 4.0 + ui.spacing().item_spacing.x
        });
        ui.add_space(8.0);
        let names = egui::vec2(
            (ui.available_width() - unit_width).max(0.0),
            ui.spacing().interact_size.y,
        );
        let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(names, left_to_right, |ui| {
            for (path, name, index) in self.selected_path_iter() {
                if ui.available_width() < 6.0 + DOT_SIZE + MIN_NAME_WIDTH {
                    // a name cut off ends in "…" already
                    let more = egui::RichText::new("…").color(weak);
                    if ui.available_width() >= text_size(ui, "…", egui::TextStyle::Body).x {
                        ui.label(more);
                    }
                    break;
                }
                ui.add_space(6.0);
                dot(ui, theme::trace_color(ui.visuals(), index));
                let label = egui::Label::new(egui::RichText::new(name).color(weak)).truncate();
                ui.add(label).on_hover_text(path);
            }
        });
        if let Some(unit) = unit {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(unit).color(weak));
        }
    }

    fn colored_paths(&self, _model: &Model) -> Vec<String> {
        self.traces.iter().cloned().collect()
    }

    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.traces, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.traces.is_empty() {
            let faint = theme::tokens(ui.visuals()).text_faint;
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new(CHECK_TRACES).color(faint));
            });
            return;
        }
        self.show(ui, context);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let unit = super::common_unit(context.model, self.traces.iter().map(String::as_str));
        let following = self.following(context.plots);
        let fitted = self.last.as_ref().map_or((0.0, 1.0), |last| last.data);
        let last_bins = self.last.as_ref().map(|last| last.bins);
        super::properties_section(ui, "Link", |ui| self.link_row(ui, context.plots));
        super::properties_section(ui, "Bins", |ui| {
            let mode = self.bins;
            super::choice_row(ui, "Mode", &mut self.bins, &BinMode::CHOICES);
            // a fixed number or width starts from the bins shown
            if let Some(bins) = last_bins.filter(|_| self.bins != mode) {
                match self.bins {
                    BinMode::Number => self.bin_count = bins.count,
                    BinMode::Width => self.bin_width = bins.width,
                    BinMode::Auto => {}
                }
            }
            match self.bins {
                BinMode::Number => {
                    super::property_label(ui, "Number");
                    ui.add(egui::DragValue::new(&mut self.bin_count).range(1..=MAX_BINS));
                    ui.end_row();
                }
                BinMode::Width => {
                    super::property_label(ui, "Width");
                    ui.horizontal(|ui| {
                        let speed = (self.bin_width.abs() * 0.01).max(1.0e-9);
                        ui.add(
                            egui::DragValue::new(&mut self.bin_width)
                                .range(f64::MIN_POSITIVE..=f64::MAX)
                                .speed(speed),
                        );
                        if let Some(unit) = unit {
                            ui.weak(unit);
                        }
                    });
                    ui.end_row();
                }
                BinMode::Auto => {}
            }
            if following {
                self.followed_range_rows(ui);
            } else {
                super::range_rows(ui, &mut self.range, fitted);
            }
        });
        super::properties_section(ui, "Counts", |ui| {
            super::choice_row(ui, "Scale", &mut self.counts, &CountScale::CHOICES);
        });
        super::properties_section(ui, "Bars", |ui| {
            super::choice_row(
                ui,
                "Orientation",
                &mut self.orientation,
                &Orientation::CHOICES,
            );
            if self.orientation == Orientation::Sideways {
                super::choice_row(ui, "Axis side", &mut self.axis_side, &AxisSide::CHOICES);
            }
            super::choice_row(ui, "Markers", &mut self.markers, &Markers::CHOICES);
        });
    }
}

/// The size of `text` in `style` on one line.
fn text_size(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> egui::Vec2 {
    let font = style.resolve(ui.style());
    (ui.painter())
        .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
}

/// The names of `plots` in the choice of the plot to link to, numbered where several
/// have the same name.
fn plot_names(plots: &Plots) -> Vec<String> {
    let names: Vec<&str> = plots.0.iter().map(|plot| plot.name.as_str()).collect();
    names
        .iter()
        .enumerate()
        .map(|(index, &name)| {
            let same = names.iter().filter(|&&other| other == name).count();
            if same > 1 {
                let number = names[..index]
                    .iter()
                    .filter(|&&other| other == name)
                    .count()
                    + 1;
                format!("{name} ({number})")
            } else {
                name.to_owned()
            }
        })
        .collect()
}

impl Histogram {
    /// Adds the rows of the range to the grid of a properties section, greyed, as the
    /// plot followed sets it.
    fn followed_range_rows(&mut self, ui: &mut egui::Ui) {
        const SET_BY_PLOT: &str = "Set by the linked plot";
        let mut fixed = self.range.is_some();
        super::property_label(ui, "Range");
        ui.add_enabled_ui(false, |ui| {
            theme::segmented(ui, &mut fixed, &[(false, "Auto"), (true, "Fixed")]);
        })
        .response
        .on_disabled_hover_text(SET_BY_PLOT);
        ui.end_row();
        if let Some((min, max)) = &mut self.range {
            for (label, value) in [("Min", min), ("Max", max)] {
                super::property_label(ui, label);
                ui.add_enabled(false, egui::DragValue::new(value))
                    .on_disabled_hover_text(SET_BY_PLOT);
                ui.end_row();
            }
        }
    }

    /// Adds a row to the grid of a properties section to choose the plot of `plots` to
    /// link to.
    fn link_row(&mut self, ui: &mut egui::Ui, plots: &Plots) {
        super::property_label(ui, "Plot");
        let names = plot_names(plots);
        let current = (plots.0.iter().zip(&names))
            .find(|(plot, _)| Some(plot.id) == self.link)
            .map_or("None", |(_, name)| name.as_str());
        let before = self.link;
        egui::ComboBox::from_id_salt("histogram link")
            .selected_text(current)
            .width(120.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.link, None, "None");
                for (plot, name) in plots.0.iter().zip(&names) {
                    ui.selectable_value(&mut self.link, Some(plot.id), name);
                }
            })
            .response
            .on_hover_text("The plot whose traces and y range the histogram shows");
        if self.link != before {
            // the plot's range replaces the one panned or zoomed to
            self.value_view = None;
        }
        ui.end_row();
    }

    /// The room beyond the bars on a plot of `frame` points for `rows` rows of
    /// markers, the counts of the samples `outside` the bins and, without a title row,
    /// the legend with `unit`. A legend too tall for the room it may take upright is
    /// wrapped across the plot.
    fn room(
        &self,
        ui: &egui::Ui,
        frame: egui::Vec2,
        rows: usize,
        (below, above): (u64, u64),
        legend: Option<Option<&str>>,
    ) -> Room {
        let markers = if rows > 0 {
            MARKER_INSET + rows as f32 * MARKER_ROW
        } else {
            0.0
        };
        let size = |text: &str| text_size(ui, text, egui::TextStyle::Body);
        let outside = (below + above > 0).then(|| {
            let [below, above] = self.outside_texts(below, above);
            let width = size(&below).x.max(size(&above).x);
            egui::vec2(width, LEGEND_ROW)
        });
        let legend = legend.map(|unit| {
            let items: Vec<f32> = (self.selected_path_iter())
                .map(|(_, name, _)| size(name).x + LEGEND_DOT_GAP + DOT_SIZE)
                .collect();
            let unit = unit.map_or(0.0, |unit| text_size(ui, unit, egui::TextStyle::Small).x);
            let unit_row = if unit > 0.0 { UNIT_ROW } else { 0.0 };
            let widest = items.iter().copied().fold(unit, f32::max);
            let column = egui::vec2(widest, items.len() as f32 * LEGEND_ROW + unit_row);
            let column = column + 2.0 * LEGEND_MARGIN;
            // upright, the room beyond the bars takes only a part of the counts axis
            let others = markers + OVERLAY_INSET + outside.map_or(0.0, |size| size.y);
            let fits = others + column.y + MARKER_GAP <= frame.y * MAX_ROOM as f32;
            if self.orientation == Orientation::Sideways || fits {
                return Legend {
                    size: column,
                    lines: None,
                };
            }
            let band = frame.x - 2.0 * (OVERLAY_INSET + LEGEND_MARGIN.x);
            let lines = wrap(&items, band, LEGEND_ITEM_GAP);
            let line_width = |line: &Vec<usize>| {
                let gaps = line.len().saturating_sub(1) as f32 * LEGEND_ITEM_GAP;
                line.iter().map(|&index| items[index]).sum::<f32>() + gaps
            };
            let width = lines.iter().map(line_width).fold(unit, f32::max);
            let height = lines.len() as f32 * LEGEND_ROW + unit_row;
            Legend {
                size: egui::vec2(width, height) + 2.0 * LEGEND_MARGIN,
                lines: Some(lines),
            }
        });
        Room {
            orientation: self.orientation,
            markers,
            outside,
            above: above > 0,
            legend,
        }
    }

    /// The texts of the counts of the samples below and above the bins, pointing to the
    /// ends they lie beyond.
    fn outside_texts(&self, below: u64, above: u64) -> [String; 2] {
        match self.orientation {
            Orientation::Upright => [format!("‹ {below}"), format!("{above} ›")],
            Orientation::Sideways => [format!("↓ {below}"), format!("↑ {above}")],
        }
    }

    /// Shows the histogram of the checked traces over the shown window.
    fn show(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let (model, formats) = (context.model, context.formats);
        let id = ui.id().with("histogram");
        let frame_id = id.with("frame");
        // the size of the plot without its axes in the last frame
        let frame = ui
            .data(|data| data.get_temp::<egui::Vec2>(frame_id))
            .unwrap_or_else(|| ui.available_size() - egui::vec2(VALUE_AXIS_THICKNESS, 24.0))
            .max(egui::Vec2::splat(1.0));
        let (value_length, count_length) = match self.orientation {
            Orientation::Upright => (frame.x, frame.y),
            Orientation::Sideways => (frame.y, frame.x),
        };

        let Counting {
            shown,
            data,
            following,
            followed,
            fixed,
            counted,
            outside,
        } = self.counting(model, context.time, context.plots, value_length);
        let fitted = fitted_values(counted.as_ref(), data, fixed);
        let values = match following {
            true => followed.unwrap_or(fitted),
            false => self.value_view.unwrap_or(fitted),
        };

        let tallest = counted.as_ref().map(|counted| counted.tallest_in(values));
        self.share_top = tallest.map(|tallest| fit_top(self.share_top, tallest));
        let one = counted.as_ref().map_or(100.0, Counted::one_sample);
        let rows = match self.markers {
            Markers::Off => 0,
            _ => shown.len(),
        };
        let paths = || self.traces.iter().map(String::as_str);
        let unit = super::common_unit(model, paths());
        let format = common_format(formats, paths());
        let legend = (!context.titled).then_some(unit);
        let room = self.room(ui, frame, rows, outside, legend);
        let axis = CountAxis::new(
            self.counts,
            self.share_top.unwrap_or(100.0),
            one,
            count_length,
            room.length(),
        );

        let plot = self.plot(id, ui.visuals(), axis, (unit, format), following);
        let visuals = ui.visuals().clone();
        let this = &*self;
        let response = plot.show(ui, |plot_ui| {
            let (low, high) = values;
            let counts = axis.floor..=axis.axis_top;
            match this.orientation {
                Orientation::Upright => {
                    plot_ui.set_plot_bounds_x(low..=high);
                    plot_ui.set_plot_bounds_y(counts);
                }
                Orientation::Sideways => {
                    plot_ui.set_plot_bounds_x(counts);
                    plot_ui.set_plot_bounds_y(low..=high);
                }
            }
            let counted = counted.as_ref()?;
            let rect = plot_ui.response().rect;
            let hovered = plot_ui.response().hover_pos().and_then(|pos| {
                let part = match this.orientation {
                    Orientation::Upright => (pos.x - rect.left()) / rect.width(),
                    Orientation::Sideways => (rect.bottom() - pos.y) / rect.height(),
                };
                counted.bins.at(low + f64::from(part) * (high - low))
            });
            this.plot_bars(plot_ui, &visuals, (&shown, counted), axis, hovered);
            hovered
        });

        let transform = response.transform;
        let shown_frame = transform.frame().size();
        if shown_frame != frame {
            ui.data_mut(|data| data.insert_temp(frame_id, shown_frame));
            ui.ctx().request_repaint();
        }
        if !following {
            super::zoom_on_wheel(&response.response);
            self.keep_value_view(&response, values);
        }
        self.hovered = response.inner;
        let frame = *transform.frame();
        // where the tallest bar can reach, short of the room beyond the bars
        let bars = match self.orientation {
            Orientation::Upright => {
                frame.with_min_y(transform.position_from_point_y(axis.bars_top))
            }
            Orientation::Sideways => {
                frame.with_max_x(transform.position_from_point_x(axis.bars_top))
            }
        };
        self.last = counted.as_ref().map(|counted| {
            Box::new(Last {
                data: data.unwrap_or(fitted),
                values,
                bins: counted.bins,
                frame,
                bars,
            })
        });

        let overlay = Overlay {
            painter: ui.painter().with_clip_rect(frame),
            transform,
            room,
        };
        self.paint_outside(ui, &overlay, outside);
        match &counted {
            Some(counted) => self.paint_markers(ui, &overlay, &shown, counted),
            // the counts of the infinite samples say there are some
            None if outside != (0, 0) => {}
            None => {
                let tokens = theme::tokens(ui.visuals());
                let size = text_size(ui, NO_SAMPLES, egui::TextStyle::Body);
                // on the card, so the grid does not cross it
                let rect = egui::Rect::from_center_size(frame.center(), size)
                    .expand2(egui::vec2(8.0, 4.0));
                (overlay.painter).rect_filled(rect, theme::CONTROL_RADIUS, tokens.card);
                let hint =
                    egui::Label::new(egui::RichText::new(NO_SAMPLES).color(tokens.text_faint));
                ui.put(rect, hint);
            }
        }
        if let (Some(rect), Some(legend)) = (overlay.legend_rect(), &overlay.room.legend) {
            self.legend_box_ui(ui, (rect, legend), frame, unit);
        }
        if let (Some(index), Some(counted)) = (self.hovered, &counted) {
            let bin = Bin {
                index,
                counted,
                unit,
                format,
            };
            response
                .response
                .on_hover_ui_at_pointer(|ui| bin.tooltip_ui(ui, &shown));
        }
    }

    /// The plot with the value and the counts `axis` along the axes of the orientation,
    /// the values written with `unit` and in `format`, which can be panned and zoomed
    /// along the values unless `following` a plot, whose grid it then has.
    fn plot(
        &self,
        id: egui::Id,
        visuals: &egui::Visuals,
        axis: CountAxis,
        (unit, format): (Option<&str>, DisplayFormat),
        following: bool,
    ) -> egui_plot::Plot<'static> {
        let movable = self.movable(following);
        let value_formatter =
            move |mark: egui_plot::GridMark, _: &std::ops::RangeInclusive<f64>| {
                write_value(mark.value, mark.step_size, format)
            };
        let count_formatter =
            move |mark: egui_plot::GridMark, _: &std::ops::RangeInclusive<f64>| {
                axis.label(mark.value, mark.step_size)
            };
        let plot = egui_plot::Plot::new(id)
            .grid_color(theme::grid_color(visuals))
            // the shares and counts of the bin under the pointer are shown in a tooltip
            .show_x(false)
            .show_y(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .allow_drag(movable)
            .allow_zoom(movable)
            .allow_scroll(movable);
        let label = |hints: egui_plot::AxisHints<'static>| match unit {
            Some(unit) => hints.label(unit.to_owned()),
            None => hints,
        };
        match self.orientation {
            Orientation::Upright => plot
                .custom_x_axes(vec![label(
                    egui_plot::AxisHints::new_x()
                        .formatter(value_formatter)
                        .label_spacing(VALUE_LABEL_SPACING),
                )])
                .x_grid_spacer(round_marks(format, *VALUE_LABEL_SPACING.end(), true))
                .custom_y_axes(vec![
                    egui_plot::AxisHints::new_y().formatter(count_formatter),
                ])
                .y_grid_spacer(axis.marks(false)),
            Orientation::Sideways => {
                let placement = match self.axis_side {
                    AxisSide::Left => egui_plot::HPlacement::Left,
                    AxisSide::Right => egui_plot::HPlacement::Right,
                };
                let mut value_axis = egui_plot::AxisHints::new_y()
                    .placement(placement)
                    .min_thickness(VALUE_AXIS_THICKNESS)
                    .formatter(value_formatter);
                if !following {
                    value_axis = value_axis.label_spacing(SIDEWAYS_LABEL_SPACING);
                }
                let plot = plot
                    .custom_x_axes(vec![
                        egui_plot::AxisHints::new_x()
                            .formatter(count_formatter)
                            // shares are short, so their labels fit closer together
                            .label_spacing(40.0..=60.0),
                    ])
                    .x_grid_spacer(axis.marks(true));
                // following a plot, the values have the grid of the plot
                if following {
                    plot.custom_y_axes(vec![label(value_axis)])
                } else {
                    let spacing = *SIDEWAYS_LABEL_SPACING.end();
                    plot.custom_y_axes(vec![label(value_axis)])
                        .y_grid_spacer(round_marks(format, spacing, false))
                }
            }
        }
    }

    /// Draws the column of the `hovered` bin, then the bars of all shown traces as
    /// washes in their colours, then their outlines, so that no trace hides the shape of
    /// another.
    fn plot_bars(
        &self,
        plot_ui: &mut egui_plot::PlotUi<'_>,
        visuals: &egui::Visuals,
        (shown, counted): (&[Shown<'_>], &Counted),
        axis: CountAxis,
        hovered: Option<usize>,
    ) {
        let bins = &counted.bins;
        if let Some(index) = hovered {
            let (low, high) = (bins.edge(index), bins.edge(index + 1));
            let corners = [
                self.point(low, axis.floor),
                self.point(high, axis.floor),
                self.point(high, axis.axis_top),
                self.point(low, axis.axis_top),
            ];
            plot_ui.polygon(
                egui_plot::Polygon::new("", corners.to_vec())
                    .fill_color(theme::tokens(visuals).hover)
                    .stroke(egui::Stroke::NONE)
                    .allow_hover(false),
            );
        }
        let shares: Vec<Vec<f64>> = counted.traces.iter().map(Counts::shares).collect();
        for (trace, shares) in shown.iter().zip(&shares) {
            let wash = theme::wash(visuals, theme::trace_color(visuals, trace.index));
            let bars = (shares.iter().enumerate())
                .filter_map(|(index, &share)| {
                    let reach = axis.position(share)?;
                    let bar = egui_plot::Bar::new(bins.center(index), reach - axis.floor)
                        .base_offset(axis.floor)
                        .width(bins.width)
                        .fill(wash)
                        .stroke(egui::Stroke::NONE);
                    Some(bar)
                })
                .collect();
            let chart = egui_plot::BarChart::new("", bars).allow_hover(false);
            plot_ui.bar_chart(match self.orientation {
                Orientation::Upright => chart.vertical(),
                Orientation::Sideways => chart.horizontal(),
            });
        }
        for (trace, shares) in shown.iter().zip(&shares) {
            let color = theme::trace_color(visuals, trace.index);
            for run in outline_runs(shares, bins, axis) {
                let points: Vec<[f64; 2]> = (run.into_iter())
                    .map(|[value, share]| self.point(value, share))
                    .collect();
                plot_ui.line(
                    egui_plot::Line::new("", egui_plot::PlotPoints::from(points))
                        .color(color)
                        .width(OUTLINE_WIDTH)
                        .allow_hover(false),
                );
            }
        }
    }

    /// Keeps the values panned or zoomed to in `response` in [`Histogram::value_view`]
    /// instead of `shown`, until a double click fits them again.
    fn keep_value_view(
        &mut self,
        response: &egui_plot::PlotResponse<Option<usize>>,
        (low, high): (f64, f64),
    ) {
        if response.response.double_clicked() {
            self.value_view = None;
            response.response.ctx.request_repaint();
            return;
        }
        let bounds = response.transform.bounds();
        let axis = match self.orientation {
            Orientation::Upright => 0,
            Orientation::Sideways => 1,
        };
        let (shown_low, shown_high) = (bounds.min()[axis], bounds.max()[axis]);
        let tolerance = (high - low) * 1.0e-6;
        if (shown_low - low).abs() > tolerance || (shown_high - high).abs() > tolerance {
            self.value_view = Some((shown_low, shown_high));
        }
    }

    /// Paints the markers of the traces in a strip at the end of the counts axis, a row
    /// per trace: a dot at the mean and, for [`Markers::MeanSigma`], a whisker over one
    /// standard deviation on each side.
    fn paint_markers(
        &self,
        ui: &egui::Ui,
        overlay: &Overlay,
        shown: &[Shown<'_>],
        counted: &Counted,
    ) {
        let frame = *overlay.transform.frame();
        for marker in self.markers(shown, counted) {
            let color = theme::trace_color(ui.visuals(), marker.index);
            let stroke = egui::Stroke::new(OUTLINE_WIDTH, color);
            let across = MARKER_INSET + (marker.row as f32 + 0.5) * MARKER_ROW;
            // the position of `value` in the row
            let at = |value: f64| match self.orientation {
                Orientation::Upright => egui::pos2(
                    overlay.transform.position_from_point_x(value),
                    frame.top() + across,
                ),
                Orientation::Sideways => egui::pos2(
                    frame.right() - across,
                    overlay.transform.position_from_point_y(value),
                ),
            };
            let painter = &overlay.painter;
            if let Some((low, high)) = marker.whisker {
                let (low, high) = (at(low), at(high));
                let cap = match self.orientation {
                    Orientation::Upright => egui::vec2(0.0, MARKER_RADIUS),
                    Orientation::Sideways => egui::vec2(MARKER_RADIUS, 0.0),
                };
                painter.line_segment([low, high], stroke);
                painter.line_segment([low - cap, low + cap], stroke);
                painter.line_segment([high - cap, high + cap], stroke);
            }
            painter.circle_filled(at(marker.mean), MARKER_RADIUS, color);
        }
    }

    /// Paints the counts of the samples below and above the bins faintly at the ends
    /// they lie beyond, in the room beyond the bars.
    fn paint_outside(&self, ui: &egui::Ui, overlay: &Overlay, (below, above): (u64, u64)) {
        let faint = theme::tokens(ui.visuals()).text_faint;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let [below_text, above_text] = self.outside_texts(below, above);
        let [below_at, above_at] = overlay.outside_positions();
        for (count, text, (pos, anchor)) in
            [(below, below_text, below_at), (above, above_text, above_at)]
        {
            if count > 0 {
                overlay.painter.text(pos, anchor, text, font.clone(), faint);
            }
        }
    }

    /// Shows the `legend` in the box `rect` on the plot `frame`, for a view without a
    /// title row: the name and the dot of each trace, a line each or wrapped, and below
    /// them the unit they share, small and faint.
    fn legend_box_ui(
        &self,
        ui: &mut egui::Ui,
        (rect, legend): (egui::Rect, &Legend),
        frame: egui::Rect,
        unit: Option<&str>,
    ) {
        let tokens = theme::tokens(ui.visuals());
        let painter = ui.painter().with_clip_rect(frame);
        painter.rect(
            rect,
            theme::CARD_RADIUS,
            tokens.card,
            egui::Stroke::new(1.0, tokens.border),
            egui::StrokeKind::Inside,
        );
        let layout = egui::Layout::top_down(egui::Align::Max);
        let inner = rect.shrink2(LEGEND_MARGIN);
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(layout));
        ui.set_clip_rect(frame.intersect(ui.clip_rect()));
        ui.spacing_mut().item_spacing = egui::vec2(LEGEND_DOT_GAP, 0.0);
        let weak = ui.visuals().weak_text_color();
        let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
        let traces: Vec<(&str, &str, usize)> = self.selected_path_iter().collect();
        let row = egui::vec2(inner.width(), LEGEND_ROW);
        match &legend.lines {
            // "name ●" on a line each
            None => {
                for &(path, name, index) in &traces {
                    ui.allocate_ui_with_layout(row, right_to_left, |ui| {
                        dot(ui, theme::trace_color(ui.visuals(), index));
                        ui.label(egui::RichText::new(name).color(weak))
                            .on_hover_text(path);
                    });
                }
            }
            // "● name" side by side, the lines right-aligned
            Some(lines) => {
                for line in lines {
                    ui.allocate_ui_with_layout(row, right_to_left, |ui| {
                        for (position, &item) in line.iter().rev().enumerate() {
                            if position > 0 {
                                ui.add_space(LEGEND_ITEM_GAP - LEGEND_DOT_GAP);
                            }
                            let (path, name, index) = traces[item];
                            ui.label(egui::RichText::new(name).color(weak))
                                .on_hover_text(path);
                            dot(ui, theme::trace_color(ui.visuals(), index));
                        }
                    });
                }
            }
        }
        if let Some(unit) = unit {
            let row = egui::vec2(inner.width(), UNIT_ROW);
            ui.allocate_ui_with_layout(row, right_to_left, |ui| {
                ui.label(egui::RichText::new(unit).small().color(tokens.text_faint));
            });
        }
    }

    /// The markers of the `shown` traces of `counted`, a row each.
    fn markers(&self, shown: &[Shown<'_>], counted: &Counted) -> Vec<Marker> {
        if self.markers == Markers::Off {
            return vec![];
        }
        let traces = shown.iter().zip(&counted.traces).enumerate();
        traces
            .filter_map(|(row, (trace, counts))| {
                let mean = counts.mean?;
                let std_dev = counts.std_dev.unwrap_or(0.0);
                let whisker = (self.markers == Markers::MeanSigma && std_dev > 0.0)
                    .then_some((mean - std_dev, mean + std_dev));
                Some(Marker {
                    row,
                    index: trace.index,
                    mean,
                    whisker,
                })
            })
            .collect()
    }
}

/// The outlines of the bars of the `shares` in percent of `bins` on the counts `axis`,
/// as points of a value and a share: one per run of bins that are not empty, from the
/// floor up and down to it again at its ends, so that empty bins and the floor stay
/// without one.
fn outline_runs(shares: &[f64], bins: &Bins, axis: CountAxis) -> Vec<Vec<[f64; 2]>> {
    let mut runs: Vec<Vec<[f64; 2]>> = vec![];
    let mut run: Vec<[f64; 2]> = vec![];
    for (index, &share) in shares.iter().enumerate() {
        match axis.position(share) {
            Some(reach) => {
                if run.is_empty() {
                    run.push([bins.edge(index), axis.floor]);
                }
                run.push([bins.edge(index), reach]);
                run.push([bins.edge(index + 1), reach]);
            }
            None if !run.is_empty() => {
                run.push([bins.edge(index), axis.floor]);
                runs.push(std::mem::take(&mut run));
            }
            None => {}
        }
    }
    if !run.is_empty() {
        run.push([bins.edge(shares.len()), axis.floor]);
        runs.push(run);
    }
    runs
}

/// The mark of a trace in the strip of markers: a dot at the mean, and a whisker
/// between its ends if there is one.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Marker {
    /// The row in the strip.
    row: usize,
    /// Picks its colour.
    index: usize,
    mean: f64,
    whisker: Option<(f64, f64)>,
}

/// The values the value axis fits: the `fixed` range, or else the values of `data`
/// with a margin on each side, or a few bins around them if they are all the same, as
/// `counted`.
fn fitted_values(
    counted: Option<&Counted>,
    data: Option<(f64, f64)>,
    fixed: Option<(f64, f64)>,
) -> (f64, f64) {
    match (counted, data, fixed) {
        (_, _, Some(fixed)) => fixed,
        (Some(counted), Some((low, high)), None) if counted.single => {
            let room = SINGLE_VALUE_ROOM * counted.bins.width;
            (low - room, high + room)
        }
        (_, Some((low, high)), None) => {
            let margin = (high - low) * VALUE_MARGIN;
            (low - margin, high + margin)
        }
        _ => (0.0, 1.0),
    }
}

/// The room at the end of the counts axis, beyond the bars: the marker strip at the
/// end, then the counts of the samples outside the bins and the legend box.
#[derive(Clone, PartialEq, Debug)]
struct Room {
    orientation: Orientation,
    /// The length of the marker strip along the counts axis.
    markers: f32,
    /// The size of the larger text of the counts outside the bins, if there are any.
    outside: Option<egui::Vec2>,
    /// Whether samples lie above the bins.
    above: bool,
    /// The legend box, if it is shown.
    legend: Option<Legend>,
}

impl Room {
    /// Its length along the counts axis, with the gap to the bars.
    fn length(&self) -> f32 {
        let stacked = match self.orientation {
            Orientation::Upright => {
                let legend = self.legend.as_ref().map_or(0.0, |legend| legend.size.y);
                self.outside.map_or(0.0, |size| size.y) + legend
            }
            Orientation::Sideways => {
                let outside = self.outside.map_or(0.0, |size| size.x);
                outside.max(self.legend.as_ref().map_or(0.0, |legend| legend.size.x))
            }
        };
        let texts = self.outside.is_some() || self.legend.is_some();
        let texts = if texts { OVERLAY_INSET + stacked } else { 0.0 };
        let gap = if self.markers > 0.0 || texts > 0.0 {
            MARKER_GAP
        } else {
            0.0
        };
        self.markers + texts + gap
    }
}

/// The legend in its box: its size, and the traces on each line if it is wrapped
/// across the plot, else a line per trace.
#[derive(Clone, PartialEq, Debug)]
struct Legend {
    size: egui::Vec2,
    lines: Option<Vec<Vec<usize>>>,
}

/// The items of `widths` in lines at most `width` wide, `gap` apart, each line as
/// the indices of its items, at least one per line.
fn wrap(widths: &[f32], width: f32, gap: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = vec![];
    let mut used = 0.0;
    for (index, &item) in widths.iter().enumerate() {
        match lines.last_mut() {
            Some(line) if used + gap + item <= width => {
                line.push(index);
                used += gap + item;
            }
            _ => {
                lines.push(vec![index]);
                used = item;
            }
        }
    }
    lines
}

/// What is painted over the plot: its painter clipped to it, its transform and the room
/// beyond the bars.
struct Overlay {
    painter: egui::Painter,
    transform: egui_plot::PlotTransform,
    room: Room,
}

impl Overlay {
    /// The corner of the room for the texts beyond the marker strip, on the side of the
    /// highest values.
    fn corner(&self) -> egui::Pos2 {
        let frame = self.transform.frame();
        match self.room.orientation {
            Orientation::Upright => egui::pos2(
                frame.right() - OVERLAY_INSET,
                frame.top() + self.room.markers + OVERLAY_INSET,
            ),
            Orientation::Sideways => egui::pos2(
                frame.right() - self.room.markers - OVERLAY_INSET,
                frame.top() + OVERLAY_INSET,
            ),
        }
    }

    /// Where the counts of the samples below and above the bins go, and how they are
    /// anchored there.
    fn outside_positions(&self) -> [(egui::Pos2, egui::Align2); 2] {
        let (frame, corner) = (self.transform.frame(), self.corner());
        let below = match self.room.orientation {
            Orientation::Upright => (
                egui::pos2(frame.left() + OVERLAY_INSET, corner.y),
                egui::Align2::LEFT_TOP,
            ),
            Orientation::Sideways => (
                egui::pos2(corner.x, frame.bottom() - OVERLAY_INSET),
                egui::Align2::RIGHT_BOTTOM,
            ),
        };
        [below, (corner, egui::Align2::RIGHT_TOP)]
    }

    /// The box of the legend, below the count of the samples above the bins.
    fn legend_rect(&self) -> Option<egui::Rect> {
        let size = self.room.legend.as_ref()?.size;
        let below_text = match self.room.orientation {
            Orientation::Upright => self.room.outside.is_some(),
            Orientation::Sideways => self.room.above,
        };
        let top = if below_text { LEGEND_ROW } else { 0.0 };
        let corner = self.corner() + egui::vec2(0.0, top);
        Some(egui::Rect::from_min_size(
            corner - egui::vec2(size.x, 0.0),
            size,
        ))
    }
}

/// The bin under the pointer, for its tooltip.
struct Bin<'a> {
    index: usize,
    counted: &'a Counted,
    unit: Option<&'a str>,
    format: DisplayFormat,
}

impl Bin<'_> {
    /// The values of the bin: its range, or the value of all samples.
    fn range_text(&self) -> String {
        let bins = &self.counted.bins;
        if self.counted.single {
            let total: u64 = self
                .counted
                .traces
                .iter()
                .map(|counts| counts.bins[0])
                .sum();
            let value = Number::Float(bins.center(0)).format(Some(SHOWN_DIGITS), self.format);
            return format!(
                "All {total} samples are {}",
                super::with_unit(value, self.unit)
            );
        }
        let write = |edge: f64| write_value(edge, bins.width, self.format);
        let text = format!(
            "{} to {}",
            write(bins.edge(self.index)),
            write(bins.edge(self.index + 1))
        );
        super::with_unit(text, self.unit)
    }

    /// The share of the bin in percent and its count of all samples of `counts`, as
    /// "12.5 %" and "42 of 336".
    fn share_texts(&self, counts: &Counts) -> [String; 2] {
        let count = counts.bins[self.index];
        let share = match count {
            0 => "0 %".to_owned(),
            count => write_share(counts.share(count)),
        };
        [share, format!("{count} of {}", counts.total())]
    }

    /// Shows the values of the bin and the share and count of each shown trace in it.
    fn tooltip_ui(&self, ui: &mut egui::Ui, shown: &[Shown<'_>]) {
        let faint = theme::tokens(ui.visuals()).text_faint;
        let weak = ui.visuals().weak_text_color();
        ui.label(egui::RichText::new(self.range_text()).color(weak));
        let texts: Vec<[String; 2]> = (self.counted.traces.iter())
            .map(|counts| self.share_texts(counts))
            .collect();
        // the values are right aligned in columns as wide as their widest text: a right
        // to left layout fills the width it is given, and the grid gives the last column
        // all of the tooltip's, which is the widest a tooltip can be when it opens
        let widths = [0, 1].map(|column| {
            (texts.iter())
                .map(|texts| text_size(ui, &texts[column], egui::TextStyle::Body).x)
                .fold(0.0, f32::max)
        });
        egui::Grid::new("histogram tooltip")
            .num_columns(3)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for ((trace, counts), [share, count]) in
                    shown.iter().zip(&self.counted.traces).zip(texts)
                {
                    let empty = counts.bins[self.index] == 0;
                    let (name, share, count) = if empty {
                        (
                            egui::RichText::new(&trace.name).color(faint),
                            egui::RichText::new(share).color(faint),
                            egui::RichText::new(count).color(faint),
                        )
                    } else {
                        (
                            egui::RichText::new(&trace.name),
                            egui::RichText::new(share).strong(),
                            egui::RichText::new(count).color(weak),
                        )
                    };
                    ui.horizontal(|ui| {
                        dot(ui, theme::trace_color(ui.visuals(), trace.index));
                        ui.label(name).on_hover_text(&trace.path);
                    });
                    for (value, width) in [share, count].into_iter().zip(widths) {
                        let layout = egui::Layout::right_to_left(egui::Align::Min);
                        ui.allocate_ui_with_layout(egui::vec2(width, 0.0), layout, |ui| {
                            ui.label(value);
                        });
                    }
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::actions::Invoker;
    use crate::app::view::stats::Stats;
    use crate::app::view::{NO_PLOTS, PlotInfo, ViewType};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};
    use proto::Data;

    /// Appends the `values` of the trace at `path` to `model`, one every `step`
    /// seconds from `start` seconds.
    fn append(model: &mut Model, path: &str, (start, step): (f64, f64), values: &[Data]) {
        for (index, value) in values.iter().enumerate() {
            let seconds = start + index as f64 * step;
            model.traces.append(&proto::Value {
                timestamp: (seconds * 1.0e9).round() as u64,
                path: path.to_owned(),
                value: value.clone(),
            });
        }
    }

    fn floats(values: impl IntoIterator<Item = f64>) -> Vec<Data> {
        values.into_iter().map(Data::Float).collect()
    }

    /// A histogram of the traces at `paths`.
    fn histogram_of(paths: &[&str]) -> Histogram {
        Histogram {
            traces: paths.iter().map(|&path| path.to_owned()).collect(),
            ..Histogram::default()
        }
    }

    /// The time range of `model` shown live.
    fn live(model: &Model) -> TimeRange {
        let mut time = TimeRange::default();
        time.update(model, 0.0);
        time
    }

    /// What `histogram` counts of `model` in the window of `time`, on a value axis
    /// 1000 points long.
    fn counted(histogram: &Histogram, model: &Model, time: &TimeRange) -> Counted {
        let counting = histogram.counting(model, time, &NO_PLOTS, 1000.0);
        counting.counted.expect("samples in the window")
    }

    /// The samples of the trace at `path` in the window of `time` that are not null.
    fn samples_in_window(model: &Model, time: &TimeRange, path: &str) -> u64 {
        let (start, end) = time.range().unwrap();
        let (start, end) = (time.timestamp(start), time.timestamp(end));
        let series = model.traces.series(path).unwrap();
        (0..series.len())
            .filter(|&index| (start..=end).contains(&series.timestamps[index]))
            .filter(|&index| !series.value(index).is_nan())
            .count() as u64
    }

    fn total(counts: &Counts) -> u64 {
        counts.bins.iter().sum()
    }

    /// Whether `value` is 1, 2 or 5 times a power of ten.
    fn is_round(value: f64) -> bool {
        let mantissa = value / 10f64.powf(value.log10().floor());
        [1.0, 2.0, 5.0, 10.0]
            .iter()
            .any(|round| (mantissa - round).abs() < 1.0e-9)
    }

    #[test]
    fn counts_add_up_to_the_samples_of_the_shown_window() {
        let values = (0..=40).map(|second| match second % 7 {
            3 => Data::Null,
            _ => Data::Float(f64::from(second % 5)),
        });
        let mut model = Model::default();
        append(&mut model, "a", (0.0, 1.0), &values.collect::<Vec<_>>());
        let histogram = histogram_of(&["a"]);
        let mut time = live(&model);
        let check = |model: &Model, time: &TimeRange| {
            let counted = counted(&histogram, model, time);
            assert_eq!(
                total(&counted.traces[0]),
                samples_in_window(model, time, "a")
            );
            total(&counted.traces[0])
        };
        // the last 20 s, live, without the nulls
        let shown = check(&model, &time);
        assert_eq!(shown, 21 - 3);
        // panned and zoomed
        time.pan_zoom(5.0, 15.0);
        assert!(check(&model, &time) < shown);
        time.pan_zoom(0.0, 40.0);
        assert_eq!(check(&model, &time), 41 - 6);
        // live, as more samples arrive
        time.go_live();
        time.pan_zoom(20.0, 40.0);
        append(&mut model, "a", (41.0, 1.0), &floats([1.0; 10]));
        time.update(&model, 0.0);
        assert_eq!(time.range(), Some((30.0, 50.0)));
        check(&model, &time);
    }

    #[test]
    fn traces_share_one_set_of_bins_and_nulls_are_not_counted() {
        let mut model = Model::default();
        append(&mut model, "a", (0.0, 0.5), &floats((0..40).map(f64::from)));
        let mut b = floats((0..40).map(|i| f64::from(i) + 7.5));
        b[3] = Data::Null;
        append(&mut model, "b", (0.0, 0.5), &b);
        let histogram = histogram_of(&["a", "b"]);
        let time = live(&model);
        let counted = counted(&histogram, &model, &time);
        assert_eq!(counted.traces.len(), 2);
        for (path, counts) in ["a", "b"].into_iter().zip(&counted.traces) {
            assert_eq!(counts.bins.len(), counted.bins.count);
            assert_eq!(total(counts), samples_in_window(&model, &time, path));
        }
        assert_eq!(total(&counted.traces[1]), 39);
    }

    #[test]
    fn numbers_and_elements_of_small_arrays_are_checked_but_not_bools_or_strings() {
        let mut model = Model::default();
        let array = |len: usize, value: Data| Data::Array(vec![value; len]);
        for (path, value) in [
            ("int", Data::Int(-3)),
            ("float", Data::Float(1.5)),
            ("bool", Data::Bool(true)),
            ("text", Data::from("on")),
            ("small", array(3, Data::Float(1.0))),
            ("large", array(MAX_BINS, Data::Float(1.0))),
            ("bools", array(3, Data::Bool(false))),
        ] {
            append(&mut model, path, (0.0, 1.0), &[value]);
        }
        let selectable = |path| ViewType::Histogram.selectable(model.traces.get(path).unwrap());
        assert!(matches!(selectable("int"), Selectable::Trace));
        assert!(matches!(selectable("float"), Selectable::Trace));
        assert!(matches!(selectable("small"), Selectable::Elements));
        for path in ["bool", "text", "large", "bools"] {
            assert!(matches!(selectable(path), Selectable::No(_)), "{path}");
        }
    }

    struct State {
        histogram: Histogram,
        model: Model,
        time: TimeRange,
        invoker: Invoker,
        plots: Plots,
        titled: bool,
        formats: Formats,
    }

    impl State {
        fn new(histogram: Histogram, model: Model) -> Self {
            Self {
                histogram,
                model,
                time: TimeRange::default(),
                invoker: Invoker::default(),
                plots: Plots::default(),
                titled: false,
                formats: Formats::default(),
            }
        }
    }

    /// A harness showing the histogram of `state`, or its title row with `title`.
    fn harness(state: State, title: bool) -> Harness<'static, State> {
        sized_harness(state, title, egui::vec2(400.0, 300.0))
    }

    /// A harness of `size` showing the histogram of `state`, or its title row with
    /// `title`.
    fn sized_harness(state: State, title: bool, size: egui::Vec2) -> Harness<'static, State> {
        Harness::builder()
            .with_size(size)
            // fast enough for double clicks
            .with_step_dt(1.0 / 60.0)
            .with_max_steps(60)
            .build_ui_state(
                move |ui, state: &mut State| {
                    state
                        .time
                        .update(&state.model, ui.input(|input| input.time));
                    let mut context = ViewContext {
                        model: &state.model,
                        time: &mut state.time,
                        invoker: &mut state.invoker,
                        formats: &state.formats,
                        titled: state.titled,
                        plots: &state.plots,
                    };
                    if title {
                        ui.horizontal(|ui| state.histogram.title_ui(ui, &mut context));
                    } else {
                        state.histogram.view(ui, &mut context);
                    }
                },
                state,
            )
    }

    /// A model of the traces `x` and `y` in `x_unit` and `y_unit`.
    fn pose(x_unit: &str, y_unit: &str) -> Model {
        let mut model = Model::default();
        append(
            &mut model,
            "pose.x",
            (0.0, 0.1),
            &floats((0..200).map(|i| (f64::from(i) * 0.37) % 10.0)),
        );
        append(
            &mut model,
            "pose.y",
            (0.0, 0.1),
            &floats((0..200).map(|i| (f64::from(i) * 0.53) % 6.0)),
        );
        model.traces.set_unit("pose.x", x_unit);
        model.traces.set_unit("pose.y", y_unit);
        model
    }

    #[test]
    fn the_legend_names_the_traces_in_the_title_row_or_in_a_corner() {
        let histogram = histogram_of(&["pose.x", "pose.y"]);
        // in the title row, with the unit both share and without values
        let mut harness = harness(State::new(histogram.clone(), pose("m", "m")), true);
        harness.run();
        harness.get_by_label("x");
        harness.get_by_label("y");
        harness.get_by_label("m");
        assert_eq!(
            harness
                .get_all_by_role(egui::accesskit::Role::Label)
                .count(),
            3
        );
        let mut harness = self::harness(State::new(histogram.clone(), pose("m", "rad")), true);
        harness.run();
        assert!(harness.query_by_label("m").is_none());

        // in a corner without a title row, not in the view with one
        let mut harness = self::harness(State::new(histogram.clone(), pose("m", "m")), false);
        harness.run();
        let x = harness.get_by_label("x").rect();
        assert!(x.center().x > 300.0 && x.center().y < 100.0, "{x:?}");
        harness.get_by_label("y");
        let mut state = State::new(histogram, pose("m", "m"));
        state.titled = true;
        let mut harness = self::harness(state, false);
        harness.run();
        assert!(harness.query_by_label("x").is_none());
    }

    #[test]
    fn values_are_written_in_the_format_the_traces_share() {
        let mut formats = Formats::default();
        formats.set("a", DisplayFormat::Hex);
        formats.set("b", DisplayFormat::Hex);
        assert_eq!(common_format(&formats, ["a", "b"]), DisplayFormat::Hex);
        assert_eq!(common_format(&formats, ["a", "c"]), DisplayFormat::Default);
        assert_eq!(write_value(16.0, 1.0, DisplayFormat::Hex), "0x10");
        assert_eq!(write_value(16.0, 1.0, DisplayFormat::Default), "16");
        // as many decimals as the step takes
        assert_eq!(write_value(0.5, 0.25, DisplayFormat::Default), "0.50");
        assert_eq!(write_value(-0.0001, 0.1, DisplayFormat::Default), "0.0");
        assert_eq!(write_value(2400.0, 50.0, DisplayFormat::Default), "2400");
    }

    #[test]
    fn bins_show_the_unit_only_if_all_traces_share_one() {
        let counted = Counted {
            bins: Bins {
                start: 0.0,
                width: 0.25,
                count: 4,
            },
            traces: vec![Counts {
                bins: vec![0, 1, 2, 3],
                ..Counts::default()
            }],
            single: false,
        };
        let bin = |unit| Bin {
            index: 2,
            counted: &counted,
            unit,
            format: DisplayFormat::Default,
        };
        assert_eq!(bin(Some("m")).range_text(), "0.50 to 0.75 m");
        assert_eq!(bin(None).range_text(), "0.50 to 0.75");
        let model = pose("m", "rad");
        assert_eq!(super::super::common_unit(&model, ["pose.x"]), Some("m"));
        assert_eq!(
            super::super::common_unit(&model, ["pose.x", "pose.y"]),
            None
        );
    }

    #[test]
    fn without_traces_or_samples_the_view_says_so() {
        let mut harness = harness(State::new(Histogram::default(), Model::default()), false);
        harness.run();
        harness.get_by_label(CHECK_TRACES);

        // only nulls in the shown window
        let mut model = Model::default();
        append(&mut model, "a", (0.0, 1.0), &floats([1.0, 2.0]));
        append(&mut model, "a", (2.0, 1.0), &vec![Data::Null; 30]);
        let mut harness = self::harness(State::new(histogram_of(&["a"]), model), false);
        harness.run();
        harness.get_by_label(NO_SAMPLES);
        assert!(harness.state().histogram.last.is_none());
    }

    #[test]
    fn samples_of_one_value_are_one_bar_centred_on_it() {
        let mut model = Model::default();
        append(&mut model, "t", (0.0, 1.0), &floats([23.4; 30]));
        append(&mut model, "n", (0.0, 1.0), &vec![Data::Int(5); 30]);
        let time = live(&model);
        let counted = counted(&histogram_of(&["t"]), &model, &time);
        assert!(counted.single);
        assert_eq!(counted.bins.count, 1);
        assert!((counted.bins.center(0) - 23.4).abs() < 1.0e-9);
        // a round step near 1 % of the value
        assert_eq!(counted.bins.width, 0.2);
        let bin = Bin {
            index: 0,
            counted: &counted,
            unit: Some("°C"),
            format: DisplayFormat::Default,
        };
        assert_eq!(bin.range_text(), "All 21 samples are 23.4 °C");
        let counted = self::counted(&histogram_of(&["n"]), &model, &time);
        assert_eq!((counted.bins.start, counted.bins.width), (4.5, 1.0));
    }

    #[test]
    fn automatic_bins_number_about_the_square_root_of_the_samples() {
        let histogram = Histogram::default();
        for samples in [4, 100, 400, 10_000, 1_000_000] {
            let bins = histogram.make_bins((0.0, 1.0), false, samples, 0.0, false);
            let target = (samples as f64).sqrt().clamp(MIN_AUTO_BINS, MAX_AUTO_BINS);
            let count = bins.count as f64;
            assert!(
                count >= target / 1.75 && count <= target * 1.75 + 1.0,
                "{samples}: {bins:?}"
            );
            assert!(is_round(bins.width), "{bins:?}");
        }
        // few and many samples
        let few = histogram.make_bins((0.0, 1.0), false, 1, 0.0, false);
        assert_eq!(few.width, 0.1);
        let many = histogram.make_bins((0.0, 1.0), false, 100_000_000, 0.0, false);
        assert_eq!(many.width, 0.02);
        // at least a few points wide, and integers at least 1 wide
        let wide = histogram.make_bins((0.0, 1.0), false, 1_000_000, 0.04, false);
        assert_eq!(wide.width, 0.05);
        let integers = histogram.make_bins((0.0, 5.0), false, 1000, 0.0, true);
        assert_eq!((integers.width, integers.count), (1.0, 6));
    }

    #[test]
    fn the_automatic_width_changes_only_clearly_past_a_step() {
        // 400 samples, 20 bins: the step between 0.1 and 0.2 lies at a span of 3
        let mut histogram = Histogram::default();
        let bins = |histogram: &Histogram, span: f64| {
            histogram.make_bins((0.0, span), false, 400, 0.0, false)
        };
        let shown = |histogram: &mut Histogram, span: f64| {
            let bins = bins(histogram, span);
            histogram.last = Some(Box::new(Last {
                data: (0.0, span),
                values: (0.0, span),
                bins,
                frame: egui::Rect::NOTHING,
                bars: egui::Rect::NOTHING,
            }));
            bins.width
        };
        // without a width before, the nearest round one
        assert_eq!(bins(&histogram, 2.9).width, 0.1);
        assert_eq!(bins(&histogram, 3.1).width, 0.2);
        // around the step, the width stays
        assert_eq!(shown(&mut histogram, 2.9), 0.1);
        for span in [3.1, 2.95, 3.3, 3.5, 2.9, 3.5] {
            assert_eq!(shown(&mut histogram, span), 0.1, "{span}");
        }
        // clearly past it, it changes, and stays when it goes back a little
        assert_eq!(shown(&mut histogram, 3.7), 0.2);
        for span in [2.9, 2.6, 3.2] {
            assert_eq!(shown(&mut histogram, span), 0.2, "{span}");
        }
        assert_eq!(shown(&mut histogram, 2.4), 0.1);
        // a width that is not round, e.g. of a fixed number of bins, is not kept
        histogram.last.as_mut().unwrap().bins.width = 0.13;
        assert_eq!(bins(&histogram, 2.6).width, 0.1);
        // nor one too narrow
        histogram.last.as_mut().unwrap().bins.width = 0.1;
        let wide = histogram.make_bins((0.0, 3.2), false, 400, 0.15, false);
        assert_eq!(wide.width, 0.2);
    }

    #[test]
    fn the_value_axis_fits_the_values_with_a_margin() {
        assert_eq!(fitted_values(None, Some((0.0, 10.0)), None), (-0.5, 10.5));
        assert_eq!(
            fitted_values(None, Some((0.0, 10.0)), Some((2.0, 3.0))),
            (2.0, 3.0)
        );
        assert_eq!(fitted_values(None, None, None), (0.0, 1.0));
    }

    #[test]
    fn bin_edges_are_multiples_of_the_width_and_stay_as_the_window_moves() {
        let histogram = Histogram::default();
        let first = histogram.make_bins((0.13, 2.73), false, 400, 0.0, false);
        for step in 0..20 {
            let low = 0.13 + f64::from(step) * 0.07;
            let bins = histogram.make_bins((low, low + 2.6), false, 400, 0.0, false);
            assert_eq!(bins.width, first.width);
            let multiple = bins.start / bins.width;
            assert!((multiple - multiple.round()).abs() < 1.0e-9, "{bins:?}");
            assert!(bins.start <= low && bins.edge(bins.count) > low + 2.6);
        }
    }

    #[test]
    fn a_fixed_number_or_width_of_bins_is_kept() {
        let number = Histogram {
            bins: BinMode::Number,
            bin_count: 7,
            ..Histogram::default()
        };
        let bins = number.make_bins((1.0, 8.0), false, 1000, 0.5, false);
        assert_eq!(bins.count, 7);
        assert_eq!((bins.start, bins.edge(7)), (1.0, 8.0));
        let width = Histogram {
            bins: BinMode::Width,
            bin_width: 0.25,
            ..Histogram::default()
        };
        let bins = width.make_bins((1.1, 3.0), false, 1000, 0.5, false);
        assert_eq!((bins.start, bins.width, bins.count), (1.0, 0.25, 9));
        // a value at the end of a fixed range is in the last bin
        let bins = width.make_bins((1.0, 3.0), true, 1000, 0.5, false);
        assert_eq!(bins.count, 8);
        assert_eq!(bins.index(3.0), 7);
    }

    #[test]
    fn samples_outside_a_fixed_range_are_counted_below_and_above_it() {
        let bins = Bins {
            start: 2.0,
            width: 1.0,
            count: 3,
        };
        let values = [0.0, 1.9, 2.0, 2.5, f64::NAN, 4.0, 5.0, 5.5, 9.0];
        let counts = count(values.into_iter(), &bins, Some((2.0, 5.0)));
        assert_eq!(counts.bins, [2, 0, 2]);
        assert_eq!((counts.below, counts.above), (2, 2));
        // the mean is of the binned samples
        assert_eq!(counts.mean, Some(3.375));
    }

    #[test]
    fn the_counts_axis_fits_the_tallest_bin_in_view() {
        let counted = Counted {
            bins: Bins {
                start: 0.0,
                width: 1.0,
                count: 4,
            },
            traces: vec![
                Counts {
                    bins: vec![1, 9, 3, 2],
                    ..Counts::default()
                },
                Counts {
                    bins: vec![0, 2, 1, 4],
                    ..Counts::default()
                },
            ],
            single: false,
        };
        // the shares of the samples of each trace, 9 of 15 and 4 of 7
        assert_eq!(counted.tallest_in((-0.5, 4.5)), 60.0);
        // zoomed in on the values
        assert_eq!(counted.tallest_in((2.2, 4.5)), 400.0 / 7.0);
        // without samples in view, the share of one sample of the most samples
        assert_eq!(counted.one_sample(), 100.0 / 15.0);
        assert_eq!(counted.tallest_in((10.0, 20.0)), 100.0 / 15.0);
        // it grows at once and shrinks only well below its top
        assert_eq!(fit_top(None, 60.0), 60.0);
        assert_eq!(fit_top(Some(60.0), 80.0), 80.0);
        assert_eq!(fit_top(Some(60.0), 45.0), 60.0);
        assert_eq!(fit_top(Some(60.0), 20.0), 20.0);
        // the bars reach the top of the axis but for some room and the marker strip
        let axis = CountAxis::new(CountScale::Linear, 60.0, 100.0 / 15.0, 200.0, 0.0);
        assert_eq!(axis.floor, 0.0);
        assert!(axis.bars_top > 60.0 && axis.bars_top < 66.0);
        assert_eq!(axis.axis_top, axis.bars_top);
        let strip = CountAxis::new(CountScale::Linear, 60.0, 100.0 / 15.0, 200.0, 50.0);
        assert!((strip.axis_top - strip.bars_top * 200.0 / 150.0).abs() < 1.0e-9);
    }

    #[test]
    fn the_log_counts_axis_marks_powers_of_ten_from_below_one_sample() {
        // one sample of 1000 is 0.1 %, half a decade above the floor
        let axis = CountAxis::new(CountScale::Log, 100.0, 0.1, 200.0, 0.0);
        assert_eq!(axis.floor, -1.5);
        let marks = axis.marks(false)(egui_plot::GridInput {
            bounds: (axis.floor, axis.axis_top),
            base_step_size: 0.01,
        });
        let labels: Vec<String> = (marks.iter())
            .map(|mark| axis.label(mark.value, mark.step_size))
            .collect();
        assert_eq!(labels, ["0.1 %", "1 %", "10 %", "100 %"]);
        // a bin of one sample is a short bar, an empty one shows nothing
        assert_eq!(axis.position(0.1), Some(-1.0));
        assert!(axis.position(0.1).unwrap() > axis.floor);
        assert_eq!(axis.position(0.0), None);
        assert_eq!(axis.position(10.0), Some(1.0));
        assert_eq!(decade_label(-3), "0.001 %");
        // the floor is rounded down to half a decade, and not marked: one sample of
        // 2000 is 0.05 %
        let axis = CountAxis::new(CountScale::Log, 100.0, 0.05, 200.0, 0.0);
        assert_eq!(axis.floor, -2.0);
        let marks = axis.marks(false)(egui_plot::GridInput {
            bounds: (axis.floor, axis.axis_top),
            base_step_size: 0.01,
        });
        assert_eq!(marks.first().map(|mark| mark.value), Some(-1.0));
        // the end of the value axis is labelled at the floor, not the counts axis
        let linear = CountAxis::new(CountScale::Linear, 40.0, 1.0, 200.0, 0.0);
        assert_eq!(linear.label(0.0, 10.0), "");
        assert_eq!(linear.label(10.0, 10.0), "10 %");
        assert_eq!(linear.label(2.5, 0.5), "2.5 %");
        // the linear axis marks round steps of percent up to the bars
        let marks = linear.marks(false)(egui_plot::GridInput {
            bounds: (0.0, linear.axis_top),
            base_step_size: 0.5,
        });
        assert!(
            marks
                .iter()
                .all(|mark| (mark.value / 5.0).fract() == 0.0 && mark.value <= linear.bars_top)
        );
        assert!(marks.len() > 2);
        // a small share is marked in fractions of a percent
        let small = CountAxis::new(CountScale::Linear, 2.0, 0.01, 200.0, 0.0);
        let marks = small.marks(false)(egui_plot::GridInput {
            bounds: (0.0, small.axis_top),
            base_step_size: 0.05,
        });
        let labels: Vec<String> = (marks.iter())
            .map(|mark| small.label(mark.value, mark.step_size))
            .collect();
        assert_eq!(labels, ["", "0.5 %", "1.0 %", "1.5 %", "2.0 %"]);
    }

    #[test]
    fn the_tooltip_shows_the_share_and_the_count_of_each_trace() {
        let counted = Counted {
            bins: Bins {
                start: 0.0,
                width: 1.0,
                count: 3,
            },
            traces: vec![
                Counts {
                    bins: vec![1, 2, 3],
                    ..Counts::default()
                },
                // samples outside the bins count towards all samples
                Counts {
                    bins: vec![0, 2, 0],
                    below: 3,
                    above: 3,
                    ..Counts::default()
                },
            ],
            single: false,
        };
        let bin = |index| Bin {
            index,
            counted: &counted,
            unit: None,
            format: DisplayFormat::Default,
        };
        let [first, second] = [&counted.traces[0], &counted.traces[1]];
        assert_eq!(bin(1).share_texts(first), ["33.3 %", "2 of 6"]);
        assert_eq!(bin(1).share_texts(second), ["25 %", "2 of 8"]);
        assert_eq!(bin(0).share_texts(second), ["0 %", "0 of 8"]);
        assert_eq!(first.shares(), [100.0 / 6.0, 100.0 / 3.0, 50.0]);
    }

    #[test]
    fn mean_and_sigma_count_each_binned_sample_once() {
        assert_eq!(Histogram::default().markers, Markers::Off);
        let bins = Bins {
            start: 0.0,
            width: 1.0,
            count: 5,
        };
        let counts = count([1.0, 2.0, 3.0, 4.0].into_iter(), &bins, None);
        assert_eq!(counts.mean, Some(2.5));
        // the population standard deviation, divided by the number of samples
        assert!((counts.std_dev.unwrap() - 1.25f64.sqrt()).abs() < 1.0e-12);

        // a sample that holds long counts once, unlike in the stats of a plot
        let mut model = Model::default();
        append(&mut model, "a", (0.0, 1.0), &floats([0.0, 10.0]));
        append(&mut model, "a", (2.0, 1.0), &floats([10.0]));
        append(&mut model, "a", (19.0, 1.0), &floats([0.0]));
        let time = live(&model);
        let counted = counted(&histogram_of(&["a"]), &model, &time);
        let mean = counted.traces[0].mean.unwrap();
        assert_eq!(mean, 5.0);
        let (start, end) = time.range().unwrap();
        let window = (time.timestamp(start), time.timestamp(end));
        let series = model.traces.series("a").unwrap();
        let timed = Stats::of_series(&series, window, None).unwrap();
        assert!((timed.mean - mean).abs() > 1.0, "{} {mean}", timed.mean);
    }

    #[test]
    fn markers_are_a_dot_or_a_dot_and_a_whisker_in_a_row_per_trace() {
        let mut model = Model::default();
        append(&mut model, "a", (0.0, 1.0), &floats([1.0, 2.0, 3.0, 4.0]));
        append(&mut model, "b", (0.0, 1.0), &floats([5.0; 4]));
        let time = live(&model);
        let mut histogram = histogram_of(&["a", "b"]);
        let counting = histogram.counting(&model, &time, &NO_PLOTS, 1000.0);
        let counted = counting.counted.as_ref().unwrap();
        assert!(histogram.markers(&counting.shown, counted).is_empty());
        histogram.markers = Markers::Mean;
        let markers = histogram.markers(&counting.shown, counted);
        assert_eq!(
            markers.iter().map(|marker| marker.row).collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(markers.iter().all(|marker| marker.whisker.is_none()));
        histogram.markers = Markers::MeanSigma;
        let markers = histogram.markers(&counting.shown, counted);
        let sigma = 1.25f64.sqrt();
        assert_eq!(markers[0].mean, 2.5);
        assert_eq!(markers[0].whisker, Some((2.5 - sigma, 2.5 + sigma)));
        // a trace without spread has no whisker
        assert_eq!((markers[1].mean, markers[1].whisker), (5.0, None));
    }

    #[test]
    fn upright_values_run_along_x_and_sideways_along_y() {
        let mut histogram = Histogram::default();
        assert_eq!(histogram.orientation, Orientation::Upright);
        assert_eq!(histogram.axis_side, AxisSide::Left);
        assert_eq!(histogram.point(1.0, 5.0), [1.0, 5.0]);
        assert_eq!(histogram.movable(false), [true, false]);
        histogram.orientation = Orientation::Sideways;
        assert_eq!(histogram.point(1.0, 5.0), [5.0, 1.0]);
        assert_eq!(histogram.movable(false), [false, true]);
        assert_eq!(histogram.movable(true), [false, false]);
    }

    /// The plots of a layout with the plot 7, "Motor speed", showing `y_range`.
    fn plots(y_range: (f64, f64)) -> Plots {
        Plots(vec![PlotInfo {
            id: ViewId(7),
            name: "Motor speed".to_owned(),
            y_range: Some(y_range),
        }])
    }

    #[test]
    fn a_linked_histogram_bins_the_y_range_of_the_plot_in_either_orientation() {
        let mut model = Model::default();
        append(
            &mut model,
            "a",
            (0.0, 0.1),
            &floats((0..200).map(f64::from)),
        );
        let time = live(&model);
        let mut histogram = Histogram {
            orientation: Orientation::Sideways,
            link: Some(ViewId(7)),
            // its own range is ignored while following
            range: Some((0.0, 1.0)),
            ..histogram_of(&["a"])
        };
        let plots = plots((100.0, 150.0));
        for (mode, count) in [
            (BinMode::Auto, None),
            (BinMode::Number, Some(9)),
            (BinMode::Width, None),
        ] {
            histogram.bins = mode;
            histogram.bin_count = 9;
            histogram.bin_width = 5.0;
            let counting = histogram.counting(&model, &time, &plots, 500.0);
            assert_eq!(counting.fixed, Some((100.0, 150.0)));
            let counted = counting.counted.unwrap();
            let bins = counted.bins;
            assert!(
                bins.start <= 100.0 && bins.edge(bins.count) >= 150.0,
                "{mode:?} {bins:?}"
            );
            if let Some(count) = count {
                assert_eq!(bins.count, count);
            }
            let counts = &counted.traces[0];
            // all 200 samples are in the last 20 s
            assert_eq!((counts.below, total(counts), counts.above), (100, 51, 49));
        }
        // upright, it bins the range of the plot too
        histogram.orientation = Orientation::Upright;
        assert_eq!(histogram.followed(&plots), Some((100.0, 150.0)));
        let counting = histogram.counting(&model, &time, &plots, 500.0);
        assert_eq!(counting.fixed, Some((100.0, 150.0)));
        // a plot not of the layout is not followed, the own range is binned again
        histogram.link = Some(ViewId(8));
        assert_eq!(histogram.followed(&plots), None);
        let counting = histogram.counting(&model, &time, &plots, 500.0);
        assert_eq!(counting.fixed, Some((0.0, 1.0)));
    }

    /// Drags in `harness` from `from` to `to` in steps.
    fn drag(harness: &mut Harness<'_, State>, from: egui::Pos2, to: egui::Pos2) {
        harness.hover_at(from);
        harness.run();
        harness.drag_at(from);
        harness.run();
        for step in 1..=5 {
            harness.hover_at(from + (to - from) * step as f32 / 5.0);
            harness.run();
        }
        harness.drop_at(to);
        harness.run();
    }

    #[test]
    fn a_linked_plot_sets_the_values_which_do_not_pan_or_zoom() {
        for orientation in Orientation::ALL {
            let mut model = Model::default();
            append(
                &mut model,
                "a",
                (0.0, 0.1),
                &floats((0..200).map(f64::from)),
            );
            let histogram = Histogram {
                orientation,
                link: Some(ViewId(7)),
                value_view: Some((0.0, 10.0)),
                ..histogram_of(&["a"])
            };
            let mut state = State::new(histogram, model);
            state.plots = plots((100.0, 150.0));
            let mut harness = harness(state, false);
            harness.run();
            let values = |harness: &Harness<'_, State>| {
                harness
                    .state()
                    .histogram
                    .last
                    .as_deref()
                    .copied()
                    .unwrap()
                    .values
            };
            assert_eq!(values(&harness), (100.0, 150.0), "{orientation:?}");
            drag(
                &mut harness,
                egui::pos2(200.0, 100.0),
                egui::pos2(100.0, 200.0),
            );
            super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
            assert_eq!(values(&harness), (100.0, 150.0), "{orientation:?}");
            // as the plot is zoomed or panned
            harness.state_mut().plots = plots((120.0, 130.0));
            harness.run();
            assert_eq!(values(&harness), (120.0, 130.0), "{orientation:?}");
        }
    }

    /// A harness showing the properties of `histogram` in a layout with the plots of
    /// [`plots`].
    fn properties_harness(histogram: Histogram) -> Harness<'static, Histogram> {
        let model = Model::default();
        let plots = plots((0.0, 1.0));
        Harness::builder()
            .with_size(egui::vec2(300.0, 600.0))
            .build_ui_state(
                move |ui, histogram: &mut Histogram| {
                    let connected = Default::default();
                    let context = PropertiesContext {
                        model: &model,
                        connected: &connected,
                        plots: &plots,
                    };
                    histogram.properties_ui(ui, &context);
                },
                histogram,
            )
    }

    #[test]
    fn the_link_is_chosen_in_either_orientation_and_greys_the_range() {
        let link = |name: &'static str| {
            move |node: &egui_kittest::kittest::AccessKitNode<'_>| {
                node.role() == egui::accesskit::Role::ComboBox
                    && node.value().as_deref() == Some(name)
                    && !node.is_disabled()
            }
        };
        let mut harness = properties_harness(Histogram::default());
        harness.run();
        harness.get_by(link("None")).click();
        harness.run();
        harness.get_by_label("Motor speed").click();
        harness.run();
        harness.run();
        assert_eq!(harness.state().link, Some(ViewId(7)));
        // the plot sets the range
        assert!(harness.get_by_label("Fixed").accesskit_node().is_disabled());

        // turning sideways keeps the link working
        harness.get_by_label("Sideways").click();
        harness.run();
        harness.run();
        assert_eq!(harness.state().link, Some(ViewId(7)));
        harness.get_by(link("Motor speed"));
        harness.get_by_label("Axis side");
        assert!(harness.get_by_label("Fixed").accesskit_node().is_disabled());

        // unlinked, the own range is back
        harness.get_by(link("Motor speed")).click();
        harness.run();
        harness.get_by_label("None").click();
        harness.run();
        harness.run();
        assert_eq!(harness.state().link, None);
        assert!(!harness.get_by_label("Fixed").accesskit_node().is_disabled());
    }

    /// A harness showing a histogram of `pose.x` in m and `pose.y` in rad.
    fn pose_harness() -> Harness<'static, State> {
        let state = State::new(histogram_of(&["pose.x", "pose.y"]), pose("m", "m"));
        let mut harness = harness(state, false);
        harness.run();
        harness
    }

    /// The rect of the tooltip shown in `harness`.
    fn tooltip_rect(harness: &Harness<'_, State>) -> egui::Rect {
        let layers = harness
            .ctx
            .memory(|memory| memory.areas().visible_layer_ids());
        let tooltip = (layers.into_iter())
            .find(|layer| layer.order == egui::Order::Tooltip)
            .expect("a tooltip");
        egui::AreaState::load(&harness.ctx, tooltip.id)
            .expect("the area of the tooltip")
            .rect()
    }

    #[test]
    fn the_tooltip_is_as_wide_as_its_texts_wherever_it_opens() {
        let state = State::new(histogram_of(&["pose.x", "pose.y"]), pose("m", "m"));
        let mut harness = sized_harness(state, false, egui::vec2(1000.0, 600.0));
        harness.run();
        for x in [150.0, 400.0, 650.0] {
            // away, so that the tooltip opens anew
            harness.hover_at(egui::pos2(999.0, 599.0));
            harness.run();
            harness.hover_at(egui::pos2(x, 200.0));
            harness.run_steps(40);
            let tooltip = tooltip_rect(&harness);
            let texts = (harness.get_all_by_label_contains(" of "))
                .map(|node| node.rect())
                .reduce(|a, b| a.union(b))
                .expect("the counts");
            // the counts end at the tooltip's margin
            assert!(
                tooltip.right() - texts.right() < 16.0,
                "the counts end at {}, the tooltip at {}",
                texts.right(),
                tooltip.right()
            );
            assert!(tooltip.width() < 250.0, "{} wide", tooltip.width());
        }
    }

    #[test]
    fn hovering_a_bin_shows_its_range_and_the_shares_and_counts_of_the_traces() {
        let mut harness = pose_harness();
        let at = egui::pos2(150.0, 150.0);
        harness.hover_at(at);
        harness.run_steps(40);
        let state = harness.state();
        let index = state.histogram.hovered.expect("a bin under the pointer");
        let bins = state.histogram.last.as_deref().copied().unwrap().bins;
        let range = format!(
            "{} to {} m",
            write_value(bins.edge(index), bins.width, DisplayFormat::Default),
            write_value(bins.edge(index + 1), bins.width, DisplayFormat::Default)
        );
        harness.get_by_label(&range);
        // the names of the traces in the legend and the tooltip
        assert_eq!(harness.get_all_by_label("x").count(), 2);
        assert_eq!(harness.get_all_by_label("y").count(), 2);
        // the share and the count of each trace
        assert_eq!(harness.get_all_by_label_contains(" %").count(), 2);
        assert_eq!(harness.get_all_by_label_contains(" of ").count(), 2);

        harness.hover_at(egui::pos2(399.0, 299.0));
        harness.run();
        assert_eq!(harness.state().histogram.hovered, None);
    }

    #[test]
    fn the_wheel_zooms_and_a_drag_pans_the_values_without_binning_anew() {
        let mut harness = pose_harness();
        let state = harness.state();
        let (range, cursor) = (state.time.range(), state.time.cursor());
        let last = state.histogram.last.as_deref().copied().unwrap();
        let (low, high) = last.values;
        assert!(state.histogram.value_view.is_none());

        super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
        let state = harness.state();
        let (zoomed_low, zoomed_high) = state.histogram.value_view.unwrap();
        assert!(zoomed_high - zoomed_low < (high - low) * 0.9);
        assert_eq!(
            state.histogram.last.as_deref().copied().unwrap().bins,
            last.bins
        );
        assert_eq!((state.time.range(), state.time.cursor()), (range, cursor));
        // the counts axis fits the tallest bin in view
        let top = state.histogram.share_top.unwrap();
        assert!(top > 0.0 && top <= 100.0);

        drag(
            &mut harness,
            egui::pos2(150.0, 150.0),
            egui::pos2(250.0, 140.0),
        );
        let state = harness.state();
        let (panned_low, panned_high) = state.histogram.value_view.unwrap();
        assert!(panned_low < zoomed_low);
        assert!(((panned_high - panned_low) - (zoomed_high - zoomed_low)).abs() < 1.0e-6);
        assert_eq!(
            state.histogram.last.as_deref().copied().unwrap().bins,
            last.bins
        );
        assert_eq!((state.time.range(), state.time.cursor()), (range, cursor));

        let at = egui::pos2(200.0, 150.0);
        harness.hover_at(at);
        for pressed in [true, false, true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.run();
        }
        let state = harness.state();
        assert!(state.histogram.value_view.is_none());
        assert_eq!(
            state.histogram.last.as_deref().copied().unwrap().values,
            (low, high)
        );
        assert_eq!((state.time.range(), state.time.cursor()), (range, cursor));
    }

    #[test]
    fn settings_are_saved_with_defaults_for_those_missing() {
        let histogram: Histogram = serde_json::from_str(r#"{"traces": ["a"]}"#).unwrap();
        assert_eq!(histogram.bins, BinMode::Auto);
        assert_eq!(histogram.bin_count, 20);
        assert_eq!(histogram.link, None);
        let json = serde_json::to_string(&Histogram::default()).unwrap();
        assert!(!json.contains("link"), "{json}");
        let histogram = Histogram {
            bins: BinMode::Width,
            bin_width: 0.5,
            bin_count: 12,
            range: Some((1.0, 2.0)),
            counts: CountScale::Log,
            orientation: Orientation::Sideways,
            axis_side: AxisSide::Right,
            link: Some(ViewId(3)),
            markers: Markers::MeanSigma,
            ..histogram_of(&["a"])
        };
        let loaded: Histogram =
            serde_json::from_str(&serde_json::to_string(&histogram).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&loaded).unwrap(),
            serde_json::to_value(&histogram).unwrap()
        );
    }

    #[test]
    fn bins_of_whole_widths_over_integers_are_centred_on_them() {
        let auto = Histogram::default();
        let bins = auto.make_bins((5.0, 6.0), false, 100, 0.0, true);
        assert_eq!((bins.start, bins.width, bins.count), (4.5, 1.0, 2));
        // the bar of 5 is where it was as the only value
        let single = auto.make_bins((5.0, 5.0), false, 100, 0.0, true);
        assert_eq!((single.start, single.width), (4.5, 1.0));
        let width = Histogram {
            bins: BinMode::Width,
            bin_width: 2.0,
            ..Histogram::default()
        };
        // no integer on an edge, two in each bin
        let bins = width.make_bins((3.0, 9.0), false, 100, 0.0, true);
        assert_eq!((bins.start, bins.edge(bins.count)), (1.5, 9.5));
        assert_eq!(
            (bins.index(3.0), bins.index(4.0), bins.index(5.0)),
            (0, 1, 1)
        );
        assert_eq!(bins.index(9.0), 3);
        // floats and widths below 1 are not
        assert_eq!(
            width.make_bins((3.0, 9.0), false, 100, 0.0, false).start,
            2.0
        );
        let narrow = Histogram {
            bin_width: 0.5,
            ..width
        };
        assert_eq!(
            narrow.make_bins((3.2, 9.0), false, 100, 0.0, true).start,
            3.0
        );
        // a fixed range of integers keeps its last value in the last bin
        let bins = Histogram::default().make_bins((0.0, 10.0), true, 10_000, 0.0, true);
        assert_eq!(
            (bins.start, bins.edge(bins.count), bins.index(10.0)),
            (-0.5, 10.5, 10)
        );
    }

    #[test]
    fn infinite_samples_are_counted_beyond_the_bins_also_without_a_fixed_range() {
        let mut model = Model::default();
        let mut values = floats([1.0, 2.0, 3.0, f64::INFINITY, f64::NEG_INFINITY]);
        values.push(Data::Float(f64::INFINITY));
        append(&mut model, "a", (0.0, 1.0), &values);
        let time = live(&model);
        let counted = counted(&histogram_of(&["a"]), &model, &time);
        let counts = &counted.traces[0];
        assert_eq!((counts.below, total(counts), counts.above), (1, 3, 2));
        assert_eq!(counts.mean, Some(2.0));
        assert_eq!(counted.outside(), (1, 2));
    }

    #[test]
    fn a_link_to_a_plot_without_a_range_yet_turns_off_the_own_range_and_zoom() {
        let mut model = Model::default();
        append(
            &mut model,
            "a",
            (0.0, 0.1),
            &floats((0..200).map(f64::from)),
        );
        let histogram = Histogram {
            orientation: Orientation::Sideways,
            link: Some(ViewId(7)),
            range: Some((0.0, 1.0)),
            ..histogram_of(&["a"])
        };
        let plots = Plots(vec![PlotInfo {
            id: ViewId(7),
            name: "Motor speed".to_owned(),
            y_range: None,
        }]);
        let time = live(&model);
        let counting = histogram.counting(&model, &time, &plots, 500.0);
        assert!(counting.following);
        assert_eq!(counting.fixed, None);
        let mut state = State::new(histogram.clone(), model);
        state.plots = plots.clone();
        let mut harness = harness(state, false);
        harness.run();
        super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
        assert_eq!(harness.state().histogram.value_view, None);
        // greyed in the properties
        let model = Model::default();
        let mut properties = Harness::builder()
            .with_size(egui::vec2(300.0, 600.0))
            .build_ui_state(
                move |ui, histogram: &mut Histogram| {
                    let connected = Default::default();
                    let context = PropertiesContext {
                        model: &model,
                        connected: &connected,
                        plots: &plots,
                    };
                    histogram.properties_ui(ui, &context);
                },
                histogram,
            );
        properties.run();
        assert!(
            properties
                .get_by_label("Fixed")
                .accesskit_node()
                .is_disabled()
        );
        properties.get_by_label("Mode");
    }

    #[test]
    fn the_legend_box_and_the_outside_counts_are_clear_of_the_bars() {
        for orientation in [Orientation::Upright, Orientation::Sideways] {
            let histogram = Histogram {
                orientation,
                markers: Markers::MeanSigma,
                // most samples lie above the range
                range: Some((0.0, 6.0)),
                ..histogram_of(&["pose.x", "pose.y"])
            };
            let mut harness = harness(State::new(histogram, pose("m", "m")), false);
            harness.run();
            harness.run();
            let last = harness.state().histogram.last.as_deref().copied().unwrap();
            for name in ["x", "y", "m"] {
                let rect = harness.get_by_label(name).rect();
                assert!(
                    !rect.intersects(last.bars),
                    "{orientation:?} {name} {rect:?} {:?}",
                    last.bars
                );
                assert!(last.frame.contains_rect(rect), "{orientation:?} {name}");
            }
        }
    }

    #[test]
    fn the_room_beyond_the_bars_holds_the_markers_the_outside_counts_and_the_legend() {
        let room = Room {
            orientation: Orientation::Upright,
            markers: 23.0,
            outside: Some(egui::vec2(40.0, LEGEND_ROW)),
            above: true,
            legend: Some(Legend {
                size: egui::vec2(60.0, 50.0),
                lines: None,
            }),
        };
        assert_eq!(
            room.length(),
            23.0 + OVERLAY_INSET + LEGEND_ROW + 50.0 + MARKER_GAP
        );
        let sideways = Room {
            orientation: Orientation::Sideways,
            ..room.clone()
        };
        assert_eq!(sideways.length(), 23.0 + OVERLAY_INSET + 60.0 + MARKER_GAP);
        let empty = Room {
            markers: 0.0,
            outside: None,
            legend: None,
            ..room.clone()
        };
        assert_eq!(empty.length(), 0.0);
        // the legend goes below the count of the samples above, at the right
        let transform = egui_plot::PlotTransform::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)),
            egui_plot::PlotBounds::from_min_max([0.0, 0.0], [1.0, 1.0]),
            egui::Vec2b::FALSE,
        );
        let overlay = Overlay {
            painter: egui::Painter::new(
                egui::Context::default(),
                egui::LayerId::background(),
                egui::Rect::EVERYTHING,
            ),
            transform,
            room,
        };
        let legend = overlay.legend_rect().unwrap();
        let [_, (above, _)] = overlay.outside_positions();
        assert_eq!(legend.right(), 400.0 - OVERLAY_INSET);
        assert_eq!(legend.top(), above.y + LEGEND_ROW);
        assert_eq!(above.y, 23.0 + OVERLAY_INSET);
    }

    /// The values and steps of the labelled marks of `marks`, those with a step of at
    /// least `label`.
    fn labelled(marks: &[egui_plot::GridMark], label: f64) -> Vec<f64> {
        (marks.iter())
            .filter(|mark| mark.step_size >= label * (1.0 - 1.0e-9))
            .map(|mark| mark.value)
            .collect()
    }

    #[test]
    fn value_labels_take_one_round_step_and_stay_within_the_axis() {
        // 100 points per unit: labels 60 points apart are 1 apart at least, so 1
        let input = |bounds| egui_plot::GridInput {
            bounds,
            base_step_size: GRID_SPACING / 100.0,
        };
        let marks = round_marks(DisplayFormat::Default, 60.0, true)(input((-2.2, 2.2)));
        assert_eq!(labelled(&marks, 1.0), [-2.0, -1.0, 0.0, 1.0, 2.0]);
        // the grid lines between them divide the step, 0.2 apart
        assert!(
            marks
                .iter()
                .all(|mark| mark.step_size == 0.2 || mark.step_size >= 1.0)
        );
        assert_eq!(marks.len(), 23);
        // 25 points per unit: labels every 5, never 2 and 5 together
        let coarse = |bounds| egui_plot::GridInput {
            bounds,
            base_step_size: GRID_SPACING / 25.0,
        };
        let marks = round_marks(DisplayFormat::Default, 60.0, true)(coarse((-1.0, 23.0)));
        assert_eq!(labelled(&marks, 5.0), [0.0, 5.0, 10.0, 15.0, 20.0]);
        // the grid lines between, too close for labels, all take the smallest step
        let mut between = marks.iter().filter(|mark| mark.value % 5.0 != 0.0);
        assert!(between.clone().count() > 10);
        assert!(between.all(|mark| mark.step_size == 1.0));
        // the powers of ten are stronger
        let ten = marks.iter().find(|mark| mark.value == 10.0).unwrap();
        assert_eq!(ten.step_size, 10.0);
        // a label at an end of the axis would run past it
        let marks = round_marks(DisplayFormat::Default, 60.0, true)(input((-2.0, 2.0)));
        assert_eq!(labelled(&marks, 1.0), [-1.0, 0.0, 1.0]);
        let marks = round_marks(DisplayFormat::Default, 40.0, false)(input((-2.0, 2.05)));
        assert_eq!(
            labelled(&marks, 0.5),
            [-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5]
        );
    }

    #[test]
    fn count_labels_stay_within_the_axis() {
        // a point per percent, the axis ends at about 110: the label of 100 % fits on a
        // vertical axis, but is too wide for the end of a horizontal one
        let axis = CountAxis::new(CountScale::Linear, 95.0, 1.0, 300.0, 20.0);
        let marks = |horizontal| -> Vec<f64> {
            let input = egui_plot::GridInput {
                bounds: (0.0, axis.axis_top),
                base_step_size: GRID_SPACING,
            };
            (axis.marks(horizontal)(input).iter())
                .map(|mark| mark.value)
                .collect()
        };
        assert_eq!(marks(false), [0.0, 50.0, 100.0], "{}", axis.axis_top);
        assert_eq!(marks(true), [0.0, 50.0]);
    }

    #[test]
    fn outlines_run_over_the_bins_that_are_not_empty() {
        let bins = Bins {
            start: 0.0,
            width: 1.0,
            count: 6,
        };
        let axis = CountAxis::new(CountScale::Linear, 3.0, 1.0, 100.0, 0.0);
        let runs = outline_runs(&[0.0, 2.0, 3.0, 0.0, 1.0, 0.0], &bins, axis);
        assert_eq!(
            runs,
            [
                vec![
                    [1.0, 0.0],
                    [1.0, 2.0],
                    [2.0, 2.0],
                    [2.0, 3.0],
                    [3.0, 3.0],
                    [3.0, 0.0]
                ],
                vec![[4.0, 0.0], [4.0, 1.0], [5.0, 1.0], [5.0, 0.0]],
            ]
        );
        assert!(outline_runs(&[0.0; 6], &bins, axis).is_empty());
        let last = outline_runs(&[0.0, 0.0, 0.0, 0.0, 0.0, 4.0], &bins, axis);
        assert_eq!(last, [vec![[5.0, 0.0], [5.0, 4.0], [6.0, 4.0], [6.0, 0.0]]]);
    }

    #[test]
    fn the_title_legend_cuts_the_names_and_keeps_the_unit() {
        let mut model = Model::default();
        for path in [
            "robot/shoulder.position_error",
            "robot/elbow.position_error",
        ] {
            append(&mut model, path, (0.0, 1.0), &floats([1.0]));
            model.traces.set_unit(path, "rad");
        }
        let histogram = histogram_of(&[
            "robot/shoulder.position_error",
            "robot/elbow.position_error",
        ]);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(180.0, 30.0))
            .build_ui_state(
                |ui, state: &mut State| {
                    let mut context = ViewContext {
                        model: &state.model,
                        time: &mut state.time,
                        invoker: &mut state.invoker,
                        formats: &state.formats,
                        titled: true,
                        plots: &state.plots,
                    };
                    ui.horizontal(|ui| state.histogram.title_ui(ui, &mut context));
                },
                State::new(histogram, model),
            );
        harness.run();
        let unit = harness.get_by_label("rad").rect();
        assert!(unit.right() <= 180.0, "{unit:?}");
        // the first name is cut off before the unit, the second one is left out
        let labels: Vec<egui::Rect> = (harness.get_all_by_role(egui::accesskit::Role::Label))
            .map(|node| node.rect())
            .collect();
        assert_eq!(labels.len(), 2, "{labels:?}");
        let first = labels[0];
        assert!(first.right() <= unit.left(), "{first:?} {unit:?}");
    }

    #[test]
    fn the_value_axis_of_a_sideways_histogram_is_on_the_side_chosen() {
        for (side, on_left) in [(AxisSide::Left, true), (AxisSide::Right, false)] {
            let histogram = Histogram {
                orientation: Orientation::Sideways,
                axis_side: side,
                ..histogram_of(&["pose.x"])
            };
            let mut harness = harness(State::new(histogram, pose("m", "m")), false);
            harness.run();
            harness.run();
            let frame = harness
                .state()
                .histogram
                .last
                .as_deref()
                .copied()
                .unwrap()
                .frame;
            let (left, right) = (frame.left(), 400.0 - frame.right());
            assert_eq!(left > right, on_left, "{side:?} {frame:?}");
            assert!(
                left.max(right) >= VALUE_AXIS_THICKNESS,
                "{side:?} {frame:?}"
            );
        }
    }

    #[test]
    fn plots_of_the_same_name_are_told_apart_by_a_number() {
        let plot = |id, name: &str| PlotInfo {
            id: ViewId(id),
            name: name.to_owned(),
            y_range: None,
        };
        let plots = Plots(vec![plot(1, "Speed"), plot(2, "Pose"), plot(3, "Speed")]);
        assert_eq!(plot_names(&plots), ["Speed (1)", "Pose", "Speed (2)"]);
    }

    #[test]
    fn the_legend_of_many_traces_wraps_across_the_plot_clear_of_the_bars() {
        assert_eq!(
            wrap(&[30.0, 30.0, 30.0, 50.0], 100.0, 10.0),
            [vec![0, 1], vec![2, 3]]
        );
        assert_eq!(wrap(&[120.0, 30.0], 100.0, 10.0), [vec![0], vec![1]]);

        let mut model = Model::default();
        let paths: Vec<String> = (0..9).map(|index| format!("trace{index}")).collect();
        for (index, path) in paths.iter().enumerate() {
            let values = (0..100).map(|i| f64::from(i % 10) + index as f64);
            append(&mut model, path, (0.0, 0.1), &floats(values));
        }
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let histogram = Histogram {
            markers: Markers::Mean,
            ..histogram_of(&paths)
        };
        let mut harness = harness(State::new(histogram, model), false);
        harness.run();
        harness.run();
        let last = harness.state().histogram.last.as_deref().copied().unwrap();
        let mut lines = vec![];
        for path in &paths {
            let rect = harness.get_by_label(path).rect();
            assert!(
                !rect.intersects(last.bars),
                "{path} {rect:?} {:?}",
                last.bars
            );
            if !lines
                .iter()
                .any(|&y: &f32| (y - rect.center().y).abs() < 1.0)
            {
                lines.push(rect.center().y);
            }
        }
        assert!(lines.len() <= 3, "{lines:?}");
    }

    #[test]
    fn infinite_samples_alone_show_their_counts_on_empty_axes() {
        let mut model = Model::default();
        let values = floats([f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY]);
        append(&mut model, "a", (0.0, 1.0), &values);
        let histogram = histogram_of(&["a"]);
        let time = live(&model);
        let counting = histogram.counting(&model, &time, &NO_PLOTS, 500.0);
        assert!(counting.counted.is_none());
        assert_eq!(counting.outside, (1, 2));
        let mut harness = harness(State::new(histogram, model), false);
        harness.run();
        assert!(harness.query_by_label(NO_SAMPLES).is_none());
    }
}
