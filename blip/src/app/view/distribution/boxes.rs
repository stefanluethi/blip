//! The boxes of a distribution: the quartiles, the whiskers and the outliers of the
//! samples of each trace, in a row per trace across the value axis.

use super::{
    MARKER_INSET, MARKER_RADIUS, OUTLINE_WIDTH, OVERLAY_INSET, Orientation, Shown, Whiskers,
    nice_near, write_value,
};
use crate::app::model::Version;
use crate::app::model::numbers::DisplayFormat;
use crate::app::theme::{self, TokensExt};
use crate::app::view::tooltip::{Header, Label, Row, Tone, Tooltip, TraceName, Value};
use crate::app::view::with_unit;
use std::collections::BTreeSet;
use std::sync::Arc;

/// How thick a box is, as a part of its row, at least and at most this many points.
const BOX_SHARE: f32 = 0.4;
const MIN_BOX: f32 = 6.0;
const MAX_BOX: f32 = 24.0;
/// The width of the median line of a box.
const MEDIAN_WIDTH: f32 = 2.5;
/// The longest half of a cap of a whisker.
const MAX_CAP: f32 = 10.0;
/// The radius of the dots of the outliers, and how opaque they are.
const OUTLIER_RADIUS: f32 = 2.0;
const OUTLIER_OPACITY: f32 = 0.45;
/// The ring around the dot at the mean, in the colour of the card.
const RING_WIDTH: f32 = 1.5;
/// Outliers closer than this along the value axis are drawn once.
const OUTLIER_SPACING: f32 = 0.5;
/// The rows of the strip of Both, and the boxes in them.
pub const STRIP_ROW: f32 = 14.0;
const STRIP_BOX: f32 = 10.0;
const STRIP_OUTLINE: f32 = 1.0;
const STRIP_MEDIAN: f32 = 2.0;
const STRIP_OUTLIER_RADIUS: f32 = 1.5;
/// The digits of the values in the tooltip, as a part of the range of the samples.
const VALUE_STEPS: f64 = 1000.0;

/// The note in the row of a trace without samples.
pub const NO_SAMPLES: &str = "No samples";

/// The box of the samples of a trace: its quartiles, the ends of its whiskers and the
/// samples beyond them.
#[derive(Clone, PartialEq, Debug)]
pub struct BoxStats {
    pub samples: usize,
    pub min: f64,
    /// Where the low whisker ends.
    pub low: f64,
    pub q1: f64,
    pub median: f64,
    pub mean: f64,
    pub q3: f64,
    /// Where the high whisker ends.
    pub high: f64,
    pub max: f64,
    /// The samples beyond the whiskers, in ascending order.
    pub outliers: Vec<f64>,
}

/// The `p` quantile of the `sorted` values, linear between the order statistics (type 7
/// of Hyndman and Fan, the default of numpy).
pub fn quantile(sorted: &[f64], p: f64) -> f64 {
    let at = (sorted.len() - 1) as f64 * p;
    let below = at.floor() as usize;
    let fraction = at - below as f64;
    match sorted.get(below + 1) {
        Some(&next) if fraction > 0.0 => sorted[below] + fraction * (next - sorted[below]),
        _ => sorted[below],
    }
}

impl BoxStats {
    /// The box of `values` with `whiskers`, each value counted once, without NaN, which
    /// marks a null, and without infinite values, which a box cannot show; `None`
    /// without values.
    pub fn of(values: impl Iterator<Item = f64>, whiskers: Whiskers) -> Option<Self> {
        let mut sorted: Vec<f64> = values.filter(|value| value.is_finite()).collect();
        sorted.sort_unstable_by(f64::total_cmp);
        let (&min, &max) = (sorted.first()?, sorted.last()?);
        let quantile = |p| quantile(&sorted, p);
        let (q1, q3) = (quantile(0.25), quantile(0.75));
        let (low, high) = match whiskers {
            Whiskers::Iqr => {
                let reach = 1.5 * (q3 - q1);
                let low = sorted.iter().find(|&&value| value >= q1 - reach);
                let high = sorted.iter().rev().find(|&&value| value <= q3 + reach);
                (*low.unwrap_or(&min), *high.unwrap_or(&max))
            }
            Whiskers::MinMax => (min, max),
            Whiskers::P5 => (quantile(0.05), quantile(0.95)),
            Whiskers::P1 => (quantile(0.01), quantile(0.99)),
        };
        let outliers = (sorted.iter().copied())
            .filter(|&value| value < low || value > high)
            .collect();
        // the sum of the values minus the lowest, precise also far from zero
        let sum: f64 = sorted.iter().map(|value| value - min).sum();
        Some(Self {
            samples: sorted.len(),
            min,
            low,
            q1,
            median: quantile(0.5),
            mean: min + sum / sorted.len() as f64,
            q3,
            high,
            max,
            outliers,
        })
    }
}

