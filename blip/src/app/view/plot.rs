use super::Selectable;
use super::plotting::{is_point, to_screen};
use super::stats::{self, Measurement};
use super::time_axis::{self, TimeAxis, visible_range};
use super::tooltip::{Header, Label, Row, Tooltip, TraceName, Value};
use super::{Formats, PropertiesContext, ViewContext, Viewable, with_unit};
use crate::app::model::numbers::{
    DisplayFormat, NumberKind, SHOWN_DIGITS, fixed_decimals, widest_fixed,
};
use crate::app::model::sorted::{closest_index, index_at};
use crate::app::model::traces::{Samples, Trace};
use crate::app::model::{Model, traces::Series};
use crate::app::theme;
use crate::app::time::{TimeBase, seconds_since};
use std::collections::BTreeSet;

const Y_MARGIN_FRACTION: f64 = 0.05;

/// The room between a plot and its stats in points.
const STATS_GAP: f32 = 6.0;

/// Points of a curve shown as markers are at least this many points apart on average.
const MIN_MARKER_SPACING: f32 = 8.0;

/// Length of the dashes of a dashed line in points.
const DASH_LENGTH: f32 = 8.0;
/// The golden ratio, by which egui_plot spaces dashes, see [`LineStyle::pattern_period`].
#[allow(clippy::excessive_precision)]
const GOLDEN_RATIO: f32 = 1.618_033_988_749_895;
/// Distance between the dots of a dotted line in points.
const DOT_SPACING: f32 = 5.0;
/// Lines longer than this many points on screen are drawn solid instead of with their
/// pattern, which takes a shape per dash or dot.
const MAX_PATTERN_LENGTH: f32 = 30_000.0;

named_enum! {
    /// How a curve goes from one sample to the next.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum Interpolation {
        /// A straight line, except for bools, which always hold their value.
        #[default]
        Linear => "Linear",
        /// Each value held until the next sample.
        Step => "Step",
    }
}

named_enum! {
    /// When the samples of a curve are marked.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum ShowPoints {
        /// When they are far enough apart to tell them apart.
        #[default]
        Auto => "Auto",
        Always => "Always",
        Never => "Never",
    }
}

impl ShowPoints {
    /// Whether `count` points are marked on a plot `width` points wide.
    pub fn shown(self, count: usize, width: f32) -> bool {
        match self {
            ShowPoints::Auto => count as f32 * MIN_MARKER_SPACING <= width,
            ShowPoints::Always => true,
            ShowPoints::Never => false,
        }
    }

    /// Adds a row to the grid of a properties section to choose when points are marked.
    pub fn choice_row(&mut self, ui: &mut egui::Ui) {
        super::choice_row(ui, "Points", self, &ShowPoints::CHOICES);
    }
}

named_enum! {
    /// How the line of a trace is drawn.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum LineStyle {
        #[default]
        Solid => "Solid",
        Dashed => "Dashed",
        Dotted => "Dotted",
        /// No line, only the points, which are then always marked.
        None => "None",
    }
}

impl LineStyle {
    /// How egui_plot draws a line through `points` on a plot with `transform`, `None`
    /// if there is no line. A line too long on screen for its pattern to be drawn
    /// quickly is solid instead, as its dashes or dots would be too dense to see anyway.
    pub fn plot_style(
        self,
        points: &[[f64; 2]],
        transform: &egui_plot::PlotTransform,
    ) -> Option<egui_plot::LineStyle> {
        let style = match self {
            LineStyle::Solid => egui_plot::LineStyle::Solid,
            LineStyle::Dashed => egui_plot::LineStyle::Dashed {
                length: DASH_LENGTH,
            },
            LineStyle::Dotted => egui_plot::LineStyle::Dotted {
                spacing: DOT_SPACING,
            },
            LineStyle::None => return None,
        };
        if self.pattern_period().is_some() && screen_length(points, transform) > MAX_PATTERN_LENGTH
        {
            Some(egui_plot::LineStyle::Solid)
        } else {
            Some(style)
        }
    }

