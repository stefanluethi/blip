//! A heatmap of an array stream: a 1D array over time, each element a row of colours,
//! or a 2D array as a grid of coloured cells at the cursor time. A 1D array can also
//! be shown at the cursor time as a line over its indices.

use super::time_axis::{self, index_at};
use super::{ViewContext, Viewable, text_color_on};
use crate::app::model::Model;
use crate::app::model::streams::{NumberKind, Samples, Stream, format_significant};
use crate::app::time::TimeRange;
use egui::Color32;
use std::collections::BTreeSet;

/// Width of the colour bar next to the heatmap, with its labels.
const COLOR_BAR_WIDTH: f32 = 70.0;
/// Upper bound of the columns of a heatmap over time.
const MAX_COLUMNS: usize = 2048;
/// Cells of a grid at least this wide in points show their value.
const MIN_LABELLED_CELL_WIDTH: f64 = 40.0;

/// The viridis colour map at equal steps from low to high values.
const COLOR_MAP: [Color32; 9] = [
    Color32::from_rgb(0x44, 0x01, 0x54),
    Color32::from_rgb(0x48, 0x28, 0x78),
    Color32::from_rgb(0x3e, 0x49, 0x89),
    Color32::from_rgb(0x31, 0x68, 0x8e),
    Color32::from_rgb(0x26, 0x82, 0x8e),
    Color32::from_rgb(0x1f, 0x9e, 0x89),
    Color32::from_rgb(0x35, 0xb7, 0x79),
    Color32::from_rgb(0x6e, 0xce, 0x58),
    Color32::from_rgb(0xfd, 0xe7, 0x25),
];

/// How a 1D array is shown.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ArrayMode {
    /// Each element as a row of colours over time.
    #[default]
    OverTime,
    /// The elements at the cursor time as a line over their indices.
    AtCursor,
}

/// The selected array stream as a heatmap.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Heatmap {
    /// Path of the shown stream, at most one.
    pub streams: BTreeSet<String>,
    pub mode: ArrayMode,
    /// The values at the ends of the colour scale, or `None` to fit the shown values.
    pub value_range: Option<(f64, f64)>,
    /// The range of the values shown in the last frame.
    #[serde(skip)]
    fitted_range: Option<(f64, f64)>,
    #[serde(skip)]
    texture: Option<egui::TextureHandle>,
}

