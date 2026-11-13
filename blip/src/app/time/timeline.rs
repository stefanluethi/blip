//! The timeline under the views: a ruler with the times, on which the cursor is set,
//! over a track of the collected data. The track shows how many values and log records
//! arrived when and the gaps without any, with a thumb for the shown window. It shows
//! all data, or a part of it when zoomed in with the mouse wheel.

use super::{
    MAX_WINDOW, MIN_WINDOW, TimeBase, TimeRange, ZOOM_SPEED, paint_cursor, seconds_since,
    timeline_wheel, timestamp_at,
};
use crate::app::model::logs::LogLevel;
use crate::app::model::{Activity, Model, Version};
use crate::app::theme::{self, TokensExt};
use egui_phosphor::regular;
use std::ops::Range;

/// Height of the ruler above the track, which the controls beside it leave free: a
/// line of text in the size of the rest of the UI.
pub(super) fn ruler_height(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body) + 4.0
}
const MIN_THUMB_WIDTH: f32 = 6.0;
/// Distance in points from an edge of the thumb within which it can be grabbed.
const EDGE_GRAB_DISTANCE: f32 = 4.0;
/// Least width of the bins in which values and log records are counted, in points.
/// Their duration is a round step, so the bins stay put while data arrives.
const MIN_BIN_WIDTH: f32 = 4.0;
/// How many times the mean time between samples a stretch without any must last to
/// show as a gap.
const GAP_FACTOR: f64 = 10.0;
/// Distance between the hatching lines of gaps in points.
const HATCH_SPACING: f32 = 5.0;
/// The share of the track the log records of a bin reach at most, below its values.
const LOG_BAR_SHARE: f32 = 0.35;
/// Least height of the part of a bar of log records of one level, in points.
const MIN_LOG_HEIGHT: f32 = 2.0;
/// How long the bars take to follow a change of their scale, in seconds.
const RESCALE_TIME: f32 = 0.5;
/// How many log records the tooltip of a bin lists at most.
const TOOLTIP_ENTRIES: usize = 8;
/// Least distance between the labels of the ruler in points.
const LABEL_SPACING: f32 = 110.0;
/// Height of the ticks of the ruler in points.
const TICK_HEIGHT: f32 = 3.0;
/// Distance of the time at the pointer from it, so that the pointer does not hide it.
const PILL_OFFSET: f32 = 14.0;
/// Showing all data, the timeline ends at the next of this many round steps of the
/// data's duration, so that it rescales now and then instead of all the time.
const FIT_STEPS: f64 = 8.0;
/// Seconds the timeline waits at least before it counts the arriving data again.
const RECOUNT_INTERVAL: f64 = 0.1;

/// What the timeline shows when zoomed in.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Overview {
    /// Start in seconds, unless it follows the newest data.
    start: f64,
    /// Duration in seconds, shorter than all data.
    duration: f64,
    /// Whether it ends at the newest data and moves on with it.
    follows: bool,
}

/// Converts between times in seconds and x coordinates of the timeline.
#[derive(Clone, Copy, Debug)]
struct Scale {
    x: egui::Rangef,
    start: f64,
    duration: f64,
}

impl Scale {
    fn x_at(&self, t: f64) -> f32 {
        self.x.min + ((t - self.start) / self.duration) as f32 * self.x.span()
    }

    fn t_at(&self, x: f32) -> f64 {
        self.start + f64::from((x - self.x.min) / self.x.span()) * self.duration
    }

    fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Seconds per point.
    fn resolution(&self) -> f64 {
        self.duration / f64::from(self.x.span().max(1.0))
    }
}

/// What a press on the track grabbed.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Grab {
    /// The left edge of the thumb, to change the start of the window.
    Start,
    /// The right edge of the thumb, to change the end of the window.
    End,
    /// The thumb, to move the window.
    Thumb,
    /// The track beside the thumb, to move the window there.
    Track,
}