    /// The length on screen after which the pattern of a line in this style repeats,
    /// `None` without a pattern. egui draws a dash, then a gap of the dash length
    /// times the golden ratio minus one.
    pub fn pattern_period(self) -> Option<f32> {
        match self {
            LineStyle::Dashed => Some(DASH_LENGTH * GOLDEN_RATIO),
            LineStyle::Dotted => Some(DOT_SPACING),
            LineStyle::Solid | LineStyle::None => None,
        }
    }

    /// Whether the points of a line of this style are marked when `points` says so for
    /// `count` points on a plot `width` points wide, always if there is no line.
    pub fn marks_points(self, points: ShowPoints, count: usize, width: f32) -> bool {
        self == LineStyle::None || points.shown(count, width)
    }
}

/// The length on the screen of a plot with `transform` of the lines through `points`,
/// broken where a point is missing.
fn screen_length(points: &[[f64; 2]], transform: &egui_plot::PlotTransform) -> f32 {
    points
        .windows(2)
        .filter(|pair| pair.iter().all(is_point))
        .map(|pair| to_screen(pair[0], transform).distance(to_screen(pair[1], transform)))
        .sum()
}

/// The line styles of the traces of a view by their paths, solid for traces without
/// one.
pub type LineStyles = super::PerPath<LineStyle>;

impl super::PerPath<LineStyle> {
    /// Shows a section of the properties to choose the line style of each trace at
    /// `paths`, if there are any.
    pub fn properties_ui(&mut self, ui: &mut egui::Ui, paths: &[&str]) {
        if paths.is_empty() {
            return;
        }
        super::properties_section(ui, "Line styles", |ui| {
            super::path_rows(ui, paths, |ui, &path| {
                let mut style = self.of(path);
                super::choice_combo(
                    ui,
                    ("line style", path),
                    Some(90.0),
                    &mut style,
                    LineStyle::ALL
                        .into_iter()
                        .map(|style| (style, style.name())),
                    None,
                );
                self.set(path, style);
            });
        });
    }
}

named_enum! {
    /// Where the legend is shown.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum LegendPlacement {
        /// In the title row of the view, with the values at the cursor.
        #[default]
        Title => "Title",
        TopLeft => "Top left",
        TopRight => "Top right",
        BottomLeft => "Bottom left",
        BottomRight => "Bottom right",
        Hidden => "Hidden",
    }
}

impl LegendPlacement {
    fn corner(self) -> Option<egui_plot::Corner> {
        match self {
            LegendPlacement::Hidden | LegendPlacement::Title => None,
            LegendPlacement::TopLeft => Some(egui_plot::Corner::LeftTop),
            LegendPlacement::TopRight => Some(egui_plot::Corner::RightTop),
            LegendPlacement::BottomLeft => Some(egui_plot::Corner::LeftBottom),
            LegendPlacement::BottomRight => Some(egui_plot::Corner::RightBottom),
        }
    }
}

/// A time-series plot of the selected traces.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Plot {
    /// Paths of the shown traces.
    pub traces: BTreeSet<String>,
    /// The bottom of the y axis, or `None` to fit all values of the shown traces.
    pub y_min: Option<f64>,
    /// The top of the y axis, or `None` to fit all values of the shown traces.
    pub y_max: Option<f64>,
    /// The y range panned or zoomed to instead of the one of `y_min` and `y_max`, until
    /// a double click.
    #[serde(skip)]
    pub y_view: Option<(f64, f64)>,
    /// Label of the y axis, none if empty.
    pub y_label: String,
    pub interpolation: Interpolation,
    /// Width of the curves in points.
    pub line_width: f32,
    pub points: ShowPoints,
    /// Whether curves go on over nulls instead of leaving gaps.
    pub connect_nulls: bool,
    pub legend: LegendPlacement,
    #[serde(skip_serializing_if = "LineStyles::is_empty")]
    pub line_styles: LineStyles,
    /// The stats shown below the plot, also of traces no longer shown, which are left
    /// out until they are shown again.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<Measurement>,
}

impl Default for Plot {
    fn default() -> Self {
        Self {
            traces: BTreeSet::new(),
            y_min: None,
            y_max: None,
            y_view: None,
            y_label: String::new(),
            interpolation: Interpolation::default(),
            line_width: 1.5,
            points: ShowPoints::default(),
            connect_nulls: false,
            legend: LegendPlacement::default(),
            line_styles: LineStyles::default(),
            stats: Vec::new(),
        }
    }
}

