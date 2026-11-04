//! An XY plot: traces plotted against other traces instead of against time, e.g.
//! positions as paths.

use super::Selectable;
use super::heatmap::{index_spacer, index_x_axis, sample_at_cursor};
use super::plot::{LineStyle, LineStyles, ShowPoints, is_point, to_screen};
use super::time_axis::{index_at, within};
use super::trace_selection::MAX_LISTED_ELEMENTS;
use super::{PropertiesContext, ViewContext, ViewType, Viewable, short_names};
use crate::app::model::Model;
use crate::app::model::numbers::{SHOWN_DIGITS, format_significant};
use crate::app::model::traces::{Samples, Series, Trace};
use crate::app::theme;
use crate::app::time::TimeRange;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// Upper bound of points drawn per trace.
const MAX_POINTS: usize = 20_000;
/// Number of steps in which the trail fades out towards older points.
const FADE_STEPS: usize = 8;
/// How far the trail fades out unless set otherwise.
const DEFAULT_FADE: f32 = 0.85;
/// Width of the choice between the x and the y axis in the properties.
const AXIS_CHOICE_WIDTH: f32 = 64.0;
/// Radius of the marks of the points.
const POINT_RADIUS: f32 = 2.0;

/// The shown traces on the y axis, each plotted against one of the shown traces on
/// the x axis, within the shown window. Or the shown arrays at the cursor time,
/// plotted over the indices of their elements, e.g. snapshots of an ADC.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Xy {
    /// Path of the single x trace in layouts before version 4, see [`Xy::migrate_v3`].
    #[serde(skip_serializing)]
    pub x: Option<String>,
    /// Paths of the shown traces, on the x axis and on the y axis, or the shown
    /// arrays, see [`Xy::array_paths`].
    #[serde(alias = "streams")] // the name before layout version 3
    pub traces: BTreeSet<String>,
    /// Paths of the shown traces on the x axis.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub xs: BTreeSet<String>,
    /// For each shown trace on the y axis, the path of the x trace it is plotted
    /// against, see [`Xy::assign_axes`].
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub against: BTreeMap<String, String>,
    /// Whether a unit has the same length on both axes, e.g. for positions.
    pub equal_axes: bool,
    /// When the points are marked.
    pub points: ShowPoints,
    /// How far the oldest part of the trail fades out, from 0 for not at all to 1 for
    /// nearly invisible.
    pub fade: f32,
    #[serde(skip_serializing_if = "LineStyles::is_empty")]
    pub line_styles: LineStyles,
}

impl Default for Xy {
    fn default() -> Self {
        Self {
            x: None,
            traces: BTreeSet::new(),
            xs: BTreeSet::new(),
            against: BTreeMap::new(),
            equal_axes: false,
            points: ShowPoints::default(),
            fade: DEFAULT_FADE,
            line_styles: LineStyles::default(),
        }
    }
}

/// The points of `y` against `x` for the samples of `y` in `indices`, each paired with
/// the sample of `x` current at its time. A coordinate is NaN where either is null or
/// `x` has no sample yet.
fn pair(x: Series<'_>, y: Series<'_>, indices: Range<usize>) -> Vec<[f64; 2]> {
    let first = indices.start;
    // the number of samples of x at or before the current sample of y
    let mut x_count = y
        .timestamps
        .get(first)
        .map_or(0, |&t| x.timestamps.partition_point(|&x_t| x_t <= t));
    indices
        .map(|index| {
            let t = y.timestamps[index];
            while x_count < x.len() && x.timestamps[x_count] <= t {
                x_count += 1;
            }
            let x_value = x_count.checked_sub(1).map_or(f64::NAN, |i| x.value(i));
            [x_value, y.value(index)]
        })
        .collect()
}

/// Every `n`th of `points` so at most `max_points` remain, always with the last one.
fn thin_out(points: Vec<[f64; 2]>, max_points: usize) -> Vec<[f64; 2]> {
    if points.len() <= max_points {
        return points;
    }
    let step = points.len().div_ceil(max_points.max(1));
    let last = points.len() - 1;
    points
        .into_iter()
        .enumerate()
        .filter(|(index, _)| (last - index) % step == 0)
        .map(|(_, point)| point)
        .collect()
}

/// The opacity of the `step`th of `steps` steps of a trail, oldest first, that fades
/// out by `fade`: the newest step is opaque.
fn trail_opacity(fade: f32, step: usize, steps: usize) -> f32 {
    1.0 - fade.clamp(0.0, 1.0) * (1.0 - (step + 1) as f32 / steps.max(1) as f32)
}