/// The times `[start, end]` in seconds the timeline of `time` shows, `None` without
/// data.
fn view(time: &TimeRange) -> Option<(f64, f64)> {
    let (first, latest) = time.span?;
    Some(match time.overview {
        None => (first, first + fit_duration(latest - first)),
        Some(overview) if overview.follows => (latest - overview.duration, latest),
        Some(overview) => (overview.start, overview.start + overview.duration),
    })
}

/// The duration the timeline shows for all data lasting `span` seconds: rounded up
/// to the next of [`FIT_STEPS`] round steps.
fn fit_duration(span: f64) -> f64 {
    let span = span.max(MIN_WINDOW);
    let step = label_step(span / FIT_STEPS);
    (span / step).ceil() * step
}

/// Zooms the timeline of `time` shown at `scale` by `factor` around `x`, back to all
/// data once it would show as much. Showing all data, it zooms from the duration of
/// the data rather than of the timeline, which [`fit_duration`] rounds up, so that
/// zooming in a little at a time gets anywhere.
fn zoom(time: &mut TimeRange, scale: &Scale, x: f32, factor: f64) {
    let Some((first, latest)) = time.span else {
        return;
    };
    let (from_start, from_duration) = match time.overview {
        None => (first, latest - first),
        Some(_) => (scale.start, scale.duration),
    };
    let duration = (from_duration * factor).max(MIN_WINDOW);
    if duration >= latest - first {
        time.overview = None;
        return;
    }
    let t = scale.t_at(x).clamp(first, latest);
    let start = clamp_within(t - (t - from_start) * factor, first, latest - duration);
    time.overview = Some(Overview {
        start,
        duration,
        follows: start + duration >= latest,
    });
}

/// `value` limited to `low..=high`, `low` if rounding puts `high` before it (where
/// `clamp` panics).
fn clamp_within(value: f64, low: f64, high: f64) -> f64 {
    value.min(high).max(low)
}

/// Moves the zoomed-in timeline of `time` shown at `scale` by `delta` seconds within
/// the data.
fn pan(time: &mut TimeRange, scale: &Scale, delta: f64) {
    let (Some((first, latest)), Some(_)) = (time.span, time.overview) else {
        return;
    };
    let start = clamp_within(scale.start + delta, first, latest - scale.duration);
    time.overview = Some(Overview {
        start,
        duration: scale.duration,
        follows: start + scale.duration >= latest,
    });
}

