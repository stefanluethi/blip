use super::time_axis::{self, closest_index, visible_range};
use super::{Formats, PropertiesContext, ViewContext, Viewable, short_names, with_unit};
use crate::app::model::numbers::{NumberKind, SHOWN_DIGITS};
use crate::app::model::{Model, traces::Series};
use crate::app::theme;
use crate::app::time::{TimeBase, timestamp_to_seconds};
use std::cmp::min;
use std::collections::BTreeSet;

const Y_MARGIN_FRACTION: f64 = 0.05;
/// Upper bound of points drawn per curve within the visible x range.
const MAX_POINTS_PER_CURVE: usize = 100_000;

/// Points of a curve shown as markers are at least this many points apart on average.
const MIN_MARKER_SPACING: f32 = 8.0;

/// How a curve goes from one sample to the next.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Interpolation {
    /// A straight line, except for bools, which always hold their value.
    #[default]
    Linear,
    /// Each value held until the next sample.
    Step,
}

/// When the samples of a curve are marked.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ShowPoints {
    /// When they are far enough apart to tell them apart.
    #[default]
    Auto,
    Always,
    Never,
}

/// Where the legend is shown.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum LegendPlacement {
    Hidden,
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl LegendPlacement {
    const ALL: [LegendPlacement; 5] = [
        LegendPlacement::Hidden,
        LegendPlacement::TopLeft,
        LegendPlacement::TopRight,
        LegendPlacement::BottomLeft,
        LegendPlacement::BottomRight,
    ];

    fn name(self) -> &'static str {
        match self {
            LegendPlacement::Hidden => "Hidden",
            LegendPlacement::TopLeft => "Top left",
            LegendPlacement::TopRight => "Top right",
            LegendPlacement::BottomLeft => "Bottom left",
            LegendPlacement::BottomRight => "Bottom right",
        }
    }

    fn corner(self) -> Option<egui_plot::Corner> {
        match self {
            LegendPlacement::Hidden => None,
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
        }
    }
}

impl Plot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws the values of `series` within `range_x` seconds, shifted by `origin`
    /// seconds, as set in the properties of the plot.
    fn plot_series(
        &self,
        name: &str,
        series: Series<'_>,
        color: egui::Color32,
        range_x: Option<(f64, f64)>,
        origin: f64,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let visible = series.slice(visible_range(series.timestamps, range_x));
        let mut points = downsample(visible, origin, MAX_POINTS_PER_CURVE);
        if self.connect_nulls {
            points.retain(|point| !point[1].is_nan());
        }
        let show_points = match self.points {
            ShowPoints::Auto => {
                points.len() as f32 * MIN_MARKER_SPACING <= plot_ui.transform().frame().width()
            }
            ShowPoints::Always => true,
            ShowPoints::Never => false,
        };
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
            let segment = if step {
                steps(segment)
            } else {
                segment.to_vec()
            };
            plot_ui.line(
                egui_plot::Line::new(name, egui_plot::PlotPoints::from(segment))
                    .color(color)
                    .width(self.line_width),
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
        base: TimeBase,
    ) {
        ui.label(base.format(t, 3));
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
        if let Some(corner) = self.legend.corner() {
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
            for (name, series, index) in this.selected_series_iter(model) {
                let color = theme::trace_color(&visuals, index);
                this.plot_series(name, series, color, axis.range, axis.origin, plot_ui);
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
            let base = response.axis.base;
            response
                .response
                .on_hover_ui_at_pointer(|ui| self.tooltip_ui(ui, model, formats, t, base));
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

/// Reduces `series` to at most `max_points` points by keeping the minimum and maximum
/// of equally sized buckets, so peaks stay visible. The x coordinates are the sample
/// times in seconds minus `origin`. Nulls are kept as points with NaN y to mark gaps.
fn downsample(series: Series<'_>, origin: f64, max_points: usize) -> Vec<[f64; 2]> {
    let to_point = |index: usize| {
        [
            timestamp_to_seconds(series.timestamps[index]) - origin,
            series.value(index),
        ]
    };
    if series.len() <= max_points {
        return (0..series.len()).map(to_point).collect();
    }

    // a bucket adds up to three points: a gap and its two extremes
    let bucket_size = series.len().div_ceil((max_points / 3).max(1));
    let mut points = Vec::with_capacity(max_points);
    for bucket_start in (0..series.len()).step_by(bucket_size) {
        let mut gap = None;
        let mut extremes: Option<(usize, usize)> = None;
        for index in bucket_start..min(bucket_start + bucket_size, series.len()) {
            let value = series.value(index);
            if value.is_nan() {
                gap.get_or_insert(index);
                continue;
            }
            extremes = Some(match extremes {
                Some((min_index, max_index)) => (
                    if value < series.value(min_index) {
                        index
                    } else {
                        min_index
                    },
                    if value > series.value(max_index) {
                        index
                    } else {
                        max_index
                    },
                ),
                None => (index, index),
            });
        }
        let mut indices: Vec<usize> = gap.into_iter().collect();
        if let Some((min_index, max_index)) = extremes {
            indices.extend([min_index, max_index]);
        }
        // keep the points in time order
        indices.sort_unstable();
        indices.dedup();
        points.extend(indices.into_iter().map(to_point));
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

impl Viewable for Plot {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
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
                &[
                    (Interpolation::Linear, "Linear"),
                    (Interpolation::Step, "Step"),
                ],
            );
            super::property_label(ui, "Width");
            ui.add(
                egui::DragValue::new(&mut self.line_width)
                    .range(0.5..=6.0)
                    .speed(0.1),
            );
            ui.end_row();
            super::choice_row(
                ui,
                "Points",
                &mut self.points,
                &[
                    (ShowPoints::Auto, "Auto"),
                    (ShowPoints::Always, "Always"),
                    (ShowPoints::Never, "Never"),
                ],
            );
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::traces::Traces;

    /// A float trace `a` of `count` values 1 ms apart, with a null every 7th value if
    /// `with_nulls`.
    fn traces(count: u64, with_nulls: bool) -> Traces {
        let mut traces = Traces::new();
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
        assert_eq!(downsample(traces.series("a").unwrap(), 0.0, 100).len(), 10);
    }

    #[test]
    fn downsample_limits_points_and_keeps_extremes() {
        let traces = traces(1_000_003, false);
        let points = downsample(traces.series("a").unwrap(), 0.0, 1_000);
        assert!(points.len() <= 1_000);
        assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(points.iter().any(|p| p[1] == 0.0));
        assert!(points.iter().any(|p| p[1] == 6.0));
    }

    #[test]
    fn downsample_keeps_gaps() {
        let traces = traces(1_000_003, true);
        let points = downsample(traces.series("a").unwrap(), 0.0, 1_000);
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
        let mut plot = Plot::new();
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
    fn steps_hold_each_value() {
        let stepped = steps(&[[0.0, 1.0], [1.0, 0.0], [3.0, 1.0]]);
        assert_eq!(
            stepped,
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [3.0, 0.0], [3.0, 1.0]]
        );
    }
}