impl Plot {
    /// Draws the values of `series` within the window of `axis`, as set in the
    /// properties of the plot, with its line in `style`.
    #[allow(clippy::too_many_arguments)]
    fn plot_series(
        &self,
        name: &str,
        series: Series<'_>,
        style: LineStyle,
        color: egui::Color32,
        axis: TimeAxis,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let width = plot_ui.transform().frame().width();
        let visible = series.slice(visible_range(series.timestamps, axis.timestamps()));
        let mut points = downsample(visible, axis.origin, width.max(1.0) as usize);
        if self.connect_nulls {
            points.retain(|point| !point[1].is_nan());
        }
        let show_points = style.marks_points(self.points, points.len(), width);
        let line_style = style.plot_style(&points, plot_ui.transform());
        let step = self.interpolation == Interpolation::Step || series.kind == NumberKind::Bool;
        // lines and markers of the same name share one legend entry
        for segment in points
            .split(|point| point[1].is_nan())
            .filter(|segment| !segment.is_empty())
        {
            if show_points {
                plot_ui.points(
                    egui_plot::Points::new(name, segment.to_vec())
                        .color(color)
                        .radius(self.line_width + 1.0),
                );
            }
            let Some(line_style) = line_style else {
                continue;
            };
            let segment = if step {
                steps(segment)
            } else {
                segment.to_vec()
            };
            plot_ui.line(
                egui_plot::Line::new(name, egui_plot::PlotPoints::from(segment))
                    .color(color)
                    .width(self.line_width)
                    .style(line_style),
            );
        }
    }

    /// The y range covered by all values of the shown traces, also outside the shown
    /// window, with a margin.
    fn range_y(&self, model: &Model) -> Option<(f64, f64)> {
        let range = self
            .selected_series_iter(model)
            .filter_map(|(_, series, _)| series.range)
            .reduce(|(low_a, high_a), (low_b, high_b)| (low_a.min(low_b), high_a.max(high_b)));
        range.map(|(low, high)| {
            let margin = if high > low {
                (high - low) * Y_MARGIN_FRACTION
            } else {
                0.5
            };
            (low - margin, high + margin)
        })
    }

    /// The y range the plot shows: the one panned or zoomed to, else the one of `y_min`
    /// and `y_max` with the ends not set fitted to all values of the shown traces.
    /// `None` if neither is known.
    pub fn shown_y(&self, model: &Model) -> Option<(f64, f64)> {
        self.y_view
            .or_else(|| y_bounds(self.y_min, self.y_max, self.range_y(model)))
    }

    /// The shown paths with their names in the legend and their indices, which pick
    /// their colors.
    fn selected_path_iter(&self) -> impl Iterator<Item = (&str, &str, usize)> {
        super::indexed_names(&self.traces)
    }

