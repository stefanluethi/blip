//! Pieces shared by the plot views: points and lines, arrays over their indices, index
//! axes, and the ranges panned to.

use super::plot::{LineStyle, ShowPoints};
use crate::app::model::numbers::Number;
use crate::app::model::sorted::index_at;
use crate::app::model::traces::Trace;
use crate::app::time::TimeRange;

/// Radius of the marks of the points.
pub(super) const POINT_RADIUS: f32 = 2.0;

/// Whether neither coordinate of `point` is NaN, which marks a gap.
pub(super) fn is_point(point: &[f64; 2]) -> bool {
    !point[0].is_nan() && !point[1].is_nan()
}

/// The position of `point` on the screen of a plot with `transform`.
pub(super) fn to_screen(point: [f64; 2], transform: &egui_plot::PlotTransform) -> egui::Pos2 {
    transform.position_from_point(&egui_plot::PlotPoint::new(point[0], point[1]))
}

/// Draws the lines through `points` in `style`, if any, broken where a point is
/// missing.
pub(super) fn plot_lines(
    name: &str,
    points: &[[f64; 2]],
    color: egui::Color32,
    style: Option<egui_plot::LineStyle>,
    plot_ui: &mut egui_plot::PlotUi<'_>,
) {
    let Some(line_style) = style else {
        return;
    };
    for segment in points.split(|point| !is_point(point)) {
        if segment.len() > 1 {
            plot_ui.line(
                egui_plot::Line::new(name, segment.to_vec())
                    .color(color)
                    .style(line_style),
            );
        }
    }
}

/// Marks the `points` that are not missing.
pub(super) fn plot_points(
    name: &str,
    points: &[[f64; 2]],
    color: egui::Color32,
    plot_ui: &mut egui_plot::PlotUi<'_>,
) {
    let points: Vec<[f64; 2]> = points.iter().copied().filter(is_point).collect();
    if !points.is_empty() {
        plot_ui.points(
            egui_plot::Points::new(name, points)
                .color(color)
                .radius(POINT_RADIUS)
                .filled(true),
        );
    }
}

/// Draws the elements `values` of an array over their indices, the lines in `style`
/// and the points marked as `points` says.
pub(super) fn plot_array(
    name: &str,
    values: &[f64],
    color: egui::Color32,
    style: LineStyle,
    points: ShowPoints,
    plot_ui: &mut egui_plot::PlotUi<'_>,
) {
    let values: Vec<[f64; 2]> = values
        .iter()
        .enumerate()
        .map(|(index, &value)| [index as f64, value])
        .collect();
    let line_style = style.plot_style(&values, plot_ui.transform());
    plot_lines(name, &values, color, line_style, plot_ui);
    let width = plot_ui.transform().frame().width();
    if style.marks_points(points, values.len(), width) {
        plot_points(name, &values, color, plot_ui);
    }
}

/// The index of the element of `len` nearest to `position` on an index axis, or `None`
/// outside the elements.
pub(super) fn index_under(position: f64, len: usize) -> Option<usize> {
    let index = position.round();
    (index >= 0.0 && index < len as f64).then_some(index as usize)
}

/// The index of the point of `points` nearest to `pointer` on the screen of a plot with
/// `transform`, with its squared distance, if within `radius` points. Gaps are skipped;
/// of equally near points the first.
pub(super) fn nearest_point(
    points: &[[f64; 2]],
    pointer: egui::Pos2,
    transform: &egui_plot::PlotTransform,
    radius: f32,
) -> Option<(usize, f32)> {
    let mut nearest: Option<(usize, f32)> = None;
    for (index, point) in points.iter().enumerate().filter(|(_, p)| is_point(p)) {
        let distance = to_screen(*point, transform).distance_sq(pointer);
        if distance <= radius * radius && nearest.is_none_or(|(_, best)| distance < best) {
            nearest = Some((index, distance));
        }
    }
    nearest
}

/// The elements of the sample of an array trace at the cursor time.
pub(super) fn sample_at_cursor(trace: Trace<'_>, time: &TimeRange) -> Vec<Number> {
    time.cursor_timestamp()
        .and_then(|t| index_at(trace.timestamps, t))
        .and_then(|index| trace.samples.array_sample(index))
        .map(|elements| elements.iter().collect())
        .unwrap_or_default()
}

/// Labels only the integer marks of an axis, negated if `negate`.
pub(super) fn index_formatter(
    negate: bool,
) -> impl Fn(egui_plot::GridMark, &std::ops::RangeInclusive<f64>) -> String {
    move |mark, _range| {
        let value = if negate { 0.0 - mark.value } else { mark.value };
        if value.fract() == 0.0 && value >= 0.0 {
            format!("{value}")
        } else {
            String::new()
        }
    }
}

/// An x axis of indices labelled `label`, used with [`index_spacer`].
pub(super) fn index_x_axis(label: &str) -> egui_plot::AxisHints<'static> {
    egui_plot::AxisHints::new_x()
        .label(label)
        .formatter(index_formatter(false))
        // indices are short, so their labels fit closer together than other numbers
        .label_spacing(30.0..=45.0)
}

/// Marks an index axis at whole multiples of 1, 2, 5, 10, 20, … far enough apart to
/// label each of them.
pub(super) fn index_spacer(input: egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
    // the base step is the value of the minimum line spacing, labels need more space
    let min_step = input.base_step_size * 6.0;
    let step = (0..)
        .map(|n| [1.0, 2.0, 5.0][n % 3] * 10f64.powi((n / 3) as i32))
        .find(|&step| step >= min_step)
        .unwrap_or(1.0);
    let (start, end) = input.bounds;
    ((start / step).ceil() as i64..=(end / step).floor() as i64)
        .map(|multiple| egui_plot::GridMark {
            value: multiple as f64 * step,
            step_size: step,
        })
        .collect()
}

/// Whether either end of the `shown` range is further from `expected` than rounding in
/// the plot explains, so the user moved it.
pub(super) fn range_moved(expected: (f64, f64), shown: (f64, f64)) -> bool {
    let tolerance = (expected.1 - expected.0) * 1.0e-6;
    (shown.0 - expected.0).abs() > tolerance || (shown.1 - expected.1).abs() > tolerance
}

/// The y axis of the time plots, as thick in all so they line up.
pub(super) fn time_y_axis() -> egui_plot::AxisHints<'static> {
    egui_plot::AxisHints::new_y()
        .placement(egui_plot::HPlacement::Left)
        .min_thickness(50.0)
}
