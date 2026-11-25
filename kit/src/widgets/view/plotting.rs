//! Pieces shared by the plot views: points and lines, arrays over their indices, index
//! axes, the ranges panned to, and the legends.

use super::plot::{LineStyle, ShowPoints};
use crate::store::numbers::Number;
use crate::store::sorted::index_at;
use crate::store::traces::Trace;
use crate::widgets::time::TimeRange;

/// Radius of the marks of the points.
pub(super) const POINT_RADIUS: f32 = 2.0;

/// The room around the values an axis fits on each side, as a part of their range, as
/// egui_plot fits the y axis of a plot.
pub(super) const Y_MARGIN: f64 = 0.05;

/// `(low, high)` with [`Y_MARGIN`] on each side, or 0.5 around a single value.
pub(super) fn with_margin((low, high): (f64, f64)) -> (f64, f64) {
    let margin = if high > low {
        (high - low) * Y_MARGIN
    } else {
        0.5
    };
    (low - margin, high + margin)
}

/// A plot under `id` with its grid in the colours of `visuals`, without the coordinates
/// at the pointer, as the views show the values there in a tooltip.
pub(super) fn base_plot<'a>(id: egui::Id, visuals: &egui::Visuals) -> egui_plot::Plot<'a> {
    egui_plot::Plot::new(id)
        .grid_color(crate::widgets::theme::grid_color(visuals))
        .show_x(false)
        .show_y(false)
}

/// Whether neither coordinate of `point` is NaN, which marks a gap.
pub(super) fn is_point(point: &[f64; 2]) -> bool {
    !point[0].is_nan() && !point[1].is_nan()
}

/// The position of `point` on the screen of a plot with `transform`.
pub(super) fn to_screen(point: [f64; 2], transform: &egui_plot::PlotTransform) -> egui::Pos2 {
    transform.position_from_point(&egui_plot::PlotPoint::new(point[0], point[1]))
}

