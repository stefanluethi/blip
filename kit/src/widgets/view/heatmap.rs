//! A heatmap of an array trace: a 1D array over time, each element a row of colours,
//! or a 2D array as a grid of coloured cells at the cursor time. A 1D array can also
//! be shown at the cursor time as a line over its indices.

use super::Selectable;
use super::plot::{LineStyles, ShowPoints};
use super::plotting::{
    index_formatter, index_spacer, index_under, index_x_axis, plot_array, range_moved,
    sample_at_cursor,
};
use super::time_axis;
use super::tooltip::{Header, Label, Row, Tooltip, TraceName, Value};
use super::{Ends, Fit, PropertiesContext, ViewContext, Viewable, short_names, text_color_on};
use crate::store::numbers::{DisplayFormat, Labels, Number, SHOWN_DIGITS, format_significant};
use crate::store::sorted::index_at;
use crate::store::traces::{Samples, Trace};
use crate::store::{Store, Window};
use crate::widgets::theme;
use crate::widgets::time::timestamp_at;
use crate::widgets::time::{TimeBase, TimeRange};
use egui::Color32;
use std::collections::BTreeSet;

/// Width of the colour bar next to the heatmap, with its labels.
const COLOR_BAR_WIDTH: f32 = 70.0;
/// Upper bound of the columns of a heatmap over time.
const MAX_COLUMNS: usize = 2048;
/// Cells of a grid at least this wide in points show their value.
const MIN_LABELLED_CELL_WIDTH: f64 = 40.0;
/// The room the axes of a grid take before its first frame is shown, in points.
const GRID_AXES: egui::Vec2 = egui::vec2(50.0, 30.0);
/// The margin of the y axis at the cursor time fitted to the values, of their span.
const Y_MARGIN_FRACTION: f64 = 0.05;

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
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Heatmap {
    /// Path of the shown trace, at most one.
    pub traces: BTreeSet<String>,
    pub mode: ArrayMode,
    /// The values at the ends of the colour scale, or of the y axis of a 1D array at
    /// the cursor time, each `None` to fit, see [`Heatmap::fit`].
    pub value_range: Ends,
    /// What the ends not set fit.
    pub fit: Fit,
    /// When the elements of a 1D array at the cursor time are marked.
    pub points: ShowPoints,
    /// Width of the line of a 1D array at the cursor time in points.
    pub line_width: f32,
    #[serde(skip_serializing_if = "LineStyles::is_empty")]
    pub line_styles: LineStyles,
    /// The y range of a 1D array at the cursor time panned or zoomed to instead of the
    /// one of `value_range`, until a double click.
    #[serde(skip)]
    y_view: Option<(f64, f64)>,
    /// The range of the values shown in the last frame.
    #[serde(skip)]
    fitted_range: Option<(f64, f64)>,
    /// The range the shown trace declares, set when the view or its properties are shown.
    #[serde(skip)]
    expected: Option<(f64, f64)>,
    #[serde(skip)]
    texture: Option<egui::TextureHandle>,
    /// A hash of what the texture shows: its values, its size and the colour scale,
    /// to upload it again only when they change.
    #[serde(skip)]
    uploaded: Option<u64>,
}

impl Default for Heatmap {
    fn default() -> Self {
        Self {
            traces: BTreeSet::new(),
            mode: ArrayMode::default(),
            value_range: Ends::default(),
            fit: Fit::default(),
            // the elements of an array are few and far apart
            points: ShowPoints::Always,
            line_width: 1.5,
            line_styles: LineStyles::default(),
            y_view: None,
            fitted_range: None,
            expected: None,
            texture: None,
            uploaded: None,
        }
    }
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

/// How the elements of the shown array are written.
#[derive(Clone, Copy)]
struct ValueText<'a> {
    format: DisplayFormat,
    unit: Option<&'a str>,
    labels: Option<&'a Labels>,
    description: Option<&'a str>,
}