/// The lines of a trail through `points` in `steps` steps of about the same number of
/// points, oldest first, each starting where the one before ends. With a pattern that
/// repeats every `period` points on the screen of a plot with `transform`, each step
/// ends where the pattern repeats, so the pattern runs on across the steps.
fn trail_lines(
    points: &[[f64; 2]],
    steps: usize,
    period: Option<f32>,
    transform: &egui_plot::PlotTransform,
) -> Vec<Vec<[f64; 2]>> {
    let step_len = points.len().div_ceil(steps.max(1)).max(1);
    let Some(period) = period.filter(|&period| period > 0.0) else {
        // the lines overlap by one point so the steps join
        return (0..points.len())
            .step_by(step_len)
            .map(|start| points[start..(start + step_len + 1).min(points.len())].to_vec())
            .collect();
    };
    let mut lines: Vec<Vec<[f64; 2]>> = vec![vec![]];
    // the length on screen since the last gap, where the pattern starts again, and
    // the length at which the current step is to end
    let mut walked = 0.0_f32;
    let mut end_at: Option<f32> = None;
    for (index, &point) in points.iter().enumerate() {
        if index > 0 && index % step_len == 0 {
            let line = lines.last_mut().expect("there is always a line");
            if line.last().is_some_and(is_point) {
                end_at = Some((walked / period).ceil() * period);
            } else if !line.is_empty() {
                // after a gap, where the pattern starts again anyway
                lines.push(vec![]);
            }
        }
        let line = lines.last_mut().expect("there is always a line");
        match line.last().copied() {
            Some(previous) if is_point(&previous) && is_point(&point) => {
                let length = to_screen(previous, transform).distance(to_screen(point, transform));
                if let Some(end) = end_at.filter(|&end| walked + length >= end) {
                    // the step ends between the points, where the pattern repeats
                    let t = if length > 0.0 {
                        f64::from((end - walked) / length)
                    } else {
                        0.0
                    };
                    let cut = [
                        previous[0] + (point[0] - previous[0]) * t,
                        previous[1] + (point[1] - previous[1]) * t,
                    ];
                    line.push(cut);
                    lines.push(vec![cut]);
                    end_at = None;
                }
                walked += length;
            }
            _ if !is_point(&point) => {
                // the pattern starts again after a gap, where a pending step ends
                walked = 0.0;
                if end_at.take().is_some() {
                    lines.push(vec![]);
                }
            }
            _ => {}
        }
        lines
            .last_mut()
            .expect("there is always a line")
            .push(point);
    }
    lines
}