/// Shows the timeline of `time` over the data of `model`, see [`TimeRange::ui`].
pub(super) fn show(time: &mut TimeRange, ui: &mut egui::Ui, model: &Model) {
    let id = ui.id().with("timeline");
    let track_height = ui.spacing().interact_size.y;
    let ruler_height = ruler_height(ui);
    let size = egui::vec2(ui.available_width(), ruler_height + track_height);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let (ruler_rect, track_rect) = rect.split_top_bottom_at_y(rect.top() + ruler_height);
    let ruler = ui.interact(ruler_rect, id.with("ruler"), egui::Sense::click_and_drag());
    let track = ui.interact(track_rect, id.with("track"), egui::Sense::click_and_drag());

    let tokens = ui.tokens();
    let painter = ui.painter_at(rect.expand(1.0));
    painter.rect_filled(track_rect, 3.0, tokens.canvas);
    let Some((start, end)) = view(time) else {
        return;
    };
    let mut scale = Scale {
        x: track_rect.x_range(),
        start,
        duration: (end - start).max(MIN_WINDOW),
    };
    // the scale stays while pressed, so nothing moves away under the pointer
    let frozen_id = id.with("frozen");
    if ruler.is_pointer_button_down_on() || track.is_pointer_button_down_on() {
        match ui.data(|data| data.get_temp::<Scale>(frozen_id)) {
            Some(frozen) => scale = frozen,
            None => ui.data_mut(|data| {
                data.insert_temp(frozen_id, scale);
            }),
        }
    } else {
        ui.data_mut(|data| data.remove::<Scale>(frozen_id));
    }

    if let Some(pos) = ruler.hover_pos().or(track.hover_pos()) {
        let wheel = timeline_wheel(ui);
        if wheel.y != 0.0 {
            // scrolling up zooms in
            zoom(time, &scale, pos.x, f64::from(-wheel.y * ZOOM_SPEED).exp());
        }
        if wheel.x != 0.0 {
            pan(time, &scale, -f64::from(wheel.x) * scale.resolution());
        }
    }

    // pressing or dragging on the ruler puts the cursor there
    let scrubbing = ruler.is_pointer_button_down_on();
    if scrubbing && let Some(pos) = ruler.interact_pointer_pos() {
        time.set_cursor(scale.t_at(pos.x));
    }

    let bins = Bins::cached(ui, id, model, time, &scale);
    let hovered_logs = track
        .hover_pos()
        .filter(|pos| pos.y >= track_rect.bottom() - track_rect.height() * LOG_BAR_SHARE)
        .and_then(|pos| bins.logs_at(model, pos.x));
    let log_time = hovered_logs.as_ref().map(|logs| logs.most_severe);
    track_input(time, ui, &track, &scale, log_time);

    // a new recording starts with its own scale, not animated from the last one's
    let scale_id = id.with(time.origin());
    bins.paint(ui, scale_id, &painter, track_rect);
    if let Some(range) = time.range() {
        let thumb = thumb_rect(range, track_rect, &scale);
        let active = track.dragged() || track.hovered();
        let stroke = if active {
            egui::Stroke::new(1.5, tokens.accent_text)
        } else {
            egui::Stroke::new(1.0, tokens.accent)
        };
        painter.rect(
            thumb,
            3.0,
            ui.visuals().selection.bg_fill.gamma_multiply(0.35),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    if let Some(logs) = hovered_logs {
        track
            .clone()
            .on_hover_ui_at_pointer(|ui| log_tooltip_ui(ui, model, logs.times));
    }

    let mut label_rect = ruler_rect;
    if time.overview.is_some() {
        // a button to show all data again, at the end of the ruler
        let button_rect = egui::Rect::from_min_max(
            egui::pos2(ruler_rect.right() - ruler_height, ruler_rect.top()),
            ruler_rect.right_bottom(),
        );
        label_rect.max.x = button_rect.left();
        let button = egui::Button::new(regular::ARROWS_OUT_LINE_HORIZONTAL).frame(false);
        if ui
            .put(button_rect, button)
            .on_hover_text("Show all data on the timeline")
            .clicked()
        {
            time.overview = None;
        }
    }
    let base = time.base();
    paint_ruler(
        ui,
        &painter,
        ruler_rect,
        label_rect.right(),
        &scale,
        base,
        time.origin(),
    );
    let cursor_x = time.cursor().map(|t| scale.x_at(t));
    let hover_x = ruler.hover_pos().map(|pos| pos.x);
    if let Some(x) = hover_x.filter(|_| !scrubbing) {
        // where pressing puts the cursor
        let color = theme::cursor_color(ui.visuals()).gamma_multiply(0.4);
        painter.vline(x, rect.y_range(), egui::Stroke::new(1.0, color));
    }
    if let Some(x) = cursor_x.filter(|x| rect.x_range().contains(*x)) {
        let width = if scrubbing { 2.0 } else { 1.5 };
        paint_cursor(&painter, x, rect.y_range(), width, ui.visuals());
    }
    let pill_x = if scrubbing { cursor_x } else { hover_x };
    if let Some(x) = pill_x {
        let decimals = decimals_for(scale.resolution());
        let text = base.format(time.timestamp(scale.t_at(x)), decimals);
        paint_pill(ui, &painter, ruler_rect, x, &text);
    }
    if ruler.hovered() || scrubbing {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
}

/// Applies a press, drag or click on the track: dragging an edge of the thumb changes
/// the start or end of the window, dragging the thumb moves it, pressing beside it
/// moves the window there, and clicking a bar of log records goes to the time
/// `log_time` of the most severe one.
fn track_input(
    time: &mut TimeRange,
    ui: &egui::Ui,
    track: &egui::Response,
    scale: &Scale,
    log_time: Option<f64>,
) {
    let Some(range) = time.range() else {
        return;
    };
    let thumb = thumb_rect(range, track.rect, scale);
    let grab_id = track.id.with("grab");

    if let Some(pos) = track.hover_pos() {
        let icon = match (log_time, grab_at(thumb, pos)) {
            (Some(_), _) => egui::CursorIcon::PointingHand,
            (None, Grab::Start | Grab::End) => egui::CursorIcon::ResizeHorizontal,
            (None, Grab::Thumb) if track.dragged() => egui::CursorIcon::Grabbing,
            (None, Grab::Thumb) => egui::CursorIcon::Grab,
            (None, Grab::Track) => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(icon);
    }

    if track.clicked() {
        match log_time {
            Some(t) => time.go_to(t),
            None => {
                let pos = track.interact_pointer_pos();
                if let Some(pos) = pos.filter(|pos| grab_at(thumb, *pos) == Grab::Track) {
                    time.move_end(scale.t_at(pos.x) + time.window / 2.0);
                }
            }
        }
        return;
    }

    if track.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        let grab = origin.map_or(Grab::Thumb, |origin| grab_at(thumb, origin));
        if let (Grab::Track, Some(origin)) = (grab, origin) {
            time.move_end(scale.t_at(origin.x) + time.window / 2.0);
        }
        ui.data_mut(|data| data.insert_temp(grab_id, grab));
    }
    let grab = ui.data(|data| data.get_temp::<Grab>(grab_id));
    if track.drag_stopped() {
        ui.data_mut(|data| data.remove::<Grab>(grab_id));
    }
    let (Some(grab), Some(pos)) = (grab, track.interact_pointer_pos()) else {
        return;
    };
    if !track.dragged() {
        return;
    }
    let Some((start, end)) = time.range() else {
        return;
    };
    match grab {
        Grab::Start => {
            let start = scale.t_at(pos.x).min(end - MIN_WINDOW);
            time.window = (end - start).clamp(MIN_WINDOW, MAX_WINDOW);
        }
        Grab::End => {
            let latest = time.latest().unwrap_or(end);
            let end = scale
                .t_at(pos.x)
                .clamp(start + MIN_WINDOW, latest.max(start + MIN_WINDOW));
            time.window = (end - start).clamp(MIN_WINDOW, MAX_WINDOW);
            time.move_end(end);
        }
        Grab::Thumb | Grab::Track => {
            let delta = f64::from(track.drag_delta().x) * scale.resolution();
            if delta != 0.0 {
                time.move_end(end + delta);
            }
        }
    }
}

/// The thumb showing the window `[start, end]` seconds on the track in `rect`, at
/// least [`MIN_THUMB_WIDTH`] wide, and at its edge if the window is outside.
fn thumb_rect((start, end): (f64, f64), rect: egui::Rect, scale: &Scale) -> egui::Rect {
    let x_at = |t: f64| scale.x_at(t).min(rect.right()).max(rect.left());
    let x = x_at(start)..=x_at(end);
    let thumb = egui::Rect::from_x_y_ranges(x, rect.y_range());
    thumb.expand2(egui::vec2(
        ((MIN_THUMB_WIDTH - thumb.width()) / 2.0).max(0.0),
        0.0,
    ))
}

/// What pressing at `pos` grabs of the track with the thumb at `thumb`: its edges are
/// grabbed a bit outside too, but leave the middle of a narrow thumb to move it.
fn grab_at(thumb: egui::Rect, pos: egui::Pos2) -> Grab {
    let inside = EDGE_GRAB_DISTANCE.min(thumb.width() / 3.0);
    if (thumb.left() - EDGE_GRAB_DISTANCE..=thumb.left() + inside).contains(&pos.x) {
        Grab::Start
    } else if (thumb.right() - inside..=thumb.right() + EDGE_GRAB_DISTANCE).contains(&pos.x) {
        Grab::End
    } else if thumb.x_range().contains(pos.x) {
        Grab::Thumb
    } else {
        Grab::Track
    }
}

/// The log records hovered in a bin.
struct HoveredLogs {
    /// The times of the bin in nanoseconds.
    times: Range<u64>,
    /// The time of the first of the most severe records in seconds.
    most_severe: f64,
}

/// What arrived in round steps of time over the shown part of the timeline.
struct Bins {
    scale: Scale,
    /// The timestamp in nanoseconds the times count from.
    origin: u64,
    /// Start of the first bin in nanoseconds, a multiple of the step.
    start: u64,
    /// Duration of a bin in nanoseconds.
    step: u64,
    activity: Vec<Activity>,
    /// Times of the first and the newest data in seconds.
    span: (f64, f64),
}

/// The activity counted for a timeline under its id, kept until the bins change, or
/// the samples change and [`RECOUNT_INTERVAL`] passed.
#[derive(Clone)]
struct CachedActivity {
    bins: (u64, u64, usize),
    version: Version,
    /// When it was counted, in seconds as egui counts them.
    at: f64,
    activity: Vec<Activity>,
}

impl Bins {
    /// The bins over `scale` of the data of `model` shown in `time`.
    #[cfg(test)]
    fn of(model: &Model, time: &TimeRange, scale: &Scale) -> Self {
        Self::counted(time, scale, |start, step, count| {
            model.activity(start, step, count)
        })
    }

    /// Like [`Self::of`], but counted anew under `id` only once the bins or the data
    /// changed.
    fn cached(ui: &egui::Ui, id: egui::Id, model: &Model, time: &TimeRange, scale: &Scale) -> Self {
        Self::counted(time, scale, |start, step, count| {
            let (bins, version) = ((start, step, count), model.samples_version());
            let now = ui.input(|input| input.time);
            let cache_id = id.with("activity");
            match ui.data(|data| data.get_temp::<CachedActivity>(cache_id)) {
                Some(cached)
                    if cached.bins == bins
                        && (cached.version == version || now - cached.at < RECOUNT_INTERVAL) =>
                {
                    if cached.version != version {
                        ui.ctx().request_repaint_after_secs(RECOUNT_INTERVAL as f32);
                    }
                    cached.activity
                }
                _ => {
                    let activity = model.activity(start, step, count);
                    let cached = CachedActivity {
                        bins,
                        version,
                        at: now,
                        activity: activity.clone(),
                    };
                    ui.data_mut(|data| data.insert_temp(cache_id, cached));
                    activity
                }
            }
        })
    }

    /// The bins over `scale` in round steps of time, with what `activity` counts in
    /// the bins from a start, of a step, in nanoseconds, and of a count.
    fn counted(
        time: &TimeRange,
        scale: &Scale,
        activity: impl FnOnce(u64, u64, usize) -> Vec<Activity>,
    ) -> Self {
        let min_step = scale.resolution() * f64::from(MIN_BIN_WIDTH);
        let step = timestamp_at(0, label_step(min_step)).max(1);
        let start = time.timestamp(scale.start) / step * step;
        let end = time.timestamp(scale.end());
        let count = ((end.saturating_sub(start)) / step + 1) as usize;
        Self {
            scale: *scale,
            origin: time.origin(),
            start,
            step,
            activity: activity(start, step, count),
            span: time.span.unwrap_or_default(),
        }
    }

    /// The times `[start, end)` of `bin` in seconds.
    fn bin_times(&self, bin: usize) -> (f64, f64) {
        let start = self.start + self.step * bin as u64;
        (
            seconds_since(self.origin, start),
            seconds_since(self.origin, start + self.step),
        )
    }

    /// The x coordinates of the bar of `bin`, with a gap to the next one.
    fn bar_x(&self, bin: usize) -> egui::Rangef {
        let (start, end) = self.bin_times(bin);
        let (left, right) = (self.scale.x_at(start), self.scale.x_at(end));
        egui::Rangef::new(left + 0.5, (right - 0.5).max(left + 1.0))
    }

    /// The share of `bin` within the collected data, to tell how many values arrive
    /// per time in the first and newest bin, which data covers only in part.
    fn covered(&self, bin: usize) -> f64 {
        let (start, end) = self.bin_times(bin);
        let (first, latest) = self.span;
        ((end.min(latest) - start.max(first)) / (end - start)).clamp(0.0, 1.0)
    }

    /// The values of `bin` per bin duration: the first bin, which the data covers in
    /// part, as if it were complete, the newest as it is while filling.
    fn rate(&self, bin: usize) -> f64 {
        let values = f64::from(self.activity[bin].values);
        if Some(bin) == self.newest_bin() {
            values
        } else {
            values / self.covered(bin).max(0.25)
        }
    }

    /// The highest [`Self::rate`] and number of log records of the complete bins, to
    /// scale the bars to, or of the newest bin until there is a complete one.
    fn scale_max(&self) -> (f64, u32) {
        let newest = self.newest_bin();
        let complete = || (0..self.activity.len()).filter(|&bin| Some(bin) != newest);
        let max_rate = complete().map(|bin| self.rate(bin)).fold(0.0, f64::max);
        let max_logs = complete()
            .map(|bin| self.activity[bin].log_count())
            .max()
            .unwrap_or(0);
        let max_rate = if max_rate > 0.0 {
            max_rate
        } else {
            newest.map_or(0.0, |bin| self.rate(bin))
        };
        let max_logs = if max_logs > 0 {
            max_logs
        } else {
            newest.map_or(0, |bin| self.activity[bin].log_count())
        };
        (max_rate, max_logs)
    }

    /// The bin of the newest data, which is still filling.
    fn newest_bin(&self) -> Option<usize> {
        let latest = timestamp_at(self.origin, self.span.1);
        let bin = (latest.checked_sub(self.start)? / self.step) as usize;
        (bin < self.activity.len()).then_some(bin)
    }

    /// The log records of the bin at `x`, if it has any.
    fn logs_at(&self, model: &Model, x: f32) -> Option<HoveredLogs> {
        let t = timestamp_at(self.origin, self.scale.t_at(x));
        let bin = (t.checked_sub(self.start)? / self.step) as usize;
        if self.activity.get(bin)?.log_count() == 0 {
            return None;
        }
        let start = self.start + self.step * bin as u64;
        let times = start..start + self.step;
        let most_severe = model.logs.most_severe_between(times.start, times.end)?;
        Some(HoveredLogs {
            most_severe: seconds_since(self.origin, most_severe.time),
            times,
        })
    }

    /// Paints a bar from the bottom of the track in `rect` for each bin: its log records
    /// in the colours of their levels, the most severe at the bottom, and its values
    /// in grey on top of them. Hatches the gaps.
    ///
    /// The bars are scaled to the highest of the complete bins, so that the newest
    /// one, which is still filling, grows up to the others instead of rescaling them,
    /// and changes of the scale under `id` are animated.
    fn paint(&self, ui: &egui::Ui, id: egui::Id, painter: &egui::Painter, rect: egui::Rect) {
        let tokens = ui.tokens();
        let room = rect.height() - 2.0;
        let log_room = room * LOG_BAR_SHARE;
        let (max_rate, max_logs) = self.scale_max();
        let ctx = ui.ctx();
        let max_rate = f64::from(ctx.animate_value_with_time(
            id.with("max values"),
            max_rate as f32,
            RESCALE_TIME,
        ));
        let max_logs =
            ctx.animate_value_with_time(id.with("max logs"), max_logs as f32, RESCALE_TIME);
        let bar = |x: egui::Rangef, bottom: f32, height: f32| {
            egui::Rect::from_x_y_ranges(x, bottom - height..=bottom)
        };

        let value_color = tokens.text_faint.gamma_multiply(0.45);
        for (bin, activity) in self.activity.iter().enumerate() {
            let x = self.bar_x(bin);
            let mut bottom = rect.bottom();
            let logs = activity.log_count();
            if logs > 0 {
                let height = (logs as f32 / max_logs).min(1.0) * log_room;
                for level in LogLevel::ALL.into_iter().rev() {
                    let count = activity.logs[level.as_index()];
                    if count == 0 {
                        continue;
                    }
                    let part = (count as f32 / logs as f32 * height).max(MIN_LOG_HEIGHT);
                    // warnings and errors stand out, the rest stays in the background
                    let alpha = if level >= LogLevel::Warn { 0.9 } else { 0.45 };
                    let color = theme::level_color(ui.visuals(), level).gamma_multiply(alpha);
                    painter.rect_filled(bar(x, bottom, part), 0.0, color);
                    bottom -= part;
                }
            }
            if activity.values > 0 {
                // the square root lets steady values show next to bursts
                let share = (self.rate(bin) / max_rate).min(1.0).sqrt() as f32;
                let height = (share * (room - log_room)).max(1.0);
                painter.rect_filled(bar(x, bottom, height), 0.0, value_color);
            }
        }

        // gaps only within the data, not in the bins it covers in part
        let complete = |bin: &usize| self.covered(*bin) >= 1.0;
        let (Some(first), Some(last)) = (
            (0..self.activity.len()).find(complete),
            (0..self.activity.len()).rfind(complete),
        ) else {
            return;
        };
        let counts: Vec<u32> = self.activity[first..=last]
            .iter()
            .map(|activity| activity.values + activity.log_count())
            .collect();
        let duration = seconds_since(0, self.step) * counts.len() as f64;
        let hatch = egui::Stroke::new(1.0, tokens.border_strong);
        for gap in gaps(&counts, duration) {
            let (start, _) = self.bin_times(first + gap.start);
            let (_, end) = self.bin_times(first + gap.end - 1);
            let x = self.scale.x_at(start)..=self.scale.x_at(end);
            let gap_rect = egui::Rect::from_x_y_ranges(x, rect.y_range()).intersect(rect);
            paint_hatching(&painter.with_clip_rect(gap_rect), gap_rect, hatch);
        }
    }
}

/// The runs of bins in `counts` without samples that last [`GAP_FACTOR`] times the
/// mean time between the samples or longer, for all bins spanning `duration` seconds.
fn gaps(counts: &[u32], duration: f64) -> Vec<Range<usize>> {
    let total: u64 = counts.iter().map(|&count| u64::from(count)).sum();
    if total < 2 || counts.is_empty() {
        return vec![];
    }
    let bin_duration = duration / counts.len() as f64;
    let mean_interval = duration / total as f64;
    let min_bins = ((GAP_FACTOR * mean_interval / bin_duration).ceil() as usize).max(1);
    let mut gaps = vec![];
    let mut start = None;
    for (bin, &count) in counts.iter().chain([&1]).enumerate() {
        match (count, start) {
            (0, None) => start = Some(bin),
            (1.., Some(from)) => {
                if bin - from >= min_bins {
                    gaps.push(from..bin);
                }
                start = None;
            }
            _ => {}
        }
    }
    gaps
}

/// Diagonal lines over `rect`, to paint with a `painter` clipped to it.
fn paint_hatching(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let height = rect.height();
    let mut x = rect.left() - height;
    while x < rect.right() {
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom()),
                egui::pos2(x + height, rect.top()),
            ],
            stroke,
        );
        x += HATCH_SPACING;
    }
}