/// Draws the lines through `points` in `style`, if any, `width` points wide, broken
/// where a point is missing.
pub(super) fn plot_lines(
    name: &str,
    points: &[[f64; 2]],
    color: egui::Color32,
    (style, width): (Option<egui_plot::LineStyle>, f32),
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
                    .width(width)
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
/// and `width` points wide, and the points marked as `points` says.
pub(super) fn plot_array(
    name: &str,
    values: &[f64],
    color: egui::Color32,
    (style, width): (LineStyle, f32),
    points: ShowPoints,
    plot_ui: &mut egui_plot::PlotUi<'_>,
) {
    let values: Vec<[f64; 2]> = values
        .iter()
        .enumerate()
        .map(|(index, &value)| [index as f64, value])
        .collect();
    let line_style = style.plot_style(&values, plot_ui.transform());
    plot_lines(name, &values, color, (line_style, width), plot_ui);
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

/// The point of the plot under the pointer, if the pointer is over it.
pub(super) fn hovered_point(plot_ui: &egui_plot::PlotUi<'_>) -> Option<egui_plot::PlotPoint> {
    plot_ui
        .response()
        .hovered()
        .then(|| plot_ui.pointer_coordinate())
        .flatten()
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

/// The room around a corner legend and inside it, in points.
const LEGEND_MARGIN: egui::Vec2 = egui::vec2(8.0, 4.0);

/// Draws a legend in `corner` of the plot `frame`: a row per entry of `entries`, the
/// name of a trace and its index, with a dot in its colour, on a card patch. It does not react to the pointer, so
/// clicks reach the plot and no trace is hidden from it.
pub(super) fn corner_legend<'a>(
    ui: &egui::Ui,
    frame: egui::Rect,
    corner: egui_plot::Corner,
    entries: impl IntoIterator<Item = (&'a str, usize)>,
) {
    let tokens = crate::widgets::theme::tokens(ui.visuals());
    let font = egui::TextStyle::Body.resolve(ui.style());
    let weak = ui.visuals().weak_text_color();
    let rows: Vec<(std::sync::Arc<egui::Galley>, egui::Color32)> = entries
        .into_iter()
        .map(|(name, index)| {
            let galley = ui
                .painter()
                .layout_no_wrap(name.to_owned(), font.clone(), weak);
            (
                galley,
                crate::widgets::theme::trace_color(ui.visuals(), index),
            )
        })
        .collect();
    if rows.is_empty() {
        return;
    }
    let dot = super::LEGEND_DOT_SIZE;
    let spacing = ui.spacing().item_spacing.x;
    let row_height = rows
        .iter()
        .map(|(galley, _)| galley.size().y)
        .fold(dot, f32::max);
    let text_width = rows
        .iter()
        .map(|(galley, _)| galley.size().x)
        .fold(0.0, f32::max);
    let size = egui::vec2(dot + spacing + text_width, row_height * rows.len() as f32)
        + 2.0 * LEGEND_MARGIN;
    let inner = frame.shrink2(LEGEND_MARGIN);
    let min = match corner {
        egui_plot::Corner::LeftTop => inner.left_top(),
        egui_plot::Corner::RightTop => egui::pos2(inner.right() - size.x, inner.top()),
        egui_plot::Corner::LeftBottom => egui::pos2(inner.left(), inner.bottom() - size.y),
        egui_plot::Corner::RightBottom => inner.right_bottom() - size,
    };
    let rect = egui::Rect::from_min_size(min, size);
    let painter = ui.painter().with_clip_rect(frame.intersect(ui.clip_rect()));
    let radius = crate::widgets::theme::CONTROL_RADIUS;
    painter.rect(
        rect,
        radius,
        tokens.card,
        egui::Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );
    let mut top = rect.top() + LEGEND_MARGIN.y;
    for (galley, color) in rows {
        let center_y = top + row_height / 2.0;
        let left = rect.left() + LEGEND_MARGIN.x;
        painter.circle_filled(egui::pos2(left + dot / 2.0, center_y), dot / 2.0, color);
        let text_at = egui::pos2(left + dot + spacing, center_y - galley.size().y / 2.0);
        painter.galley(text_at, galley, weak);
        top += row_height;
    }
}

/// An entry of the legend of a title row, see [`title_legend`].
pub(super) struct TitleEntry<'a> {
    /// The path of the trace, shown on hover of its name.
    pub path: &'a str,
    /// The name of the trace.
    pub name: &'a str,
    /// The colour of its dot.
    pub color: egui::Color32,
    /// The value at the cursor, and the value in full for its hover.
    pub value: Option<(String, String)>,
    /// The width of the slot of the value, in which it keeps its place while it
    /// changes; 0 in a legend without values.
    pub slot: f32,
}

impl TitleEntry<'_> {
    /// Whether the entry has a slot for a value.
    fn has_value(&self) -> bool {
        self.slot > 0.0 || self.value.is_some()
    }
}

/// The room before the first entry of a title legend, and before each entry, in points.
const TITLE_LEGEND_INDENT: f32 = 8.0;
const TITLE_ENTRY_INDENT: f32 = 6.0;
/// The room between the entries of a title legend and their unit, in points.
const TITLE_UNIT_GAP: f32 = 4.0;
/// The least width of a name in a title legend, in points; names that would get less
/// are left out for "…".
const MIN_NAME_WIDTH: f32 = 16.0;

/// Shows the legend of the title row of a view: a dot, the name and the value of each
/// of `entries`, its path on hover, and the `unit` they share once at the end. The
/// entries that do not fit are left out for "…", the last one shown may have its name
/// cut off. The unit keeps its place at the end, unless leaving it out lets all entries
/// fit.
pub(super) fn title_legend(ui: &mut egui::Ui, entries: &[TitleEntry<'_>], unit: Option<&str>) {
    let weak = ui.visuals().weak_text_color();
    let gap = ui.spacing().item_spacing.x;
    let width_of = |text: &str| super::text_size(ui, text, egui::TextStyle::Body).x;
    let more = width_of("…") + gap;
    // each part counts with the gap after it; the unit, or the end, takes the last one
    let widths: Vec<(f32, f32)> = (entries.iter())
        .map(|entry| {
            let value = if entry.has_value() {
                entry.slot + gap
            } else {
                0.0
            };
            let fixed = TITLE_ENTRY_INDENT + super::LEGEND_DOT_SIZE + 2.0 * gap + value;
            (fixed, width_of(entry.name))
        })
        .collect();
    // the unit, which the axis shows too, gives way if all entries fit without it
    let total: f32 = widths.iter().map(|(fixed, name)| fixed + name).sum();
    let free = ui.available_width() - TITLE_LEGEND_INDENT;
    let unit =
        unit.filter(|unit| total + TITLE_UNIT_GAP + width_of(unit) <= free || total - gap > free);
    let end = unit.map_or(-gap, |unit| TITLE_UNIT_GAP + width_of(unit));
    let room = free - end;
    let names = fit_title_legend(&widths, room, more);
    ui.add_space(TITLE_LEGEND_INDENT);
    let height = ui.spacing().interact_size.y;
    let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
    for ((entry, name_width), (_, full)) in entries.iter().zip(&names).zip(&widths) {
        ui.add_space(TITLE_ENTRY_INDENT);
        super::legend_dot(ui, entry.color);
        let name = egui::RichText::new(entry.name).color(weak);
        if name_width < full {
            let size = egui::vec2(*name_width, height);
            ui.allocate_ui_with_layout(size, left_to_right, |ui| {
                ui.add(egui::Label::new(name).truncate())
                    .on_hover_text(entry.path);
            });
        } else {
            let label = egui::Label::new(name).wrap_mode(egui::TextWrapMode::Extend);
            ui.add(label).on_hover_text(entry.path);
        }
        if entry.has_value() {
            let value = entry.value.as_ref();
            let text = value.map(|(text, _)| text.as_str());
            let exact = value.map(|(_, exact)| exact.as_str());
            legend_value(ui, text, exact, entry.slot);
        }
    }
    // a name cut off at the end, with no value after it, ends in "…" already
    let shown = names.len();
    let ends_cut = shown > 0 && names[shown - 1] < widths[shown - 1].1;
    let ends_in_dots = ends_cut && !entries[shown - 1].has_value();
    if shown < entries.len() && !ends_in_dots && ui.available_width() >= more {
        ui.label(egui::RichText::new("…").color(weak));
    }
    if let Some(unit) = unit {
        ui.add_space(TITLE_UNIT_GAP);
        ui.label(egui::RichText::new(unit).color(weak));
    }
}

/// The widths of the names of the entries of a title legend that fit in `room`, in
/// order. Each entry is the width of all but its name and the width of its name, each
/// with the gap after it; `more` is the width of the "…" for the entries left out. All
/// entries keep their names whole if they fit; else the entries that fit whole are
/// shown, then the next one with its name cut off if it keeps [`MIN_NAME_WIDTH`] of
/// it, with room for "…" while entries are left out after it.
pub(super) fn fit_title_legend(entries: &[(f32, f32)], room: f32, more: f32) -> Vec<f32> {
    let total: f32 = entries.iter().map(|(fixed, name)| fixed + name).sum();
    if total <= room {
        return entries.iter().map(|&(_, name)| name).collect();
    }
    let mut names = Vec::new();
    let mut used = 0.0;
    for (index, &(fixed, name)) in entries.iter().enumerate() {
        let after = if index + 1 < entries.len() { more } else { 0.0 };
        let left = room - used - after;
        if fixed + name <= left {
            names.push(name);
            used += fixed + name;
            continue;
        }
        let cut = left - fixed;
        if cut >= MIN_NAME_WIDTH.min(name) {
            names.push(cut);
        }
        break;
    }
    names
}

/// Shows `value` right-aligned in a slot `width` wide, `exact` on hover.
fn legend_value(ui: &mut egui::Ui, value: Option<&str>, exact: Option<&str>, width: f32) {
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        if let Some(value) = value {
            let label = ui.label(egui::RichText::new(value).strong());
            if let Some(exact) = exact.filter(|&exact| exact != value) {
                label.on_hover_text(exact);
            }
        }
    });
}

/// The least thickness of the y axis of the time plots, in points.
pub(super) const TIME_Y_AXIS_THICKNESS: f32 = 50.0;

/// The y axis of the time plots, as thick in all so they line up.
pub(super) fn time_y_axis() -> egui_plot::AxisHints<'static> {
    egui_plot::AxisHints::new_y()
        .placement(egui_plot::HPlacement::Left)
        .min_thickness(TIME_Y_AXIS_THICKNESS)
}

#[cfg(test)]
mod tests;
