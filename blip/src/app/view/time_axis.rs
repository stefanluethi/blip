//! Plots with the shared time range on the x axis, and finding samples by time.

use crate::app::theme;
use crate::app::time::{self, TimeBase, TimeRange, timestamp_at};
use std::cmp::min;
use std::ops::Range;

/// How far in points a drag goes before it is locked to panning the time or the y axis.
const DRAG_LOCK_DISTANCE: f32 = 6.0;

/// The x axis of a time plot: x is the time in seconds since `origin`.
#[derive(Clone, Copy)]
pub struct TimeAxis {
    /// The timestamp in nanoseconds the times count from, see [`TimeRange::origin`].
    pub origin: u64,
    pub base: TimeBase,
    /// The shown window `[start, end]` in seconds, or `None` if there is no data yet.
    pub range: Option<(f64, f64)>,
}

impl TimeAxis {
    fn of(time: &TimeRange) -> Self {
        Self {
            origin: time.origin(),
            base: time.base(),
            range: time.range(),
        }
    }

    /// The timestamp in nanoseconds at the x coordinate `x`.
    pub fn timestamp(&self, x: f64) -> u64 {
        timestamp_at(self.origin, x)
    }

    /// The shown window in nanoseconds.
    pub fn timestamps(&self) -> Option<(u64, u64)> {
        self.range
            .map(|(start, end)| (self.timestamp(start), self.timestamp(end)))
    }
}

pub struct TimePlotResponse {
    /// The time in seconds hovered in this plot.
    pub hovered: Option<f64>,
    /// The y coordinate hovered in this plot.
    pub hovered_y: Option<f64>,
    /// The shown bounds after panning and zooming in this frame.
    pub bounds: egui_plot::PlotBounds,
    pub response: egui::Response,
    pub axis: TimeAxis,
}

/// A plot with the time on the x axis that can only be panned and zoomed along it, to
/// show with [`show`] under the same `id`, its grid in the colors of `visuals`.
pub fn plot<'a>(id: egui::Id, time: &TimeRange, visuals: &egui::Visuals) -> egui_plot::Plot<'a> {
    let origin = time.origin();
    let base = time.base();
    egui_plot::Plot::new(id)
        .grid_color(theme::grid_color(visuals))
        // the values at the pointer are shown in a tooltip
        .show_x(false)
        .show_y(false)
        .allow_drag([true, false])
        .allow_zoom([true, false])
        .allow_scroll([true, false])
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .x_axis_formatter(move |mark, _range| {
            let decimals = (-mark.step_size.log10().floor()).max(0.0) as usize;
            base.format(timestamp_at(origin, mark.value), decimals)
        })
}

/// Whether the drag in progress pans the y axis of the plot under `id` instead of the
/// time: whether it went more vertically than horizontally when it went far enough to
/// tell. A drag pans the time until then.
fn drags_y(ui: &egui::Ui, id: egui::Id) -> bool {
    let lock_id = id.with("drag lock");
    let (down, moved) = ui.input(|input| {
        let pointer = &input.pointer;
        let moved = pointer
            .press_origin()
            .zip(pointer.interact_pos())
            .map(|(origin, pos)| pos - origin);
        (pointer.primary_down(), moved)
    });
    if !down {
        ui.data_mut(|data| data.remove::<bool>(lock_id));
        return false;
    }
    if let Some(vertical) = ui.data(|data| data.get_temp(lock_id)) {
        return vertical;
    }
    match moved {
        Some(moved) if moved.length() > DRAG_LOCK_DISTANCE => {
            let vertical = moved.y.abs() > moved.x.abs();
            ui.data_mut(|data| data.insert_temp(lock_id, vertical));
            vertical
        }
        _ => false,
    }
}

