//! The boxes of a distribution: the quartiles, the whiskers and the outliers of the
//! samples of each trace, in a row per trace across the value axis.

use super::{
    MARKER_INSET, MARKER_RADIUS, OUTLINE_WIDTH, OVERLAY_INSET, Orientation, Shown, Whiskers, dot,
    nice_near, text_size, write_value,
};
use crate::app::model::Version;
use crate::app::model::numbers::DisplayFormat;
use crate::app::theme;
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
        let (first, length) = match orientation {
            Orientation::Upright => (frame.top(), frame.height()),
            Orientation::Sideways => (frame.left(), frame.width()),
        };
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
        let (first, step) = match orientation {
            Orientation::Upright => (frame.top() + MARKER_INSET, STRIP_ROW),
            Orientation::Sideways => (frame.right() - MARKER_INSET, -STRIP_ROW),
        };
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
        match self.orientation {
            Orientation::Upright => egui::pos2(along, across),
            Orientation::Sideways => egui::pos2(across, along),
        }
    }

    /// The `row` along the whole value axis.
    pub fn rect(&self, row: usize) -> egui::Rect {
        let start = self.first + row as f32 * self.step;
        let across = egui::Rangef::new(start, start + self.step);
        let across = egui::Rangef::new(across.min.min(across.max), across.min.max(across.max));
        match self.orientation {
            Orientation::Upright => egui::Rect::from_x_y_ranges(self.frame.x_range(), across),
            Orientation::Sideways => egui::Rect::from_x_y_ranges(across, self.frame.y_range()),
        }
    }

    /// The row at `pos`, `None` outside the rows.
    pub fn at(&self, pos: egui::Pos2) -> Option<usize> {
        let across = match self.orientation {
            Orientation::Upright => pos.y,
            Orientation::Sideways => pos.x,
        };
        let row = ((across - self.first) / self.step).floor();
        (self.frame.contains(pos) && row >= 0.0 && (row as usize) < self.count)
            .then_some(row as usize)
    }
}

