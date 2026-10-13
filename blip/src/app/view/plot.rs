use super::{ViewContext, Viewable};
use crate::app::model::{Model, streams::{Stream, Value}};
use crate::app::time::{TimeRange, timestamp_to_seconds};
use std::cmp::min;
use std::collections::{BTreeMap, HashMap};
use egui::Ui;

const MAX_PLOT_COUNT: usize = 6;
const Y_MARGIN_FRACTION: f64 = 0.05;
/// Upper bound of points drawn per curve within the visible x range.
const MAX_POINTS_PER_CURVE: usize = 100_000;

/// Stream paths split into a tree at each "/".
#[derive(Default)]
struct StreamTree<'a> {
    /// Full stream path if a stream ends at this node.
    path: Option<&'a str>,
    children: BTreeMap<&'a str, StreamTree<'a>>,
}

impl<'a> StreamTree<'a> {
    fn build(paths: impl Iterator<Item = &'a str>) -> Self {
        let mut root = Self::default();
        for path in paths {
            let mut node = &mut root;
            for segment in path.split('/').filter(|segment| !segment.is_empty()) {
                node = node.children.entry(segment).or_default();
            }
            node.path = Some(path);
        }
        root
    }
}

fn show_stream_checkbox(ui: &mut Ui, label: &str, path: &str, selectors: &mut HashMap<String, bool>) {
    let selected = selectors.entry(path.to_string()).or_insert(false);
    ui.checkbox(selected, label).on_hover_text(path);
}