    /// The shown numeric series that were received, with their names in the legend and
    /// their indices, which pick their colors.
    fn selected_series_iter<'a>(
        &'a self,
        model: &'a Model,
    ) -> impl Iterator<Item = (&'a str, Series<'a>, usize)> {
        self.selected_path_iter()
            .filter_map(|(path, name, index)| Some((name, model.traces.series(path)?, index)))
    }

    /// The tooltip with the values of the shown traces at `t` nanoseconds, a faint dash
    /// for a trace without a series.
    fn tooltip(&self, model: &Model, formats: &Formats, t: u64, base: TimeBase) -> Tooltip {
        let rows = self
            .selected_path_iter()
            .map(|(path, name, index)| {
                let value = model
                    .traces
                    .series(path)
                    .and_then(|series| {
                        let number = series.number(closest_index(series.timestamps, t)?);
                        let unit = model.traces.unit(path);
                        Some(Value::number(number, SHOWN_DIGITS, formats.of(path), unit))
                    })
                    .unwrap_or_else(Value::missing);
                Row::new(Label::Trace(TraceName::new(name, path, index)), [value])
            })
            .collect();
        Tooltip::new(Header::Text(base.format(t, 3))).group(rows)
    }

    /// Shows the values of the shown traces at `t` seconds.
    fn tooltip_ui(
        &self,
        ui: &mut egui::Ui,
        model: &Model,
        formats: &Formats,
        t: f64,
        axis: TimeAxis,
    ) {
        self.tooltip(model, formats, axis.timestamp(t), axis.base)
            .ui(ui);
    }

    /// Whether a stat of a shown trace is shown below the plot.
    fn shows_stats(&self) -> bool {
        self.stats
            .iter()
            .any(|stat| self.traces.contains(&stat.trace))
    }

    /// Shows the plot and below it the stats of the shown traces, which take the room
    /// they took in the last frame, so that the plot fills the rest.
    fn plot_and_stats_ui(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let id = ui.id().with("stats height");
        let height = ui
            .data(|data| data.get_temp::<f32>(id))
            .unwrap_or(ui.spacing().interact_size.y);
        let plot_size = egui::vec2(
            ui.available_width(),
            (ui.available_height() - height - STATS_GAP).max(0.0),
        );
        ui.allocate_ui(plot_size, |ui| self.show_plot(context, ui));
        ui.add_space(STATS_GAP);
        let shown: Vec<_> = self.selected_path_iter().collect();
        let time = &*context.time;
        let window = time.timestamps();
        let response = ui.scope(|ui| {
            stats::measurements_ui(
                ui,
                &self.stats,
                &shown,
                (context.model, context.formats),
                window,
                |series| stats::is_linear(series, self.interpolation),
            );
        });
        let shown_height = response.response.rect.height();
        if shown_height != height {
            ui.data_mut(|data| data.insert_temp(id, shown_height));
            ui.ctx().request_repaint();
        }
    }

    /// Shows the shared time range of `time`, and applies panning and zooming of the
    /// plot to it, and to [`Plot::y_view`] along the y axis.
    fn show_plot(&mut self, context: &mut ViewContext<'_>, ui: &mut egui::Ui) {
        let (model, formats, time) = (context.model, context.formats, &mut *context.time);
        let id = ui.id().with("plot");
        let mut y_axis = super::plotting::time_y_axis();
        let label = match self.y_label.as_str() {
            "" => super::common_unit(model, self.traces.iter().map(String::as_str)),
            label => Some(label),
        };
        if let Some(label) = label {
            y_axis = y_axis.label(label);
        }
        let mut plot = time_axis::plot(id, time, ui.visuals()).custom_y_axes(vec![y_axis]);
        // without a title row, the legend of the title row goes to a corner
        let legend = match self.legend {
            LegendPlacement::Title if !context.titled => LegendPlacement::TopRight,
            legend => legend,
        };
        if let Some(corner) = legend.corner() {
            plot = plot.legend(egui_plot::Legend::default().position(corner));
        }

        let visuals = ui.visuals().clone();
        let shown_y = self.shown_y(model);
        let this = &*self;
        let response = time_axis::show(id, plot, ui, time, true, |plot_ui, axis| {
            if let Some((min_y, max_y)) = shown_y {
                plot_ui.set_plot_bounds_y(min_y..=max_y);
            }
            for (path, name, index) in this.selected_path_iter() {
                let Some(series) = model.traces.series(path) else {
                    continue;
                };
                let color = theme::trace_color(&visuals, index);
                let style = this.line_styles.of(path);
                this.plot_series(name, series, style, color, axis, plot_ui);
            }
        });

        // the first click puts the cursor under the pointer, which gets the second one
        let double_clicked = ui.input(|input| {
            input
                .pointer
                .button_double_clicked(egui::PointerButton::Primary)
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| response.response.rect.contains(pos))
        });
        if double_clicked {
            self.y_view = None;
            ui.ctx().request_repaint();
        } else if let Some((min_y, max_y)) = shown_y {
            let (low, high) = (response.bounds.min()[1], response.bounds.max()[1]);
            if super::plotting::range_moved((min_y, max_y), (low, high)) {
                self.y_view = Some((low, high));
            }
        }

        if self.traces.is_empty() {
            theme::hint_over(ui, response.frame, super::CHECK_TRACES);
        }
        if let Some(t) = response.hovered
            && !self.traces.is_empty()
        {
            let axis = response.axis;
            response
                .response
                .on_hover_ui_at_pointer(|ui| self.tooltip_ui(ui, model, formats, t, axis));
        }
    }
}

