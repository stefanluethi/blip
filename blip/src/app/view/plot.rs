use super::{ViewContext, Viewable};
use crate::app::model::{
    Model,
    streams::{NumberKind, Samples, Series},
};
use crate::app::time::{TimeBase, TimeRange, timestamp_to_seconds};
use std::cmp::min;
use std::collections::BTreeSet;
use std::ops::Range;

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

    /// The shown paths with their curve colors.
    fn selected_path_iter(&self) -> impl Iterator<Item = (&str, egui::Color32)> {
        // colors follow the order of the selection, so they do not change as streams arrive
        self.streams
            .iter()
            .enumerate()
            .map(|(index, path)| (path.as_str(), curve_color(index)))
    }

    /// The shown numeric series that were received, with their curve colors.
    fn selected_series_iter<'a>(
        &'a self,
        model: &'a Model,
    ) -> impl Iterator<Item = (&'a str, Series<'a>, egui::Color32)> {
        self.selected_path_iter()
            .filter_map(|(path, color)| Some((path, model.streams.series(path)?, color)))
    }

    /// Shows the values of the shown streams at `t` seconds.
    fn tooltip_ui(&self, ui: &mut egui::Ui, model: &Model, t: f64, base: TimeBase) {
        ui.label(base.format(t, 3));
        egui::Grid::new("plot_tooltip")
            .num_columns(2)
            .show(ui, |ui| {
                for (path, color) in self.selected_path_iter() {
                    let value = if let Some(series) = model.streams.series(path) {
                        closest_index(series.timestamps, t)
                            .map(|index| series.kind.format(series.value(index)))
                    } else if let Some(stream) = model.streams.get(path)
                        && let Samples::Strings(values) = &stream.samples
                    {
                        // strings are not plotted, but shown here
                        closest_index(&stream.timestamps, t)
                            .map(|index| values[index].clone().unwrap_or_else(|| "null".to_owned()))
                    } else {
                        None
                    };
                    if let Some(value) = value {
                        ui.colored_label(color, path);
                        ui.label(value);
                        ui.end_row();
                    }
                }
            });
    }

    /// Shows the shared time range of `time`, and applies panning and zooming of the
    /// plot to it.
    fn show_plot(&self, model: &Model, time: &mut TimeRange, ui: &mut egui::Ui) {
        // x is the time since the first collected data, labelled with the timestamps
        let origin = time.origin();
        let base = time.base();
        let plot = egui_plot::Plot::new(ui.id().with("plot"))
            .legend(egui_plot::Legend::default())
            // the values at the pointer are shown in a tooltip
            .show_x(false)
            .show_y(false)
            .allow_drag([true, false])
            .allow_zoom([true, false])
            .allow_scroll([true, false])
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .x_axis_label(base.axis_label())
            .x_axis_formatter(move |mark, _range| {
                let decimals = (-mark.step_size.log10().floor()).max(0.0) as usize;
                base.format(mark.value + origin, decimals)
            })
            .custom_y_axes(vec![
                egui_plot::AxisHints::new_y()
                    .placement(egui_plot::HPlacement::Left)
                    .min_thickness(50.0),
            ]);

        let range = time.range();
        let cursor_color = ui.visuals().weak_text_color();
        let response = plot.show(ui, |plot_ui| {
            if let Some((start, end)) = range {
                plot_ui.set_plot_bounds_x(start - origin..=end - origin);
                if let Some((min_y, max_y)) = self.range_y(model, (start, end)) {
                    plot_ui.set_plot_bounds_y(min_y..=max_y);
                }
            }

            for (path, series, color) in self.selected_series_iter(model) {
                Self::plot_series(path, series, color, range, origin, plot_ui);
            }

            let hovered_x = plot_ui
                .response()
                .hovered()
                .then(|| plot_ui.pointer_coordinate())
                .flatten()
                .map(|point| point.x);
            if let Some(x) = hovered_x {
                plot_ui.vline(egui_plot::VLine::new("", x).color(cursor_color).width(1.0));
            }
            hovered_x
        });

        if let Some((start, end)) = range {
            let bounds = response.transform.bounds();
            let tolerance = (end - start) * 1.0e-6;
            if (bounds.min()[0] - (start - origin)).abs() > tolerance
                || (bounds.max()[0] - (end - origin)).abs() > tolerance
            {
                time.pan_zoom(bounds.min()[0] + origin, bounds.max()[0] + origin);
                ui.ctx().request_repaint();
            }
        }

        if let Some(x) = response.inner
            && !self.streams.is_empty()
        {
            response
                .response
                .on_hover_ui_at_pointer(|ui| self.tooltip_ui(ui, model, x + origin, base));
        }
    }
}

/// Color of the `index`th curve, picked like egui_plot picks its colors.
fn curve_color(index: usize) -> egui::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
    egui::ecolor::Hsva::new(index as f32 * golden_ratio, 0.85, 0.5, 1.0).into()
}

/// Index of the timestamp closest to `t` seconds.
fn closest_index(timestamps: &[u64], t: f64) -> Option<usize> {
    // timestamps are ascending
    let next = timestamps.partition_point(|&timestamp| timestamp_to_seconds(timestamp) < t);
    let candidates = [next.checked_sub(1), Some(next)];
    candidates
        .into_iter()
        .flatten()
        .filter(|&index| index < timestamps.len())
        .min_by(|&a, &b| {
            let distance = |index: usize| (timestamp_to_seconds(timestamps[index]) - t).abs();
            distance(a).total_cmp(&distance(b))
        })
}

/// Indices of the timestamps within `[start, end]` seconds.
fn within(timestamps: &[u64], (start, end): (f64, f64)) -> Range<usize> {
    // timestamps are ascending
    let first = timestamps.partition_point(|&timestamp| timestamp_to_seconds(timestamp) < start);
    let last = timestamps.partition_point(|&timestamp| timestamp_to_seconds(timestamp) <= end);
    first..last.max(first)
}

/// Indices of the timestamps within `range_x` seconds, plus one neighbour on each side
/// so the line continues to the plot edges. All indices if `range_x` is `None`.
fn visible_range(timestamps: &[u64], range_x: Option<(f64, f64)>) -> Range<usize> {
    let Some(range_x) = range_x else {
        return 0..timestamps.len();
    };
    let Range { start, end } = within(timestamps, range_x);
    start.saturating_sub(1)..min(end + 1, timestamps.len())
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

    #[test]
    fn closest_index_picks_the_closest_timestamp() {
        let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
        assert_eq!(closest_index(&timestamps, 0.0024), Some(2));
        assert_eq!(closest_index(&timestamps, 0.0026), Some(3));
        assert_eq!(closest_index(&timestamps, 1.0), Some(9));
        assert!(closest_index(&[], 1.0).is_none());
    }

    #[test]
    fn visible_range_includes_neighbours() {
        let timestamps: Vec<u64> = (0..100).map(|i| i * 1_000_000).collect(); // 1 ms apart
        assert_eq!(visible_range(&timestamps, Some((0.0105, 0.0205))), 10..22);
        assert_eq!(visible_range(&timestamps, None), 0..100);
        assert!(visible_range(&timestamps, Some((1.0, 2.0))).len() <= 1);
        assert!(within(&timestamps, (1.0, 2.0)).is_empty());
    }
}
