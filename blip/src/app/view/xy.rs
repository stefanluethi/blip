//! An XY plot: traces plotted against other traces instead of against time, e.g.
//! positions as paths.

use super::Selectable;
use super::plot::{LineStyle, LineStyles, ShowPoints};
use super::plotting::{
    index_spacer, index_under, index_x_axis, is_point, nearest_point, plot_array, plot_lines,
    plot_points, sample_at_cursor, to_screen,
};
use super::tooltip::{Header, Label, Row, Tooltip, TraceName, Value};
use super::trace_selection::MAX_LISTED_ELEMENTS;
use super::{PropertiesContext, ViewContext, ViewType, Viewable, short_names, with_unit};
use crate::app::model::Model;
use crate::app::model::numbers::{DisplayFormat, Number, SHOWN_DIGITS, format_significant};
use crate::app::model::sorted::{index_at, upper_bound, within};
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

/// The shown traces on the y axis, each plotted against one of the shown traces on
/// the x axis, within the shown window. Or the shown arrays at the cursor time,
/// plotted over the indices of their elements, e.g. snapshots of an ADC.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Xy {
    /// Paths of the shown traces, on the x axis and on the y axis, or the shown
    /// arrays, see [`Xy::array_paths`].
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
        .map_or(0, |&t| upper_bound(x.timestamps, t));
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

/// The parent of `path` and the name of its last level.
fn split_last(path: &str) -> (&str, &str) {
    match path.rfind(['/', '.']) {
        Some(index) => (&path[..index], &path[index + 1..]),
        None => ("", path),
    }
}

/// The path of the sibling of `path` named after its name in `names`, pairs of a name
/// and the name of the sibling.
fn sibling(path: &str, names: [(&str, &str); 2]) -> Option<String> {
    let (_, name) = split_last(path);
    let (_, other) = names.into_iter().find(|&(from, _)| from == name)?;
    Some(format!("{}{other}", &path[..path.len() - name.len()]))
}

/// For a trace named y, the path of its sibling named x, e.g. `pose.x` for `pose.y`.
fn x_sibling(path: &str) -> Option<String> {
    sibling(path, [("y", "x"), ("Y", "X")])
}

