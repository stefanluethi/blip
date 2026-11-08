//! A heatmap of an array trace: a 1D array over time, each element a row of colours,
//! or a 2D array as a grid of coloured cells at the cursor time. A 1D array can also
//! be shown at the cursor time as a line over its indices.

use super::Selectable;
use super::plot::{LineStyle, ShowPoints};
use super::plotting::{
    index_formatter, index_spacer, index_under, index_x_axis, plot_array, sample_at_cursor,
};
use super::time_axis;
use super::tooltip::{Header, Label, Row, Tooltip, TraceName, Value};
use super::{PropertiesContext, ViewContext, Viewable, short_names, text_color_on};
use crate::app::model::Model;
use crate::app::model::numbers::{DisplayFormat, Number, format_significant};
use crate::app::model::sorted::index_at;
use crate::app::model::traces::{Samples, Trace};
use crate::app::theme;
use crate::app::time::timestamp_at;
use crate::app::time::{TimeBase, TimeRange};
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

named_enum! {
    /// How a 1D array is shown.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum ArrayMode {
        /// Each element as a row of colours over time.
        #[default]
        OverTime => "Over time",
        /// The elements at the cursor time as a line over their indices.
        AtCursor => "At cursor",
    }
}

/// The selected array trace as a heatmap.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Heatmap {
    /// Path of the shown trace, at most one.
    pub traces: BTreeSet<String>,
    pub mode: ArrayMode,
    /// The values at the ends of the colour scale, or `None` to fit the shown values.
    pub value_range: Option<(f64, f64)>,
    /// The range of the values shown in the last frame.
    #[serde(skip)]
    fitted_range: Option<(f64, f64)>,
    #[serde(skip)]
    texture: Option<egui::TextureHandle>,
    /// A hash of what the texture shows: its values, its size and the colour scale,
    /// to upload it again only when they change.
    #[serde(skip)]
    uploaded: Option<u64>,
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

/// How the elements of the shown array are written.
#[derive(Clone, Copy)]
struct ValueText<'a> {
    format: DisplayFormat,
    unit: Option<&'a str>,
}

impl ValueText<'_> {
    /// `value` written for a cell.
    fn write(self, value: Number) -> String {
        value.format(Some(DIGITS), self.format)
    }

    /// `value` as a tooltip shows it, with its unit.
    fn tooltip_value(self, value: Number) -> Value {
        Value::number(value, DIGITS, self.format, self.unit)
    }
}

/// The tooltip of element `element` of the 1D array `trace` at `t` nanoseconds in the
/// bands: the time and the element, a dash without a sample at `t`.
fn band_tooltip(
    trace: Trace<'_>,
    text: ValueText<'_>,
    base: TimeBase,
    t: u64,
    element: usize,
) -> Tooltip {
    let value = index_at(trace.timestamps, t)
        .and_then(|index| trace.samples.array_sample(index))
        .and_then(|elements| elements.get(element))
        .map_or_else(Value::missing, |number| text.tooltip_value(number));
    let row = Row::new(Label::Weak(format!("[{element}]")), [value]);
    Tooltip::new(Header::Text(base.format(t, 3))).group(vec![row])
}

/// The tooltip of the cell at `(row, column)` of the 2D array `name` with `value`.
fn cell_tooltip(
    name: &str,
    text: ValueText<'_>,
    (row, column): (usize, usize),
    value: Number,
) -> Tooltip {
    let rows = vec![Row::new(
        Label::Weak(name.to_owned()),
        [text.tooltip_value(value)],
    )];
    Tooltip::new(Header::Text(format!("[{row}][{column}]"))).group(rows)
}

/// The tooltip of element `index` of the 1D array trace `name` at `path` with `value`.
fn element_tooltip(
    name: &str,
    path: &str,
    text: ValueText<'_>,
    index: usize,
    value: Number,
) -> Tooltip {
    let trace = TraceName::new(name, path, 0);
    let row = Row::new(Label::Trace(trace), [text.tooltip_value(value)]);
    Tooltip::new(Header::Text(format!("[{index}]"))).group(vec![row])
}

/// The values of an array trace of `len` elements within `(start, end)` seconds as a
/// grid of `columns` columns, each with the sample current at its middle, and a row per
/// element with element 0 in the bottom row. NaN where there is no sample, also after
/// `latest` seconds. The times count from `origin` in nanoseconds.
fn over_time(
    trace: Trace<'_>,
    len: usize,
    (start, end): (f64, f64),
    latest: f64,
    columns: usize,
    origin: u64,
) -> Vec<f64> {
    let mut grid = vec![f64::NAN; len * columns];
    let column_width = (end - start) / columns as f64;
    for column in 0..columns {
        let t = start + (column as f64 + 0.5) * column_width;
        if t > latest {
            break;
        }
        let Some(elements) = index_at(trace.timestamps, timestamp_at(origin, t))
            .and_then(|index| trace.samples.array_sample(index))
        else {
            continue;
        };
        for (element, value) in elements.iter().enumerate() {
            grid[(len - 1 - element) * columns + column] = value.as_f64();
        }
    }
    grid
}

