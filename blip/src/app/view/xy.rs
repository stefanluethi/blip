//! An XY plot: traces plotted against another trace instead of against time, e.g. a
//! position as a path.

use super::time_axis::{index_at, within};
use super::{PropertiesContext, ViewContext, Viewable, short_names};
use crate::app::model::Model;
use crate::app::model::traces::Series;
use crate::app::time::TimeRange;
use std::collections::BTreeSet;
use std::ops::Range;

/// Upper bound of points drawn per trace.
const MAX_POINTS: usize = 20_000;
/// Number of steps in which the trail fades out towards older points.
const FADE_STEPS: usize = 8;
/// Opacity of the oldest part of the trail.
const MIN_OPACITY: f32 = 0.15;

/// The shown traces plotted against one of them, the x trace, within the shown
/// window.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Xy {
    /// Path of the trace on the x axis, see [`Xy::x_path`].
    pub x: Option<String>,
    /// Paths of the shown traces, the x trace and the traces on the y axis.
    #[serde(alias = "streams")] // the name before layout version 3
    pub traces: BTreeSet<String>,
    /// Whether a unit has the same length on both axes, e.g. for positions.
    pub equal_axes: bool,
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

fn is_point(point: &[f64; 2]) -> bool {
    !point[0].is_nan() && !point[1].is_nan()
}