/// The colour of `value` on the scale from `min` to `max`, transparent for NaN.
fn color_of(value: f64, (min, max): (f64, f64)) -> Color32 {
    if value.is_nan() {
        return Color32::TRANSPARENT;
    }
    let fraction = if max > min {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let scaled = fraction * (COLOR_MAP.len() - 1) as f64;
    let index = (scaled.floor() as usize).min(COLOR_MAP.len() - 2);
    let t = (scaled - index as f64) as f32;
    let (low, high) = (COLOR_MAP[index], COLOR_MAP[index + 1]);
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Color32::from_rgb(
        mix(low.r(), high.r()),
        mix(low.g(), high.g()),
        mix(low.b(), high.b()),
    )
}

/// The smallest and the largest of `values` that are not NaN.
fn value_range(values: &[f64]) -> Option<(f64, f64)> {
    values
        .iter()
        .filter(|value| !value.is_nan())
        .fold(None, |range, &value| match range {
            None => Some((value, value)),
            Some((min, max)) => Some((value.min(min), value.max(max))),
        })
}

/// Significant digits of the values shown in a heatmap.
const DIGITS: usize = 3;

/// `value` of an array of `kind` written for a cell or a tooltip.
fn format_element(value: f64, kind: NumberKind) -> String {
    kind.format_digits(value, Some(DIGITS))
}

/// The values of an array stream of `len` elements within `(start, end)` seconds as a
/// grid of `columns` columns, each with the sample current at its middle, and a row per
/// element with element 0 in the bottom row. NaN where there is no sample, also after
/// `latest` seconds.
fn over_time(
    stream: &Stream,
    len: usize,
    (start, end): (f64, f64),
    latest: f64,
    columns: usize,
) -> Vec<f64> {
    let mut grid = vec![f64::NAN; len * columns];
    let column_width = (end - start) / columns as f64;
    for column in 0..columns {
        let t = start + (column as f64 + 0.5) * column_width;
        if t > latest {
            break;
        }
        let Some(elements) =
            index_at(&stream.timestamps, t).and_then(|index| stream.samples.array_sample(index))
        else {
            continue;
        };
        for (element, &value) in elements.iter().enumerate() {
            grid[(len - 1 - element) * columns + column] = value;
        }
    }
    grid
}

/// Labels only the integer marks of an axis, negated if `negate`.
fn index_formatter(
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
fn index_x_axis(label: &str) -> egui_plot::AxisHints<'static> {
    egui_plot::AxisHints::new_x()
        .label(label)
        .formatter(index_formatter(false))
        // indices are short, so their labels fit closer together than other numbers
        .label_spacing(30.0..=45.0)
}

/// Marks an index axis at whole multiples of 1, 2, 5, 10, 20, … far enough apart to
/// label each of them.
fn index_spacer(input: egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
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

impl Heatmap {
    pub fn new() -> Self {
        Self::default()
    }

    /// The values at the ends of the colour scale.
    fn shown_range(&self) -> (f64, f64) {
        self.value_range.or(self.fitted_range).unwrap_or((0.0, 1.0))
    }

    /// Shows the mode for a 1D array, the range of the values and the cursor time
    /// described by `cursor_label` if values at the cursor are shown.
    fn options_ui(&mut self, ui: &mut egui::Ui, dimensions: usize, cursor_label: Option<String>) {
        ui.horizontal(|ui| {
            if dimensions == 1 {
                ui.selectable_value(&mut self.mode, ArrayMode::OverTime, "Over time");
                ui.selectable_value(&mut self.mode, ArrayMode::AtCursor, "At cursor");
                ui.separator();
            }
            let mut fixed = self.value_range.is_some();
            if ui
                .checkbox(&mut fixed, "Fixed range")
                .on_hover_text("Keep the range of the values instead of fitting the shown ones")
                .changed()
            {
                self.value_range = fixed.then(|| self.shown_range());
            }
            if let Some((min, max)) = &mut self.value_range {
                let speed = ((*max - *min).abs() * 0.01).max(1.0e-6);
                ui.add(egui::DragValue::new(min).speed(speed));
                ui.label("to");
                ui.add(egui::DragValue::new(max).speed(speed));
            }
            let at_cursor = dimensions == 2 || self.mode == ArrayMode::AtCursor;
            if at_cursor && let Some(label) = cursor_label {
                ui.separator();
                ui.weak(format!("at {label}"));
            }
        });
    }

    /// Uploads the colours of `values` as an image of `size` pixels, returns its texture.
    fn texture(
        &mut self,
        ctx: &egui::Context,
        size: [usize; 2],
        values: &[f64],
    ) -> egui::TextureId {
        let range = self.shown_range();
        let pixels = values.iter().map(|&value| color_of(value, range)).collect();
        let image = egui::ColorImage::new(size, pixels);
        let options = egui::TextureOptions::NEAREST;
        match &mut self.texture {
            Some(texture) => {
                texture.set(image, options);
                texture.id()
            }
            None => {
                let texture = ctx.load_texture("heatmap", image, options);
                let id = texture.id();
                self.texture = Some(texture);
                id
            }
        }
    }

    /// Shows the elements of a 1D array stream over the shared window of `time`.
    fn over_time_ui(
        &mut self,
        ui: &mut egui::Ui,
        stream: &Stream,
        kind: NumberKind,
        len: usize,
        time: &mut TimeRange,
        size: egui::Vec2,
    ) {
        let columns = (size.x.max(1.0) as usize).min(MAX_COLUMNS);
        let values = match (time.range(), time.latest()) {
            (Some(range), Some(latest)) => over_time(stream, len, range, latest, columns),
            _ => vec![],
        };
        self.fitted_range = value_range(&values);
        let texture = (!values.is_empty()).then(|| self.texture(ui.ctx(), [columns, len], &values));

        let plot = time_axis::plot(ui.id().with("heatmap"), time)
            .width(size.x)
            .height(size.y)
            .show_grid(false)
            .y_axis_label("index")
            .y_grid_spacer(index_spacer)
            .y_axis_formatter(index_formatter(false));
        let response = time_axis::show(plot, ui, time, move |plot_ui, axis| {
            plot_ui.set_plot_bounds_y(-0.5..=len as f64 - 0.5);
            if let (Some(texture), Some((start, end))) = (texture, axis.range) {
                let center = egui_plot::PlotPoint::new(
                    (start + end) / 2.0 - axis.origin,
                    (len as f64 - 1.0) / 2.0,
                );
                let size = egui::vec2((end - start) as f32, len as f32);
                plot_ui
                    .image(egui_plot::PlotImage::new("", texture, center, size).allow_hover(false));
            }
        });

        if let (Some(t), Some(y)) = (response.hovered, response.hovered_y) {
            let element = y.round();
            let value = index_at(&stream.timestamps, t)
                .and_then(|index| stream.samples.array_sample(index))
                .filter(|_| element >= 0.0)
                .and_then(|elements| elements.get(element as usize).copied());
            let base = response.axis.base;
            response.response.on_hover_ui_at_pointer(|ui| {
                ui.label(base.format(t, 3));
                if let Some(value) = value {
                    ui.label(format!("[{element}] = {}", format_element(value, kind)));
                }
            });
        }
    }

    /// Shows the elements of a 2D array stream of `rows` × `columns` at the cursor time
    /// as a grid of cells, row 0 at the top.
    fn grid_ui(
        &mut self,
        ui: &mut egui::Ui,
        stream: &Stream,
        kind: NumberKind,
        (rows, columns): (usize, usize),
        time: &TimeRange,
        size: egui::Vec2,
    ) {
        let values: Vec<f64> = time
            .cursor()
            .and_then(|t| index_at(&stream.timestamps, t))
            .and_then(|index| stream.samples.array_sample(index))
            .map(<[f64]>::to_vec)
            .unwrap_or_default();
        self.fitted_range = value_range(&values);
        let range = self.shown_range();
        let texture =
            (!values.is_empty()).then(|| self.texture(ui.ctx(), [columns, rows], &values));

        let plot = egui_plot::Plot::new(ui.id().with("grid"))
            .width(size.x)
            .height(size.y)
            .data_aspect(1.0)
            .show_x(false)
            .show_y(false)
            .show_grid(false)
            .allow_boxed_zoom(false)
            .custom_x_axes(vec![index_x_axis("column")])
            .y_axis_label("row")
            .x_grid_spacer(index_spacer)
            .y_grid_spacer(index_spacer)
            .y_axis_formatter(index_formatter(true));
        let response = plot.show(ui, |plot_ui| {
            let texture = texture?;
            // cell (row, column) is centered at (column, -row)
            let center =
                egui_plot::PlotPoint::new((columns as f64 - 1.0) / 2.0, -(rows as f64 - 1.0) / 2.0);
            let size = egui::vec2(columns as f32, rows as f32);
            plot_ui.image(egui_plot::PlotImage::new("", texture, center, size).allow_hover(false));

            if plot_ui.transform().dpos_dvalue()[0].abs() >= MIN_LABELLED_CELL_WIDTH {
                for (index, &value) in values.iter().enumerate() {
                    let (row, column) = (index / columns, index % columns);
                    let position = egui_plot::PlotPoint::new(column as f64, -(row as f64));
                    let color = text_color_on(color_of(value, range));
                    plot_ui.text(
                        egui_plot::Text::new("", position, format_element(value, kind))
                            .color(color)
                            .allow_hover(false),
                    );
                }
            }

            let pointer = plot_ui
                .response()
                .hovered()
                .then(|| plot_ui.pointer_coordinate())
                .flatten()?;
            let (row, column) = ((-pointer.y).round(), pointer.x.round());
            let inside =
                (0.0..rows as f64).contains(&row) && (0.0..columns as f64).contains(&column);
            inside.then(|| {
                (
                    row,
                    column,
                    values[row as usize * columns + column as usize],
                )
            })
        });

        if let Some((row, column, value)) = response.inner {
            response.response.on_hover_ui_at_pointer(|ui| {
                ui.label(format!(
                    "[{row}][{column}] = {}",
                    format_element(value, kind)
                ));
            });
        }
    }

    /// Shows the elements of a 1D array stream at the cursor time as a line over their
    /// indices.
    fn line_ui(&mut self, ui: &mut egui::Ui, stream: &Stream, kind: NumberKind, time: &TimeRange) {
        let values: Vec<f64> = time
            .cursor()
            .and_then(|t| index_at(&stream.timestamps, t))
            .and_then(|index| stream.samples.array_sample(index))
            .map(<[f64]>::to_vec)
            .unwrap_or_default();
        self.fitted_range = value_range(&values);
        let fixed_range = self.value_range;
        let color = super::plot::curve_color(0);
        let points: Vec<[f64; 2]> = values
            .iter()
            .enumerate()
            .map(|(index, &value)| [index as f64, value])
            .collect();

        let plot = egui_plot::Plot::new(ui.id().with("line"))
            .custom_x_axes(vec![index_x_axis("index")])
            .x_grid_spacer(index_spacer)
            .allow_boxed_zoom(false)
            .label_formatter(move |hover| match hover {
                egui_plot::HoverPosition::NearDataPoint { position, .. } => Some(format!(
                    "[{}] = {}",
                    position.x,
                    format_element(position.y, kind)
                )),
                egui_plot::HoverPosition::Elsewhere { .. } => None,
            });
        plot.show(ui, |plot_ui| {
            if let Some((min, max)) = fixed_range {
                plot_ui.set_plot_bounds_y(min..=max);
            }
            for segment in points.split(|point| point[1].is_nan()) {
                plot_ui.line(egui_plot::Line::new(&stream.path, segment.to_vec()).color(color));
            }
            let markers: Vec<[f64; 2]> = points
                .iter()
                .copied()
                .filter(|point| !point[1].is_nan())
                .collect();
            plot_ui.points(
                egui_plot::Points::new(&stream.path, markers)
                    .color(color)
                    .radius(2.5),
            );
        });
    }

    /// Shows the colour scale from the low values at the bottom to the high ones at the
    /// top.
    fn color_bar_ui(&self, ui: &mut egui::Ui, height: f32) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(COLOR_BAR_WIDTH, height), egui::Sense::hover());
        let bar = egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(8.0, 10.0),
            egui::pos2(rect.left() + 22.0, rect.bottom() - 40.0),
        );
        if bar.height() <= 0.0 {
            return;
        }
        let (min, max) = self.shown_range();
        let painter = ui.painter();
        const STEPS: usize = 64;
        for step in 0..STEPS {
            let fraction = step as f32 / STEPS as f32;
            let slice = egui::Rect::from_x_y_ranges(
                bar.x_range(),
                bar.bottom() - bar.height() * (fraction + 1.0 / STEPS as f32)
                    ..=bar.bottom() - bar.height() * fraction,
            );
            let value = min + (max - min) * (f64::from(fraction) + 0.5 / STEPS as f64);
            painter.rect_filled(
                slice.expand2(egui::vec2(0.0, 0.5)),
                0.0,
                color_of(value, (min, max)),
            );
        }
        let font = egui::TextStyle::Small.resolve(ui.style());
        let color = ui.visuals().text_color();
        for (fraction, value) in [(0.0, min), (0.5, (min + max) / 2.0), (1.0, max)] {
            let y = bar.bottom() - bar.height() * fraction;
            painter.text(
                egui::pos2(bar.right() + 4.0, y),
                egui::Align2::LEFT_CENTER,
                format_significant(value, DIGITS),
                font.clone(),
                color,
            );
        }
    }
}

