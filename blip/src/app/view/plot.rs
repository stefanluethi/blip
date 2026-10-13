use super::{ViewContext, Viewable};
use crate::app::model::{Model, streams::{Stream, Value}};
use crate::app::time::{TimeRange, timestamp_to_seconds};
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

    /// Draws the values of `stream` within `range_x` seconds, shifted by `origin` seconds.
    fn plot_stream_series(
        stream: &Stream,
        color: egui::Color32,
        range_x: Option<(f64, f64)>,
        origin: f64,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let values = visible_values(&stream.values, range_x);
        let points = downsample(values, origin, MAX_POINTS_PER_CURVE);

        plot_ui.line(
            egui_plot::Line::new(stream.path.as_str(), egui_plot::PlotPoints::from(points)).color(color),
        );
    }

    /// The y range covered by the shown streams within the sample times `[start, end]`
    /// seconds.
    fn range_y(&self, model: &Model, (start, end): (f64, f64)) -> Option<(f64, f64)> {
        let mut range: Option<(f64, f64)> = None;
        for (stream, _) in self.selected_stream_iter(model) {
            // values are appended in arrival order, so timestamps are ascending
            let first = stream
                .values
                .partition_point(|v| timestamp_to_seconds(v.timestamp) < start);
            let last = stream
                .values
                .partition_point(|v| timestamp_to_seconds(v.timestamp) <= end);
            for v in &stream.values[first..last] {
                let y = v.value as f64;
                range = Some(match range {
                    Some((low, high)) => (low.min(y), high.max(y)),
                    None => (y, y),
                });
            }
        }

        range.map(|(low, high)| {
            let margin = if high > low { (high - low) * Y_MARGIN_FRACTION } else { 0.5 };
            (low - margin, high + margin)
        })
    }

    /// The shown streams that were received, with their curve colors.
    fn selected_stream_iter<'a>(&'a self, model: &'a Model) -> impl Iterator<Item = (&'a Stream, egui::Color32)> {
        // colors follow the order of the selection, so they do not change as streams arrive
        self.streams
            .iter()
            .enumerate()
            .filter_map(|(index, path)| Some((model.streams.get(path)?, curve_color(index))))
    }

    /// Shows the values of the shown streams at `t` seconds.
    fn tooltip_ui(&self, ui: &mut egui::Ui, model: &Model, t: f64, origin: f64) {
        ui.label(format!("t = {:.3} s", t - origin));
        egui::Grid::new("plot_tooltip").num_columns(2).show(ui, |ui| {
            for (stream, color) in self.selected_stream_iter(model) {
                if let Some(value) = value_at(&stream.values, t) {
                    ui.colored_label(color, &stream.path);
                    ui.label(value.value.to_string());
                    ui.end_row();
                }
            }
        });
    }

    /// Shows the shared time range of `time`, and applies panning and zooming of the
    /// plot to it.
    fn show_plot(&self, model: &Model, time: &mut TimeRange, ui: &mut egui::Ui) {
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
            .x_axis_label("t / s")
            .custom_y_axes(vec![
                egui_plot::AxisHints::new_y()
                    .placement(egui_plot::HPlacement::Left)
                    .min_thickness(50.0),
            ]);

        // x is the time since the first collected data
        let origin = time.origin();
        let range = time.range();
        let cursor_color = ui.visuals().weak_text_color();
        let response = plot.show(ui, |plot_ui| {
            if let Some((start, end)) = range {
                plot_ui.set_plot_bounds_x(start - origin..=end - origin);
                if let Some((min_y, max_y)) = self.range_y(model, (start, end)) {
                    plot_ui.set_plot_bounds_y(min_y..=max_y);
                }
            }

            for (stream, color) in self.selected_stream_iter(model) {
                Self::plot_stream_series(stream, color, range, origin, plot_ui);
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
                .on_hover_ui_at_pointer(|ui| self.tooltip_ui(ui, model, x + origin, origin));
        }
    }
}

/// Color of the `index`th curve, picked like egui_plot picks its colors.
fn curve_color(index: usize) -> egui::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
    egui::ecolor::Hsva::new(index as f32 * golden_ratio, 0.85, 0.5, 1.0).into()
}

