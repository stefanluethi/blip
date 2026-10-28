use super::Selectable;
use super::time_axis::{self, TimeAxis, closest_index, index_at, visible_range};
use super::{Formats, PropertiesContext, ViewContext, Viewable, short_names, with_unit};
use crate::app::model::numbers::{NumberKind, SHOWN_DIGITS};
use crate::app::model::traces::{Samples, Trace};
use crate::app::model::{Model, traces::Series};
use crate::app::theme;
use crate::app::time::{TimeRange, seconds_since};
use std::collections::{BTreeMap, BTreeSet};

const Y_MARGIN_FRACTION: f64 = 0.05;

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

/// Whether neither coordinate of `point` is NaN, which marks a gap.
pub fn is_point(point: &[f64; 2]) -> bool {
    !point[0].is_nan() && !point[1].is_nan()
}

/// The position of `point` on the screen of a plot with `transform`.
pub fn to_screen(point: [f64; 2], transform: &egui_plot::PlotTransform) -> egui::Pos2 {
    transform.position_from_point(&egui_plot::PlotPoint::new(point[0], point[1]))
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
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct LineStyles(BTreeMap<String, LineStyle>);

impl LineStyles {
    pub fn of(&self, path: &str) -> LineStyle {
        self.0.get(path).copied().unwrap_or_default()
    }

    pub fn set(&mut self, path: &str, style: LineStyle) {
        if style == LineStyle::Solid {
            self.0.remove(path);
        } else {
            self.0.insert(path.to_owned(), style);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Forgets the styles of the traces that are not among `shown`, so layouts do not
    /// keep them and traces shown again start solid.
    pub fn retain_shown(&mut self, shown: &BTreeSet<String>) {
        self.0.retain(|path, _| shown.contains(path));
    }

    /// Shows a section of the properties to choose the line style of each trace at
    /// `paths`, if there are any.
    pub fn properties_ui(&mut self, ui: &mut egui::Ui, paths: &[&str]) {
        if paths.is_empty() {
            return;
        }
        let names = short_names(paths.iter().copied());
        super::properties_section(ui, "Line styles", |ui| {
            for (&path, name) in paths.iter().zip(names) {
                super::property_label(ui, name).on_hover_text(path);
                let current = self.of(path);
                egui::ComboBox::from_id_salt(("line style", path))
                    .selected_text(current.name())
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for style in LineStyle::ALL {
                            if ui
                                .selectable_label(current == style, style.name())
                                .clicked()
                            {
                                self.set(path, style);
                            }
                        }
                    });
                ui.end_row();
            }
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
    #[serde(alias = "streams")] // the name before layout version 3
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

    /// The shown paths with their names in the legend and their indices, which pick
    /// their colors.
    fn selected_path_iter(&self) -> impl Iterator<Item = (&str, &str, usize)> {
        let names = short_names(self.traces.iter().map(String::as_str));
        // colors follow the order of the selection, so they do not change as traces arrive
        self.traces
            .iter()
            .zip(names)
            .enumerate()
            .map(|(index, (path, name))| (path.as_str(), name, index))
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

    /// Shows the values of the shown traces at `t` seconds.
    fn tooltip_ui(
        &self,
        ui: &mut egui::Ui,
        model: &Model,
        formats: &Formats,
        t: f64,
        axis: TimeAxis,
    ) {
        let t = axis.timestamp(t);
        ui.label(axis.base.format(t, 3));
        egui::Grid::new("plot_tooltip")
            .num_columns(2)
            .show(ui, |ui| {
                for (path, name, index) in self.selected_path_iter() {
                    let value = model.traces.series(path).and_then(|series| {
                        let value = series.number(closest_index(series.timestamps, t)?);
                        let text = value.format(Some(SHOWN_DIGITS), formats.of(path));
                        let unit = model.traces.unit(path).filter(|_| !value.is_null());
                        Some(with_unit(text, unit))
                    });
                    if let Some(value) = value {
                        ui.colored_label(theme::trace_color(ui.visuals(), index), name);
                        ui.label(value);
                        ui.end_row();
                    }
                }
            });
    }

    /// Shows the shared time range of `time`, and applies panning and zooming of the
    /// plot to it, and to [`Plot::y_view`] along the y axis.
    fn show_plot(&mut self, context: &mut ViewContext<'_>, ui: &mut egui::Ui) {
        let (model, formats, time) = (context.model, context.formats, &mut *context.time);
        let id = ui.id().with("plot");
        let mut y_axis = egui_plot::AxisHints::new_y()
            .placement(egui_plot::HPlacement::Left)
            .min_thickness(50.0);
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
        let shown_y = self
            .y_view
            .or_else(|| y_bounds(self.y_min, self.y_max, self.range_y(model)));
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
            let tolerance = (max_y - min_y) * 1.0e-6;
            if (low - min_y).abs() > tolerance || (high - max_y).abs() > tolerance {
                self.y_view = Some((low, high));
            }
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

impl Viewable for Plot {
    fn title_ui(&self, ui: &mut egui::Ui, model: &Model, time: &TimeRange, formats: &Formats) {
        if self.legend != LegendPlacement::Title {
            return;
        }
        let t = time.cursor().map(|t| time.timestamp(t));
        // one unit shared by all traces is written once, at the end
        let unit = super::common_unit(model, self.traces.iter().map(String::as_str));
        let weak = ui.visuals().weak_text_color();
        ui.add_space(8.0);
        for (path, name, index) in self.selected_path_iter() {
            ui.add_space(6.0);
            let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            let color = theme::trace_color(ui.visuals(), index);
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(egui::RichText::new(name).color(weak))
                .on_hover_text(path);
            let value = t.and_then(|t| {
                let series = model.traces.series(path)?;
                let value = series.number(index_at(series.timestamps, t)?);
                let text = value.format(Some(SHOWN_DIGITS), formats.of(path));
                let own_unit = model.traces.unit(path).filter(|_| unit.is_none());
                Some(with_unit(text, own_unit.filter(|_| !value.is_null())))
            });
            if let Some(value) = value {
                ui.label(egui::RichText::new(value).strong());
            }
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
        self.show_plot(context, ui);
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
            ui.add(
                egui::TextEdit::singleline(&mut self.y_label)
                    .hint_text(unit)
                    .margin(theme::FIELD_MARGIN)
                    .desired_width(f32::INFINITY),
            )
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
            egui::ComboBox::from_id_salt("plot_legend")
                .selected_text(self.legend.name())
                .show_ui(ui, |ui| {
                    for placement in LegendPlacement::ALL {
                        ui.selectable_value(&mut self.legend, placement, placement.name());
                    }
                });
            ui.end_row();
        });
        let paths: Vec<&str> = self.traces.iter().map(String::as_str).collect();
        self.line_styles.properties_ui(ui, &paths);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::traces::Traces;

    /// A float trace `a` of `count` values 1 ms apart, with a null every 7th value if
    /// `with_nulls`.
    fn traces(count: u64, with_nulls: bool) -> Traces {
        let mut traces = Traces::default();
        for i in 0..count {
            let value = if with_nulls && i % 7 == 3 {
                proto::Data::Null
            } else {
                proto::Data::Float((i % 7) as f64)
            };
            traces.append(&proto::Value {
                timestamp: i * 1_000_000,
                path: "a".to_owned(),
                value,
            });
        }
        traces
    }

    #[test]
    fn downsample_keeps_small_series() {
        let traces = traces(10, false);
        assert_eq!(downsample(traces.series("a").unwrap(), 0, 100).len(), 10);
    }

    #[test]
    fn downsample_limits_points_and_keeps_extremes() {
        let traces = traces(1_000_003, false);
        let points = downsample(traces.series("a").unwrap(), 0, 300);
        assert!(points.len() <= 1_000);
        assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(points.iter().any(|p| p[1] == 0.0));
        assert!(points.iter().any(|p| p[1] == 6.0));
    }

    #[test]
    fn downsample_keeps_gaps() {
        let traces = traces(1_000_003, true);
        let points = downsample(traces.series("a").unwrap(), 0, 300);
        assert!(points.len() <= 1_000);
        assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(points.iter().any(|p| p[1].is_nan()));
        assert!(points.iter().any(|p| p[1] == 6.0));
    }

    #[test]
    fn y_bounds_fit_the_ends_that_are_not_set() {
        let fitted = Some((-1.0, 5.0));
        assert_eq!(y_bounds(None, None, fitted), fitted);
        assert_eq!(y_bounds(Some(0.0), None, fitted), Some((0.0, 5.0)));
        assert_eq!(y_bounds(None, Some(2.0), fitted), Some((-1.0, 2.0)));
        assert_eq!(y_bounds(Some(0.0), Some(1.0), None), Some((0.0, 1.0)));
        assert_eq!(y_bounds(Some(0.0), None, None), None);
        assert_eq!(y_bounds(Some(6.0), None, fitted), None);
    }

    struct State {
        plot: Plot,
        model: Model,
        time: crate::app::time::TimeRange,
        invoker: crate::app::model::actions::Invoker,
    }

    /// A harness showing a plot of a trace `a` whose value is the time in seconds from 0
    /// to 40 seconds, of which the last 20 seconds are shown.
    fn plot_harness() -> egui_kittest::Harness<'static, State> {
        let mut model = Model::default();
        for second in 0..=40 {
            model.traces.append(&proto::Value {
                timestamp: second * 1_000_000_000,
                path: "a".to_owned(),
                value: proto::Data::Float(second as f64),
            });
        }
        let mut plot = Plot::default();
        plot.traces.insert("a".to_owned());
        let state = State {
            plot,
            model,
            time: Default::default(),
            invoker: Default::default(),
        };
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            // fast enough for double clicks
            .with_step_dt(1.0 / 60.0)
            .with_max_steps(60)
            .build_ui_state(
                |ui, state: &mut State| {
                    state
                        .time
                        .update(&state.model, ui.input(|input| input.time));
                    let mut context = ViewContext {
                        model: &state.model,
                        time: &mut state.time,
                        invoker: &mut state.invoker,
                        formats: &super::super::NO_FORMATS,
                        titled: false,
                    };
                    state.plot.view(ui, &mut context);
                },
                state,
            )
    }

    /// The y range of `plot`, the panned or zoomed one or else the fitted one.
    fn shown_y(state: &State) -> (f64, f64) {
        let plot = &state.plot;
        plot.y_view
            .or_else(|| y_bounds(plot.y_min, plot.y_max, plot.range_y(&state.model)))
            .unwrap()
    }

    fn drag(harness: &mut egui_kittest::Harness<'_, State>, from: egui::Pos2, to: egui::Pos2) {
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
    fn the_y_axis_fits_all_values_and_not_only_the_shown_ones() {
        let mut harness = plot_harness();
        harness.run();
        let (start, _) = harness.state().time.range().unwrap();
        assert!(start > 10.0, "{start}");
        let (low, high) = shown_y(harness.state());
        assert!(low < 0.0 && high > 40.0, "{low} {high}");
        assert!(harness.state().plot.y_view.is_none());
    }

    #[test]
    fn a_vertical_drag_pans_the_y_axis_and_a_double_click_fits_it_again() {
        let mut harness = plot_harness();
        harness.run();
        let range = harness.state().time.range();
        let (low, high) = shown_y(harness.state());

        // a horizontal drag pans the time and leaves the y axis
        drag(
            &mut harness,
            egui::pos2(150.0, 150.0),
            egui::pos2(250.0, 152.0),
        );
        assert_ne!(harness.state().time.range(), range);
        assert!(harness.state().plot.y_view.is_none());

        // a vertical drag pans the y axis and leaves the time
        let range = harness.state().time.range();
        drag(
            &mut harness,
            egui::pos2(150.0, 100.0),
            egui::pos2(152.0, 200.0),
        );
        assert_eq!(harness.state().time.range(), range);
        let (panned_low, panned_high) = harness.state().plot.y_view.unwrap();
        assert!(panned_low > low + 1.0, "{panned_low} {low}");
        assert!(((panned_high - panned_low) - (high - low)).abs() < 1.0e-6);
        // and stays so in later frames
        harness.run();
        assert_eq!(harness.state().plot.y_view, Some((panned_low, panned_high)));

        // without `drop_at`, which takes the pointer away
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
        assert!(harness.state().plot.y_view.is_none());
    }

    #[test]
    fn the_wheel_with_alt_zooms_the_y_axis_only() {
        let mut harness = plot_harness();
        harness.run();
        let range = harness.state().time.range();
        let (low, high) = shown_y(harness.state());

        let at = egui::pos2(200.0, 150.0);
        harness.hover_at(at);
        harness.run();
        harness.event(egui::Event::ModifiersChanged(egui::Modifiers::ALT));
        harness.run();
        let ctx = harness.ctx.clone();
        let input = harness.input_mut();
        input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 100.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::ALT,
        });
        super::super::zoom_with_wheel(&ctx, input);
        harness.run_steps(60);

        assert_eq!(harness.state().time.range(), range);
        let (zoomed_low, zoomed_high) = harness.state().plot.y_view.unwrap();
        assert!(
            zoomed_high - zoomed_low < (high - low) * 0.9,
            "{zoomed_low} {zoomed_high}"
        );
    }

    #[test]
    fn line_styles_of_traces_no_longer_shown_are_forgotten() {
        let mut styles = LineStyles::default();
        styles.set("a", LineStyle::Dashed);
        styles.set("b", LineStyle::Dotted);
        styles.set("c", LineStyle::Solid);
        assert_eq!(styles.0.len(), 2);
        styles.retain_shown(&["a".to_owned()].into());
        assert_eq!(styles.of("a"), LineStyle::Dashed);
        assert_eq!(styles.of("b"), LineStyle::Solid);
        assert_eq!(styles.0.len(), 1);
    }

    #[test]
    fn patterns_of_long_lines_on_screen_become_solid() {
        let transform = egui_plot::PlotTransform::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
            egui_plot::PlotBounds::from_min_max([0.0, 0.0], [1.0, 1.0]),
            egui::Vec2b::FALSE,
        );
        let short = [[0.0, 0.0], [1.0, 1.0]];
        assert_eq!(screen_length(&short, &transform), 2f32.sqrt() * 100.0);
        let dashed = LineStyle::Dashed.plot_style(&short, &transform);
        assert!(matches!(dashed, Some(egui_plot::LineStyle::Dashed { .. })));
        // zigzagging from bottom to top, a gap does not count
        let mut long: Vec<[f64; 2]> = (0..1_000).map(|i| [0.0, f64::from(i % 2)]).collect();
        long.push([f64::NAN, 0.0]);
        assert!(screen_length(&long, &transform) > MAX_PATTERN_LENGTH);
        let dotted = LineStyle::Dotted.plot_style(&long, &transform);
        assert_eq!(dotted, Some(egui_plot::LineStyle::Solid));
        assert_eq!(LineStyle::None.plot_style(&short, &transform), None);
    }

    #[test]
    fn steps_hold_each_value() {
        let stepped = steps(&[[0.0, 1.0], [1.0, 0.0], [3.0, 1.0]]);
        assert_eq!(
            stepped,
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [3.0, 0.0], [3.0, 1.0]]
        );
    }
}
