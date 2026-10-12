use super::Viewable;
use crate::app::model::{Model, streams::{Stream, Value}};
use std::cmp::min;
use std::collections::{BTreeMap, HashMap};
use egui::Ui;

const MAX_PLOT_COUNT: usize = 6;
const NANOS_PER_SECOND: f64 = 1.0e9;
const SCAN_Y_MARGIN_FRACTION: f64 = 0.05;
/// Upper bound of points drawn per curve within the visible x range.
const MAX_POINTS_PER_CURVE: usize = 100_000;

fn timestamp_to_seconds(timestamp: u64) -> f64 {
    timestamp as f64 / NANOS_PER_SECOND
}

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

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug, Clone, Copy)]
pub enum ScopeMode {
    Full,
    Scan,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Plot {
    number_of_plots: usize,
    selected_plot: usize,
    selected_streams: [HashMap<String, bool>; MAX_PLOT_COUNT],
    scope_mode: ScopeMode,
    scan_window: f32,
    #[serde(skip)]
    reset_bounds: bool,
    #[serde(skip)]
    latest_timestamp: Option<u64>,
    #[serde(skip)]
    scan_range_x: Option<(f64, f64)>,
    /// Seconds subtracted from the sample times to get plot x coordinates. In scan mode
    /// this is the newest sample, so the x axis shows the time before it.
    #[serde(skip)]
    x_offset: f64,
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
            scope_mode: ScopeMode::Full,
            scan_window: 20.0,
            reset_bounds: false,
            latest_timestamp: None,
            scan_range_x: None,
            x_offset: 0.0,
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

    fn show_plot_settings(&mut self, model: &mut Model, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Clear Data").clicked() {
                model.streams.clear();
            }
            let previous_mode = self.scope_mode;
            egui::ComboBox::from_label("Mode")
                .selected_text(format!("{:?}", self.scope_mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.scope_mode, ScopeMode::Full, "Full");
                    ui.selectable_value(&mut self.scope_mode, ScopeMode::Scan, "Scan");
                });
            if self.scope_mode != previous_mode {
                self.reset_bounds = true;
            }