/// The value with the timestamp closest to `t` seconds.
fn value_at(values: &[Value], t: f64) -> Option<&Value> {
    // values are appended in arrival order, so timestamps are ascending
    let next = values.partition_point(|v| timestamp_to_seconds(v.timestamp) < t);
    let candidates = [next.checked_sub(1), Some(next)];
    candidates
        .into_iter()
        .flatten()
        .filter_map(|index| values.get(index))
        .min_by(|a, b| {
            let distance = |v: &Value| (timestamp_to_seconds(v.timestamp) - t).abs();
            distance(a).total_cmp(&distance(b))
        })
}

/// The values within `range_x` seconds, plus one neighbour on each side so the line
/// continues to the plot edges. All values if `range_x` is `None`.
fn visible_values(values: &[Value], range_x: Option<(f64, f64)>) -> &[Value] {
    let Some((start, end)) = range_x else {
        return values;
    };
    // values are appended in arrival order, so timestamps are ascending
    let first = values.partition_point(|v| timestamp_to_seconds(v.timestamp) < start);
    let last = values.partition_point(|v| timestamp_to_seconds(v.timestamp) <= end);
    &values[first.saturating_sub(1)..min(last + 1, values.len())]
}

/// Reduces `values` to at most `max_points` points by keeping the minimum and maximum
/// of equally sized buckets, so peaks stay visible. The x coordinates are the sample
/// times in seconds minus `origin`.
fn downsample(values: &[Value], origin: f64, max_points: usize) -> Vec<[f64; 2]> {
    let to_point = |v: &Value| [timestamp_to_seconds(v.timestamp) - origin, v.value as f64];
    if values.len() <= max_points {
        return values.iter().map(to_point).collect();
    }

    let bucket_size = values.len().div_ceil((max_points / 2).max(1));
    let mut points = Vec::with_capacity(max_points);
    for bucket in values.chunks(bucket_size) {
        let mut min_index = 0;
        let mut max_index = 0;
        for (i, v) in bucket.iter().enumerate() {
            if v.value < bucket[min_index].value {
                min_index = i;
            }
            if v.value > bucket[max_index].value {
                max_index = i;
            }
        }
        // keep the two extremes in time order
        points.push(to_point(&bucket[min_index.min(max_index)]));
        if min_index != max_index {
            points.push(to_point(&bucket[min_index.max(max_index)]));
        }
    }
    points
}

impl Viewable for Plot {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.show_plot(context.model, context.time, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(count: u64) -> Vec<Value> {
        (0..count)
            .map(|i| Value {
                timestamp: i * 1_000_000,
                value: (i % 7) as f32,
            })
            .collect()
    }

    #[test]
    fn downsample_keeps_small_series() {
        let values = values(10);
        assert_eq!(downsample(&values, 0.0, 100).len(), 10);
    }

    #[test]
    fn downsample_limits_points_and_keeps_extremes() {
        let values = values(1_000_003);
        let points = downsample(&values, 0.0, 1_000);
        assert!(points.len() <= 1_000);
        assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
        assert!(points.iter().any(|p| p[1] == 0.0));
        assert!(points.iter().any(|p| p[1] == 6.0));
    }

    #[test]
    fn value_at_picks_the_closest_value() {
        let values = values(10); // 1 ms apart
        assert_eq!(value_at(&values, 0.0024).unwrap().timestamp, 2_000_000);
        assert_eq!(value_at(&values, 0.0026).unwrap().timestamp, 3_000_000);
        assert_eq!(value_at(&values, 1.0).unwrap().timestamp, 9_000_000);
        assert!(value_at(&[], 1.0).is_none());
    }

    #[test]
    fn visible_values_includes_neighbours() {
        let values = values(100); // 1 ms apart
        let visible = visible_values(&values, Some((0.0105, 0.0205)));
        assert_eq!(visible.first().unwrap().timestamp, 10_000_000);
        assert_eq!(visible.last().unwrap().timestamp, 21_000_000);
        assert_eq!(visible_values(&values, None).len(), 100);
        assert!(visible_values(&values, Some((1.0, 2.0))).len() <= 1);
    }
}