impl Viewable for Heatmap {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let model: &Model = context.model;
        let Some(path) = self.streams.first() else {
            ui.weak("Select an array stream in the streams panel");
            return;
        };
        let Some(stream) = model.streams.get(path) else {
            ui.weak(format!("{path} has not been received"));
            return;
        };
        let Samples::Arrays { shape, kind, .. } = &stream.samples else {
            ui.weak(format!("{path} is not an array"));
            return;
        };
        if shape.contains(&0) || !(1..=2).contains(&shape.len()) {
            ui.weak(format!(
                "{path} has no elements or more than two dimensions"
            ));
            return;
        }
        let kind = kind.unwrap_or(NumberKind::Float);
        let cursor_label = context.time.cursor_label();
        super::settings_below(
            ui,
            self,
            |heatmap, ui| heatmap.options_ui(ui, shape.len(), cursor_label),
            |heatmap, ui| heatmap.contents_ui(ui, stream, kind, shape, context.time),
        );
    }
}

impl Heatmap {
    /// Shows the array stream of `shape` as chosen in the options.
    fn contents_ui(
        &mut self,
        ui: &mut egui::Ui,
        stream: &Stream,
        kind: NumberKind,
        shape: &[usize],
        time: &mut TimeRange,
    ) {
        if shape.len() == 1 && self.mode == ArrayMode::AtCursor {
            self.line_ui(ui, stream, kind, time);
            return;
        }

        let height = ui.available_height();
        let size = egui::vec2((ui.available_width() - COLOR_BAR_WIDTH).max(50.0), height);
        ui.horizontal(|ui| {
            match *shape {
                [len] => self.over_time_ui(ui, stream, kind, len, time, size),
                [rows, columns] => {
                    self.grid_ui(ui, stream, kind, (rows, columns), time, size);
                }
                _ => {}
            }
            self.color_bar_ui(ui, height);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::streams::Streams;
    use proto::Data;

    #[test]
    fn colors_follow_the_scale() {
        assert_eq!(color_of(0.0, (0.0, 1.0)), COLOR_MAP[0]);
        assert_eq!(color_of(1.0, (0.0, 1.0)), COLOR_MAP[8]);
        assert_eq!(color_of(5.0, (0.0, 1.0)), COLOR_MAP[8]);
        assert_eq!(color_of(0.5, (0.0, 1.0)), COLOR_MAP[4]);
        assert_eq!(color_of(f64::NAN, (0.0, 1.0)), Color32::TRANSPARENT);
        assert_eq!(color_of(3.0, (3.0, 3.0)), COLOR_MAP[4]);
    }

    #[test]
    fn value_range_skips_nulls() {
        assert_eq!(value_range(&[f64::NAN, 2.0, -1.0]), Some((-1.0, 2.0)));
        assert_eq!(value_range(&[f64::NAN]), None);
    }

    #[test]
    fn over_time_holds_each_sample_until_the_next() {
        let mut streams = Streams::new();
        for (second, elements) in [(1, [1.0, 2.0]), (3, [3.0, 4.0])] {
            streams.append(&proto::Value {
                timestamp: second * 1_000_000_000,
                path: "a".to_owned(),
                value: Data::Array(elements.iter().copied().map(Data::Float).collect()),
            });
        }
        let stream = streams.get("a").unwrap();
        // columns centered at 0.5, 1.5, ... 5.5 seconds
        let grid = over_time(stream, 2, (0.0, 6.0), 4.0, 6);
        let bits = |values: &[f64]| {
            values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        };
        let nan = f64::NAN;
        // element 1 is the top row
        assert_eq!(bits(&grid[..6]), bits(&[nan, 2.0, 2.0, 4.0, nan, nan]));
        assert_eq!(bits(&grid[6..]), bits(&[nan, 1.0, 1.0, 3.0, nan, nan]));
    }
}