            if self.scope_mode == ScopeMode::Scan {
                ui.separator();
                ui.label("Scan window:");
                ui.add(
                    egui::DragValue::new(&mut self.scan_window)
                        .range(0.001..=1.0e6)
                        .suffix("s")
                        .speed(0.01),
                );
            }
        });
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

    fn plot_stream_series(
        &self,
        stream: &Stream,
        range_x: Option<(f64, f64)>,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let values = visible_values(&stream.values, range_x);
        let points = downsample(values, self.x_offset, MAX_POINTS_PER_CURVE);

        plot_ui.line(egui_plot::Line::new(
            format!("{} / {}", stream.path, "none"),
            egui_plot::PlotPoints::from(points),
        ));
    }

    /// The range of sample times in seconds the plot shows this frame, or `None` if all
    /// data is needed, e.g. while the bounds are fitted automatically to the data.
    fn visible_range_x(&self, plot_ui: &egui_plot::PlotUi<'_>) -> Option<(f64, f64)> {
        if let Some((start, end)) = self.scan_range_x {
            return Some((start + self.x_offset, end + self.x_offset));
        }
        if (self.reset_bounds && self.scope_mode == ScopeMode::Full) || plot_ui.auto_bounds().x {
            return None;
        }
        let bounds = plot_ui.plot_bounds();
        bounds
            .is_valid_x()
            .then(|| (bounds.min()[0] + self.x_offset, bounds.max()[0] + self.x_offset))
    }

    fn latest_timestamp(model: &Model) -> Option<u64> {
        model
            .streams
            .iter()
            .filter_map(|stream| stream.values.last())
            .map(|v| v.timestamp)
            .max()
    }

    /// The scan viewport in plot coordinates: the last `scan_window` seconds up to the
    /// newest sample at 0. Only returned when new data arrived, so the viewport can be
    /// moved freely otherwise.
    fn scan_range_x(&self, latest_timestamp: Option<u64>) -> Option<(f64, f64)> {
        if self.scope_mode != ScopeMode::Scan
            || (latest_timestamp == self.latest_timestamp && !self.reset_bounds)
        {
            return None;
        }
        latest_timestamp.map(|_| (-self.scan_window as f64, 0.0))
    }

    /// The y range covered by the selected streams of a plot within the sample times
    /// `[start, end]` seconds.
    fn scan_range_y(&self, model: &Model, plot_index: usize, (start, end): (f64, f64)) -> Option<(f64, f64)> {
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
            let margin = if high > low { (high - low) * SCAN_Y_MARGIN_FRACTION } else { 0.5 };
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

    fn show_plot_stack(&mut self, model: &Model, ui: &mut egui::Ui) {
        let plot_height = self.calculate_plot_height(ui);

        let link_group_id = ui.id().with("stream_plot_group");
        let link_x = egui::Vec2b::new(true, false);

        let latest_timestamp = Self::latest_timestamp(model);
        self.x_offset = match (self.scope_mode, latest_timestamp) {
            (ScopeMode::Scan, Some(timestamp)) => timestamp_to_seconds(timestamp),
            _ => 0.0,
        };
        self.scan_range_x = self.scan_range_x(latest_timestamp);

        self.selected_plot = min(self.selected_plot, self.number_of_plots - 1);
        for i in 0..self.number_of_plots {
            self.show_single_plot(model, ui, i, plot_height, link_group_id, link_x);
        }
        self.reset_bounds = false;
        self.latest_timestamp = latest_timestamp;
    }

    fn show_single_plot(
        &mut self,
        model: &Model,
        ui: &mut egui::Ui,
        index: usize,
        plot_height: f32,
        link_group_id: egui::Id,
        link_x: egui::Vec2b,
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
            let plot_response = self.show_plot_contents(model, ui, index, link_group_id, link_x);

            if plot_response.response.clicked() {
                self.selected_plot = index;
            }
        });
    }

    fn show_plot_contents(
        &self,
        model: &Model,
        ui: &mut egui::Ui,
        plot_index: usize,
        link_group_id: egui::Id,
        link_x: egui::Vec2b,
    ) -> egui_plot::PlotResponse<()> {
        let mut plot = egui_plot::Plot::new(format!("Stream View {}", plot_index))
            .link_axis(link_group_id, link_x)
            .link_cursor(link_group_id, link_x)
            .legend(egui_plot::Legend::default())
            .custom_y_axes(vec![
                egui_plot::AxisHints::new_y()
                    .placement(egui_plot::HPlacement::Left)
                    .min_thickness(50.0),
            ]);

        if self.scope_mode == ScopeMode::Scan {
            // x is the time relative to the newest sample, label it as seconds before it
            plot = plot.x_axis_formatter(|mark, _range| {
                let num_decimals = (-mark.step_size.log10().round()).max(0.0) as usize;
                // `+ 0.0` turns -0 into 0
                let seconds_before = -mark.value + 0.0;
                egui::emath::format_with_decimals_in_range(seconds_before, num_decimals..=num_decimals)
            });
        }
        if plot_index + 1 == self.number_of_plots {
            plot = plot.x_axis_label(match self.scope_mode {
                ScopeMode::Full => "t / s",
                ScopeMode::Scan => "t before latest / s",
            });
        }

        plot.show(ui, |plot_ui| {
            if self.reset_bounds && self.scope_mode == ScopeMode::Full {
                plot_ui.set_auto_bounds(true);
            }

            if let Some((start, end)) = self.scan_range_x {
                plot_ui.set_plot_bounds_x(start..=end);
                let sample_range = (start + self.x_offset, end + self.x_offset);
                if let Some((min_y, max_y)) = self.scan_range_y(model, plot_index, sample_range) {
                    plot_ui.set_plot_bounds_y(min_y..=max_y);
                }
            }

            let range_x = self.visible_range_x(plot_ui);
            for stream in self.selected_stream_iter(model, plot_index) {
                self.plot_stream_series(stream, range_x, plot_ui);
            }
        })
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
/// times in seconds minus `x_offset`.
fn downsample(values: &[Value], x_offset: f64, max_points: usize) -> Vec<[f64; 2]> {
    let to_point = |v: &Value| [timestamp_to_seconds(v.timestamp) - x_offset, v.value as f64];
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
    fn view(&mut self, ui: &mut egui::Ui, model: &mut Model) {
        ui.vertical(|ui| {
            let available_size = ui.available_size();

            ui.horizontal(|ui| {
                ui.set_height(available_size.y);

                self.show_stream_selection(model, ui);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    self.show_plot_settings(model, ui);
                    ui.vertical(|ui| {
                        self.show_plot_stack(model, ui);
                    })
                });
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
