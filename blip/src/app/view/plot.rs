use super::time_axis::{self, closest_index, visible_range, within};
use super::{ViewContext, Viewable, short_names};
use crate::app::model::{
    Model,
    streams::{NumberKind, Series},
};
use crate::app::time::{TimeBase, TimeRange, timestamp_to_seconds};
use std::cmp::min;
use std::collections::BTreeSet;

const Y_MARGIN_FRACTION: f64 = 0.05;
/// Upper bound of points drawn per curve within the visible x range.
const MAX_POINTS_PER_CURVE: usize = 100_000;

/// A time-series plot of the selected streams.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Plot {
    /// Paths of the shown streams.
    pub streams: BTreeSet<String>,
}

impl Plot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws the values of `series` within `range_x` seconds, shifted by `origin`
    /// seconds, with gaps at nulls.
    fn plot_series(
        name: &str,
        series: Series<'_>,
        color: egui::Color32,
        range_x: Option<(f64, f64)>,
        origin: f64,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let visible = series.slice(visible_range(series.timestamps, range_x));
        let points = downsample(visible, origin, MAX_POINTS_PER_CURVE);
        // lines of the same name share one legend entry
        for segment in points
            .split(|point| point[1].is_nan())
            .filter(|segment| !segment.is_empty())
        {
            let segment = if series.kind == NumberKind::Bool {
                steps(segment)
            } else {
                segment.to_vec()
            };
            plot_ui.line(
                egui_plot::Line::new(name, egui_plot::PlotPoints::from(segment)).color(color),
            );
        }
    }

    /// The y range covered by the shown streams within the sample times `[start, end]`
    /// seconds.
    fn range_y(&self, model: &Model, range_x: (f64, f64)) -> Option<(f64, f64)> {
        let mut range: Option<(f64, f64)> = None;
        for (_, series, _) in self.selected_series_iter(model) {
            let visible = within(series.timestamps, range_x);
            for y in visible
                .map(|index| series.value(index))
                .filter(|y| !y.is_nan())
            {
                range = Some(match range {
                    Some((low, high)) => (low.min(y), high.max(y)),
                    None => (y, y),
                });
            }
        }

        range.map(|(low, high)| {
            let margin = if high > low {
                (high - low) * Y_MARGIN_FRACTION
            } else {
                0.5
            };
            (low - margin, high + margin)
        })
    }

    /// The shown paths with their names in the legend and their curve colors.
    fn selected_path_iter(&self) -> impl Iterator<Item = (&str, &str, egui::Color32)> {
        let names = short_names(self.streams.iter().map(String::as_str));
        // colors follow the order of the selection, so they do not change as streams arrive
        self.streams
            .iter()
            .zip(names)
            .enumerate()
            .map(|(index, (path, name))| (path.as_str(), name, curve_color(index)))
    }

    /// The shown numeric series that were received, with their names in the legend and
    /// their curve colors.
    fn selected_series_iter<'a>(
        &'a self,
        model: &'a Model,
    ) -> impl Iterator<Item = (&'a str, Series<'a>, egui::Color32)> {
        self.selected_path_iter()
            .filter_map(|(path, name, color)| Some((name, model.streams.series(path)?, color)))
    }

    /// Shows the values of the shown streams at `t` seconds.
    fn tooltip_ui(&self, ui: &mut egui::Ui, model: &Model, t: f64, base: TimeBase) {
        ui.label(base.format(t, 3));
        egui::Grid::new("plot_tooltip")
            .num_columns(2)
            .show(ui, |ui| {
                for (path, name, color) in self.selected_path_iter() {
                    let value = model.streams.series(path).and_then(|series| {
                        closest_index(series.timestamps, t)
                            .map(|index| series.kind.format(series.value(index)))
                    });
                    if let Some(value) = value {
                        ui.colored_label(color, name);
                        ui.label(value);
                        ui.end_row();
                    }
                }
            });
    }

    /// Shows the shared time range of `time`, and applies panning and zooming of the
    /// plot to it.
    fn show_plot(&self, model: &Model, time: &mut TimeRange, ui: &mut egui::Ui) {
        let plot = time_axis::plot(ui.id().with("plot"), time)
            .legend(egui_plot::Legend::default())
            .custom_y_axes(vec![
                egui_plot::AxisHints::new_y()
                    .placement(egui_plot::HPlacement::Left)
                    .min_thickness(50.0),
            ]);

        let response = time_axis::show(plot, ui, time, |plot_ui, axis| {
            if let Some(range) = axis.range
                && let Some((min_y, max_y)) = self.range_y(model, range)
            {
                plot_ui.set_plot_bounds_y(min_y..=max_y);
            }
            for (name, series, color) in self.selected_series_iter(model) {
                Self::plot_series(name, series, color, axis.range, axis.origin, plot_ui);
            }
        });

        if let Some(t) = response.hovered
            && !self.streams.is_empty()
        {
            let base = response.axis.base;
            response
                .response
                .on_hover_ui_at_pointer(|ui| self.tooltip_ui(ui, model, t, base));
        }
    }
}

/// Color of the `index`th curve, picked like egui_plot picks its colors.
pub fn curve_color(index: usize) -> egui::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
    egui::ecolor::Hsva::new(index as f32 * golden_ratio, 0.85, 0.5, 1.0).into()
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
        self.show_plot(context.model, context.time, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::streams::Streams;

    /// A float stream `a` of `count` values 1 ms apart, with a null every 7th value if
    /// `with_nulls`.
    fn streams(count: u64, with_nulls: bool) -> Streams {
        let mut streams = Streams::new();
        for i in 0..count {
            let value = if with_nulls && i % 7 == 3 {
                proto::Data::Null
            } else {
                proto::Data::Float((i % 7) as f64)
            };
            streams.append(&proto::Value {
                timestamp: i * 1_000_000,
                path: "a".to_owned(),
                value,
            });
        }
        streams
    }

    #[test]
    fn downsample_keeps_small_series() {
        let streams = streams(10, false);
        assert_eq!(downsample(streams.series("a").unwrap(), 0.0, 100).len(), 10);
    }

    #[test]
    fn downsample_limits_points_and_keeps_extremes() {
        let streams = streams(1_000_003, false);
        let points = downsample(streams.series("a").unwrap(), 0.0, 1_000);
        assert!(points.len() <= 1_000);
        assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(points.iter().any(|p| p[1] == 0.0));
        assert!(points.iter().any(|p| p[1] == 6.0));
    }

    #[test]
    fn downsample_keeps_gaps() {
        let streams = streams(1_000_003, true);
        let points = downsample(streams.series("a").unwrap(), 0.0, 1_000);
        assert!(points.len() <= 1_000);
        assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(points.iter().any(|p| p[1].is_nan()));
        assert!(points.iter().any(|p| p[1] == 6.0));
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