/// The range of the y axis from `min` and `max` where they are set and from the ends of
/// `fitted` where not, or `None` if an end is unknown or the ends are not in order.
fn y_bounds(min: Option<f64>, max: Option<f64>, fitted: Option<(f64, f64)>) -> Option<(f64, f64)> {
    let min = min.or(fitted.map(|(low, _)| low))?;
    let max = max.or(fitted.map(|(_, high)| high))?;
    (min < max).then_some((min, max))
}

/// Reduces `series` to about two points per one of `buckets` buckets of equally many
/// samples, the lowest and the highest, so peaks stay visible, and a point with a NaN y
/// before them for a gap. The extremes come from the blocks of the series, so the cost
/// follows the buckets and not the samples. The x coordinates are the sample times in
/// seconds since `origin` in nanoseconds. A series of few samples per bucket is kept
/// as it is.
fn downsample(series: Series<'_>, origin: u64, buckets: usize) -> Vec<[f64; 2]> {
    let x = |index: usize| seconds_since(origin, series.timestamps[index]);
    let len = series.len();
    let buckets = buckets.max(1);
    if len <= buckets * 4 {
        return (0..len)
            .map(|index| [x(index), series.value(index)])
            .collect();
    }
    let mut points: Vec<[f64; 2]> = Vec::with_capacity(buckets * 3);
    for bucket in 0..buckets {
        let (start, end) = (len * bucket / buckets, len * (bucket + 1) / buckets);
        let extent = series.extent(start..end);
        if extent.gap {
            points.push([x(start), f64::NAN]);
        }
        if extent.has_values() {
            // the extreme nearer to the point before first, so the line does not zigzag
            let (first, second) = match points.last() {
                Some(&[_, y]) if (y - extent.max).abs() < (y - extent.min).abs() => {
                    (extent.max, extent.min)
                }
                _ => (extent.min, extent.max),
            };
            points.push([x(start), first]);
            points.push([x(end - 1), second]);
        }
    }
    points
}

/// The line through `points` as steps that hold each value until the next point.
fn steps(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut stepped = Vec::with_capacity(points.len() * 2);
    for (index, &point) in points.iter().enumerate() {
        if index > 0 {
            stepped.push([point[0], points[index - 1][1]]);
        }
        stepped.push(point);
    }
    stepped
}

impl super::ViewInfo for Plot {
    const NAME: &'static str = "Plot";
    const ICON: &'static str = egui_phosphor::regular::CHART_LINE;

    fn selectable(trace: Trace<'_>) -> Selectable {
        match trace.samples {
            Samples::Strings(_) => Selectable::No("Plots show numbers".into()),
            Samples::Arrays { shape, .. } => super::elements(shape),
            _ => Selectable::Trace,
        }
    }
}

/// Shows `value` of the trace at `path` in the legend of the title row, right-aligned
/// in a slot as wide as `widest` and as the widest value it showed since `widest` or
/// its `format` last changed, so that its digits keep their place while it changes.
fn legend_value(
    ui: &mut egui::Ui,
    path: &str,
    value: Option<&str>,
    (widest, format): (Option<&str>, DisplayFormat),
) {
    let width_of = |text: &str| super::text_size(ui, text, egui::TextStyle::Body).x;
    let needed = value
        .map_or(0.0, width_of)
        .max(widest.map_or(0.0, width_of));
    // a new range or format, e.g. of another recording, starts the slot anew
    let slot = format!("{format:?} {widest:?}");
    let id = ui.id().with(("legend value width", path));
    let width = super::sticky_width(ui.ctx(), id, slot, needed);
    let size = egui::vec2(width, ui.spacing().interact_size.y);
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        if let Some(value) = value {
            ui.label(egui::RichText::new(value).strong());
        }
    });
}

/// How the values of `series` written in `format` are kept in place in the legend: the
/// decimals of floats, if fixed, and the widest text of a value, if known. Floats get the
/// decimals of their largest magnitude, integers are written whole, bools as words.
fn legend_slot(series: &Series<'_>, format: DisplayFormat) -> (Option<usize>, Option<String>) {
    if format != DisplayFormat::Default {
        return (None, None);
    }
    match (series.kind, series.range) {
        (NumberKind::Float, Some(range)) => match fixed_decimals(range, SHOWN_DIGITS) {
            Some(decimals) => (Some(decimals), Some(widest_fixed(range, decimals))),
            None => (None, None),
        },
        (NumberKind::Integer, Some(range)) => (None, Some(widest_fixed(range, 0))),
        (NumberKind::Bool, _) => (None, Some("false".to_owned())),
        _ => (None, None),
    }
}