impl ValueText<'_> {
    /// `value` written for a cell.
    fn write(self, value: Number) -> String {
        value.format(Some(SHOWN_DIGITS), self.format)
    }

    /// `value` as a tooltip shows it, with its unit and label.
    fn tooltip_value(self, value: Number) -> Vec<Value> {
        Value::labelled(value, SHOWN_DIGITS, self.format, self.unit, self.labels)
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
        .map_or_else(
            || vec![Value::missing()],
            |number| text.tooltip_value(number),
        );
    let row = Row::new(Label::Weak(format!("[{element}]")), value);
    Tooltip::new(Header::Text(base.format(t, 3)))
        .group(vec![row])
        .describe(short_names([trace.path])[0], text.description)
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
        text.tooltip_value(value),
    )];
    Tooltip::new(Header::Text(format!("[{row}][{column}]")))
        .group(rows)
        .describe(name, text.description)
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
    let row = Row::new(Label::Trace(trace), text.tooltip_value(value));
    Tooltip::new(Header::Text(format!("[{index}]")))
        .group(vec![row])
        .describe(name, text.description)
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

/// How many times as long as wide a cell of a grid stretched to its card may be.
const MAX_CELL_STRETCH: f64 = 3.0;

/// The x and the y range that show a grid of `rows` × `columns` cells, cell `(row,
/// column)` centred at `(column, -row)`, in a plot `frame` large: with square cells
/// while they fill more than half the frame, else stretched to fill it as far as
/// [`MAX_CELL_STRETCH`] allows.
fn grid_bounds(frame: egui::Vec2, (rows, columns): (usize, usize)) -> [(f64, f64); 2] {
    let (width, height) = (f64::from(frame.x.max(1.0)), f64::from(frame.y.max(1.0)));
    let (columns, rows) = (columns.max(1) as f64, rows.max(1) as f64);
    let cell = (width / columns).min(height / rows);
    let filled = columns * rows * cell * cell / (width * height);
    let (x_span, y_span) = if filled > 0.5 {
        (width / cell, height / cell)
    } else {
        // stretched, but no cell more than MAX_CELL_STRETCH times as long as wide
        let (cell_x, cell_y) = (width / columns, height / rows);
        let cell_x = cell_x.min(cell_y * MAX_CELL_STRETCH);
        let cell_y = cell_y.min(cell_x * MAX_CELL_STRETCH);
        (width / cell_x, height / cell_y)
    };
    let (x_centre, y_centre) = ((columns - 1.0) / 2.0, -(rows - 1.0) / 2.0);
    [
        (x_centre - x_span / 2.0, x_centre + x_span / 2.0),
        (y_centre - y_span / 2.0, y_centre + y_span / 2.0),
    ]
}

impl Heatmap {
    /// The range the ends of the colour scale fit, by `fit`: the expected range grown
    /// by the shown values, or the shown values.
    fn fitted(&self) -> Option<(f64, f64)> {
        match self.fit {
            Fit::Expected => super::union(self.expected, self.fitted_range),
            Fit::Values => self.fitted_range,
        }
    }

    /// The values at the ends of the colour scale.
    fn shown_range(&self) -> (f64, f64) {
        self.value_range.over(self.fitted()).unwrap_or((0.0, 1.0))
    }

    /// The range the y axis of a 1D array at the cursor time fits, by `fit`: the
    /// expected range grown by the shown values, or the shown values with a margin.
    fn fitted_y(&self) -> Option<(f64, f64)> {
        match (self.fit, self.expected) {
            (Fit::Expected, Some(expected)) => super::union(Some(expected), self.fitted_range),
            _ => self.fitted_range.map(|(low, high)| {
                let margin = if high > low {
                    (high - low) * Y_MARGIN_FRACTION
                } else {
                    0.5
                };
                (low - margin, high + margin)
            }),
        }
    }

