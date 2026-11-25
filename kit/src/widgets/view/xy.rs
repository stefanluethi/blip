//! An XY plot: traces plotted against other traces instead of against time, e.g.
//! positions as paths.

use super::Selectable;
use super::plot::{LegendPlacement, LineStyle, LineStyles, ShowPoints};
use super::plotting::{
    TitleEntry, corner_legend, index_spacer, index_under, index_x_axis, is_point, nearest_point,
    plot_array, plot_lines, plot_points, sample_at_cursor, title_legend, to_screen,
};
use super::tooltip::{Header, Label, Row, Tooltip, TraceName, Value};
use super::trace_selection::MAX_LISTED_ELEMENTS;
use super::{Ends, Fit, PropertiesContext, ViewContext, ViewType, Viewable, short_names};
use crate::store::numbers::{DisplayFormat, Number, SHOWN_DIGITS};
use crate::store::sorted::{index_at, upper_bound, within};
use crate::store::traces::{Samples, Series, Trace};
use crate::store::{Store, Window};
use crate::widgets::theme;
use crate::widgets::time::TimeRange;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// Upper bound of points drawn per trace.
const MAX_POINTS: usize = 20_000;
/// Number of steps in which the trail fades out towards older points.
const FADE_STEPS: usize = 8;
/// How far the trail fades out unless set otherwise.
const DEFAULT_FADE: f32 = 0.85;
/// The least width of the choice of a trace of a curve in the properties.
const MIN_TRACE_CHOICE_WIDTH: f32 = 48.0;

/// The shown traces on the y axis, each plotted against one of the shown traces on
/// the x axis, within the shown window. Or the shown arrays at the cursor time,
/// plotted over the indices of their elements, e.g. snapshots of an ADC.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Xy {
    /// Paths of the shown traces, on the x axis and on the y axis, or the shown
    /// arrays, see `Xy::array_paths`.
    pub traces: BTreeSet<String>,
    /// Paths of the shown traces on the x axis.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub xs: BTreeSet<String>,
    /// For each shown trace on the y axis, the path of the x trace it is plotted
    /// against, see `Xy::assign_axes`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub against: BTreeMap<String, String>,
    /// Whether a unit has the same length on both axes, e.g. for positions.
    pub equal_axes: bool,
    /// When the points are marked.
    pub points: ShowPoints,
    /// How far the oldest part of the trail fades out, from 0 for not at all to 1 for
    /// nearly invisible.
    pub fade: f32,
    /// What the axes fit.
    pub fit: Fit,
    /// The ends of the x axis, each `None` to fit.
    pub x_range: Ends,
    /// The ends of the y axis, each `None` to fit.
    pub y_range: Ends,
    /// Label of the x axis, the names and the unit of its traces if empty.
    pub x_label: String,
    /// Label of the y axis, the names and the unit of its traces if empty.
    pub y_label: String,
    /// Width of the lines in points.
    pub line_width: f32,
    /// Where the legend is shown; at the top right in views saved before the choice.
    #[serde(default = "legend_before_the_choice")]
    pub legend: LegendPlacement,
    #[serde(skip_serializing_if = "LineStyles::is_empty")]
    pub line_styles: LineStyles,
}

/// Where the legend of a view saved before the choice of its placement is shown.
fn legend_before_the_choice() -> LegendPlacement {
    LegendPlacement::TopRight
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
            fit: Fit::default(),
            x_range: Ends::default(),
            y_range: Ends::default(),
            x_label: String::new(),
            y_label: String::new(),
            line_width: 1.5,
            legend: LegendPlacement::default(),
            line_styles: LineStyles::default(),
        }
    }
}

/// The range an axis with `ends` is set to over `fitted`, the range it fits, `None` to
/// fit: while neither end is set or they are not in order.
fn pinned(ends: Ends, fitted: Option<(f64, f64)>) -> Option<(f64, f64)> {
    let wrong = ends.problems(fitted).iter().any(Option::is_some);
    if ends.is_auto() || wrong {
        return None;
    }
    ends.over(fitted)
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
        .filter(|(index, _)| (last - index).is_multiple_of(step))
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
    store: &dyn Store,
    (x, y): (&str, &str),
    name: &str,
    index: usize,
    point: [f64; 2],
) -> Tooltip {
    let header = TraceName::new(name, y, index);
    let names = short_names([x, y]);
    let row = |path: &str, name: &str, value: f64| {
        let values = Value::labelled(
            Number::Float(value),
            SHOWN_DIGITS,
            DisplayFormat::Labels,
            store.unit(path),
            store.labels(path),
        );
        Row::new(Label::Weak(name.to_owned()), values)
    };
    let rows = vec![row(x, names[0], point[0]), row(y, names[1], point[1])];
    Tooltip::new(Header::Trace(header))
        .group(rows)
        .describe(names[1], store.description(y))
}