/// The boxes of the shown traces, kept while their samples, the window and the whiskers
/// stay the same, as sorting the samples is slower than counting them in bins.
#[derive(Clone)]
pub struct Cache {
    key: (Version, (u64, u64), Whiskers, BTreeSet<String>),
    boxes: Arc<[Option<BoxStats>]>,
}

/// The boxes of the samples of `shown` in `window`, in nanoseconds, of the samples of
/// `version` of the traces at `paths`, from `cache` if it holds them, else computed and
/// kept in it.
pub fn boxes(
    cache: &mut Option<Box<Cache>>,
    (version, window): (Version, (u64, u64)),
    whiskers: Whiskers,
    paths: &BTreeSet<String>,
    shown: &[Shown<'_>],
) -> Arc<[Option<BoxStats>]> {
    if let Some(cache) = cache.as_ref()
        && (cache.key.0, cache.key.1, cache.key.2) == (version, window, whiskers)
        && &cache.key.3 == paths
    {
        return cache.boxes.clone();
    }
    let boxes: Arc<[Option<BoxStats>]> = (shown.iter())
        .map(|trace| BoxStats::of(super::values(&trace.series), whiskers))
        .collect();
    *cache = Some(Box::new(Cache {
        key: (version, window, whiskers, paths.clone()),
        boxes: boxes.clone(),
    }));
    boxes
}

/// How the boxes in the rows are drawn, in points.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BoxStyle {
    /// Across the value axis.
    pub thickness: f32,
    pub outline: f32,
    pub median: f32,
    /// The half length of the caps of the whiskers.
    pub cap: f32,
    pub outlier_radius: f32,
}

/// Where the rows of the boxes lie across the value axis of a distribution, one per
/// shown trace.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rows {
    pub orientation: Orientation,
    /// The plot without its axes.
    pub frame: egui::Rect,
    /// Where the first row starts across the value axis.
    first: f32,
    /// The length of a row across the value axis, negative where the rows follow each
    /// other to the left.
    step: f32,
    pub count: usize,
    pub style: BoxStyle,
}

impl Rows {
    /// The rows of Boxes, which share the `frame` of the plot: from the top down
    /// upright, from the left sideways.
    pub fn boxes(orientation: Orientation, frame: egui::Rect, count: usize) -> Self {
        let (first, length) =
            orientation.pick((frame.top(), frame.height()), (frame.left(), frame.width()));
        let step = length / count.max(1) as f32;
        let thickness = (BOX_SHARE * step).clamp(MIN_BOX, MAX_BOX);
        Self {
            orientation,
            frame,
            first,
            step,
            count,
            style: BoxStyle {
                thickness,
                outline: OUTLINE_WIDTH,
                median: MEDIAN_WIDTH,
                cap: (thickness / 2.0).min(MAX_CAP),
                outlier_radius: OUTLIER_RADIUS,
            },
        }
    }