/// Draws the lines through `points` in `style`, if any, broken where a point is
/// missing.
fn plot_lines(
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
fn plot_points(
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

/// The parent of `path` and the name of its last level.
fn split_last(path: &str) -> (&str, &str) {
    match path.rfind(['/', '.']) {
        Some(index) => (&path[..index], &path[index + 1..]),
        None => ("", path),
    }
}

/// For a trace named y, the path of its sibling named x, e.g. `pose.x` for `pose.y`.
fn x_sibling(path: &str) -> Option<String> {
    let (_, name) = split_last(path);
    let x = match name {
        "y" => "x",
        "Y" => "X",
        _ => return None,
    };
    Some(format!("{}{x}", &path[..path.len() - name.len()]))
}

/// The names of the `lines`, each an x and a y path: the parent of both if they share
/// it and no other line does, e.g. `pose` for `pose.x` and `pose.y`, else the name of
/// the y trace; as short as they can be told apart.
fn line_names<'a>(lines: &[(&'a str, &'a str)]) -> Vec<&'a str> {
    let parent = |(x, y): (&'a str, &'a str)| {
        let (x_parent, _) = split_last(x);
        let (y_parent, _) = split_last(y);
        (x_parent == y_parent && !y_parent.is_empty()).then_some(y_parent)
    };
    let keys: Vec<&str> = lines
        .iter()
        .map(|&line| match parent(line) {
            Some(shared)
                if lines
                    .iter()
                    .filter(|&&other| parent(other) == Some(shared))
                    .count()
                    == 1 =>
            {
                shared
            }
            _ => line.1,
        })
        .collect();
    short_names(keys)
}

/// The label of an axis: the name of its trace and the unit of its traces.
fn axis_label(name: &str, unit: Option<&str>) -> String {
    match unit {
        Some(unit) if name.is_empty() => format!("[{unit}]"),
        Some(unit) => format!("{name} [{unit}]"),
        None => name.to_owned(),
    }
}

impl Xy {
    /// Before layout version 2, the x trace was not one of the shown traces.
    pub fn migrate_v1(&mut self) {
        if let Some(x) = &self.x {
            self.traces.insert(x.clone());
        }
    }

    /// Before layout version 4, there was one x trace: the chosen one if it was shown,
    /// else the first shown trace, and all other traces were plotted against it.
    pub fn migrate_v3(&mut self) {
        let x = (self.x.take())
            .filter(|x| self.traces.contains(x))
            .or_else(|| self.traces.first().cloned());
        if let Some(x) = x {
            for y in self.traces.iter().filter(|&path| *path != x) {
                self.against.insert(y.clone(), x.clone());
            }
            self.xs.insert(x);
        }
    }

    /// Paths of the shown traces that are whole arrays, plotted over the indices of
    /// their elements instead of against the x trace.
    fn array_paths<'a>(&'a self, model: &Model) -> Vec<&'a str> {
        self.traces
            .iter()
            .map(String::as_str)
            .filter(|path| {
                (model.traces.get(path)).is_some_and(|trace| ViewType::Xy.shows_apart(trace))
            })
            .collect()
    }

    /// Puts each shown trace on an axis and forgets the traces that are not shown.
    /// Each newly shown trace named y whose sibling named x is shown too is plotted
    /// against it, e.g. `pose.y` against `pose.x`. The other new traces and those
    /// whose x trace is gone are plotted against the first x trace, or the first of
    /// them is the x trace if there is none.
    fn assign_axes(&mut self, model: &Model) {
        if !self.array_paths(model).is_empty() {
            // arrays are plotted over the indices of their elements
            self.xs.clear();
            self.against.clear();
            return;
        }
        let traces = &self.traces;
        self.xs.retain(|path| traces.contains(path));
        let xs = &self.xs;
        self.against
            .retain(|y, x| traces.contains(y) && !xs.contains(y) && xs.contains(x));
        let new: Vec<&String> = (traces.iter())
            .filter(|&path| !xs.contains(path) && !self.against.contains_key(path))
            .collect();
        let mut rest = vec![];
        for &path in &new {
            let x = x_sibling(path).filter(|x| self.xs.contains(x) || new.contains(&x));
            match x {
                Some(x) => {
                    self.xs.insert(x.clone());
                    self.against.insert(path.clone(), x);
                }
                None => rest.push(path),
            }
        }
        for path in rest {
            if self.xs.contains(path) {
                // the sibling of a trace named y
                continue;
            }
            match self.xs.first() {
                Some(x) => {
                    self.against.insert(path.clone(), x.clone());
                }
                None => {
                    self.xs.insert(path.clone());
                }
            }
        }
    }

    /// Puts the trace at `path` on the x axis if `is_x`, else on the y axis against
    /// the first x trace. If it was the only x trace, another trace takes its place.
    fn set_axis(&mut self, path: &str, is_x: bool) {
        if is_x {
            self.against.remove(path);
            self.xs.insert(path.to_owned());
            return;
        }
        self.xs.remove(path);
        if self.xs.is_empty()
            && let Some(other) = self.traces.iter().find(|&other| other != path)
        {
            self.against.remove(other);
            self.xs.insert(other.clone());
        }
        if let Some(x) = self.xs.first() {
            self.against.insert(path.to_owned(), x.clone());
        }
    }

    /// The lines plotted, each the path of an x trace and of a y trace plotted
    /// against it, in the order of the y traces.
    fn lines(&self) -> Vec<(&str, &str)> {
        (self.against.iter())
            .map(|(y, x)| (x.as_str(), y.as_str()))
            .collect()
    }

    /// A plot with `x_axis` and the y axis labelled `y_label`, whose tooltips name the
    /// x coordinate `x_name`.
    fn plot(
        &self,
        ui: &egui::Ui,
        x_name: &'static str,
        x_axis: egui_plot::AxisHints<'static>,
        y_label: String,
    ) -> egui_plot::Plot<'static> {
        let plot = egui_plot::Plot::new(ui.id().with("xy"))
            .grid_color(theme::grid_color(ui.visuals()))
            .legend(
                // the newest point is drawn first in full color, the trail fades out
                egui_plot::Legend::default()
                    .color_conflict_handling(egui_plot::ColorConflictHandling::PickFirst),
            )
            .custom_x_axes(vec![x_axis])
            .y_axis_label(y_label)
            .label_formatter(move |hover| {
                let (name, point) = match hover {
                    egui_plot::HoverPosition::NearDataPoint {
                        plot_name,
                        position,
                        ..
                    } => (*plot_name, position),
                    egui_plot::HoverPosition::Elsewhere { position } => ("", position),
                };
                Some(format!(
                    "{name}\n{x_name} = {}\ny = {}",
                    format_significant(point.x, SHOWN_DIGITS),
                    format_significant(point.y, SHOWN_DIGITS)
                ))
            });
        if self.equal_axes {
            plot.data_aspect(1.0)
        } else {
            plot
        }
    }

    /// Draws `points` as a line fading out towards the first points, and the last point
    /// as a dot.
    fn plot_trail(
        &self,
        name: &str,
        points: &[[f64; 2]],
        color: egui::Color32,
        style: LineStyle,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        if let Some(&last) = points.last().filter(|point| is_point(point)) {
            plot_ui.points(
                egui_plot::Points::new(name, vec![last])
                    .color(color)
                    .radius(4.0)
                    .filled(true),
            );
        }
        let transform = *plot_ui.transform();
        let steps = if self.fade > 0.0 { FADE_STEPS } else { 1 };
        let line_style = style.plot_style(points, &transform);
        let period = line_style
            .filter(|&line_style| line_style != egui_plot::LineStyle::Solid)
            .and(style.pattern_period());
        let lines = trail_lines(points, steps, period, &transform);
        for (step, line) in lines.iter().enumerate() {
            let color = color.gamma_multiply(trail_opacity(self.fade, step, lines.len()));
            plot_lines(name, line, color, line_style, plot_ui);
        }

        let count = points.iter().filter(|point| is_point(point)).count();
        if style.marks_points(self.points, count, transform.frame().width()) {
            let step_len = points.len().div_ceil(steps).max(1);
            let steps = points.len().div_ceil(step_len);
            for (step, chunk) in points.chunks(step_len).enumerate() {
                let color = color.gamma_multiply(trail_opacity(self.fade, step, steps));
                plot_points(name, chunk, color, plot_ui);
            }
        }
    }

    /// Shows the y traces against their x traces within the shown window.
    fn show_plot(&self, ui: &mut egui::Ui, model: &Model, time: &TimeRange) {
        let lines = self.lines();
        let names = line_names(&lines);
        let x_paths: BTreeSet<&str> = lines.iter().map(|&(x, _)| x).collect();
        let y_paths: Vec<&str> = lines.iter().map(|&(_, y)| y).collect();
        let x_unit = super::common_unit(model, x_paths.iter().copied());
        let x_label = match short_names(x_paths.iter().copied())[..] {
            [name] => axis_label(name, x_unit),
            _ => axis_label("", x_unit),
        };
        let y_unit = super::common_unit(model, y_paths.iter().copied());
        let y_label = match short_names(y_paths.iter().copied())[..] {
            [name] => axis_label(name, y_unit),
            _ => axis_label("", y_unit),
        };
        let x_axis = egui_plot::AxisHints::new_x().label(x_label);
        let mut plot = self.plot(ui, "x", x_axis, y_label);

        // the plot fits all values, also outside the shown window, until panned or
        // zoomed, and again after a double click
        for (low, high) in (x_paths.iter()).filter_map(|path| model.traces.series(path)?.range) {
            plot = plot.include_x(low).include_x(high);
        }
        for (low, high) in (y_paths.iter()).filter_map(|path| model.traces.series(path)?.range) {
            plot = plot.include_y(low).include_y(high);
        }
        let range = time
            .range()
            .map(|(start, end)| (time.timestamp(start), time.timestamp(end)));
        let cursor = time.cursor().map(|t| time.timestamp(t));
        let ring_color = ui.visuals().strong_text_color();
        let visuals = ui.visuals().clone();
        let response = plot.show(ui, |plot_ui| {
            let Some(range) = range else {
                return;
            };
            for (index, (&(x_path, y_path), name)) in lines.iter().zip(&names).enumerate() {
                let (Some(x), Some(y)) = (model.traces.series(x_path), model.traces.series(y_path))
                else {
                    continue;
                };
                let color = theme::trace_color(&visuals, index);
                let points = thin_out(pair(x, y, within(y.timestamps, range)), MAX_POINTS);
                let style = self.line_styles.of(y_path);
                self.plot_trail(name, &points, color, style, plot_ui);

                // where the trace was at the cursor
                let at_cursor = cursor
                    .and_then(|t| index_at(y.timestamps, t))
                    .map(|index| pair(x, y, index..index + 1)[0])
                    .filter(is_point);
                if let Some(point) = at_cursor {
                    plot_ui.points(
                        egui_plot::Points::new("", vec![point])
                            .color(ring_color)
                            .radius(6.0)
                            .filled(false)
                            .allow_hover(false),
                    );
                }
            }
        });
        super::zoom_on_wheel(&response.response);
    }

    /// Shows the elements of the arrays at `paths` at the cursor time over their
    /// indices.
    fn show_arrays(&self, ui: &mut egui::Ui, model: &Model, time: &TimeRange, paths: &[&str]) {
        let names = short_names(paths.iter().copied());
        let unit = super::common_unit(model, paths.iter().copied());
        let y_label = match names[..] {
            [name] => axis_label(name, unit),
            _ => axis_label("", unit),
        };
        let mut plot = self
            .plot(ui, "index", index_x_axis("index"), y_label)
            .x_grid_spacer(index_spacer);
        // the plot fits all elements and all their values until panned or zoomed
        for trace in paths.iter().filter_map(|path| model.traces.get(path)) {
            if let Samples::Arrays { shape, .. } = &trace.samples {
                let len = shape.iter().product::<usize>();
                plot = plot.include_x(0.0).include_x(len.saturating_sub(1) as f64);
            }
            if let Some((low, high)) = trace.value_range() {
                plot = plot.include_y(low).include_y(high);
            }
        }
        let visuals = ui.visuals().clone();
        let response = plot.show(ui, |plot_ui| {
            for (index, (path, name)) in paths.iter().zip(&names).enumerate() {
                let Some(trace) = model.traces.get(path) else {
                    continue;
                };
                let values: Vec<f64> = (sample_at_cursor(trace, time).iter())
                    .map(|number| number.as_f64())
                    .collect();
                let color = theme::trace_color(&visuals, index);
                let style = self.line_styles.of(path);
                plot_array(name, &values, color, style, self.points, plot_ui);
            }
        });
        super::zoom_on_wheel(&response.response);
    }
}