/// Element `index` of the `arrays`, each its path, name and elements at the cursor:
/// the index as header, a row per array that has the element.
fn element_tooltip(
    store: &dyn Store,
    index: usize,
    arrays: &[(&str, &str, Vec<Number>)],
) -> Tooltip {
    let rows = (arrays.iter().enumerate())
        .filter_map(|(line, (path, name, numbers))| {
            let number = *numbers.get(index)?;
            let trace = TraceName::new(name, path, line);
            let values = Value::labelled(
                number,
                SHOWN_DIGITS,
                DisplayFormat::Labels,
                store.unit(path),
                store.labels(path),
            );
            Some(Row::new(Label::Trace(trace), values))
        })
        .collect();
    let tooltip = Tooltip::new(Header::Text(format!("[{index}]"))).group(rows);
    match arrays {
        [(path, name, _)] => tooltip.describe(name, store.description(path)),
        _ => tooltip,
    }
}

/// A combo box to choose one of `paths` by its name, `current` chosen, "—" for none.
/// Gives the path chosen in this frame.
fn trace_choice(
    ui: &mut egui::Ui,
    id: (&str, usize),
    current: Option<&String>,
    paths: &[String],
    width: f32,
    name_of: &impl Fn(&str) -> String,
) -> Option<String> {
    let mut chosen = None;
    let text = current.map_or_else(|| "—".to_owned(), |path| name_of(path));
    let response = egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .width(width)
        .truncate()
        .show_ui(ui, |ui| {
            for path in paths {
                let selected = current == Some(path);
                let option = ui.add(theme::selectable(selected, name_of(path)));
                if option.on_hover_text(path).clicked() && !selected {
                    chosen = Some(path.clone());
                }
            }
        })
        .response;
    if let Some(current) = current {
        response.on_hover_text(current);
    }
    chosen
}