/// Shows `plot` made with [`plot`] under `id` over the shared window of `time` with
/// the contents added by `build`, and applies panning and zooming of the plot to
/// `time`. Shows the cursor of `time` as a vertical line, which a click in the plot puts
/// there and which can be dragged.
///
/// With `pan_zoom_y`, a vertical drag pans the y axis and the wheel with Alt zooms it,
/// for `build` to keep in [`TimePlotResponse::bounds`].
pub fn show<'a>(
    id: egui::Id,
    plot: egui_plot::Plot<'a>,
    ui: &mut egui::Ui,
    time: &mut TimeRange,
    pan_zoom_y: bool,
    build: impl FnOnce(&mut egui_plot::PlotUi<'a>, TimeAxis) + 'a,
) -> TimePlotResponse {
    let axis = TimeAxis::of(time);
    let cursor_id = id.with("cursor");
    let cursor_grabbed = time::cursor_grabbed(ui, cursor_id);
    let drag_y = pan_zoom_y && drags_y(ui, id);
    // dragging the cursor does not pan the plot
    let plot = plot.allow_drag([!cursor_grabbed && !drag_y, !cursor_grabbed && drag_y]);
    // the wheel zooms the time, and with Alt the y axis only, see `zoom_with_wheel`
    let zoom_y = pan_zoom_y && ui.input(|input| input.modifiers.alt);
    let plot = plot.allow_zoom([true, zoom_y]);
    let response = plot.show(ui, move |plot_ui| {
        if let Some((start, end)) = axis.range {
            plot_ui.set_plot_bounds_x(start..=end);
        }
        build(plot_ui, axis);
        plot_ui
            .response()
            .hovered()
            .then(|| plot_ui.pointer_coordinate())
            .flatten()
    });
    let transform = response.transform;
    super::zoom_on_wheel(&response.response);

    if let Some((start, end)) = axis.range {
        let bounds = transform.bounds();
        let tolerance = (end - start) * 1.0e-6;
        if (bounds.min()[0] - start).abs() > tolerance || (bounds.max()[0] - end).abs() > tolerance
        {
            time.pan_zoom(bounds.min()[0], bounds.max()[0]);
            ui.ctx().request_repaint();
        }
    }

    let set_at = (cursor_grabbed || response.response.clicked())
        .then(|| ui.ctx().pointer_interact_pos())
        .flatten();
    if let Some(pos) = set_at {
        let cursor = time.cursor();
        time.set_cursor(transform.value_from_position(pos).x);
        if time.cursor() != cursor {
            // the views shown before this one show it in the next frame
            ui.ctx().request_repaint();
        }
    }
    let cursor_x = time.cursor().map(|t| transform.position_from_point_x(t));
    let line = time::CursorLook {
        grabbed: cursor_grabbed,
        set: time.cursor_is_set(),
    };
    time::show_cursor(ui, cursor_id, *transform.frame(), cursor_x, line);

    TimePlotResponse {
        hovered: response.inner.map(|point| point.x),
        hovered_y: response.inner.map(|point| point.y),
        bounds: *transform.bounds(),
        response: response.response,
        axis,
    }
}

/// Index of the timestamp closest to `t`, all in nanoseconds.
pub fn closest_index(timestamps: &[u64], t: u64) -> Option<usize> {
    // timestamps are ascending
    let next = timestamps.partition_point(|&timestamp| timestamp < t);
    let candidates = [next.checked_sub(1), Some(next)];
    candidates
        .into_iter()
        .flatten()
        .filter(|&index| index < timestamps.len())
        .min_by_key(|&index| timestamps[index].abs_diff(t))
}

/// Index of the newest timestamp at or before `t`, i.e. of the sample that is current
/// at `t`, all in nanoseconds.
pub fn index_at(timestamps: &[u64], t: u64) -> Option<usize> {
    // timestamps are ascending
    timestamps
        .partition_point(|&timestamp| timestamp <= t)
        .checked_sub(1)
}

/// Indices of the timestamps within `[start, end]`, all in nanoseconds.
pub fn within(timestamps: &[u64], (start, end): (u64, u64)) -> Range<usize> {
    // timestamps are ascending
    let first = timestamps.partition_point(|&timestamp| timestamp < start);
    let last = timestamps.partition_point(|&timestamp| timestamp <= end);
    first..last.max(first)
}

