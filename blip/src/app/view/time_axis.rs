//! Plots with the shared time range on the x axis, and finding samples by time.

use crate::app::time::{TimeBase, TimeRange, timestamp_to_seconds};
use std::cmp::min;
use std::ops::Range;

/// The x axis of a time plot: x is the time in seconds since `origin`.
#[derive(Clone, Copy)]
pub struct TimeAxis {
    /// Time of the first collected data in seconds.
    pub origin: f64,
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
}

pub struct TimePlotResponse {
    /// The time in seconds hovered in this plot.
    pub hovered: Option<f64>,
    /// The y coordinate hovered in this plot.
    pub hovered_y: Option<f64>,
    pub response: egui::Response,
    pub axis: TimeAxis,
}

/// A plot with the time on the x axis that can only be panned and zoomed along it, to
/// show with [`show`].
pub fn plot<'a>(id: egui::Id, time: &TimeRange) -> egui_plot::Plot<'a> {
    let origin = time.origin();
    let base = time.base();
    egui_plot::Plot::new(id)
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
}

/// Shows `plot` over the shared window of `time` with the contents added by `build`,
/// applies panning and zooming of the plot to `time`, and marks the time hovered in
/// any view with a vertical line.
pub fn show<'a>(
    plot: egui_plot::Plot<'a>,
    ui: &mut egui::Ui,
    time: &mut TimeRange,
    build: impl FnOnce(&mut egui_plot::PlotUi<'a>, TimeAxis) + 'a,
) -> TimePlotResponse {
    let axis = TimeAxis::of(time);
    let origin = axis.origin;
    let hovered_elsewhere = time.hovered();
    let cursor_color = ui.visuals().weak_text_color();
    let response = plot.show(ui, move |plot_ui| {
        if let Some((start, end)) = axis.range {
            plot_ui.set_plot_bounds_x(start - origin..=end - origin);
        }
        build(plot_ui, axis);

        let hovered = plot_ui
            .response()
            .hovered()
            .then(|| plot_ui.pointer_coordinate())
            .flatten();
        let x = hovered
            .map(|point| point.x)
            .or(hovered_elsewhere.map(|t| t - origin));
        if let Some(x) = x {
            plot_ui.vline(egui_plot::VLine::new("", x).color(cursor_color).width(1.0));
        }
        hovered
    });

    if let Some((start, end)) = axis.range {
        let bounds = response.transform.bounds();
        let tolerance = (end - start) * 1.0e-6;
        if (bounds.min()[0] - (start - origin)).abs() > tolerance
            || (bounds.max()[0] - (end - origin)).abs() > tolerance
        {
            time.pan_zoom(bounds.min()[0] + origin, bounds.max()[0] + origin);
            ui.ctx().request_repaint();
        }
    }

    let hovered = response.inner.map(|point| point.x + origin);
    if let Some(t) = hovered {
        time.hover(t);
    }
    TimePlotResponse {
        hovered,
        hovered_y: response.inner.map(|point| point.y),
        response: response.response,
        axis,
    }
}

/// Index of the timestamp closest to `t` seconds.
pub fn closest_index(timestamps: &[u64], t: f64) -> Option<usize> {
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

/// Index of the newest timestamp at or before `t` seconds, i.e. of the sample that is
/// current at `t`.
pub fn index_at(timestamps: &[u64], t: f64) -> Option<usize> {
    // timestamps are ascending
    timestamps
        .partition_point(|&timestamp| timestamp_to_seconds(timestamp) <= t)
        .checked_sub(1)
}

/// Indices of the timestamps within `[start, end]` seconds.
pub fn within(timestamps: &[u64], (start, end): (f64, f64)) -> Range<usize> {
    // timestamps are ascending
    let first = timestamps.partition_point(|&timestamp| timestamp_to_seconds(timestamp) < start);
    let last = timestamps.partition_point(|&timestamp| timestamp_to_seconds(timestamp) <= end);
    first..last.max(first)
}

/// Indices of the timestamps within `range_x` seconds, plus one neighbour on each side
/// so the line continues to the plot edges. All indices if `range_x` is `None`.
pub fn visible_range(timestamps: &[u64], range_x: Option<(f64, f64)>) -> Range<usize> {
    let Some(range_x) = range_x else {
        return 0..timestamps.len();
    };
    let Range { start, end } = within(timestamps, range_x);
    start.saturating_sub(1)..min(end + 1, timestamps.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_index_picks_the_closest_timestamp() {
        let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
        assert_eq!(closest_index(&timestamps, 0.0024), Some(2));
        assert_eq!(closest_index(&timestamps, 0.0026), Some(3));
        assert_eq!(closest_index(&timestamps, 1.0), Some(9));
        assert!(closest_index(&[], 1.0).is_none());
    }

    #[test]
    fn index_at_picks_the_current_sample() {
        let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
        assert_eq!(index_at(&timestamps, 0.0026), Some(2));
        assert_eq!(index_at(&timestamps, 0.003), Some(3));
        assert_eq!(index_at(&timestamps, 1.0), Some(9));
        assert!(index_at(&timestamps, -1.0).is_none());
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
