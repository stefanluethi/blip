//! Plots with the shared time range on the x axis.

use crate::store::sorted::within;
use crate::widgets::time::{self, TimeBase, TimeRange, timestamp_at};
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

pub(crate) struct TimePlotResponse {
    /// The time in seconds hovered in this plot.
    pub hovered: Option<f64>,
    /// The y coordinate hovered in this plot.
    pub hovered_y: Option<f64>,
    /// The shown bounds after panning and zooming in this frame.
    pub bounds: egui_plot::PlotBounds,
    /// The screen rectangle of the plot inside its axes.
    pub frame: egui::Rect,
    /// Whether the plot was double clicked, which left the time as it was.
    pub double_clicked: bool,
    pub response: egui::Response,
    pub axis: TimeAxis,
}

impl TimePlotResponse {
    /// The decimals of the seconds of a time read off the plot, see [`time_decimals`].
    pub(crate) fn time_decimals(&self) -> usize {
        time_decimals(self.bounds.width() / f64::from(self.frame.width().max(1.0)))
    }
}

/// A plot with the time on the x axis that can only be panned and zoomed along it, to
/// show with `show` under the same `id`, its grid in the colors of `visuals`.
pub fn plot<'a>(id: egui::Id, time: &TimeRange, visuals: &egui::Visuals) -> egui_plot::Plot<'a> {
    let origin = time.origin();
    let base = time.base();
    super::plotting::base_plot(id, visuals)
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

/// Which axis the drag in progress pans in the plot under `id`: the y axis
/// (`Some(true)`) if it went more vertically than horizontally when it went far enough
/// to tell, else the time (`Some(false)`). `None` until then and without a drag, when
/// it pans nothing, so that a vertical drag does not move the time first.
fn drag_lock(ui: &egui::Ui, id: egui::Id) -> Option<bool> {
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
        return None;
    }
    if let Some(vertical) = ui.data(|data| data.get_temp(lock_id)) {
        return Some(vertical);
    }
    let moved = moved.filter(|moved| moved.length() > DRAG_LOCK_DISTANCE)?;
    let vertical = moved.y.abs() > moved.x.abs();
    ui.data_mut(|data| data.insert_temp(lock_id, vertical));
    Some(vertical)
}

/// The time range as it was before the first click of what may become a double click
/// on a plot, and when that click ended, `None` while its press did not end in a
/// click, e.g. while it drags.
#[derive(Clone)]
struct BeforeClick {
    time: TimeRange,
    released: Option<f64>,
}

/// Keeps `before`, the time range before a press in `rect` that may start a double
/// click, and puts it back in `time` on a double click there, so that a double click
/// leaves the time as it was before its first click. Returns whether the pointer double
/// clicked in `rect`.
fn undo_double_click(
    ui: &egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    before: Option<TimeRange>,
    time: &mut TimeRange,
) -> bool {
    let memory_id = id.with("before click");
    let delay = ui
        .ctx()
        .options(|options| options.input_options.max_double_click_delay);
    let (now, inside, clicked, double) = ui.input(|input| {
        let pointer = &input.pointer;
        (
            input.time,
            pointer.interact_pos().is_some_and(|pos| rect.contains(pos)),
            pointer.button_clicked(egui::PointerButton::Primary),
            pointer.button_double_clicked(egui::PointerButton::Primary),
        )
    });
    if !inside {
        return false;
    }
    let memory: Option<BeforeClick> = ui.data(|data| data.get_temp(memory_id));
    if double {
        if let Some(memory) = memory {
            *time = memory.time;
            ui.data_mut(|data| data.remove::<BeforeClick>(memory_id));
            ui.ctx().request_repaint();
        }
        return true;
    }
    if let Some(before) = before {
        // the press of a second click keeps the time of before the first one
        let second = (memory.as_ref())
            .and_then(|memory| memory.released)
            .is_some_and(|released| now - released < delay);
        if !second {
            let memory = BeforeClick {
                time: before,
                released: None,
            };
            ui.data_mut(|data| data.insert_temp(memory_id, memory));
        }
    } else if clicked && let Some(mut memory) = memory {
        memory.released = Some(now);
        ui.data_mut(|data| data.insert_temp(memory_id, memory));
    }
    false
}