/// Indices of the timestamps within `range_x`, plus one neighbour on each side so the
/// line continues to the plot edges, all in nanoseconds. All indices if `range_x` is
/// `None`.
pub fn visible_range(timestamps: &[u64], range_x: Option<(u64, u64)>) -> Range<usize> {
    let Some(range_x) = range_x else {
        return 0..timestamps.len();
    };
    let Range { start, end } = within(timestamps, range_x);
    start.saturating_sub(1)..min(end + 1, timestamps.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::Model;
    use crate::app::model::traces::Traces;
    use egui_kittest::Harness;

    /// A harness showing a time plot of data from 0 to 40 seconds, its state is the
    /// shared time range.
    fn time_plot_harness() -> Harness<'static, TimeRange> {
        let mut traces = Traces::default();
        for second in 0..=40 {
            traces.append(&proto::Value {
                timestamp: second * 1_000_000_000,
                path: "a".to_owned(),
                value: proto::Data::Float(1.0),
            });
        }
        let mut model = Model::default();
        model.traces = traces;
        Harness::builder()
            .with_size(egui::vec2(400.0, 200.0))
            .build_ui_state(
                move |ui, time: &mut TimeRange| {
                    time.update(&model, ui.input(|input| input.time));
                    let id = ui.id().with("plot");
                    show(id, plot(id, time, ui.visuals()), ui, time, false, |_, _| {});
                },
                TimeRange::default(),
            )
    }

    /// Drags the pointer from `from` to `to` in steps.
    fn drag(harness: &mut Harness<'_, TimeRange>, from: egui::Pos2, to: egui::Pos2) {
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
    fn clicking_a_time_plot_sets_the_cursor_which_can_be_dragged() {
        let mut harness = time_plot_harness();
        harness.run();
        assert!(harness.state().is_live());
        assert_eq!(harness.state().cursor(), Some(40.0));

        // the window from 20 to 40 seconds spans the plot
        let click = egui::pos2(300.0, 100.0);
        harness.drag_at(click);
        harness.run();
        harness.drop_at(click);
        harness.run();
        let time = harness.state();
        assert!(!time.is_live());
        let clicked = time.cursor().unwrap();
        assert!((30.0..40.0).contains(&clicked), "{clicked}");
        let range = time.range();

        // dragging the cursor moves it and not the window
        drag(&mut harness, click, egui::pos2(350.0, 100.0));
        let time = harness.state();
        assert!(time.cursor().unwrap() > clicked + 1.0);
        assert_eq!(time.range(), range);

        // dragging elsewhere pans the window and leaves the cursor
        let cursor = time.cursor();
        drag(
            &mut harness,
            egui::pos2(100.0, 100.0),
            egui::pos2(150.0, 100.0),
        );
        let time = harness.state();
        assert_eq!(time.cursor(), cursor);
        assert_ne!(time.range(), range);
    }

    #[test]
    fn scrolling_a_time_plot_zooms_the_window() {
        let mut harness = time_plot_harness();
        harness.run();
        let window = |harness: &Harness<'_, TimeRange>| {
            let (start, end) = harness.state().range().unwrap();
            end - start
        };
        let before = window(&harness);

        // scrolling up shows a shorter window
        super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 100.0), 100.0);
        let zoomed_in = window(&harness);
        assert!(zoomed_in < before * 0.9, "{zoomed_in} {before}");

        // scrolling down shows a longer window
        super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 100.0), -200.0);
        assert!(window(&harness) > zoomed_in * 1.1);
    }

    #[test]
    fn closest_index_picks_the_closest_timestamp() {
        let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
        assert_eq!(closest_index(&timestamps, 2_400_000), Some(2));
        assert_eq!(closest_index(&timestamps, 2_600_000), Some(3));
        assert_eq!(closest_index(&timestamps, 1_000_000_000), Some(9));
        assert!(closest_index(&[], 1).is_none());
    }

    #[test]
    fn index_at_picks_the_current_sample() {
        let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
        assert_eq!(index_at(&timestamps, 2_600_000), Some(2));
        assert_eq!(index_at(&timestamps, 3_000_000), Some(3));
        assert_eq!(index_at(&timestamps, 1_000_000_000), Some(9));
        assert!(index_at(&timestamps[1..], 0).is_none());
    }

    #[test]
    fn visible_range_includes_neighbours() {
        let timestamps: Vec<u64> = (0..100).map(|i| i * 1_000_000).collect(); // 1 ms apart
        let ms = |ms: f64| (ms * 1.0e6) as u64;
        assert_eq!(
            visible_range(&timestamps, Some((ms(10.5), ms(20.5)))),
            10..22
        );
        assert_eq!(visible_range(&timestamps, None), 0..100);
        assert!(visible_range(&timestamps, Some((ms(1000.0), ms(2000.0)))).len() <= 1);
        assert!(within(&timestamps, (ms(1000.0), ms(2000.0))).is_empty());
    }
}