impl Xy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Before layout version 2, the x trace was not one of the shown traces.
    pub fn migrate_v1(&mut self) {
        if let Some(x) = &self.x {
            self.traces.insert(x.clone());
        }
    }

    /// Path of the trace on the x axis: the chosen one if it is shown, else the first
    /// shown trace.
    pub fn x_path(&self) -> Option<&str> {
        self.x
            .as_deref()
            .filter(|x| self.traces.contains(*x))
            .or_else(|| self.traces.first().map(String::as_str))
    }

    /// Paths of the traces on the y axis: the shown traces but the x trace.
    fn y_paths(&self) -> impl Iterator<Item = &str> {
        let x = self.x_path();
        self.traces
            .iter()
            .map(String::as_str)
            .filter(move |&path| Some(path) != x)
    }

    /// Draws `points` as a line fading out towards the first points, and the last point
    /// as a dot.
    fn plot_trail(
        name: &str,
        points: &[[f64; 2]],
        color: egui::Color32,
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
        let step_len = points.len().div_ceil(FADE_STEPS).max(1);
        for (step, start) in (0..points.len()).step_by(step_len).enumerate() {
            // overlap by one point so the steps join
            let end = (start + step_len + 1).min(points.len());
            let opacity = MIN_OPACITY
                + (1.0 - MIN_OPACITY) * (step + 1) as f32 / points.len().div_ceil(step_len) as f32;
            for segment in points[start..end].split(|point| !is_point(point)) {
                if segment.len() > 1 {
                    plot_ui.line(
                        egui_plot::Line::new(name, segment.to_vec())
                            .color(color.gamma_multiply(opacity)),
                    );
                }
            }
        }
    }

    fn show_plot(&self, ui: &mut egui::Ui, model: &Model, time: &TimeRange) {
        let x_path = self.x_path();
        let y_paths: Vec<&str> = self.y_paths().collect();
        let names = short_names(y_paths.iter().copied());
        // an axis is labelled with the name of its trace and the unit of its traces
        let label = |name: &str, unit: Option<&str>| match unit {
            Some(unit) if name.is_empty() => format!("[{unit}]"),
            Some(unit) => format!("{name} [{unit}]"),
            None => name.to_owned(),
        };
        let x_unit = x_path.and_then(|x| model.traces.unit(x));
        let x_label = label(&short_names(x_path).concat(), x_unit);
        let y_unit = super::common_unit(model, y_paths.iter().copied());
        let y_label = match names[..] {
            [name] => label(name, y_unit),
            _ => label("", y_unit),
        };
        let mut plot = egui_plot::Plot::new(ui.id().with("xy"))
            .legend(
                // the newest point is drawn first in full color, the trail fades out
                egui_plot::Legend::default()
                    .color_conflict_handling(egui_plot::ColorConflictHandling::PickFirst),
            )
            .x_axis_label(x_label)
            .y_axis_label(y_label)
            .label_formatter(|hover| {
                let (name, point) = match hover {
                    egui_plot::HoverPosition::NearDataPoint {
                        plot_name,
                        position,
                        ..
                    } => (*plot_name, position),
                    egui_plot::HoverPosition::Elsewhere { position } => ("", position),
                };
                Some(format!("{name}\nx = {:.4}\ny = {:.4}", point.x, point.y))
            });
        if self.equal_axes {
            plot = plot.data_aspect(1.0);
        }

        let x = x_path.and_then(|path| model.traces.series(path));
        let range = time.range();
        let cursor = time.cursor();
        let ring_color = ui.visuals().strong_text_color();
        let response = plot.show(ui, |plot_ui| {
            let (Some(x), Some(range)) = (x, range) else {
                return;
            };
            for (index, (path, name)) in y_paths.iter().zip(&names).enumerate() {
                let Some(y) = model.traces.series(path) else {
                    continue;
                };
                let color = super::plot::curve_color(index);
                let points = thin_out(pair(x, y, within(y.timestamps, range)), MAX_POINTS);
                Self::plot_trail(name, &points, color, plot_ui);

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
}

impl Viewable for Xy {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.traces.len() < 2 {
            ui.weak("Select an x and a y trace in the traces panel");
            return;
        }
        self.show_plot(ui, context.model, context.time);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _context: &PropertiesContext<'_>) {
        let x_path = self.x_path().map(str::to_owned);
        let paths: Vec<String> = self.traces.iter().cloned().collect();
        let names = short_names(paths.iter().map(String::as_str));
        super::properties_section(ui, "Axes", |ui| {
            for (path, name) in paths.iter().zip(names) {
                ui.label(name).on_hover_text(path);
                let is_x = x_path.as_ref() == Some(path);
                ui.horizontal(|ui| {
                    if ui.selectable_label(is_x, "X").clicked() {
                        self.x = Some(path.clone());
                    }
                    if ui.selectable_label(!is_x, "Y").clicked() && is_x {
                        // another trace becomes the x trace
                        self.x = paths.iter().find(|other| *other != path).cloned();
                    }
                });
                ui.end_row();
            }
            ui.label("Equal axes");
            ui.checkbox(&mut self.equal_axes, "")
                .on_hover_text("Give a unit the same length on both axes");
            ui.end_row();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::traces::Traces;
    use proto::Data;

    /// Traces of the given samples at the given timestamps.
    fn traces(samples: &[(u64, &str, Data)]) -> Traces {
        let mut traces = Traces::new();
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

    #[test]
    fn the_x_trace_is_the_chosen_or_else_the_first_shown_trace() {
        let mut xy = Xy {
            traces: ["a", "b", "c"].map(str::to_owned).into(),
            ..Xy::default()
        };
        assert_eq!(xy.x_path(), Some("a"));
        assert_eq!(xy.y_paths().collect::<Vec<_>>(), ["b", "c"]);
        xy.x = Some("b".to_owned());
        assert_eq!(xy.x_path(), Some("b"));
        assert_eq!(xy.y_paths().collect::<Vec<_>>(), ["a", "c"]);
        xy.traces.remove("b");
        assert_eq!(xy.x_path(), Some("a"));
    }

    #[test]
    fn thinning_out_keeps_the_last_point() {
        let points: Vec<[f64; 2]> = (0..10).map(|i| [i as f64, 0.0]).collect();
        let thinned = thin_out(points, 4);
        assert!(thinned.len() <= 4);
        assert_eq!(thinned.last(), Some(&[9.0, 0.0]));
    }
}