/// Lists the log records of `model` of all paths in a bin, the first ones by time.
fn log_tooltip_ui(ui: &mut egui::Ui, model: &Model, times: Range<u64>) {
    let (first, count) = (model.logs).first_between(times.start, times.end, TOOLTIP_ENTRIES);
    let base = model.time_base().unwrap_or_default();
    for entry in first {
        ui.horizontal(|ui| {
            ui.weak(base.format(entry.time, 3));
            let level = egui::RichText::new(entry.level.as_str())
                .color(theme::level_color(ui.visuals(), entry.level))
                .monospace();
            ui.label(level);
            ui.label(&entry.message);
        });
    }
    if count > TOOLTIP_ENTRIES {
        ui.weak(format!("and {} more", count - TOOLTIP_ENTRIES));
    }
    ui.weak(if count > 1 {
        "Click to go to the most severe"
    } else {
        "Click to go there"
    });
}

/// A step between the labels of the ruler of at least `min` seconds: 1, 2 or 5 times a
/// power of ten, or whole minutes and hours that divide an hour or a day.
fn label_step(min: f64) -> f64 {
    const MINUTES: [f64; 7] = [60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0];
    const HOURS: [f64; 6] = [7200.0, 10800.0, 21600.0, 43200.0, 86400.0, 172800.0];
    if min > 30.0 {
        if let Some(step) = MINUTES.into_iter().chain(HOURS).find(|&step| step >= min) {
            return step;
        }
        return (min / 86400.0).ceil() * 86400.0;
    }
    let power = 10f64.powf(min.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|factor| factor * power)
        .find(|&step| step >= min)
        .unwrap_or(10.0 * power)
}