impl Xy {
    /// Paths of the shown traces that are whole arrays, plotted over the indices of
    /// their elements instead of against the x trace.
    fn array_paths<'a>(&'a self, store: &dyn Store) -> Vec<&'a str> {
        self.traces
            .iter()
            .map(String::as_str)
            .filter(|path| {
                (store.trace(path, Window::NONE).data)
                    .is_some_and(|trace| ViewType::Xy.shows_apart(trace))
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
    fn assign_axes(&mut self, store: &dyn Store) {
        let traces = &self.traces;
        self.xs.retain(|path| traces.contains(path));
        let xs = &self.xs;
        self.against
            .retain(|y, x| traces.contains(y) && !xs.contains(y) && xs.contains(x));
        if !self.array_paths(store).is_empty() {
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
    fn assigned(&self, store: &dyn Store) -> Self {
        let mut axes = Self {
            traces: self.traces.clone(),
            xs: self.xs.clone(),
            against: self.against.clone(),
            ..Self::default()
        };
        axes.assign_axes(store);
        axes
    }

    /// Exchanges the traces at `a` and `b` in all curves: each takes the axis and the
    /// partners of the other, e.g. to swap the axes of a curve.
    fn swap(&mut self, a: &str, b: &str) {
        let other = |path: &String| match path.as_str() {
            path if path == a => b.to_owned(),
            path if path == b => a.to_owned(),
            _ => path.clone(),
        };
        self.xs = self.xs.iter().map(other).collect();
        self.against = (self.against.iter())
            .map(|(y, x)| (other(y), other(x)))
            .collect();
    }

    /// Puts `chosen` on the x side of the curve of the row `(x, y)` of the properties,
    /// `y` `None` for an x trace with no curve: the curve is plotted against `chosen` if
    /// it is an x trace, else `chosen` and `x` swap places.
    fn choose_x(&mut self, (x, y): (&str, Option<&str>), chosen: &str) {
        match y {
            Some(y) if self.xs.contains(chosen) => {
                self.against.insert(y.to_owned(), chosen.to_owned());
            }
            _ => self.swap(x, chosen),
        }
    }

    /// Puts `chosen` on the y side of the curve of the row `(x, y)` of the properties:
    /// `chosen` and `y` swap places, or, for an x trace with no curve, the y trace
    /// `chosen` is plotted against `x`.
    fn choose_y(&mut self, (x, y): (&str, Option<&str>), chosen: &str) {
        match y {
            Some(y) => self.swap(y, chosen),
            None if self.against.contains_key(chosen) => {
                self.against.insert(chosen.to_owned(), x.to_owned());
            }
            None => {}
        }
    }

    /// The rows of the axes in the properties: each curve as its x and y trace, then
    /// each x trace with no curve.
    fn axis_rows(&self) -> Vec<(String, Option<String>)> {
        let curves = (self.against.iter()).map(|(y, x)| (x.clone(), Some(y.clone())));
        let alone = (self.xs.iter())
            .filter(|x| !self.against.values().any(|against| against == *x))
            .map(|x| (x.clone(), None));
        curves.chain(alone).collect()
    }

    /// The lines plotted, each the path of an x trace and of a y trace plotted
    /// against it, in the order of the y traces.
    fn lines(&self) -> Vec<(&str, &str)> {
        (self.against.iter())
            .map(|(y, x)| (x.as_str(), y.as_str()))
            .collect()
    }

    /// A plot with `x_axis` and the y axis labelled `y_label`, unless their labels are
    /// set, that fits the `fitted` ranges of its x and y axis, or the ranges set.
    fn plot(
        &mut self,
        ui: &egui::Ui,
        x_axis: egui_plot::AxisHints<'static>,
        y_label: String,
        fitted: [Option<(f64, f64)>; 2],
    ) -> egui_plot::Plot<'static> {
        let x_axis = match self.x_label.as_str() {
            "" => x_axis,
            label => x_axis.label(label.to_owned()),
        };
        let y_label = match self.y_label.as_str() {
            "" => y_label,
            label => label.to_owned(),
        };
        let id = ui.id().with("xy");
        let mut plot = egui_plot::Plot::new(id)
            .grid_color(theme::grid_color(ui.visuals()))
            // the values at the pointer are shown in a tooltip
            .show_x(false)
            .show_y(false)
            .custom_x_axes(vec![x_axis])
            .y_axis_label(y_label);
        if self.equal_axes {
            plot = plot.data_aspect(1.0);
        }
        // ends set anew show as set, also after panning or zooming
        let set = (self.x_range, self.y_range, self.fit);
        let changed = ui.ctx().data_mut(|data| {
            let id = id.with("ends");
            let last = data.get_temp::<(Ends, Ends, Fit)>(id);
            data.insert_temp(id, set);
            last.is_some_and(|last| last != set)
        });
        if changed {
            plot = plot.reset();
        }
        // the plot fits the values, or the ends set, until panned or zoomed, and again
        // after a double click
        let [x, y] = fitted;
        match pinned(self.x_range, x) {
            Some((low, high)) => plot = plot.default_x_bounds(low, high),
            None => plot = include_ranges(plot, [x, None]),
        }
        match pinned(self.y_range, y) {
            Some((low, high)) => plot = plot.default_y_bounds(low, high),
            None => plot = include_ranges(plot, [None, y]),
        }
        plot
    }

    /// The legend of the plot in a corner, if it goes there: the title row's goes to the
    /// top right of a view without one.
    fn corner(&self, titled: bool) -> Option<egui_plot::Corner> {
        match self.legend {
            LegendPlacement::Title if !titled => Some(egui_plot::Corner::RightTop),
            placement => placement.corner(),
        }
    }

    /// The curves or the arrays, each a path its name and the index that picks its
    /// colour, as the legend shows them.
    fn legend_entries(&self, store: &dyn Store) -> Vec<(String, String, usize)> {
        let arrays = self.array_paths(store);
        if !arrays.is_empty() {
            let names = short_names(arrays.iter().copied());
            return (arrays.iter().zip(names).enumerate())
                .map(|(index, (path, name))| ((*path).to_owned(), name.to_owned(), index))
                .collect();
        }
        let assigned = self.assigned(store);
        let lines = assigned.lines();
        let names = line_names(&lines);
        (lines.iter().zip(names).enumerate())
            .map(|(index, (&(x, y), name))| (format!("{y} against {x}"), name.to_owned(), index))
            .collect()
    }

    /// The labels of the x and the y axis made of the names and the units of their
    /// traces, or of the shown `arrays`.
    fn automatic_labels(&self, store: &dyn Store, arrays: &[&str]) -> [String; 2] {
        if !arrays.is_empty() {
            let unit = super::common_unit(store, arrays.iter().copied());
            let names = short_names(arrays.iter().copied());
            return ["index".to_owned(), axis_label(&names, unit)];
        }
        let lines = self.lines();
        let x_paths: BTreeSet<&str> = lines.iter().map(|&(x, _)| x).collect();
        let y_paths: Vec<&str> = lines.iter().map(|&(_, y)| y).collect();
        let x_unit = super::common_unit(store, x_paths.iter().copied());
        let y_unit = super::common_unit(store, y_paths.iter().copied());
        [
            axis_label(&short_names(x_paths.iter().copied()), x_unit),
            axis_label(&short_names(y_paths.iter().copied()), y_unit),
        ]
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
            let line_style = (line_style, self.line_width);
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

    /// The ranges the x and the y axis fit: the values of the shown traces, also outside
    /// the shown window, or of the elements of the shown `arrays` over their indices,
    /// with the declared ranges of the traces if the fit is to the expected ones.
    fn axis_ranges(&self, store: &dyn Store, arrays: &[&str]) -> [Option<(f64, f64)>; 2] {
        let expected = |path: &str| match self.fit {
            Fit::Expected => store.expected(path),
            Fit::Values => None,
        };
        let range = |paths: &mut dyn Iterator<Item = &str>| {
            paths
                .map(|path| {
                    let values = store
                        .series(path, Window::NONE)
                        .data
                        .and_then(|series| series.range);
                    super::union(values, expected(path))
                })
                .fold(None, super::union)
        };
        if arrays.is_empty() {
            let lines = self.lines();
            let x_paths: BTreeSet<&str> = lines.iter().map(|&(x, _)| x).collect();
            [
                range(&mut x_paths.iter().copied()),
                range(&mut lines.iter().map(|&(_, y)| y)),
            ]
        } else {
            let x = (arrays.iter())
                .filter_map(|path| store.trace(path, Window::NONE).data)
                .filter_map(|trace| match &trace.samples {
                    Samples::Arrays { shape, .. } => {
                        let len = shape.iter().product::<usize>();
                        Some((0.0, len.saturating_sub(1) as f64))
                    }
                    _ => None,
                })
                .fold(None, |all, range| super::union(all, Some(range)));
            let y = (arrays.iter())
                .map(|path| {
                    let values = store
                        .trace(path, Window::NONE)
                        .data
                        .and_then(|t| t.value_range());
                    super::union(values, expected(path))
                })
                .fold(None, super::union);
            [x, y]
        }
    }

    /// Shows the y traces against their x traces within the shown window, the legend
    /// in `corner`, if any.
    fn show_plot(
        &mut self,
        ui: &mut egui::Ui,
        store: &dyn Store,
        time: &TimeRange,
        corner: Option<egui_plot::Corner>,
    ) {
        let [x_label, y_label] = self.automatic_labels(store, &[]);
        let x_axis = egui_plot::AxisHints::new_x().label(x_label);
        // all values, also outside the shown window
        let fitted = self.axis_ranges(store, &[]);
        let plot = self.plot(ui, x_axis, y_label, fitted);
        let lines = self.lines();
        let names = line_names(&lines);
        let range = time.timestamps();
        let cursor = time.cursor_timestamp();
        let ring_color = ui.visuals().strong_text_color();
        let visuals = ui.visuals().clone();
        let radius = ui.style().interaction.interact_radius;
        let response = plot.show(ui, |plot_ui| {
            let range = range?;
            let mut hovered: Option<(Hovered, f32)> = None;
            for (index, (&(x_path, y_path), name)) in lines.iter().zip(&names).enumerate() {
                let (Some(x), Some(y)) = (
                    store.series(x_path, Window::of(Some(range))).data,
                    store.series(y_path, Window::of(Some(range))).data,
                ) else {
                    continue;
                };
                let color = theme::trace_color(&visuals, index);
                let points = thin_out(pair(x, y, within(y.timestamps, range)), MAX_POINTS);
                let near = (plot_ui.response().hover_pos())
                    .and_then(|pos| nearest_point(&points, pos, plot_ui.transform(), radius));
                if let Some((point, distance)) = near
                    && hovered.is_none_or(|(_, best)| distance <= best)
                {
                    let point = points[point];
                    hovered = Some((Hovered { line: index, point }, distance));
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
        if let Some(corner) = corner {
            let entries: Vec<(&str, egui::Color32)> = (names.iter().enumerate())
                .map(|(index, &name)| (name, theme::trace_color(ui.visuals(), index)))
                .collect();
            corner_legend(ui, *response.transform.frame(), corner, entries);
        }
        if let Some(Hovered { line, point }) = response.inner {
            let tooltip = point_tooltip(store, lines[line], names[line], line, point);
            (response.response).on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
        }
    }

    /// Shows the elements of the arrays at `paths` at the cursor time over their
    /// indices, the legend in `corner`, if any.
    fn show_arrays(
        &mut self,
        ui: &mut egui::Ui,
        store: &dyn Store,
        time: &TimeRange,
        (paths, corner): (&[&str], Option<egui_plot::Corner>),
    ) {
        let names = short_names(paths.iter().copied());
        let [_, y_label] = self.automatic_labels(store, paths);
        // all elements and all their values
        let fitted = self.axis_ranges(store, paths);
        let plot = self
            .plot(ui, index_x_axis("index"), y_label, fitted)
            .x_grid_spacer(index_spacer);
        let visuals = ui.visuals().clone();
        let arrays: Vec<(&str, &str, Vec<Number>)> = (paths.iter().zip(&names))
            .filter_map(|(&path, &name)| {
                Some((
                    path,
                    name,
                    sample_at_cursor(store.trace(path, Window::of(time.timestamps())).data?, time),
                ))
            })
            .collect();
        let longest = arrays.iter().map(|(_, _, numbers)| numbers.len()).max();
        let response = plot.show(ui, |plot_ui| {
            for (index, (path, name, numbers)) in arrays.iter().enumerate() {
                let values: Vec<f64> = numbers.iter().map(|number| number.as_f64()).collect();
                let color = theme::trace_color(&visuals, index);
                let style = (self.line_styles.of(path), self.line_width);
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
        if let Some(corner) = corner {
            let entries: Vec<(&str, egui::Color32)> = (arrays.iter().enumerate())
                .map(|(index, (_, name, _))| (*name, theme::trace_color(ui.visuals(), index)))
                .collect();
            corner_legend(ui, *response.transform.frame(), corner, entries);
        }
        if let Some(index) = response.inner {
            let tooltip = element_tooltip(store, index, &arrays);
            (response.response).on_hover_ui_at_pointer(|ui| tooltip.ui(ui));
        }
    }
}

/// Adds the row Label to the grid of a properties section, for the `label` of an axis,
/// empty for the `automatic` one, shown as its hint.
fn label_row(ui: &mut egui::Ui, label: &mut String, automatic: String) {
    super::property_label(ui, "Label");
    ui.add(theme::wide_field(label).hint_text(automatic))
        .on_hover_text("The names and the unit of the traces on the axis if this is empty");
    ui.end_row();
}

/// Makes `plot` include `ranges`, those of its x and y axis.
fn include_ranges(
    plot: egui_plot::Plot<'_>,
    [x, y]: [Option<(f64, f64)>; 2],
) -> egui_plot::Plot<'_> {
    let mut plot = plot;
    if let Some((low, high)) = x {
        plot = plot.include_x(low).include_x(high);
    }
    if let Some((low, high)) = y {
        plot = plot.include_y(low).include_y(high);
    }
    plot
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
    fn title_ui(&self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.legend != LegendPlacement::Title {
            return;
        }
        let entries = self.legend_entries(context.store);
        let entries: Vec<TitleEntry<'_>> = (entries.iter())
            .map(|(path, name, index)| TitleEntry {
                path,
                name,
                color: theme::trace_color(ui.visuals(), *index),
                value: None,
                slot: 0.0,
            })
            .collect();
        // the units are on the axes
        title_legend(ui, &entries, None);
    }

    fn colored_paths(&self, store: &dyn Store) -> Vec<String> {
        let arrays = self.array_paths(store);
        if arrays.is_empty() {
            // the y traces as the view plots them, also before it assigns the axes of
            // the traces checked since it was last shown
            self.assigned(store).against.into_keys().collect()
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
        self.assign_axes(context.store);
        let corner = self.corner(context.titled);
        let arrays: Vec<String> = (self.array_paths(context.store).into_iter())
            .map(str::to_owned)
            .collect();
        if !arrays.is_empty() {
            let arrays: Vec<&str> = arrays.iter().map(String::as_str).collect();
            self.show_arrays(ui, context.store, context.time, (&arrays, corner));
        } else if self.traces.is_empty() {
            theme::hint(ui, super::CHECK_TRACES);
        } else if self.against.is_empty() {
            theme::hint(ui, "Check a second trace to plot against the first");
        } else {
            self.show_plot(ui, context.store, context.time, corner);
        }
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let arrays: Vec<String> = (self.array_paths(context.store).into_iter())
            .map(str::to_owned)
            .collect();
        self.assign_axes(context.store);
        let paths: Vec<String> = self.traces.iter().cloned().collect();
        let names = short_names(paths.iter().map(String::as_str));
        let name_by_path: BTreeMap<&str, &str> = (paths.iter().map(String::as_str))
            .zip(names.iter().copied())
            .collect();
        let name_of = |path: &str| name_by_path.get(path).copied().unwrap_or("").to_owned();
        let declared = (self.traces.iter()).any(|path| context.store.expected(path).is_some());
        super::properties_section(ui, "Axes", |ui| {
            super::fit_row(ui, &mut self.fit, declared);
            if arrays.is_empty() {
                let rows = self.axis_rows();
                // the choice of a row, as the axis, the row and the trace chosen
                let mut chosen = None;
                for (index, (x, y)) in rows.iter().enumerate() {
                    let label = match rows.len() {
                        1 => "Curve".to_owned(),
                        _ => format!("Curve {}", index + 1),
                    };
                    super::property_label(ui, &label);
                    ui.horizontal(|ui| {
                        let gap = ui.spacing().item_spacing.x;
                        let letters = super::text_size(ui, "XY", egui::TextStyle::Body).x;
                        let width = ((ui.available_width() - letters - 3.0 * gap) / 2.0)
                            .max(MIN_TRACE_CHOICE_WIDTH);
                        ui.weak("X");
                        let id = ("xy x", index);
                        if let Some(path) = trace_choice(ui, id, Some(x), &paths, width, &name_of) {
                            chosen = Some((true, index, path));
                        }
                        ui.weak("Y");
                        let id = ("xy y", index);
                        let y = y.as_ref();
                        if let Some(path) = trace_choice(ui, id, y, &paths, width, &name_of) {
                            chosen = Some((false, index, path));
                        }
                    });
                    ui.end_row();
                }
                if let Some((is_x, index, path)) = chosen {
                    let (x, y) = &rows[index];
                    let row = (x.as_str(), y.as_deref());
                    if is_x {
                        self.choose_x(row, &path);
                    } else {
                        self.choose_y(row, &path);
                    }
                }
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
        let array_paths: Vec<&str> = arrays.iter().map(String::as_str).collect();
        let [x_fitted, y_fitted] = self.axis_ranges(context.store, &array_paths);
        let [x_auto, y_auto] = self.automatic_labels(context.store, &array_paths);
        super::properties_section(ui, "X axis", |ui| {
            super::range_rows(ui, &mut self.x_range, x_fitted);
            label_row(ui, &mut self.x_label, x_auto);
        });
        super::properties_section(ui, "Y axis", |ui| {
            super::range_rows(ui, &mut self.y_range, y_fitted);
            label_row(ui, &mut self.y_label, y_auto);
        });
        super::properties_section(ui, "Lines", |ui| {
            self.points.choice_row(ui);
            super::property_label(ui, "Width");
            ui.add(
                egui::DragValue::new(&mut self.line_width)
                    .range(0.5..=6.0)
                    .speed(0.1),
            );
            ui.end_row();
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
        super::properties_section(ui, "Legend", |ui| {
            super::property_label(ui, "Placement");
            super::choice_combo(
                ui,
                "xy_legend",
                None,
                &mut self.legend,
                LegendPlacement::ALL
                    .into_iter()
                    .map(|placement| (placement, placement.name())),
                None,
            );
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