/// For a trace named x, the path of its sibling named y, e.g. `pose.y` for `pose.x`.
fn y_sibling(path: &str) -> Option<String> {
    sibling(path, [("x", "y"), ("X", "Y")])
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

/// The label of an axis: the name of its trace, if it has one, and the unit of its traces.
fn axis_label(names: &[&str], unit: Option<&str>) -> String {
    let name = match names {
        [name] => name,
        _ => "",
    };
    match unit {
        Some(unit) if name.is_empty() => format!("[{unit}]"),
        Some(unit) => format!("{name} [{unit}]"),
        None => name.to_owned(),
    }
}

/// The point nearest the pointer: the index of its line and its coordinates.
#[derive(Clone, Copy)]
struct Hovered {
    line: usize,
    point: [f64; 2],
}

/// `point` of the line `name` of `y` against `x`, the line's `index` picking its
/// colour: the line as header, the names of both traces with their values and units.
fn point_tooltip(
    model: &Model,
    (x, y): (&str, &str),
    name: &str,
    index: usize,
    point: [f64; 2],
) -> Tooltip {
    let header = TraceName::new(name, y, index);
    let names = short_names([x, y]);
    let row = |path: &str, name: &str, value: f64| {
        let text = with_unit(
            format_significant(value, SHOWN_DIGITS),
            model.traces.unit(path),
        );
        Row::new(Label::Weak(name.to_owned()), [Value::text(text)])
    };
    let rows = vec![row(x, names[0], point[0]), row(y, names[1], point[1])];
    Tooltip::new(Header::Trace(header)).group(rows)
}

/// Element `index` of the `arrays`, each its path, name and elements at the cursor:
/// the index as header, a row per array that has the element.
fn element_tooltip(model: &Model, index: usize, arrays: &[(&str, &str, Vec<Number>)]) -> Tooltip {
    let rows = (arrays.iter().enumerate())
        .filter_map(|(line, (path, name, numbers))| {
            let number = *numbers.get(index)?;
            let trace = TraceName::new(name, path, line);
            let value = Value::number(
                number,
                SHOWN_DIGITS,
                DisplayFormat::Default,
                model.traces.unit(path),
            );
            Some(Row::new(Label::Trace(trace), [value]))
        })
        .collect();
    Tooltip::new(Header::Text(format!("[{index}]"))).group(rows)
}

impl Xy {
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
    /// against it, e.g. `pose.y` against `pose.x`, and so is a shown trace named y when
    /// its sibling named x is newly shown, unless traces are plotted against it. The
    /// other new traces and those whose x trace is gone are plotted against the first x
    /// trace, or the first of them is the x trace if there is none. While arrays are
    /// shown, which are plotted over the indices of their elements, the axes of the
    /// other traces are kept for when they are not.
    fn assign_axes(&mut self, model: &Model) {
        let traces = &self.traces;
        self.xs.retain(|path| traces.contains(path));
        let xs = &self.xs;
        self.against
            .retain(|y, x| traces.contains(y) && !xs.contains(y) && xs.contains(x));
        if !self.array_paths(model).is_empty() {
            return;
        }
        let new: Vec<&String> = (traces.iter())
            .filter(|&path| !xs.contains(path) && !self.against.contains_key(path))
            .collect();
        let mut rest = vec![];
        for &path in &new {
            let x = x_sibling(path).filter(|x| self.xs.contains(x) || new.contains(&x));
            if let Some(x) = x {
                self.xs.insert(x.clone());
                self.against.insert(path.clone(), x);
                continue;
            }
            // a trace named y shown before its sibling named x, unless it is an x
            // trace others are plotted against
            let y = y_sibling(path).filter(|y| {
                traces.contains(y) && !new.contains(&y) && !self.against.values().any(|x| x == y)
            });
            if let Some(y) = y {
                self.xs.remove(&y);
                self.xs.insert(path.clone());
                self.against.insert(y, path.clone());
                continue;
            }
            rest.push(path);
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

    /// The traces of the view with their axes assigned, see [`Xy::assign_axes`], without
    /// changing the view.
    fn assigned(&self, model: &Model) -> Self {
        let mut axes = Self {
            traces: self.traces.clone(),
            xs: self.xs.clone(),
            against: self.against.clone(),
            ..Self::default()
        };
        axes.assign_axes(model);
        axes
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

    /// A plot with `x_axis` and the y axis labelled `y_label`.
    fn plot(
        &self,
        ui: &egui::Ui,
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
            .y_axis_label(y_label);
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
        let x_label = axis_label(&short_names(x_paths.iter().copied()), x_unit);
        let y_unit = super::common_unit(model, y_paths.iter().copied());
        let y_label = axis_label(&short_names(y_paths.iter().copied()), y_unit);
        let x_axis = egui_plot::AxisHints::new_x().label(x_label);
        let mut plot = self.plot(ui, x_axis, y_label);

        // the plot fits all values, also outside the shown window, until panned or
        // zoomed, and again after a double click
        for (low, high) in (x_paths.iter()).filter_map(|path| model.traces.series(path)?.range) {
            plot = plot.include_x(low).include_x(high);
        }
        for (low, high) in (y_paths.iter()).filter_map(|path| model.traces.series(path)?.range) {
            plot = plot.include_y(low).include_y(high);
        }
        let range = time.timestamps();
        let cursor = time.cursor_timestamp();
        let ring_color = ui.visuals().strong_text_color();
        let visuals = ui.visuals().clone();
        let radius = ui.style().interaction.interact_radius;
        let response = plot.show(ui, |plot_ui| {
            let range = range?;
            let mut hovered: Option<(Hovered, f32)> = None;
            for (index, (&(x_path, y_path), name)) in lines.iter().zip(&names).enumerate() {
                let (Some(x), Some(y)) = (model.traces.series(x_path), model.traces.series(y_path))
                else {
                    continue;
                };
                let color = theme::trace_color(&visuals, index);
                let points = thin_out(pair(x, y, within(y.timestamps, range)), MAX_POINTS);
                let near = (plot_ui.response().hover_pos())
                    .and_then(|pos| nearest_point(&points, pos, plot_ui.transform(), radius));
                if let Some((point, distance)) = near {
                    if hovered.is_none_or(|(_, best)| distance <= best) {
                        let point = points[point];
                        hovered = Some((Hovered { line: index, point }, distance));
                    }
                }
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
            hovered.map(|(hovered, _)| hovered)
        });
        super::zoom_on_wheel(&response.response);
        if let Some(Hovered { line, point }) = response.inner {
            let tooltip = point_tooltip(model, lines[line], names[line], line, point);
            (response.response).on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
        }
    }

    /// Shows the elements of the arrays at `paths` at the cursor time over their
    /// indices.
    fn show_arrays(&self, ui: &mut egui::Ui, model: &Model, time: &TimeRange, paths: &[&str]) {
        let names = short_names(paths.iter().copied());
        let unit = super::common_unit(model, paths.iter().copied());
        let y_label = axis_label(&names, unit);
        let mut plot = self
            .plot(ui, index_x_axis("index"), y_label)
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
        let arrays: Vec<(&str, &str, Vec<Number>)> = (paths.iter().zip(&names))
            .filter_map(|(&path, &name)| {
                Some((path, name, sample_at_cursor(model.traces.get(path)?, time)))
            })
            .collect();
        let longest = arrays.iter().map(|(_, _, numbers)| numbers.len()).max();
        let response = plot.show(ui, |plot_ui| {
            for (index, (path, name, numbers)) in arrays.iter().enumerate() {
                let values: Vec<f64> = numbers.iter().map(|number| number.as_f64()).collect();
                let color = theme::trace_color(&visuals, index);
                let style = self.line_styles.of(path);
                plot_array(name, &values, color, style, self.points, plot_ui);
            }
            let pointer = plot_ui
                .response()
                .hovered()
                .then(|| plot_ui.pointer_coordinate())
                .flatten()?;
            index_under(pointer.x, longest?)
        });
        super::zoom_on_wheel(&response.response);
        if let Some(index) = response.inner {
            let tooltip = element_tooltip(model, index, &arrays);
            (response.response).on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
        }
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
        if arrays.is_empty() {
            // the y traces as the view plots them, also before it assigns the axes of
            // the traces checked since it was last shown
            self.assigned(model).against.into_keys().collect()
        } else {
            arrays.into_iter().map(str::to_owned).collect()
        }
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
            theme::hint(
                ui,
                "Select an x and a y trace, or an array, in the traces panel",
            );
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
                super::path_rows(ui, &paths, |ui, path| {
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
                });
            } else {
                super::property_label(ui, "X");
                ui.weak("Index of the elements")
                    .on_hover_text("Arrays are plotted over the indices of their elements");
                ui.end_row();
            }
            super::checkbox_row(
                ui,
                "Equal axes",
                &mut self.equal_axes,
                "Give a unit the same length on both axes",
            );
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
mod tests;
