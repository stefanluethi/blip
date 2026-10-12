use super::Viewable;
use crate::app::model::{Model, streams::Stream};
use std::cmp::min;
use std::collections::HashMap;
use egui::Ui;

const MAX_PLOT_COUNT: usize = 6;
const NANOS_PER_SECOND: f64 = 1.0e9;
const SCAN_Y_MARGIN_FRACTION: f64 = 0.05;

fn timestamp_to_seconds(timestamp: u64) -> f64 {
    timestamp as f64 / NANOS_PER_SECOND
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
                    ui.vertical(|ui| {
                        let mut streams = model.streams.iter().collect::<Vec<_>>();
                        streams.sort_by_key(|stream| stream.path.clone());

                        for stream in streams {
                            let stream_selected = *active_selectors
                                .entry(stream.path.clone())
                                .or_insert(false);
                            let button_name =stream.path.clone();
                            if ui.add_sized(
                                egui::Vec2::new(ui.available_width(), 20.0),
                                egui::Button::new(button_name).selected(stream_selected),
                            ).clicked() {
                                *active_selectors.get_mut(&stream.path).unwrap() = !stream_selected;
                            }
                        }
                    });
                });
            });
    }

    fn plot_stream_series(&self, stream: &Stream, plot_ui: &mut egui_plot::PlotUi<'_>) {
        // todo: downsampling
        let points = stream
            .values
            .iter()
            .map(|v| [timestamp_to_seconds(v.timestamp), v.value as f64])
            .collect::<Vec<[f64; 2]>>();

        plot_ui.line(egui_plot::Line::new(
            format!("{} / {}", stream.path, "none"),
            egui_plot::PlotPoints::from(points),
        ));
    }

    fn latest_timestamp(model: &Model) -> Option<u64> {
        model
            .streams
            .iter()
            .filter_map(|stream| stream.values.last())
            .map(|v| v.timestamp)
            .max()
    }

    /// The scan viewport: the last `scan_window` seconds up to the newest sample.
    /// Only returned when new data arrived, so the viewport can be moved freely otherwise.
    fn scan_range_x(&self, latest_timestamp: Option<u64>) -> Option<(f64, f64)> {
        if self.scope_mode != ScopeMode::Scan
            || (latest_timestamp == self.latest_timestamp && !self.reset_bounds)
        {
            return None;
        }
        let end = timestamp_to_seconds(latest_timestamp?);
        Some((end - self.scan_window as f64, end))
    }

    /// The y range covered by the selected streams of a plot within `[start, end]` seconds.
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

        if plot_index == self.number_of_plots {
            plot = plot.x_axis_label("t / s");
        }

        plot.show(ui, |plot_ui| {
            if self.reset_bounds && self.scope_mode == ScopeMode::Full {
                plot_ui.set_auto_bounds(true);
            }

            if let Some(range_x) = self.scan_range_x {
                plot_ui.set_plot_bounds_x(range_x.0..=range_x.1);
                if let Some((min_y, max_y)) = self.scan_range_y(model, plot_index, range_x) {
                    plot_ui.set_plot_bounds_y(min_y..=max_y);
                }
            }

            for stream in self.selected_stream_iter(model, plot_index) {
                self.plot_stream_series(stream, plot_ui);
            }
        })
    }
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