    /// Whether a 1D array is shown at the cursor time as a line, for an array of
    /// `dimensions`.
    fn shows_line(&self, dimensions: Option<usize>) -> bool {
        dimensions != Some(2) && self.mode == ArrayMode::AtCursor
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
            // the rows of the elements always fill the height
            .allow_axis_zoom_drag([true, false])
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

        if let (Some(t), Some(y)) = (response.hovered, response.hovered_y)
            && let Some(element) = index_under(y, len)
        {
            let t = response.axis.timestamp(t);
            let tooltip = band_tooltip(trace, text, response.axis.base, t, element);
            response
                .response
                .on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
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

        let id = ui.id().with("grid");
        let frame_id = id.with("frame");
        // the size of the plot without its axes in the last frame
        let frame = ui
            .data(|data| data.get_temp::<egui::Vec2>(frame_id))
            .unwrap_or(size - GRID_AXES);
        let [x, y] = grid_bounds(frame, (rows, columns));
        let plot = egui_plot::Plot::new(id)
            .grid_color(theme::grid_color(ui.visuals()))
            .width(size.x)
            .height(size.y)
            // the grid fits the frame until panned or zoomed, and again on a double click
            .default_x_bounds(x.0, x.1)
            .default_y_bounds(y.0, y.1)
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

        let shown_frame = response.transform.frame().size();
        if shown_frame != frame {
            ui.data_mut(|data| data.insert_temp(frame_id, shown_frame));
            ui.ctx().request_repaint();
        }
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
        let shown_y = self.y_view.or(self.value_range.over(self.fitted_y()));
        let color = theme::trace_color(ui.visuals(), 0);
        let style = (self.line_styles.of(trace.path), self.line_width);

        let plot = egui_plot::Plot::new(ui.id().with("line"))
            .grid_color(theme::grid_color(ui.visuals()))
            // the value at the pointer is shown in a tooltip
            .show_x(false)
            .show_y(false)
            .custom_x_axes(vec![index_x_axis("index")])
            .x_grid_spacer(index_spacer)
            .allow_boxed_zoom(false);
        let points = self.points;
        let response = plot.show(ui, |plot_ui| {
            if let Some((min, max)) = shown_y {
                plot_ui.set_plot_bounds_y(min..=max);
            }
            plot_array(trace.path, &values, color, style, points, plot_ui);
            let pointer = plot_ui
                .response()
                .hovered()
                .then(|| plot_ui.pointer_coordinate())
                .flatten()?;
            index_under(pointer.x, numbers.len())
        });
        super::zoom_on_wheel(&response.response);
        // the y range is set until the user moves it, and again on a double click
        if response.response.double_clicked() {
            self.y_view = None;
            ui.ctx().request_repaint();
        } else if let Some(shown) = shown_y {
            let bounds = response.transform.bounds();
            let moved = (bounds.min()[1], bounds.max()[1]);
            if range_moved(shown, moved) {
                self.y_view = Some(moved);
            }
        }
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
                format_significant(value, SHOWN_DIGITS),
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
        let store: &dyn Store = context.store;
        self.expected = self.traces.first().and_then(|path| store.expected(path));
        let Some(path) = self.traces.first() else {
            theme::hint(ui, super::CHECK_TRACES);
            return;
        };
        let window = Window::of(context.time.timestamps());
        let Some(trace) = store.trace(path, window).data else {
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
            format: context
                .formats
                .format_of(path, store.labels(path).is_some()),
            unit: store.unit(path),
            labels: store.labels(path),
            description: store.description(path),
        };
        self.contents_ui(ui, trace, text, shape, context.time);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let store = context.store;
        self.expected = self.traces.first().and_then(|path| store.expected(path));
        let dimensions = self
            .traces
            .first()
            .and_then(|path| store.trace(path, Window::NONE).data)
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
        // the ends of the colour scale are those of the y axis of the line
        let line = self.shows_line(dimensions);
        let (title, fitted) = match line {
            true => ("Y axis", self.fitted_y()),
            false => ("Colour scale", self.fitted()),
        };
        let declared = self.expected.is_some();
        super::properties_section(ui, title, |ui| {
            let before = (self.value_range, self.fit);
            super::fit_row(ui, &mut self.fit, declared);
            super::range_rows(ui, &mut self.value_range, fitted);
            if (self.value_range, self.fit) != before {
                // show the range as set
                self.y_view = None;
            }
        });
        if !line {
            return;
        }
        super::properties_section(ui, "Lines", |ui| {
            self.points.choice_row(ui);
            super::property_label(ui, "Width");
            ui.add(
                egui::DragValue::new(&mut self.line_width)
                    .range(0.5..=6.0)
                    .speed(0.1),
            );
            ui.end_row();
        });
        let paths: Vec<&str> = self.traces.iter().map(String::as_str).collect();
        self.line_styles.properties_ui(ui, &paths);
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