/// How many decimals of seconds tell apart times `resolution` seconds apart.
fn decimals_for(resolution: f64) -> usize {
    (-resolution.log10().floor()).clamp(0.0, 9.0) as usize
}

/// Paints the ticks and labels of the times on the ruler in `rect`, the labels left
/// of `labels_end`, the times counting from `origin` in nanoseconds.
fn paint_ruler(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    labels_end: f32,
    scale: &Scale,
    base: TimeBase,
    origin: u64,
) {
    let tokens = ui.tokens();
    let step = label_step(scale.resolution() * f64::from(LABEL_SPACING));
    let decimals = decimals_for(step);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let offset = base.display_offset(timestamp_at(origin, scale.start));
    let mut t = first_tick(scale.start, step, offset, origin);
    while t <= scale.end() {
        let x = scale.x_at(t);
        painter.vline(
            x,
            rect.bottom() - TICK_HEIGHT..=rect.bottom(),
            egui::Stroke::new(1.0, tokens.border_strong),
        );
        let text = base.format(timestamp_at(origin, t), decimals);
        let galley = painter.layout_no_wrap(text, font.clone(), tokens.text_faint);
        // right of the tick, if there is room
        let pos = egui::pos2(x + 3.0, rect.center().y - galley.size().y / 2.0 - 1.0);
        if pos.x + galley.size().x <= labels_end {
            painter.galley(pos, galley, tokens.text_faint);
        }
        t += step;
    }
}