    /// The rows of the strip of Both at the end of the counts axis of the `frame` of
    /// the plot, where the markers of Bars are: from the top down upright, from the
    /// right sideways.
    pub fn strip(orientation: Orientation, frame: egui::Rect, count: usize) -> Self {
        let (first, step) = orientation.pick(
            (frame.top() + MARKER_INSET, STRIP_ROW),
            (frame.right() - MARKER_INSET, -STRIP_ROW),
        );
        Self {
            orientation,
            frame,
            first,
            step,
            count,
            style: BoxStyle {
                thickness: STRIP_BOX,
                outline: STRIP_OUTLINE,
                median: STRIP_MEDIAN,
                cap: MARKER_RADIUS,
                outlier_radius: STRIP_OUTLIER_RADIUS,
            },
        }
    }

    /// The length of a row across the value axis.
    pub fn length(&self) -> f32 {
        self.step.abs()
    }

    /// The middle of the `row` across the value axis.
    pub fn centre(&self, row: usize) -> f32 {
        self.first + (row as f32 + 0.5) * self.step
    }

    /// The point `along` the value axis and `offset` from the middle of `row` across it.
    fn point(&self, row: usize, along: f32, offset: f32) -> egui::Pos2 {
        let across = self.centre(row) + offset;
        egui::Pos2::from(self.orientation.xy(along, across))
    }

    /// The `row` along the whole value axis.
    pub fn rect(&self, row: usize) -> egui::Rect {
        let start = self.first + row as f32 * self.step;
        let across = egui::Rangef::new(start, start + self.step);
        let across = egui::Rangef::new(across.min.min(across.max), across.min.max(across.max));
        let [x, y] = self.orientation.pick(
            [self.frame.x_range(), across],
            [across, self.frame.y_range()],
        );
        egui::Rect::from_x_y_ranges(x, y)
    }

    /// The row at `pos`, `None` outside the rows.
    pub fn at(&self, pos: egui::Pos2) -> Option<usize> {
        let across = self.orientation.pick(pos.y, pos.x);
        let row = ((across - self.first) / self.step).floor();
        (self.frame.contains(pos) && row >= 0.0 && (row as usize) < self.count)
            .then_some(row as usize)
    }
}

/// Where `value` lies along the value axis of `transform`.
fn along(transform: &egui_plot::PlotTransform, orientation: Orientation, value: f64) -> f32 {
    orientation.pick(
        transform.position_from_point_x(value),
        transform.position_from_point_y(value),
    )
}