impl Heatmap {
    /// The values at the ends of the colour scale.
    fn shown_range(&self) -> (f64, f64) {
        self.value_range.or(self.fitted_range).unwrap_or((0.0, 1.0))
    }

    /// Uploads the colours of `values` as an image of `size` pixels unless they are
    /// uploaded already, returns its texture.
    fn texture(
        &mut self,
        ctx: &egui::Context,
        size: [usize; 2],
        values: &[f64],
    ) -> egui::TextureId {
        use std::hash::{Hash, Hasher};
        let range = self.shown_range();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for value in values {
            value.to_bits().hash(&mut hasher);
        }
        (size, range.0.to_bits(), range.1.to_bits()).hash(&mut hasher);
        let shown = hasher.finish();
        if let Some(texture) = &self.texture
            && self.uploaded == Some(shown)
        {
            return texture.id();
        }
        self.uploaded = Some(shown);
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

    /// Shows the elements of a 1D array trace over the shared window of `time`.
    fn over_time_ui(
        &mut self,
        ui: &mut egui::Ui,
        trace: Trace<'_>,
        text: ValueText<'_>,
        len: usize,
        time: &mut TimeRange,
        size: egui::Vec2,
    ) {
        let columns = (size.x.max(1.0) as usize).min(MAX_COLUMNS);
        let values = match (time.range(), time.latest()) {
            (Some(range), Some(latest)) => {
                over_time(trace, len, range, latest, columns, time.origin())
            }
            _ => vec![],
        };
        self.fitted_range = value_range(&values);
        let texture = (!values.is_empty()).then(|| self.texture(ui.ctx(), [columns, len], &values));

        let id = ui.id().with("heatmap");
        let plot = time_axis::plot(id, time, ui.visuals())
            .width(size.x)
            .height(size.y)
            .show_grid(false)
            .y_axis_label("index")
            .y_grid_spacer(index_spacer)
            .y_axis_formatter(index_formatter(false));
        let response = time_axis::show(id, plot, ui, time, false, move |plot_ui, axis| {
            plot_ui.set_plot_bounds_y(-0.5..=len as f64 - 0.5);
            if let (Some(texture), Some((start, end))) = (texture, axis.range) {
                let center =
                    egui_plot::PlotPoint::new((start + end) / 2.0, (len as f64 - 1.0) / 2.0);
                let size = egui::vec2((end - start) as f32, len as f32);
                plot_ui
                    .image(egui_plot::PlotImage::new("", texture, center, size).allow_hover(false));
            }
        });

        if let (Some(t), Some(y)) = (response.hovered, response.hovered_y) {
            if let Some(element) = index_under(y, len) {
                let t = response.axis.timestamp(t);
                let tooltip = band_tooltip(trace, text, response.axis.base, t, element);
                response
                    .response
                    .on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
            }
        }
    }

    /// Shows the elements of a 2D array trace of `rows` × `columns` at the cursor time
    /// as a grid of cells, row 0 at the top.
    fn grid_ui(
        &mut self,
        ui: &mut egui::Ui,
        trace: Trace<'_>,
        text: ValueText<'_>,
        (rows, columns): (usize, usize),
        time: &TimeRange,
        size: egui::Vec2,
    ) {
        let numbers = sample_at_cursor(trace, time);
        let values: Vec<f64> = numbers.iter().map(|number| number.as_f64()).collect();
        self.fitted_range = value_range(&values);
        let range = self.shown_range();
        let texture =
            (!values.is_empty()).then(|| self.texture(ui.ctx(), [columns, rows], &values));

        let plot = egui_plot::Plot::new(ui.id().with("grid"))
            .grid_color(theme::grid_color(ui.visuals()))
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
                for (index, &number) in numbers.iter().enumerate() {
                    let (row, column) = (index / columns, index % columns);
                    let position = egui_plot::PlotPoint::new(column as f64, -(row as f64));
                    let color = text_color_on(color_of(number.as_f64(), range));
                    plot_ui.text(
                        egui_plot::Text::new("", position, text.write(number))
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
            let row = index_under(-pointer.y, rows)?;
            let column = index_under(pointer.x, columns)?;
            Some((row, column, numbers[row * columns + column]))
        });

        super::zoom_on_wheel(&response.response);
        if let Some((row, column, value)) = response.inner {
            let name = short_names([trace.path])[0];
            let tooltip = cell_tooltip(name, text, (row, column), value);
            response
                .response
                .on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
        }
    }

    /// Shows the elements of a 1D array trace at the cursor time as a line over their
    /// indices.
    fn line_ui(
        &mut self,
        ui: &mut egui::Ui,
        trace: Trace<'_>,
        text: ValueText<'_>,
        time: &TimeRange,
    ) {
        let numbers = sample_at_cursor(trace, time);
        let values: Vec<f64> = numbers.iter().map(|number| number.as_f64()).collect();
        self.fitted_range = value_range(&values);
        let fixed_range = self.value_range;
        let color = theme::trace_color(ui.visuals(), 0);

        let plot = egui_plot::Plot::new(ui.id().with("line"))
            .grid_color(theme::grid_color(ui.visuals()))
            .custom_x_axes(vec![index_x_axis("index")])
            .x_grid_spacer(index_spacer)
            .allow_boxed_zoom(false);
        let response = plot.show(ui, |plot_ui| {
            if let Some((min, max)) = fixed_range {
                plot_ui.set_plot_bounds_y(min..=max);
            }
            let (style, points) = (LineStyle::Solid, ShowPoints::Always);
            plot_array(trace.path, &values, color, style, points, plot_ui);
            let pointer = plot_ui
                .response()
                .hovered()
                .then(|| plot_ui.pointer_coordinate())
                .flatten()?;
            index_under(pointer.x, numbers.len())
        });
        super::zoom_on_wheel(&response.response);
        if let Some(index) = response.inner {
            let name = short_names([trace.path])[0];
            let tooltip = element_tooltip(name, trace.path, text, index, numbers[index]);
            response
                .response
                .on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
        }
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

impl super::ViewInfo for Heatmap {
    const NAME: &'static str = "Heatmap";
    const ICON: &'static str = egui_phosphor::regular::GRID_NINE;
    const SHOWS_ONE_TRACE: bool = true;

    fn selectable(trace: Trace<'_>) -> Selectable {
        match trace.samples {
            Samples::Arrays { shape, .. } if (1..=2).contains(&shape.len()) => Selectable::Trace,
            _ => Selectable::No("Heatmaps show arrays of one or two dimensions".into()),
        }
    }
}

impl Viewable for Heatmap {
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.traces, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let model: &Model = context.model;
        let Some(path) = self.traces.first() else {
            theme::hint(ui, "Select an array trace in the traces panel");
            return;
        };
        let Some(trace) = model.traces.get(path) else {
            theme::hint(ui, &format!("{path} has not been received"));
            return;
        };
        let Samples::Arrays { shape, .. } = &trace.samples else {
            theme::hint(ui, &format!("{path} is not an array"));
            return;
        };
        if shape.contains(&0) || !(1..=2).contains(&shape.len()) {
            theme::hint(
                ui,
                &format!("{path} has no elements or more than two dimensions"),
            );
            return;
        }
        let text = ValueText {
            format: context.formats.of(path),
            unit: model.traces.unit(path),
        };
        self.contents_ui(ui, trace, text, shape, context.time);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let model = context.model;
        let dimensions = self
            .traces
            .first()
            .and_then(|path| model.traces.get(path))
            .and_then(|trace| match &trace.samples {
                Samples::Arrays { shape, .. } => Some(shape.len()),
                _ => None,
            });
        // the mode is kept for a 2D array, but only applies to a 1D one
        if dimensions != Some(2) {
            super::properties_section(ui, "Array", |ui| {
                super::choice_row(ui, "Show", &mut self.mode, &ArrayMode::CHOICES);
            });
        }
        let fitted = self.shown_range();
        super::properties_section(ui, "Colour scale", |ui| {
            super::range_rows(ui, &mut self.value_range, fitted);
        });
    }
}

impl Heatmap {
    /// Shows the array trace of `shape` as chosen in the options.
    fn contents_ui(
        &mut self,
        ui: &mut egui::Ui,
        trace: Trace<'_>,
        text: ValueText<'_>,
        shape: &[usize],
        time: &mut TimeRange,
    ) {
        if shape.len() == 1 && self.mode == ArrayMode::AtCursor {
            self.line_ui(ui, trace, text, time);
            return;
        }

        let height = ui.available_height();
        let size = egui::vec2((ui.available_width() - COLOR_BAR_WIDTH).max(50.0), height);
        ui.horizontal(|ui| {
            match *shape {
                [len] => self.over_time_ui(ui, trace, text, len, time, size),
                [rows, columns] => {
                    self.grid_ui(ui, trace, text, (rows, columns), time, size);
                }
                _ => {}
            }
            self.color_bar_ui(ui, height);
        });
    }
}

#[cfg(test)]
mod tests;