/// Shows the children of `node` as collapsible branches and stream checkboxes.
fn show_stream_tree(
    ui: &mut Ui,
    node: &StreamTree<'_>,
    prefix: &str,
    selectors: &mut HashMap<String, bool>,
) {
    for (segment, child) in &node.children {
        let child_prefix = format!("{prefix}/{segment}");
        if child.children.is_empty() {
            if let Some(path) = child.path {
                show_stream_checkbox(ui, segment, path, selectors);
            }
        } else {
            egui::CollapsingHeader::new(*segment)
                .id_salt(("stream_tree", &child_prefix))
                .default_open(true)
                .show(ui, |ui| {
                    // a stream can also be the prefix of other streams
                    if let Some(path) = child.path {
                        show_stream_checkbox(ui, segment, path, selectors);
                    }
                    show_stream_tree(ui, child, &child_prefix, selectors);
                });
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Plot {
    number_of_plots: usize,
    selected_plot: usize,
    selected_streams: [HashMap<String, bool>; MAX_PLOT_COUNT],
}

impl Default for Plot {
    fn default() -> Self {
        Self::new()
    }
}

impl Plot {
    pub fn new() -> Self {
        Self {
            number_of_plots: 1,
            selected_plot: 0,
            selected_streams: Self::init_selected_streams(),
        }
    }

    fn init_selected_streams() -> [HashMap<String, bool>; MAX_PLOT_COUNT] {
        let mut array: [HashMap<String, bool>; MAX_PLOT_COUNT] =
            Default::default();
        for item in &mut array {
            *item = HashMap::default();
        }
        array
    }

    fn show_stream_selection(&mut self, model: &Model, ui: &mut egui::Ui) {
        egui::Frame::default()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_height(ui.available_height());
                ui.vertical(|ui| {
                    ui.set_width(100.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.number_of_plots)
                                .speed(1.0)
                                .range(1..=MAX_PLOT_COUNT),
                        );
                        ui.label("plots");
                    });

                    ui.add_space(10.0);

                    ui.label("Streams:");
                    let active_selectors = &mut self.selected_streams[self.selected_plot];

                    ui.set_width(ui.available_width());
                    let tree =
                        StreamTree::build(model.streams.iter().map(|stream| stream.path.as_str()));
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        show_stream_tree(ui, &tree, "", active_selectors);
                    });
                });
            });
    }

    /// Draws the values of `stream` within `range_x` seconds, shifted by `origin` seconds.
    fn plot_stream_series(
        stream: &Stream,
        range_x: Option<(f64, f64)>,
        origin: f64,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let values = visible_values(&stream.values, range_x);
        let points = downsample(values, origin, MAX_POINTS_PER_CURVE);

        plot_ui.line(egui_plot::Line::new(
            format!("{} / {}", stream.path, "none"),
            egui_plot::PlotPoints::from(points),
        ));
    }

    /// The y range covered by the selected streams of a plot within the sample times
    /// `[start, end]` seconds.
    fn range_y(&self, model: &Model, plot_index: usize, (start, end): (f64, f64)) -> Option<(f64, f64)> {
        let mut range: Option<(f64, f64)> = None;
        for stream in self.selected_stream_iter(model, plot_index) {
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

    fn selected_stream_iter<'a>(
        &'a self,
        model: &'a Model,
        plot_index: usize,
    ) -> impl Iterator<Item = &'a Stream> {
        model.streams.iter().filter(move |stream| {
            *self.selected_streams[plot_index].get(&stream.path).unwrap_or(&false)
        })
    }

    fn calculate_plot_height(&mut self, ui: &mut Ui) -> f32 {
        let plot_count = self.number_of_plots as f32;
        let frame_stroke_width = 1.0;
        let frame_inner_margin = 4.0;
        let spacing_between_plots = ui.spacing().item_spacing.y * (plot_count - 1.0);
        let frame_vertical_overhead = plot_count * (2.0 * frame_stroke_width + 2.0 * frame_inner_margin);

        ((ui.available_height() - spacing_between_plots - frame_vertical_overhead) / plot_count)
            .max(0.0)
    }

    fn show_plot_stack(&mut self, model: &Model, time: &mut TimeRange, ui: &mut egui::Ui) {
        let plot_height = self.calculate_plot_height(ui);

        self.selected_plot = min(self.selected_plot, self.number_of_plots - 1);
        for i in 0..self.number_of_plots {
            self.show_single_plot(model, time, ui, i, plot_height);
        }
    }

    fn show_single_plot(
        &mut self,
        model: &Model,
        time: &mut TimeRange,
        ui: &mut egui::Ui,
        index: usize,
        plot_height: f32,
    ) {
        let stroke = if self.selected_plot == index {
            egui::Stroke::new(1.0, ui.visuals().widgets.active.bg_fill)
        } else {
            egui::Stroke::new(1.0, ui.visuals().window_fill())
        };
        let fille = if self.selected_plot == index {
            ui.visuals().faint_bg_color
        } else {
            ui.visuals().window_fill()
        };

        let frame = egui::Frame::default()
            .stroke(stroke)
            .fill(fille)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(4));

        frame.show(ui, |ui| {
            ui.set_height(plot_height);
            let plot_response = self.show_plot_contents(model, time, ui, index);

            if plot_response.response.clicked() {
                self.selected_plot = index;
            }
        });
    }

    /// Shows the shared time range of `time`, and applies panning and zooming of the
    /// plot to it.
    fn show_plot_contents(
        &self,
        model: &Model,
        time: &mut TimeRange,
        ui: &mut egui::Ui,
        plot_index: usize,
    ) -> egui_plot::PlotResponse<()> {
        let mut plot = egui_plot::Plot::new(format!("Stream View {}", plot_index))
            .link_cursor(egui::Id::new("stream_plot_cursor"), [true, false])
            .legend(egui_plot::Legend::default())
            .allow_drag([true, false])
            .allow_zoom([true, false])
            .allow_scroll([true, false])
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .custom_y_axes(vec![
                egui_plot::AxisHints::new_y()
                    .placement(egui_plot::HPlacement::Left)
                    .min_thickness(50.0),
            ]);

        if plot_index + 1 == self.number_of_plots {
            plot = plot.x_axis_label("t / s");
        }

        // x is the time since the first collected data
        let origin = time.origin();
        let range = time.range();
        let response = plot.show(ui, |plot_ui| {
            if let Some((start, end)) = range {
                plot_ui.set_plot_bounds_x(start - origin..=end - origin);
                if let Some((min_y, max_y)) = self.range_y(model, plot_index, (start, end)) {
                    plot_ui.set_plot_bounds_y(min_y..=max_y);
                }
            }

            for stream in self.selected_stream_iter(model, plot_index) {
                Self::plot_stream_series(stream, range, origin, plot_ui);
            }
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

        response
    }
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
        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            self.show_stream_selection(context.model, ui);
            ui.vertical(|ui| {
                self.show_plot_stack(context.model, context.time, ui);
            });
        });
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
    fn visible_values_includes_neighbours() {
        let values = values(100); // 1 ms apart
        let visible = visible_values(&values, Some((0.0105, 0.0205)));
        assert_eq!(visible.first().unwrap().timestamp, 10_000_000);
        assert_eq!(visible.last().unwrap().timestamp, 21_000_000);
        assert_eq!(visible_values(&values, None).len(), 100);
        assert!(visible_values(&values, Some((1.0, 2.0))).len() <= 1);
    }

    #[test]
    fn stream_tree_branches_at_slashes() {
        let tree = StreamTree::build(["acc/x", "acc/y", "/cpu", "acc"].into_iter());
        assert_eq!(tree.children.keys().copied().collect::<Vec<_>>(), ["acc", "cpu"]);
        let acc = &tree.children["acc"];
        assert_eq!(acc.path, Some("acc"));
        assert_eq!(acc.children.keys().copied().collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(acc.children["x"].path, Some("acc/x"));
        assert_eq!(tree.children["cpu"].path, Some("/cpu"));
    }
}