impl Viewable for Plot {
    fn title_ui(&self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.legend != LegendPlacement::Title {
            return;
        }
        let (model, time, formats) = (context.model, &*context.time, context.formats);
        let t = time.cursor_timestamp();
        // one unit shared by all traces is written once, at the end
        let unit = super::common_unit(model, self.traces.iter().map(String::as_str));
        let weak = ui.visuals().weak_text_color();
        ui.add_space(8.0);
        for (path, name, index) in self.selected_path_iter() {
            ui.add_space(6.0);
            super::legend_dot(ui, theme::trace_color(ui.visuals(), index));
            ui.label(egui::RichText::new(name).color(weak))
                .on_hover_text(path);
            let Some(series) = model.traces.series(path) else {
                continue;
            };
            let format = formats.of(path);
            let own_unit = model.traces.unit(path).filter(|_| unit.is_none());
            let (decimals, widest) = legend_slot(&series, format);
            let value = t.and_then(|t| {
                let value = series.number(index_at(series.timestamps, t)?);
                let text = match decimals {
                    Some(decimals) => value.format_fixed(decimals),
                    None => value.format(Some(SHOWN_DIGITS), format),
                };
                Some(with_unit(text, own_unit.filter(|_| !value.is_null())))
            });
            let widest = widest.map(|widest| with_unit(widest, own_unit));
            legend_value(ui, path, value.as_deref(), (widest.as_deref(), format));
        }
        if let Some(unit) = unit {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(unit).color(weak));
        }
    }

    fn colored_paths(&self, _model: &Model) -> Vec<String> {
        self.selected_path_iter()
            .map(|(path, _, _)| path.to_owned())
            .collect()
    }

    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.traces, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.line_styles.retain_shown(&self.traces);
        if self.shows_stats() {
            self.plot_and_stats_ui(ui, context);
        } else {
            self.show_plot(context, ui);
        }
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let model = context.model;
        let unit = super::common_unit(model, self.traces.iter().map(String::as_str))
            .unwrap_or_default()
            .to_owned();
        super::properties_section(ui, "Y axis", |ui| {
            let range = (self.y_min, self.y_max);
            super::optional_number_row(ui, "Min", &mut self.y_min);
            super::optional_number_row(ui, "Max", &mut self.y_max);
            if (self.y_min, self.y_max) != range {
                // show the range as set
                self.y_view = None;
            }
            super::property_label(ui, "Label");
            ui.add(theme::wide_field(&mut self.y_label).hint_text(unit))
                .on_hover_text("The unit of the traces if they share one and this is empty");
            ui.end_row();
        });
        super::properties_section(ui, "Lines", |ui| {
            super::choice_row(
                ui,
                "Interpolation",
                &mut self.interpolation,
                &Interpolation::CHOICES,
            );
            super::property_label(ui, "Width");
            ui.add(
                egui::DragValue::new(&mut self.line_width)
                    .range(0.5..=6.0)
                    .speed(0.1),
            );
            ui.end_row();
            self.points.choice_row(ui);
            super::choice_row(
                ui,
                "Nulls",
                &mut self.connect_nulls,
                &[(false, "Gap"), (true, "Connect")],
            );
        });
        super::properties_section(ui, "Legend", |ui| {
            super::property_label(ui, "Placement");
            super::choice_combo(
                ui,
                "plot_legend",
                None,
                &mut self.legend,
                LegendPlacement::ALL
                    .into_iter()
                    .map(|placement| (placement, placement.name())),
                None,
            );
            ui.end_row();
        });
        let paths: Vec<&str> = self.traces.iter().map(String::as_str).collect();
        self.line_styles.properties_ui(ui, &paths);
        stats::properties_ui(ui, &mut self.stats, &paths);
    }
}

#[cfg(test)]
mod tests;
