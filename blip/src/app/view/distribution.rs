//! A distribution: how the values of the checked traces spread over the shown window,
//! as bars, a histogram counted in bins shared by all traces and drawn over each other
//! in their colours, as boxes, a box plot of each trace in a row of its own, or both.

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

mod boxes;

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
/// The most of the counts axis the room beyond the bars takes, for the markers and the
/// counts outside the bins.
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
/// The room between the edges of the plot and the texts on it.
const OVERLAY_INSET: f32 = 6.0;
/// The room egui_plot leaves on each side of the labels of a y axis.
const NAME_MARGIN: f32 = 4.0;
/// The legend dots, as in the legends of plots.
const DOT_SIZE: f32 = 8.0;
/// The height of the counts of the samples outside the bins.
const TEXT_ROW: f32 = 18.0;
/// The least width left for a name in the legend of the title row, else it ends in "…".
const MIN_NAME_WIDTH: f32 = 16.0;
/// The least width of the value axis of a sideways distribution, as of the y axis of a
/// plot, so that both line up.
const VALUE_AXIS_THICKNESS: f32 = 50.0;

/// The hint of a distribution without traces.
const CHECK_TRACES: &str = "Check the traces to show\nin the tree on the left";
/// The hint of a distribution without samples in the shown window.
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
    /// The side of the value axis of a sideways distribution.
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

named_enum! {
    /// What a distribution shows of the samples of each trace.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum Show {
        /// The histogram, with the markers.
        #[default]
        Bars => "Bars",
        /// A box plot, a row per trace.
        Boxes => "Boxes",
        /// The histogram with a box per trace in the strip of the markers.
        Both => "Both",
    }
}

named_enum! {
    /// Where the whiskers of the boxes end.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum Whiskers {
        /// At the furthest samples within 1.5 times the interquartile range of the box.
        #[default]
        Iqr => "1.5×IQR",
        MinMax => "Min–max",
        /// At the 5th and the 95th percentile.
        P5 => "5–95 %",
        /// At the 1st and the 99th percentile.
        P1 => "1–99 %",
    }
}

impl Whiskers {
    /// What the choice means, for its hover text.
    fn hint(self) -> &'static str {
        match self {
            Whiskers::Iqr => "Up to the last sample within 1.5 times the box length",
            Whiskers::MinMax => "Down to the lowest and up to the highest sample",
            Whiskers::P5 => "From the 5th to the 95th percentile",
            Whiskers::P1 => "From the 1st to the 99th percentile",
        }
    }
}

/// How the values of the checked traces spread over the shown window: counted in bins
/// of their values, as boxes of their quartiles, or both.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Distribution {
    /// Paths of the shown traces.
    pub traces: BTreeSet<String>,
    pub show: Show,
    pub whiskers: Whiskers,
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
    /// The plot linked to, whose traces the distribution shows and whose y range its value
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
    /// The bin or the row under the pointer in the last frame.
    #[serde(skip)]
    hovered: Option<Hovered>,
    /// The boxes of the last frame, kept while their samples stay the same.
    #[serde(skip)]
    boxes: Option<Box<boxes::Cache>>,
}

/// What is under the pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hovered {
    /// The bin of this index.
    Bin(usize),
    /// The row of the box of the trace of this index of the shown traces.
    Row(usize),
}

impl Default for Distribution {
    fn default() -> Self {
        Self {
            traces: BTreeSet::new(),
            show: Show::default(),
            whiskers: Whiskers::default(),
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
            boxes: None,
        }
    }
}

/// What a distribution showed in a frame.
#[derive(Clone, PartialEq, Debug)]
struct Last {
    /// The lowest and the highest value of the samples.
    data: (f64, f64),
    /// The range of the value axis.
    values: (f64, f64),
    bins: Bins,
    /// The plot without its axes.
    frame: egui::Rect,
    /// The rows of the boxes, in Boxes and Both.
    rows: Option<boxes::Rows>,
    /// The names of the rows on the category axis of Boxes, cut to fit.
    names: Vec<String>,
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

impl Distribution {
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