/// Where `value` lies along the value axis of `transform`.
fn along(transform: &egui_plot::PlotTransform, orientation: Orientation, value: f64) -> f32 {
    match orientation {
        Orientation::Upright => transform.position_from_point_x(value),
        Orientation::Sideways => transform.position_from_point_y(value),
    }
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
    let card = theme::tokens(visuals).card;
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
    let faint = theme::tokens(ui.visuals()).text_faint;
    let font = style.resolve(ui.style());
    let frame = rows.frame;
    for (row, stats) in boxes.iter().enumerate() {
        let centre = rows.centre(row);
        let Some(stats) = stats else {
            let galley = painter.layout_no_wrap(NO_SAMPLES.to_owned(), font.clone(), faint);
            let size = galley.size();
            let middle = match rows.orientation {
                Orientation::Upright => egui::pos2(frame.center().x, centre),
                Orientation::Sideways => egui::pos2(centre, frame.center().y),
            };
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
    pub trace: &'a Shown<'a>,
    pub stats: Option<&'a BoxStats>,
    pub whiskers: Whiskers,
    pub orientation: Orientation,
    pub format: DisplayFormat,
    pub unit: Option<&'a str>,
}

impl Tip<'_> {
    /// Shows the dot and the name of the trace, the values of its box and, under a rule,
    /// its samples and outliers.
    pub fn ui(&self, ui: &mut egui::Ui) {
        let faint = theme::tokens(ui.visuals()).text_faint;
        let (weak, text) = (ui.visuals().weak_text_color(), ui.visuals().text_color());
        ui.horizontal(|ui| {
            dot(ui, theme::trace_color(ui.visuals(), self.trace.index));
            ui.label(&self.trace.name).on_hover_text(&self.trace.path);
        });
        let Some(stats) = self.stats else {
            ui.label(egui::RichText::new(NO_SAMPLES).color(faint));
            return;
        };
        let step = value_step(stats);
        let write =
            |value: f64| super::super::with_unit(write_value(value, step, self.format), self.unit);
        let values: Vec<(&str, String, egui::Color32)> =
            (tooltip_rows(stats, self.whiskers, self.orientation).into_iter())
                .map(|(name, value, weight)| {
                    let color = match weight {
                        Weight::Box => text,
                        Weight::End => weak,
                    };
                    (name, write(value), color)
                })
                .collect();
        let mut counts = vec![("Samples", stats.samples.to_string(), text)];
        if self.whiskers != Whiskers::MinMax {
            let outliers = stats.outliers.len();
            let color = if outliers == 0 { faint } else { text };
            counts.push(("Outliers", outliers.to_string(), color));
        }
        // both grids have columns as wide as the widest name and value, so that they
        // line up, and the values are right aligned in theirs
        let widest = |texts: &mut dyn Iterator<Item = &str>| {
            texts
                .map(|text| text_size(ui, text, egui::TextStyle::Body).x)
                .fold(0.0, f32::max)
        };
        let rows = || values.iter().chain(&counts);
        let widths = [
            widest(&mut rows().map(|row| row.0)),
            widest(&mut rows().map(|row| row.1.as_str())),
        ];
        let spacing = 12.0;
        let grid = |ui: &mut egui::Ui, id: &str, rows: Vec<(&str, String, egui::Color32)>| {
            egui::Grid::new(id)
                .num_columns(2)
                .spacing([spacing, 2.0])
                .show(ui, |ui| {
                    let height = ui.spacing().interact_size.y;
                    for (name, value, color) in rows {
                        let left = egui::Layout::left_to_right(egui::Align::Center);
                        ui.allocate_ui_with_layout(egui::vec2(widths[0], height), left, |ui| {
                            // as wide as the widest name, also in the other grid
                            ui.set_min_width(widths[0]);
                            ui.label(egui::RichText::new(name).color(weak));
                        });
                        let right = egui::Layout::right_to_left(egui::Align::Center);
                        ui.allocate_ui_with_layout(egui::vec2(widths[1], height), right, |ui| {
                            ui.label(egui::RichText::new(value).color(color));
                        });
                        ui.end_row();
                    }
                });
        };
        grid(ui, "box tooltip values", values);
        let (rule, _) = ui.allocate_exact_size(
            egui::vec2(widths[0] + spacing + widths[1], 7.0),
            egui::Sense::hover(),
        );
        let line = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().hline(rule.x_range(), rule.center().y, line);
        grid(ui, "box tooltip counts", counts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_of(values: &[f64], whiskers: Whiskers) -> BoxStats {
        BoxStats::of(values.iter().copied(), whiskers).expect("a box")
    }

    #[test]
    fn quartiles_interpolate_between_the_samples_as_numpy() {
        // np.percentile([1, 2, 3, 4], [25, 50, 75]) is 1.75, 2.5, 3.25
        let stats = box_of(&[4.0, 1.0, 3.0, 2.0], Whiskers::Iqr);
        assert_eq!((stats.q1, stats.median, stats.q3), (1.75, 2.5, 3.25));
        assert_eq!((stats.min, stats.max, stats.mean), (1.0, 4.0, 2.5));
        // np.percentile([7, 15, 36, 39, 40, 41], [25, 50, 75]) is 20.25, 37.5, 39.75
        let stats = box_of(&[41.0, 7.0, 39.0, 15.0, 40.0, 36.0], Whiskers::Iqr);
        assert_eq!((stats.q1, stats.median, stats.q3), (20.25, 37.5, 39.75));
        // on an order statistic, the sample itself
        let sorted: Vec<f64> = (0..=100).map(f64::from).collect();
        for p in [0.0, 0.01, 0.05, 0.5, 0.95, 0.99, 1.0] {
            assert_eq!(quantile(&sorted, p), 100.0 * p);
        }
        assert_eq!(quantile(&[3.0], 0.25), 3.0);
    }

    #[test]
    fn whiskers_end_at_the_fences_the_percentiles_or_the_extremes() {
        let mut values: Vec<f64> = (1..=20).map(f64::from).collect();
        values.extend([-50.0, 100.0]);
        // 1.5×IQR: the furthest samples within 1.5 box lengths of the box
        let stats = box_of(&values, Whiskers::Iqr);
        let reach = 1.5 * (stats.q3 - stats.q1);
        assert!(stats.q1 - reach <= 1.0 && stats.q3 + reach >= 20.0);
        assert!(stats.q1 - reach > -50.0 && stats.q3 + reach < 100.0);
        assert_eq!((stats.low, stats.high), (1.0, 20.0));
        assert_eq!(stats.outliers, [-50.0, 100.0]);
        assert_eq!((stats.min, stats.max, stats.samples), (-50.0, 100.0, 22));
        // Min–max: the extremes, without outliers
        let stats = box_of(&values, Whiskers::MinMax);
        assert_eq!((stats.low, stats.high), (-50.0, 100.0));
        assert!(stats.outliers.is_empty());
        // the percentiles, with the samples beyond them as outliers
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);
        for (whiskers, (low, high)) in [(Whiskers::P5, (0.05, 0.95)), (Whiskers::P1, (0.01, 0.99))]
        {
            let stats = box_of(&values, whiskers);
            assert_eq!(
                (stats.low, stats.high),
                (quantile(&sorted, low), quantile(&sorted, high))
            );
            let beyond: Vec<f64> = (sorted.iter().copied())
                .filter(|&value| value < stats.low || value > stats.high)
                .collect();
            assert_eq!(stats.outliers, beyond);
            assert!(stats.outliers.contains(&-50.0) && stats.outliers.contains(&100.0));
        }
    }

    #[test]
    fn nulls_infinite_samples_and_equal_values() {
        // nulls, NaN in the samples, and infinite samples are not in the box
        let stats = box_of(
            &[f64::NAN, 1.0, f64::INFINITY, 3.0, f64::NEG_INFINITY, 2.0],
            Whiskers::Iqr,
        );
        assert_eq!((stats.samples, stats.min, stats.max), (3, 1.0, 3.0));
        assert!(BoxStats::of([f64::NAN].into_iter(), Whiskers::Iqr).is_none());
        assert!(BoxStats::of(std::iter::empty(), Whiskers::Iqr).is_none());
        // all of one value: the box is the median alone, with the mean on it
        for whiskers in Whiskers::ALL {
            let stats = box_of(&[2.5; 7], whiskers);
            let ends = [
                stats.min, stats.low, stats.q1, stats.q3, stats.high, stats.max,
            ];
            assert!(ends.iter().all(|&end| end == 2.5), "{whiskers:?}");
            assert_eq!((stats.median, stats.mean), (2.5, 2.5));
            assert!(stats.outliers.is_empty());
        }
        // far from zero, the mean stays precise
        let stats = box_of(&[1.0e12 + 1.0, 1.0e12 + 2.0], Whiskers::Iqr);
        assert_eq!(stats.mean, 1.0e12 + 1.5);
    }

    #[test]
    fn a_box_wholly_beyond_the_value_axis_points_to_its_edge() {
        let stats = box_of(&[10.0, 11.0, 12.0, 13.0], Whiskers::Iqr);
        assert_eq!(beyond(&stats, (0.0, 5.0)), Some(true));
        assert_eq!(beyond(&stats, (20.0, 30.0)), Some(false));
        // a box partly shown is cut at the edge instead
        assert_eq!(beyond(&stats, (11.0, 30.0)), None);
        assert_eq!(beyond(&stats, (0.0, 11.0)), None);
    }

    #[test]
    fn a_box_is_40_percent_of_its_row_at_most_24_pt() {
        let frame = |height: f32| {
            egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, height))
        };
        for (height, count, thickness) in [(200.0, 4, 20.0), (300.0, 1, 24.0), (60.0, 6, 6.0)] {
            let rows = Rows::boxes(Orientation::Upright, frame(height), count);
            assert_eq!(rows.style.thickness, thickness, "{height} {count}");
            assert_eq!(rows.style.cap, (thickness / 2.0).min(MAX_CAP));
        }
        // each trace an equal row, from the top down upright, from the left sideways
        let rows = Rows::boxes(Orientation::Upright, frame(200.0), 4);
        assert_eq!(rows.length(), 50.0);
        assert_eq!(rows.centre(0), 45.0);
        assert_eq!(rows.at(egui::pos2(100.0, 130.0)), Some(2));
        assert_eq!(rows.rect(2).x_range(), frame(200.0).x_range());
        let rows = Rows::boxes(Orientation::Sideways, frame(200.0), 3);
        assert_eq!((rows.length(), rows.centre(0)), (100.0, 60.0));
        assert_eq!(rows.at(egui::pos2(250.0, 100.0)), Some(2));
        assert_eq!(rows.at(egui::pos2(250.0, 300.0)), None);
        // in the strip of Both, 10 pt boxes in 14 pt rows from the end of the counts
        let strip = Rows::strip(Orientation::Upright, frame(200.0), 2);
        assert_eq!((strip.length(), strip.style.thickness), (14.0, 10.0));
        assert_eq!(strip.centre(0), 20.0 + MARKER_INSET + 7.0);
        assert_eq!(
            strip.at(egui::pos2(100.0, 20.0 + MARKER_INSET + 20.0)),
            Some(1)
        );
        assert_eq!(strip.at(egui::pos2(100.0, 100.0)), None);
        let strip = Rows::strip(Orientation::Sideways, frame(200.0), 2);
        assert_eq!(strip.centre(0), 310.0 - MARKER_INSET - 7.0);
        assert_eq!(strip.rect(1).width(), 14.0);
    }

    #[test]
    fn the_tooltip_lists_the_values_in_the_order_drawn() {
        let stats = box_of(&[1.0, 2.0, 3.0, 4.0, 5.0, 30.0], Whiskers::Iqr);
        let names = |whiskers, orientation| -> Vec<&str> {
            (tooltip_rows(&stats, whiskers, orientation).into_iter())
                .map(|(name, _, _)| name)
                .collect()
        };
        let upright = [
            "Min",
            "Low whisker",
            "Q1",
            "Median",
            "Mean",
            "Q3",
            "High whisker",
            "Max",
        ];
        assert_eq!(names(Whiskers::Iqr, Orientation::Upright), upright);
        let mut sideways = upright;
        sideways.reverse();
        assert_eq!(names(Whiskers::Iqr, Orientation::Sideways), sideways);
        assert_eq!(
            names(Whiskers::MinMax, Orientation::Upright),
            ["Min", "Q1", "Median", "Mean", "Q3", "Max"]
        );
        assert_eq!(names(Whiskers::P5, Orientation::Upright)[1], "5 %");
        assert_eq!(names(Whiskers::P5, Orientation::Upright)[6], "95 %");
        assert_eq!(names(Whiskers::P1, Orientation::Sideways)[1], "99 %");
        // the box in the text colour, its ends weak
        let rows = tooltip_rows(&stats, Whiskers::Iqr, Orientation::Upright);
        let weights: Vec<Weight> = rows.iter().map(|&(_, _, weight)| weight).collect();
        assert_eq!(weights[2..6], [Weight::Box; 4]);
        assert!([0, 1, 6, 7].iter().all(|&row| weights[row] == Weight::End));
        assert_eq!(rows[7].1, 30.0);
        // written to a thousandth of the range
        assert_eq!(value_step(&stats), 0.02);
        assert_eq!(value_step(&box_of(&[0.0], Whiskers::Iqr)), 1.0);
    }
}