/// The time of the first tick of the ruler at or after `start` seconds since `origin`
/// in nanoseconds: a multiple of `step` seconds of the shown time, which is `offset`
/// seconds ahead, e.g. a whole local hour.
fn first_tick(start: f64, step: f64, offset: f64, origin: u64) -> f64 {
    // how far the origin is past a multiple of the step, exactly
    let phase = seconds_since(0, origin % timestamp_at(0, step).max(1));
    ((start + phase + offset) / step).ceil() * step - phase - offset
}

/// Paints `text` in a small pill beside `x` on the ruler in `rect`, e.g. the time at
/// the pointer: right of it, or left of it near the end.
fn paint_pill(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, x: f32, text: &str) {
    let tokens = ui.tokens();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text_color = ui.visuals().strong_text_color();
    let galley = painter.layout_no_wrap(text.to_owned(), font, text_color);
    let size = galley.size() + egui::vec2(8.0, 0.0);
    let left = if x + PILL_OFFSET + size.x <= rect.right() {
        x + PILL_OFFSET
    } else {
        x - PILL_OFFSET - size.x
    };
    let pill = egui::Rect::from_min_size(egui::pos2(left, rect.center().y - size.y / 2.0), size);
    painter.rect(
        pill,
        3.0,
        ui.visuals().extreme_bg_color,
        egui::Stroke::new(1.0, tokens.border_strong),
        egui::StrokeKind::Inside,
    );
    painter.galley(pill.min + egui::vec2(4.0, 0.0), galley, text_color);
}

#[cfg(test)]
mod tests;