    /// Whether the distribution follows a plot of `plots`: while it is linked to one.
    pub fn following(&self, plots: &Plots) -> bool {
        self.link.and_then(|id| plots.get(id)).is_some()
    }

    /// The y range of the plot the distribution follows, if it shows one.
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
            window,
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

/// What a distribution counts in a frame, see [`Distribution::counting`].
struct Counting<'a> {
    /// The shown window in nanoseconds, `None` before there is one.
    window: Option<(u64, u64)>,
    shown: Vec<Shown<'a>>,
    /// The lowest and the highest value of the shown samples, `None` without one.
    data: Option<(f64, f64)>,
    /// Whether a plot is followed, see [`Distribution::following`].
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

/// What a distribution counted in a frame.
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

    /// The marks of the axis up to the bars, so that the room beyond them stays clear:
    /// those of egui_plot, or a mark at each power of ten above the floor.
    fn marks(self) -> impl Fn(egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
        let linear = egui_plot::log_grid_spacer(10);
        move |input| match self.scale {
            CountScale::Linear => (linear(input).into_iter())
                .filter(|mark| mark.value <= self.bars_top)
                .collect(),
            CountScale::Log => (self.floor.floor() as i64 + 1..=self.bars_top.floor() as i64)
                .map(|decade| egui_plot::GridMark {
                    value: decade as f64,
                    step_size: 1.0,
                })
                .collect(),
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

impl super::ViewInfo for Distribution {
    const NAME: &'static str = "Distribution";
    const ICON: &'static str = egui_phosphor::regular::CHART_BAR;

    fn selectable(trace: Trace<'_>) -> Selectable {
        const NO_BOOLS: &str = "Distributions show numbers, not bools";
        match trace.samples {
            Samples::Strings(_) => Selectable::No("Distributions show numbers".into()),
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

impl Viewable for Distribution {
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
        super::properties_section(ui, "Display", |ui| {
            super::choice_row(ui, "Show", &mut self.show, &Show::CHOICES);
            super::choice_row(
                ui,
                "Orientation",
                &mut self.orientation,
                &Orientation::CHOICES,
            );
            if self.orientation == Orientation::Sideways {
                super::choice_row(ui, "Axis side", &mut self.axis_side, &AxisSide::CHOICES);
            }
            // the markers of the bars and the whiskers of the boxes take the same place
            match self.show {
                Show::Bars => {
                    super::choice_row(ui, "Markers", &mut self.markers, &Markers::CHOICES);
                }
                Show::Boxes | Show::Both => self.whiskers_row(ui),
            }
        });
        super::properties_section(ui, "Values", |ui| {
            if following {
                self.followed_range_rows(ui);
            } else {
                super::range_rows(ui, &mut self.range, fitted);
            }
        });
        // the bins and counts are kept, but play no part in the boxes alone
        if self.show == Show::Boxes {
            return;
        }
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
        });
        super::properties_section(ui, "Counts", |ui| {
            super::choice_row(ui, "Scale", &mut self.counts, &CountScale::CHOICES);
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

impl Distribution {
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

    /// Adds a row to the grid of a properties section to choose where the whiskers of
    /// the boxes end, a combo box as four choices do not fit beside the label.
    fn whiskers_row(&mut self, ui: &mut egui::Ui) {
        super::property_label(ui, "Whiskers");
        egui::ComboBox::from_id_salt("distribution whiskers")
            .selected_text(self.whiskers.name())
            .width(120.0)
            .show_ui(ui, |ui| {
                for (whiskers, name) in Whiskers::CHOICES {
                    ui.selectable_value(&mut self.whiskers, whiskers, name)
                        .on_hover_text(whiskers.hint());
                }
            })
            .response
            .on_hover_text(self.whiskers.hint());
        ui.end_row();
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
        egui::ComboBox::from_id_salt("distribution link")
            .selected_text(current)
            .width(120.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.link, None, "None");
                for (plot, name) in plots.0.iter().zip(&names) {
                    ui.selectable_value(&mut self.link, Some(plot.id), name);
                }
            })
            .response
            .on_hover_text("The plot whose traces and y range the distribution shows");
        if self.link != before {
            // the plot's range replaces the one panned or zoomed to
            self.value_view = None;
        }
        ui.end_row();
    }

    /// The room beyond the bars for the strip of the markers or boxes of `traces`
    /// traces and the counts of the samples `outside` the bins.
    fn room(&self, ui: &egui::Ui, traces: usize, (below, above): (u64, u64)) -> Room {
        let row = match (self.show, self.markers) {
            (Show::Both, _) => boxes::STRIP_ROW,
            (_, Markers::Off) => 0.0,
            _ => MARKER_ROW,
        };
        let markers = if traces > 0 && row > 0.0 {
            MARKER_INSET + traces as f32 * row
        } else {
            0.0
        };
        let outside = (below + above > 0).then(|| {
            let width = (self.outside_texts(below, above).iter())
                .map(|text| text_size(ui, text, egui::TextStyle::Body).x)
                .fold(0.0, f32::max);
            egui::vec2(width, TEXT_ROW)
        });
        Room {
            orientation: self.orientation,
            markers,
            outside,
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

    /// Shows the distribution of the checked traces over the shown window.
    fn show(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let (model, formats) = (context.model, context.formats);
        let id = ui.id().with("distribution");
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
            window,
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
        let boxes = match (self.show, window) {
            (Show::Bars, _) | (_, None) => None,
            (_, Some(window)) => Some(boxes::boxes(
                &mut self.boxes,
                (model.samples_version(), window),
                self.whiskers,
                &self.traces,
                &shown,
            )),
        };

        let paths = || self.traces.iter().map(String::as_str);
        let unit = super::common_unit(model, paths());
        let format = common_format(formats, paths());
        let room = self.room(ui, shown.len(), outside);
        let (across, names) = match self.show {
            Show::Boxes => {
                let names = self.row_names(ui, &shown, frame);
                (Across::Names(names.clone()), names)
            }
            Show::Bars | Show::Both => {
                let tallest = counted.as_ref().map(|counted| counted.tallest_in(values));
                self.share_top = tallest.map(|tallest| fit_top(self.share_top, tallest));
                let one = counted.as_ref().map_or(100.0, Counted::one_sample);
                let axis = CountAxis::new(
                    self.counts,
                    self.share_top.unwrap_or(100.0),
                    one,
                    count_length,
                    room.length(),
                );
                (Across::Counts(axis), vec![])
            }
        };

        let mut plot = self.plot(id, ui.visuals(), &across, (unit, format), following);
        // without a title row, the legend of the title row goes to a corner; the names
        // of the rows of the boxes need none
        if !context.titled && self.show != Show::Boxes {
            plot = plot.legend(egui_plot::Legend::default().position(egui_plot::Corner::RightTop));
        }
        let visuals = ui.visuals().clone();
        let this = &*self;
        let response = plot.show(ui, |plot_ui| {
            let (low, high) = values;
            let across_range = match &across {
                Across::Counts(axis) => axis.floor..=axis.axis_top,
                Across::Names(names) => 0.0..=names.len().max(1) as f64,
            };
            match this.orientation {
                Orientation::Upright => {
                    plot_ui.set_plot_bounds_x(low..=high);
                    plot_ui.set_plot_bounds_y(across_range);
                }
                Orientation::Sideways => {
                    plot_ui.set_plot_bounds_x(across_range);
                    plot_ui.set_plot_bounds_y(low..=high);
                }
            }
            let pos = plot_ui.response().hover_pos();
            let frame = *plot_ui.transform().frame();
            let row = this.rows(frame, shown.len());
            if let Some(row) = row.zip(pos).and_then(|(rows, pos)| rows.at(pos)) {
                return Some(Hovered::Row(row));
            }
            let (Across::Counts(axis), Some(counted)) = (&across, &counted) else {
                return None;
            };
            let hovered = pos.and_then(|pos| {
                let part = match this.orientation {
                    Orientation::Upright => (pos.x - frame.left()) / frame.width(),
                    Orientation::Sideways => (frame.bottom() - pos.y) / frame.height(),
                };
                counted.bins.at(low + f64::from(part) * (high - low))
            });
            this.plot_bars(plot_ui, &visuals, (&shown, counted), *axis, hovered);
            hovered.map(Hovered::Bin)
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
        let rows = self.rows(frame, shown.len());
        self.last = counted.as_ref().map(|counted| {
            Box::new(Last {
                data: data.unwrap_or(fitted),
                values,
                bins: counted.bins,
                frame,
                rows,
                names,
            })
        });

        let overlay = Overlay {
            painter: ui.painter().with_clip_rect(frame),
            transform,
            room,
        };
        if self.show != Show::Boxes {
            self.paint_outside(ui, &overlay, outside);
        }
        let any_box = boxes
            .as_ref()
            .is_some_and(|boxes| boxes.iter().any(Option::is_some));
        if let (Some(rows), Some(boxes)) = (&rows, &boxes) {
            if let Some(Hovered::Row(row)) = self.hovered {
                let hover = theme::tokens(ui.visuals()).hover;
                overlay.painter.rect_filled(rows.rect(row), 0.0, hover);
            }
            let traces = (shown.as_slice(), &boxes[..]);
            boxes::paint(&overlay.painter, ui.visuals(), rows, &transform, traces);
            if any_box || self.show == Show::Both {
                let style = match self.show {
                    Show::Both => egui::TextStyle::Small,
                    _ => egui::TextStyle::Body,
                };
                boxes::paint_notes(ui, &overlay.painter, rows, values, boxes, style);
            }
        }
        let empty = match self.show {
            Show::Boxes => !any_box,
            // the counts of the infinite samples say there are some
            Show::Bars | Show::Both => counted.is_none() && outside == (0, 0),
        };
        if let Some(counted) = &counted {
            self.paint_markers(ui, &overlay, &shown, counted);
        }
        if empty {
            let tokens = theme::tokens(ui.visuals());
            let size = text_size(ui, NO_SAMPLES, egui::TextStyle::Body);
            // on the card, so the grid does not cross it
            let rect =
                egui::Rect::from_center_size(frame.center(), size).expand2(egui::vec2(8.0, 4.0));
            (overlay.painter).rect_filled(rect, theme::CONTROL_RADIUS, tokens.card);
            let hint = egui::Label::new(egui::RichText::new(NO_SAMPLES).color(tokens.text_faint));
            ui.put(rect, hint);
        }
        match self.hovered {
            Some(Hovered::Bin(index)) => {
                let Some(counted) = &counted else { return };
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
            Some(Hovered::Row(row)) => {
                let (Some(trace), Some(boxes)) = (shown.get(row), &boxes) else {
                    return;
                };
                let tip = boxes::Tip {
                    trace,
                    stats: boxes.get(row).and_then(Option::as_ref),
                    whiskers: self.whiskers,
                    orientation: self.orientation,
                    format: formats.of(&trace.path),
                    unit: model.traces.unit(&trace.path),
                };
                response.response.on_hover_ui_at_pointer(|ui| tip.ui(ui));
            }
            None => {}
        }
    }

    /// The rows of the boxes of `traces` traces in the plot `frame`, `None` in Bars.
    fn rows(&self, frame: egui::Rect, traces: usize) -> Option<boxes::Rows> {
        match self.show {
            Show::Bars => None,
            Show::Boxes => Some(boxes::Rows::boxes(self.orientation, frame, traces)),
            Show::Both => Some(boxes::Rows::strip(self.orientation, frame, traces)),
        }
    }

    /// The names of the `shown` traces on the category axis of Boxes in a plot `frame`
    /// large, cut with "…" to at most a third of the view and, sideways, to their
    /// columns.
    fn row_names(&self, ui: &egui::Ui, shown: &[Shown<'_>], frame: egui::Vec2) -> Vec<String> {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let third = ui.available_width() / 3.0;
        let width = match self.orientation {
            // the axis keeps a margin on each side of its labels
            Orientation::Upright => third - 2.0 * NAME_MARGIN,
            Orientation::Sideways => {
                third.min(frame.x / shown.len().max(1) as f32 - 2.0 * NAME_MARGIN)
            }
        };
        (shown.iter())
            .map(|trace| boxes::cut(ui, &trace.name, width.max(0.0), &font))
            .collect()
    }

    /// The plot with the value axis and the counts axis or the names of the rows
    /// `across` it, along the axes of the orientation, the values written with `unit`
    /// and in `format`, which can be panned and zoomed along the values unless
    /// `following` a plot, whose grid it then has.
    fn plot(
        &self,
        id: egui::Id,
        visuals: &egui::Visuals,
        across: &Across,
        (unit, format): (Option<&str>, DisplayFormat),
        following: bool,
    ) -> egui_plot::Plot<'static> {
        let movable = self.movable(following);
        let value_formatter =
            move |mark: egui_plot::GridMark, _: &std::ops::RangeInclusive<f64>| {
                write_value(mark.value, mark.step_size, format)
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
        let value_axis = match self.orientation {
            Orientation::Upright => egui_plot::AxisHints::new_x(),
            // as the y axis of a plot, so that both line up
            Orientation::Sideways => egui_plot::AxisHints::new_y()
                .placement(match self.axis_side {
                    AxisSide::Left => egui_plot::HPlacement::Left,
                    AxisSide::Right => egui_plot::HPlacement::Right,
                })
                .min_thickness(VALUE_AXIS_THICKNESS),
        };
        let value_axis = label(value_axis.formatter(value_formatter));
        let across_axis = match self.orientation {
            Orientation::Upright => egui_plot::AxisHints::new_y(),
            Orientation::Sideways => egui_plot::AxisHints::new_x(),
        };
        let (across_axis, spacer): (_, Box<dyn Fn(egui_plot::GridInput) -> _>) = match across {
            Across::Counts(axis) => {
                let axis = *axis;
                let count_formatter =
                    move |mark: egui_plot::GridMark, _: &std::ops::RangeInclusive<f64>| {
                        axis.label(mark.value, mark.step_size)
                    };
                let hints = across_axis.formatter(count_formatter);
                let hints = match self.orientation {
                    Orientation::Upright => hints,
                    // shares are short, so their labels fit closer together
                    Orientation::Sideways => hints.label_spacing(40.0..=60.0),
                };
                (hints, Box::new(axis.marks()))
            }
            Across::Names(names) => {
                let rows = Categories {
                    orientation: self.orientation,
                    count: names.len(),
                };
                let names = names.clone();
                let name_formatter =
                    move |mark: egui_plot::GridMark, _: &std::ops::RangeInclusive<f64>| {
                        (rows.row(mark.value))
                            .and_then(|row| names.get(row).cloned())
                            .unwrap_or_default()
                    };
                let hints = across_axis
                    .formatter(name_formatter)
                    .tick_label_color(visuals.weak_text_color())
                    // a name is shown in any row it fits in
                    .label_spacing(0.0..=1.0);
                (hints, Box::new(move |_| rows.marks()))
            }
        };
        // no lines between the rows of the boxes
        let grid = match (across, self.orientation) {
            (Across::Counts(_), _) => [true, true],
            (Across::Names(_), Orientation::Upright) => [true, false],
            (Across::Names(_), Orientation::Sideways) => [false, true],
        };
        let plot = plot.show_grid(grid);
        match self.orientation {
            Orientation::Upright => plot
                .custom_x_axes(vec![value_axis])
                .custom_y_axes(vec![across_axis])
                .y_grid_spacer(spacer),
            Orientation::Sideways => plot
                .custom_x_axes(vec![across_axis])
                .x_grid_spacer(spacer)
                .custom_y_axes(vec![value_axis]),
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
        // the bars and outlines of a trace share its name, its entry in the legend
        let shares: Vec<Vec<f64>> = counted.traces.iter().map(Counts::shares).collect();
        for (trace, shares) in shown.iter().zip(&shares) {
            let color = theme::trace_color(visuals, trace.index);
            let wash = theme::wash(visuals, color);
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
            let chart = egui_plot::BarChart::new(&trace.name, bars)
                .color(color)
                .allow_hover(false);
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
                    egui_plot::Line::new(&trace.name, egui_plot::PlotPoints::from(points))
                        .color(color)
                        .width(OUTLINE_WIDTH)
                        .allow_hover(false),
                );
            }
        }
    }

    /// Keeps the values panned or zoomed to in `response` in [`Distribution::value_view`]
    /// instead of `shown`, until a double click fits them again.
    fn keep_value_view(
        &mut self,
        response: &egui_plot::PlotResponse<Option<Hovered>>,
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

    /// The markers of the `shown` traces of `counted`, a row each.
    fn markers(&self, shown: &[Shown<'_>], counted: &Counted) -> Vec<Marker> {
        // in Both, the boxes take their place
        if self.markers == Markers::Off || self.show != Show::Bars {
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

/// What runs across the value axis: the counts of the bars, or the names of the rows
/// of the boxes.
enum Across {
    Counts(CountAxis),
    Names(Vec<String>),
}

/// The rows of Boxes on the category axis of the plot, one unit each: from the top down
/// upright, from the left sideways.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Categories {
    orientation: Orientation,
    count: usize,
}

impl Categories {
    /// The middle of `row` on the axis.
    fn centre(self, row: usize) -> f64 {
        match self.orientation {
            Orientation::Upright => (self.count - row) as f64 - 0.5,
            Orientation::Sideways => row as f64 + 0.5,
        }
    }

    /// The row at `value` on the axis, `None` beyond the rows.
    fn row(self, value: f64) -> Option<usize> {
        let unit = value.floor();
        if unit < 0.0 || unit >= self.count as f64 {
            return None;
        }
        Some(match self.orientation {
            Orientation::Upright => self.count - 1 - unit as usize,
            Orientation::Sideways => unit as usize,
        })
    }

    /// A mark in the middle of each row, for its name.
    fn marks(self) -> Vec<egui_plot::GridMark> {
        (0..self.count)
            .map(|row| egui_plot::GridMark {
                value: self.centre(row),
                step_size: 1.0,
            })
            .collect()
    }
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
/// end, then the counts of the samples outside the bins.
#[derive(Clone, PartialEq, Debug)]
struct Room {
    orientation: Orientation,
    /// The length of the marker strip along the counts axis.
    markers: f32,
    /// The size of the larger text of the counts outside the bins, if there are any.
    outside: Option<egui::Vec2>,
}

impl Room {
    /// Its length along the counts axis, with the gap to the bars.
    fn length(&self) -> f32 {
        let texts = self.outside.map_or(0.0, |size| {
            OVERLAY_INSET
                + match self.orientation {
                    Orientation::Upright => size.y,
                    Orientation::Sideways => size.x,
                }
        });
        let gap = if self.markers > 0.0 || texts > 0.0 {
            MARKER_GAP
        } else {
            0.0
        };
        self.markers + texts + gap
    }
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
        egui::Grid::new("bin tooltip")
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
                    // as high as the row of the name, so that the texts line up
                    let height = ui.spacing().interact_size.y;
                    for (value, width) in [share, count].into_iter().zip(widths) {
                        let layout = egui::Layout::right_to_left(egui::Align::Center);
                        ui.allocate_ui_with_layout(egui::vec2(width, height), layout, |ui| {
                            ui.label(value);
                        });
                    }
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests;