impl super::ViewInfo for Xy {
    const NAME: &'static str = "XY";
    const ICON: &'static str = egui_phosphor::regular::CHART_SCATTER;
    const WRITES_VALUES: bool = false;

    fn selectable(trace: Trace<'_>) -> Selectable {
        match trace.samples {
            Samples::Strings(_) => Selectable::No("Plots show numbers".into()),
            Samples::Arrays { shape, .. } if Self::shows_apart(trace) => Selectable::Array {
                elements: shape.iter().product::<usize>() <= MAX_LISTED_ELEMENTS,
            },
            Samples::Arrays { shape, .. } => super::elements(shape.as_slice()),
            _ => Selectable::Trace,
        }
    }

    /// A 1D array is plotted as a whole over the indices of its elements.
    fn shows_apart(trace: Trace<'_>) -> bool {
        matches!(trace.samples, Samples::Arrays { shape, .. } if shape.len() == 1)
    }
}

impl Viewable for Xy {
    fn colored_paths(&self, model: &Model) -> Vec<String> {
        let arrays = self.array_paths(model);
        let paths = if arrays.is_empty() {
            self.against.keys().map(String::as_str).collect()
        } else {
            arrays
        };
        paths.into_iter().map(str::to_owned).collect()
    }

    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = false;
        Some((&mut self.traces, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.line_styles.retain_shown(&self.traces);
        self.assign_axes(context.model);
        let arrays = self.array_paths(context.model);
        if !arrays.is_empty() {
            self.show_arrays(ui, context.model, context.time, &arrays);
        } else if self.against.is_empty() {
            ui.weak("Select an x and a y trace, or an array, in the traces panel");
        } else {
            self.show_plot(ui, context.model, context.time);
        }
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let arrays: Vec<String> = (self.array_paths(context.model).into_iter())
            .map(str::to_owned)
            .collect();
        self.assign_axes(context.model);
        let paths: Vec<String> = self.traces.iter().cloned().collect();
        let names = short_names(paths.iter().map(String::as_str));
        let name_by_path: BTreeMap<&str, &str> = (paths.iter().map(String::as_str))
            .zip(names.iter().copied())
            .collect();
        let name_of = |path: &str| name_by_path.get(path).copied().unwrap_or("").to_owned();
        super::properties_section(ui, "Axes", |ui| {
            if arrays.is_empty() {
                for (path, &name) in paths.iter().zip(&names) {
                    super::property_label(ui, name).on_hover_text(path);
                    ui.horizontal(|ui| {
                        let mut is_x = self.xs.contains(path);
                        let height = ui.spacing().interact_size.y;
                        let changed = ui
                            .allocate_ui(egui::vec2(AXIS_CHOICE_WIDTH, height), |ui| {
                                theme::segmented(ui, &mut is_x, &[(true, "X"), (false, "Y")])
                            })
                            .inner;
                        if changed {
                            self.set_axis(path, is_x);
                            self.assign_axes(context.model);
                        }
                        let Some(current) = self.against.get(path).cloned() else {
                            return;
                        };
                        egui::ComboBox::from_id_salt(("xy against", path))
                            .selected_text(name_of(&current))
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for x in &self.xs {
                                    if ui
                                        .selectable_label(*x == current, name_of(x))
                                        .on_hover_text(x)
                                        .clicked()
                                    {
                                        self.against.insert(path.clone(), x.clone());
                                    }
                                }
                            })
                            .response
                            .on_hover_text(format!("Plotted against {current}"));
                    });
                    ui.end_row();
                }
            } else {
                super::property_label(ui, "X");
                ui.weak("Index of the elements")
                    .on_hover_text("Arrays are plotted over the indices of their elements");
                ui.end_row();
            }
            super::property_label(ui, "Equal axes");
            ui.checkbox(&mut self.equal_axes, "")
                .on_hover_text("Give a unit the same length on both axes");
            ui.end_row();
        });
        super::properties_section(ui, "Lines", |ui| {
            self.points.choice_row(ui);
            super::property_label(ui, "Fade");
            let mut percent = self.fade * 100.0;
            let response = ui
                .add_enabled(
                    arrays.is_empty(),
                    egui::DragValue::new(&mut percent)
                        .range(0.0..=100.0)
                        .max_decimals(0)
                        .suffix(" %"),
                )
                .on_hover_text("How far the oldest part of the trail fades out")
                .on_disabled_hover_text(
                    "Arrays are shown at the cursor time only, without a trail",
                );
            if response.changed() {
                self.fade = percent / 100.0;
            }
            ui.end_row();
        });
        let styled: Vec<String> = if arrays.is_empty() {
            self.against.keys().cloned().collect()
        } else {
            arrays
        };
        let styled: Vec<&str> = styled.iter().map(String::as_str).collect();
        self.line_styles.properties_ui(ui, &styled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::traces::Traces;
    use proto::Data;

    /// Traces of the given samples at the given timestamps.
    fn traces(samples: &[(u64, &str, Data)]) -> Traces {
        let mut traces = Traces::default();
        for (timestamp, path, value) in samples {
            traces.append(&proto::Value {
                timestamp: *timestamp,
                path: path.to_string(),
                value: value.clone(),
            });
        }
        traces
    }

    #[test]
    fn samples_are_paired_with_the_current_x() {
        let traces = traces(&[
            (1, "y", Data::Float(10.0)),
            (2, "x", Data::Float(1.0)),
            (2, "y", Data::Float(20.0)),
            (3, "y", Data::Float(30.0)),
            (4, "x", Data::Float(2.0)),
            (5, "y", Data::Null),
        ]);
        let (x, y) = (traces.series("x").unwrap(), traces.series("y").unwrap());
        let points = pair(x, y, 0..y.len());
        assert!(points[0][0].is_nan());
        assert_eq!(points[1], [1.0, 20.0]);
        assert_eq!(points[2], [1.0, 30.0]);
        assert_eq!(points[3][0], 2.0);
        assert!(!is_point(&points[3]));
        assert_eq!(pair(x, y, 2..3), [[1.0, 30.0]]);
    }

    /// An XY view of the traces at `paths` with their axes assigned.
    fn xy_of(paths: &[&str]) -> Xy {
        let mut xy = Xy {
            traces: paths.iter().map(|&path| path.to_owned()).collect(),
            ..Xy::default()
        };
        xy.assign_axes(&Model::default());
        xy
    }

    /// The lines of `xy` as owned pairs of x and y paths.
    fn lines_of(xy: &Xy) -> Vec<(String, String)> {
        (xy.lines().into_iter())
            .map(|(x, y)| (x.to_owned(), y.to_owned()))
            .collect()
    }

    fn line(x: &str, y: &str) -> (String, String) {
        (x.to_owned(), y.to_owned())
    }

    #[test]
    fn the_first_trace_is_the_x_trace_unless_siblings_pair_up() {
        let xy = xy_of(&["a", "b", "c"]);
        assert_eq!(lines_of(&xy), [line("a", "b"), line("a", "c")]);

        let xy = xy_of(&[
            "heading",
            "one/pose.x",
            "one/pose.y",
            "two/pos/X",
            "two/pos/Y",
        ]);
        assert_eq!(
            lines_of(&xy),
            [
                line("one/pose.x", "heading"),
                line("one/pose.x", "one/pose.y"),
                line("two/pos/X", "two/pos/Y"),
            ]
        );
    }

    #[test]
    fn newly_shown_traces_keep_the_axes_of_the_others() {
        let mut xy = xy_of(&["a", "b"]);
        // a y trace whose sibling x is already an x trace
        for path in ["c", "p.x", "p.y"] {
            xy.traces.insert(path.to_owned());
            xy.assign_axes(&Model::default());
        }
        assert_eq!(
            lines_of(&xy),
            [
                line("a", "b"),
                line("a", "c"),
                line("a", "p.x"),
                line("a", "p.y")
            ]
        );
        xy.set_axis("p.x", true);
        xy.against.remove("p.y");
        xy.assign_axes(&Model::default());
        assert_eq!(lines_of(&xy)[2], line("p.x", "p.y"));
    }

    #[test]
    fn y_traces_of_a_gone_x_trace_move_to_another_one() {
        let mut xy = xy_of(&["a", "b", "p.x", "p.y"]);
        xy.set_axis("a", true);
        xy.against.insert("b".to_owned(), "p.x".to_owned());
        xy.traces.remove("p.x");
        xy.assign_axes(&Model::default());
        assert_eq!(lines_of(&xy), [line("a", "b"), line("a", "p.y")]);

        // without an x trace, the first trace takes its place
        xy.traces.remove("a");
        xy.assign_axes(&Model::default());
        assert_eq!(lines_of(&xy), [line("b", "p.y")]);
    }

    #[test]
    fn the_only_x_trace_put_on_the_y_axis_gives_way_to_another() {
        let mut xy = xy_of(&["a", "b", "c"]);
        xy.set_axis("a", false);
        xy.assign_axes(&Model::default());
        assert_eq!(xy.xs, ["b".to_owned()].into());
        assert_eq!(lines_of(&xy), [line("b", "a"), line("b", "c")]);

        // with another x trace, the y traces of the one moved stay on that
        xy.set_axis("c", true);
        xy.against.insert("a".to_owned(), "c".to_owned());
        xy.set_axis("c", false);
        xy.assign_axes(&Model::default());
        assert_eq!(lines_of(&xy), [line("b", "a"), line("b", "c")]);
    }

    #[test]
    fn version_3_layouts_plot_all_traces_against_their_one_x_trace() {
        let mut xy: Xy =
            serde_json::from_str(r#"{"x": "b", "traces": ["b", "p.x", "p.y"]}"#).unwrap();
        xy.migrate_v3();
        xy.assign_axes(&Model::default());
        assert_eq!(lines_of(&xy), [line("b", "p.x"), line("b", "p.y")]);
        let json = serde_json::to_string(&xy).unwrap();
        assert!(!json.contains(r#""x""#), "{json}");

        let mut xy: Xy = serde_json::from_str(r#"{"traces": ["a", "b"]}"#).unwrap();
        xy.migrate_v3();
        assert_eq!(lines_of(&xy), [line("a", "b")]);
    }

    #[test]
    fn lines_are_named_after_the_parent_they_share() {
        assert_eq!(
            line_names(&[
                ("one/pose.x", "one/pose.y"),
                ("two/pose.x", "two/pose.y"),
                ("one/pose.x", "speed"),
            ]),
            ["one/pose", "two/pose", "speed"]
        );
        // two lines of one parent are named after their y traces
        assert_eq!(
            line_names(&[("pose.x", "pose.y"), ("pose.x", "pose.z"), ("x", "y")]),
            ["pose.y", "z", "y"]
        );
    }

    #[test]
    fn the_trail_fades_towards_the_oldest_step() {
        assert_eq!(trail_opacity(0.0, 0, 8), 1.0);
        assert_eq!(trail_opacity(0.85, 7, 8), 1.0);
        assert!((trail_opacity(1.0, 0, 4) - 0.25).abs() < 1.0e-6);
        assert!((trail_opacity(0.5, 1, 4) - 0.75).abs() < 1.0e-6);
        // out of range in a layout file
        assert!(trail_opacity(2.0, 0, 4) > 0.0);
    }

    /// A transform showing plot coordinates as screen points, 1 per unit.
    fn unit_transform() -> egui_plot::PlotTransform {
        egui_plot::PlotTransform::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
            egui_plot::PlotBounds::from_min_max([0.0, 0.0], [100.0, 100.0]),
            egui::Vec2b::FALSE,
        )
    }

    #[test]
    fn trail_steps_join_and_end_where_the_pattern_repeats() {
        let transform = unit_transform();
        let points: Vec<[f64; 2]> = (0..8).map(|i| [f64::from(i) * 3.0, 0.0]).collect();

        // without a pattern, the steps split the points evenly and share one point
        let lines = trail_lines(&points, 4, None, &transform);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], [[0.0, 0.0], [3.0, 0.0], [6.0, 0.0]]);
        assert_eq!(lines[3], [[18.0, 0.0], [21.0, 0.0]]);

        // with a pattern of 5, the steps end at whole multiples of it
        let lines = trail_lines(&points, 4, Some(5.0), &transform);
        assert_eq!(lines.len(), 4);
        for pair in lines.windows(2) {
            let end = *pair[0].last().unwrap();
            assert_eq!(pair[1][0], end);
            let periods = end[0] / 5.0;
            assert!((periods - periods.round()).abs() < 1.0e-6, "{end:?}");
        }
        // the first step was to end after 2 points, at 3, so it ends at 5
        assert!((lines[0].last().unwrap()[0] - 5.0).abs() < 1.0e-6);
        assert_eq!(lines[3].last(), Some(&[21.0, 0.0]));
        // all points are drawn
        let drawn: usize = lines.iter().map(Vec::len).sum();
        assert_eq!(drawn, points.len() + 2 * (lines.len() - 1));
    }

    #[test]
    fn a_gap_starts_the_pattern_and_a_pending_step_again() {
        let transform = unit_transform();
        let points = [
            [0.0, 0.0],
            [1.0, 0.0],
            [f64::NAN, 0.0],
            [7.0, 0.0],
            [9.0, 0.0],
        ];
        let lines = trail_lines(&points, 2, Some(5.0), &transform);
        assert_eq!(lines.len(), 2);
        // the step ends at the gap, before the length it was to end at
        assert_eq!(lines[0].len(), 3);
        assert_eq!(lines[0][..2], [[0.0, 0.0], [1.0, 0.0]]);
        assert!(!is_point(&lines[0][2]));
        assert_eq!(lines[1], [[7.0, 0.0], [9.0, 0.0]]);
    }

    #[test]
    fn layouts_without_the_new_properties_fade_as_before() {
        let xy: Xy = serde_json::from_str(r#"{"traces": ["a", "b"]}"#).unwrap();
        assert_eq!(xy.fade, DEFAULT_FADE);
        assert_eq!(xy.points, ShowPoints::Auto);
        let json = serde_json::to_string(&xy).unwrap();
        assert!(!json.contains("line_styles"), "{json}");
    }

    struct State {
        xy: Xy,
        model: Model,
        time: TimeRange,
        invoker: crate::app::model::actions::Invoker,
    }

    /// A harness showing `xy` of an array trace `adc` of 32 elements and a float trace
    /// `rpm`, both with two samples.
    fn xy_harness(xy: Xy) -> egui_kittest::Harness<'static, State> {
        let mut model = Model::default();
        for (second, value) in [(0, 1.0), (1, 2.0)] {
            let timestamp = second * 1_000_000_000;
            let elements = (0..32).map(|i| Data::Float(value * f64::from(i))).collect();
            for (path, data) in [("adc", Data::Array(elements)), ("rpm", Data::Float(value))] {
                model.traces.append(&proto::Value {
                    timestamp,
                    path: path.to_owned(),
                    value: data,
                });
            }
        }
        let state = State {
            xy,
            model,
            time: Default::default(),
            invoker: Default::default(),
        };
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
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
                    state.xy.view(ui, &mut context);
                },
                state,
            )
    }

    #[test]
    fn a_single_array_is_plotted_over_its_indices() {
        use egui_kittest::kittest::Queryable;

        let xy = Xy {
            traces: ["adc".to_owned()].into(),
            ..Xy::default()
        };
        let mut harness = xy_harness(xy);
        harness.run();
        let state = harness.state();
        assert_eq!(state.xy.array_paths(&state.model), ["adc"]);
        assert!(harness.query_by_label_contains("Select").is_none());

        // a single trace that is no array is not enough
        let xy = Xy {
            traces: ["rpm".to_owned()].into(),
            ..Xy::default()
        };
        let mut harness = xy_harness(xy);
        harness.run();
        assert!(
            harness
                .state()
                .xy
                .array_paths(&harness.state().model)
                .is_empty()
        );
        assert!(harness.query_by_label_contains("Select").is_some());
    }

    #[test]
    fn thinning_out_keeps_the_last_point() {
        let points: Vec<[f64; 2]> = (0..10).map(|i| [i as f64, 0.0]).collect();
        let thinned = thin_out(points, 4);
        assert!(thinned.len() <= 4);
        assert_eq!(thinned.last(), Some(&[9.0, 0.0]));
    }
}