/// Paints the boxes of the `shown` traces in their colours, a row of `rows` each: all
/// washes first, then the outlines, whiskers, medians and outliers, then the dots at the
/// means, so that none hides another.
pub fn paint(
    painter: &egui::Painter,
    visuals: &egui::Visuals,
    rows: &Rows,
    transform: &egui_plot::PlotTransform,
    (shown, boxes): (&[Shown<'_>], &[Option<BoxStats>]),
) {
    let style = rows.style;
    let half = style.thickness / 2.0;
    let at = |row: usize, value: f64, offset: f32| {
        rows.point(row, along(transform, rows.orientation, value), offset)
    };
    // the box from the first to the third quartile, none if they are the same
    let rect = |row: usize, stats: &BoxStats| {
        (stats.q3 > stats.q1)
            .then(|| egui::Rect::from_two_pos(at(row, stats.q1, -half), at(row, stats.q3, half)))
    };
    let traces = || {
        (shown.iter().zip(boxes).enumerate()).filter_map(|(row, (trace, stats))| {
            Some((
                row,
                theme::trace_color(visuals, trace.index),
                stats.as_ref()?,
            ))
        })
    };
    for (row, color, stats) in traces() {
        if let Some(rect) = rect(row, stats) {
            painter.rect_filled(rect, 0.0, theme::wash(visuals, color));
        }
    }
    for (row, color, stats) in traces() {
        let faint = color.gamma_multiply(OUTLIER_OPACITY);
        let mut last: Option<f32> = None;
        for &value in &stats.outliers {
            let along = along(transform, rows.orientation, value);
            if last.is_some_and(|last| (along - last).abs() < OUTLIER_SPACING) {
                continue;
            }
            last = Some(along);
            painter.circle_filled(rows.point(row, along, 0.0), style.outlier_radius, faint);
        }
        let stroke = egui::Stroke::new(OUTLINE_WIDTH, color);
        for (end, edge) in [(stats.low, stats.q1), (stats.high, stats.q3)] {
            if end != edge {
                painter.line_segment([at(row, end, 0.0), at(row, edge, 0.0)], stroke);
                painter.line_segment([at(row, end, -style.cap), at(row, end, style.cap)], stroke);
            }
        }
        if let Some(rect) = rect(row, stats) {
            let outline = egui::Stroke::new(style.outline, color);
            painter.rect_stroke(rect, 0.0, outline, egui::StrokeKind::Middle);
        }
        let median = egui::Stroke::new(style.median, color);
        painter.line_segment(
            [at(row, stats.median, -half), at(row, stats.median, half)],
            median,
        );
    }
    let card = visuals.tokens().card;
    for (row, color, stats) in traces() {
        let mean = at(row, stats.mean, 0.0);
        painter.circle_filled(mean, MARKER_RADIUS + RING_WIDTH, card);
        painter.circle_filled(mean, MARKER_RADIUS, color);
    }
}

/// Paints "No samples" in the rows of `rows` of the traces of `boxes` without samples,
/// and a faint arrow at the edge of the value axis beyond which a box lies wholly,
/// with the value axis showing `values`, in the font of `style`.
pub fn paint_notes(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rows: &Rows,
    values: (f64, f64),
    boxes: &[Option<BoxStats>],
    style: egui::TextStyle,
) {
    let faint = ui.tokens().text_faint;
    let font = style.resolve(ui.style());
    let frame = rows.frame;
    for (row, stats) in boxes.iter().enumerate() {
        let centre = rows.centre(row);
        let Some(stats) = stats else {
            let galley = painter.layout_no_wrap(NO_SAMPLES.to_owned(), font.clone(), faint);
            let size = galley.size();
            let middle = rows.orientation.pick(
                egui::pos2(frame.center().x, centre),
                egui::pos2(centre, frame.center().y),
            );
            // up the column if it is too narrow for the note
            let shape = if rows.orientation == Orientation::Sideways && size.x > rows.length() {
                let corner = middle + egui::vec2(-size.y / 2.0, size.x / 2.0);
                egui::epaint::TextShape::new(corner, galley, faint)
                    .with_angle(-std::f32::consts::FRAC_PI_2)
            } else {
                egui::epaint::TextShape::new(middle - size / 2.0, galley, faint)
            };
            painter.add(shape);
            continue;
        };
        let Some(above) = beyond(stats, values) else {
            continue;
        };
        let (pos, anchor, text) = match (rows.orientation, above) {
            (Orientation::Upright, false) => (
                egui::pos2(frame.left() + OVERLAY_INSET, centre),
                egui::Align2::LEFT_CENTER,
                "‹",
            ),
            (Orientation::Upright, true) => (
                egui::pos2(frame.right() - OVERLAY_INSET, centre),
                egui::Align2::RIGHT_CENTER,
                "›",
            ),
            (Orientation::Sideways, false) => (
                egui::pos2(centre, frame.bottom() - OVERLAY_INSET),
                egui::Align2::CENTER_BOTTOM,
                "↓",
            ),
            (Orientation::Sideways, true) => (
                egui::pos2(centre, frame.top() + OVERLAY_INSET),
                egui::Align2::CENTER_TOP,
                "↑",
            ),
        };
        painter.text(pos, anchor, text, font.clone(), faint);
    }
}

/// Whether the box of `stats` lies wholly above the `values` on the value axis
/// (`true`) or wholly below them (`false`), `None` if they show some of it.
pub fn beyond(stats: &BoxStats, (low, high): (f64, f64)) -> Option<bool> {
    if stats.q3 < low {
        Some(false)
    } else if stats.q1 > high {
        Some(true)
    } else {
        None
    }
}

/// `text` cut with "…" to at most `width` points in `font`.
pub fn cut(ui: &egui::Ui, text: &str, width: f32, font: &egui::FontId) -> String {
    let fits = |text: &str| {
        let galley =
            ui.painter()
                .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER);
        galley.size().x <= width
    };
    if fits(text) {
        return text.to_owned();
    }
    (text.char_indices().rev())
        .map(|(end, _)| format!("{}…", &text[..end]))
        .find(|cut| fits(cut))
        .unwrap_or_else(|| "…".to_owned())
}

/// How a value of the tooltip is written: one of the box in the text colour, one of
/// its ends weak.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weight {
    Box,
    End,
}

