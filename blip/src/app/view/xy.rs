//! An XY plot: streams plotted against another stream instead of against time, e.g. a
//! position as a path.

use super::stream_selection::stream_paths;
use super::time_axis::{index_at, within};
use super::{ViewContext, ViewType, Viewable};
use crate::app::model::Model;
use crate::app::model::streams::Series;
use crate::app::time::TimeRange;
use std::collections::BTreeSet;
use std::ops::Range;

/// Upper bound of points drawn per stream.
const MAX_POINTS: usize = 20_000;
/// Number of steps in which the trail fades out towards older points.
const FADE_STEPS: usize = 8;
/// Opacity of the oldest part of the trail.
const MIN_OPACITY: f32 = 0.15;

/// Streams plotted against the stream `x` within the shown window.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Xy {
    /// Path of the stream on the x axis.
    pub x: Option<String>,
    /// Paths of the streams on the y axis.
    pub streams: BTreeSet<String>,
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

    /// Shows the choice of the x stream and the options.
    fn options_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        ui.horizontal(|ui| {
            ui.label("X:");
            let selected = self.x.as_deref().unwrap_or("Select a stream");
            egui::ComboBox::from_id_salt("xy_x")
                .selected_text(selected)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for stream in model.streams.iter() {
                        for path in stream_paths(stream, ViewType::Xy) {
                            let checked = self.x.as_ref() == Some(&path);
                            if ui.selectable_label(checked, &path).clicked() {
                                self.x = Some(path);
                            }
                        }
                    }
                });
            ui.checkbox(&mut self.equal_axes, "Equal axes")
                .on_hover_text("Give a unit the same length on both axes");
        });
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
        let x_label = self.x.clone().unwrap_or_default();
        let y_label = match self.streams.len() {
            1 => self.streams.first().cloned().unwrap_or_default(),
            _ => String::new(),
        };
        let mut plot = egui_plot::Plot::new(ui.id().with("xy"))
            .legend(egui_plot::Legend::default())
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

        let x = self
            .x
            .as_deref()
            .and_then(|path| model.streams.series(path));
        let range = time.range();
        let hovered = time.hovered();
        let ring_color = ui.visuals().strong_text_color();
        plot.show(ui, |plot_ui| {
            let (Some(x), Some(range)) = (x, range) else {
                return;
            };
            for (index, path) in self.streams.iter().enumerate() {
                let Some(y) = model.streams.series(path) else {
                    continue;
                };
                let color = super::plot::curve_color(index);
                let points = thin_out(pair(x, y, within(y.timestamps, range)), MAX_POINTS);
                Self::plot_trail(path, &points, color, plot_ui);

                // where the stream was at the time hovered in another view
                let at_hovered = hovered
                    .and_then(|t| index_at(y.timestamps, t))
                    .map(|index| pair(x, y, index..index + 1)[0])
                    .filter(is_point);
                if let Some(point) = at_hovered {
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
    }
}

impl Viewable for Xy {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.options_ui(ui, context.model);
        self.show_plot(ui, context.model, context.time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::streams::Streams;
    use proto::Data;

    /// Streams of the given samples at the given timestamps.
    fn streams(samples: &[(u64, &str, Data)]) -> Streams {
        let mut streams = Streams::new();
        for (timestamp, path, value) in samples {
            streams.append(&proto::Value {
                timestamp: *timestamp,
                path: path.to_string(),
                value: value.clone(),
            });
        }
        streams
    }

    #[test]
    fn samples_are_paired_with_the_current_x() {
        let streams = streams(&[
            (1, "y", Data::Float(10.0)),
            (2, "x", Data::Float(1.0)),
            (2, "y", Data::Float(20.0)),
            (3, "y", Data::Float(30.0)),
            (4, "x", Data::Float(2.0)),
            (5, "y", Data::Null),
        ]);
        let (x, y) = (streams.series("x").unwrap(), streams.series("y").unwrap());
        let points = pair(x, y, 0..y.len());
        assert!(points[0][0].is_nan());
        assert_eq!(points[1], [1.0, 20.0]);
        assert_eq!(points[2], [1.0, 30.0]);
        assert_eq!(points[3][0], 2.0);
        assert!(!is_point(&points[3]));
        assert_eq!(pair(x, y, 2..3), [[1.0, 30.0]]);
    }

    #[test]
    fn thinning_out_keeps_the_last_point() {
        let points: Vec<[f64; 2]> = (0..10).map(|i| [i as f64, 0.0]).collect();
        let thinned = thin_out(points, 4);
        assert!(thinned.len() <= 4);
        assert_eq!(thinned.last(), Some(&[9.0, 0.0]));
    }
}