/// The decimals of the seconds of a time read off a plot with `seconds_per_point`
/// seconds per point, as the time axis labels its marks: enough to tell apart times a
/// point apart, at most nanoseconds.
pub(crate) fn time_decimals(seconds_per_point: f64) -> usize {
    (-seconds_per_point.abs().log10().floor()).clamp(0.0, 9.0) as usize
}

/// Shows `plot` made with [`plot`] under `id` over the shared window of `time` with
/// the contents added by `build`, and applies panning and zooming of the plot to
/// `time`. Shows the cursor of `time` as a vertical line, which a click in the plot puts
/// there and which can be dragged. A double click leaves the time as it was before its
/// first click.
///
/// A drag pans once it went far enough to tell its direction: the time if it went
/// more horizontally, else, with `pan_zoom_y`, the y axis. With `pan_zoom_y` the wheel
/// with Alt zooms the y axis too, for `build` to keep in [`TimePlotResponse::bounds`].
pub(crate) fn show<'a>(
    id: egui::Id,
    plot: egui_plot::Plot<'a>,
    ui: &mut egui::Ui,
    time: &mut TimeRange,
    pan_zoom_y: bool,
    build: impl FnOnce(&mut egui_plot::PlotUi<'a>, TimeAxis) + 'a,
) -> TimePlotResponse {
    let axis = TimeAxis::of(time);
    // as it is before a press changes it, for a double click to put back
    let before = ui
        .input(|input| input.pointer.primary_pressed())
        .then(|| time.clone());
    let cursor_id = id.with("cursor");
    let cursor_grabbed = time::cursor_grabbed(ui, cursor_id);
    let lock = drag_lock(ui, id);
    // dragging the cursor does not pan the plot
    let pans = |vertical: bool| !cursor_grabbed && lock == Some(vertical);
    let plot = plot.allow_drag([pans(false), pan_zoom_y && pans(true)]);
    // the wheel zooms the time, and with Alt the y axis only, see `zoom_with_wheel`
    let zoom_y = pan_zoom_y && ui.input(|input| input.modifiers.alt);
    let plot = plot.allow_zoom([true, zoom_y]);
    let response = plot.show(ui, move |plot_ui| {
        if let Some((start, end)) = axis.range {
            plot_ui.set_plot_bounds_x(start..=end);
        }
        build(plot_ui, axis);
        super::plotting::hovered_point(plot_ui)
    });
    let transform = response.transform;
    super::zoom_on_wheel(&response.response);

    if let Some((start, end)) = axis.range {
        let bounds = transform.bounds();
        if super::plotting::range_moved((start, end), (bounds.min()[0], bounds.max()[0])) {
            time.pan_zoom(bounds.min()[0], bounds.max()[0]);
            ui.ctx().request_repaint();
        }
    }

    // the first click puts the cursor under the pointer, which gets the second one, so
    // the double click is told by the pointer in the plot
    let rect = response.response.rect;
    let double_clicked = undo_double_click(ui, id, rect, before, time);
    let set_at = (!double_clicked && (cursor_grabbed || response.response.clicked()))
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
        frame: *transform.frame(),
        double_clicked,
        response: response.response,
        axis,
    }
}

/// Indices of the timestamps within `range_x`, plus one neighbour on each side so the
/// line continues to the plot edges, all in nanoseconds. All indices if `range_x` is
/// `None`.
pub(crate) fn visible_range(timestamps: &[u64], range_x: Option<(u64, u64)>) -> Range<usize> {
    let Some(range_x) = range_x else {
        return 0..timestamps.len();
    };
    let Range { start, end } = within(timestamps, range_x);
    start.saturating_sub(1)..min(end + 1, timestamps.len())
}

#[cfg(test)]
mod tests;