/// The names and values of the tooltip of `stats` with `whiskers`, in the order drawn:
/// from low to high upright, from high to low sideways. Min–max leaves out the
/// whiskers, which end at the minimum and the maximum.
pub fn tooltip_rows(
    stats: &BoxStats,
    whiskers: Whiskers,
    orientation: Orientation,
) -> Vec<(&'static str, f64, Weight)> {
    let ends = match whiskers {
        Whiskers::Iqr => Some(("Low whisker", "High whisker")),
        Whiskers::MinMax => None,
        Whiskers::P5 => Some(("5 %", "95 %")),
        Whiskers::P1 => Some(("1 %", "99 %")),
    };
    let mut rows = vec![("Min", stats.min, Weight::End)];
    rows.extend(ends.map(|(low, _)| (low, stats.low, Weight::End)));
    rows.extend([
        ("Q1", stats.q1, Weight::Box),
        ("Median", stats.median, Weight::Box),
        ("Mean", stats.mean, Weight::Box),
        ("Q3", stats.q3, Weight::Box),
    ]);
    rows.extend(ends.map(|(_, high)| (high, stats.high, Weight::End)));
    rows.push(("Max", stats.max, Weight::End));
    if orientation == Orientation::Sideways {
        rows.reverse();
    }
    rows
}

/// The step the values of the tooltip of `stats` are written to, a thousandth of the
/// range of the samples.
pub fn value_step(stats: &BoxStats) -> f64 {
    let span = stats.max - stats.min;
    let span = match span > 0.0 {
        true => span,
        false => stats.max.abs(),
    };
    match span > 0.0 {
        true => nice_near(span / VALUE_STEPS),
        false => 1.0,
    }
}

/// The trace of a row under the pointer, for its tooltip.
pub struct Tip<'a> {
    pub trace: TraceName,
    pub stats: Option<&'a BoxStats>,
    pub whiskers: Whiskers,
    pub orientation: Orientation,
    pub format: DisplayFormat,
    pub unit: Option<&'a str>,
    /// What the target says of the trace.
    pub description: Option<&'a str>,
}

impl Tip<'_> {
    /// The dot and the name of the trace, the values of its box and, in a second group,
    /// its samples and outliers; or "No samples".
    pub fn tooltip(&self) -> Tooltip {
        let tooltip = Tooltip::new(Header::Trace(self.trace.clone()));
        let Some(stats) = self.stats else {
            return tooltip
                .note(NO_SAMPLES)
                .describe(&self.trace.name, self.description);
        };
        let step = value_step(stats);
        let values = (tooltip_rows(stats, self.whiskers, self.orientation).into_iter())
            .map(|(name, value, weight)| {
                let text = with_unit(write_value(value, step, self.format), self.unit);
                let tone = match weight {
                    Weight::Box => Tone::Text,
                    Weight::End => Tone::Weak,
                };
                Row::new(Label::Weak(name.to_owned()), [Value::new(text, tone)])
            })
            .collect();
        let mut counts = vec![Row::new(
            Label::Weak("Samples".to_owned()),
            [Value::text(stats.samples.to_string())],
        )];
        if self.whiskers != Whiskers::MinMax {
            let outliers = stats.outliers.len();
            counts.push(Row::new(
                Label::Weak("Outliers".to_owned()),
                [Value::text(outliers.to_string()).faint_if(outliers == 0)],
            ));
        }
        tooltip
            .group(values)
            .group(counts)
            .describe(&self.trace.name, self.description)
    }

    /// Shows the tooltip.
    pub fn ui(&self, ui: &mut egui::Ui) {
        self.tooltip().ui(ui);
    }
}

#[cfg(test)]
mod tests;
