//! The time range shared by all views, and the controls to play and pause it.

use crate::app::model::Model;
use crate::app::theme;
use egui_phosphor::regular;

const NANOS_PER_SECOND: f64 = 1.0e9;
const MIN_WINDOW: f64 = 1.0e-3;
const MAX_WINDOW: f64 = 1.0e6;
const MIN_THUMB_WIDTH: f32 = 6.0;
/// Relative change of the window length per scrolled point.
const WINDOW_SCROLL_SPEED: f32 = 0.005;
/// Distance in points from the cursor line within which it can be grabbed.
const CURSOR_GRAB_DISTANCE: f32 = 5.0;
/// Width and height of the handle on top of the cursor line in points.
const CURSOR_HANDLE_SIZE: egui::Vec2 = egui::vec2(10.0, 6.0);

pub fn timestamp_to_seconds(timestamp: u64) -> f64 {
    timestamp as f64 / NANOS_PER_SECOND
}

/// Timestamps after this many seconds are taken as Unix time (after 2001-09-09), earlier
/// ones as time since the device booted.
const UNIX_TIME_THRESHOLD: f64 = 1.0e9;

/// What the timestamps count from, guessed from their value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeBase {
    /// Seconds since the Unix epoch, shown as local time.
    Unix,
    /// Seconds since the device booted.
    SinceBoot,
}

impl TimeBase {
    pub fn guess(seconds: f64) -> Self {
        if seconds >= UNIX_TIME_THRESHOLD {
            TimeBase::Unix
        } else {
            TimeBase::SinceBoot
        }
    }

    /// Formats `seconds` as local time of day, or as hours, minutes and seconds since
    /// boot, with `decimals` decimals of the seconds.
    pub fn format(self, seconds: f64, decimals: usize) -> String {
        let decimals = decimals.min(9);
        let scale = 10f64.powi(decimals as i32);
        // round the fraction alone, the whole seconds can be too large for exact decimals
        let mut whole = seconds.floor();
        let mut digits = ((seconds - whole) * scale).round();
        if digits >= scale {
            whole += 1.0;
            digits = 0.0;
        }
        let fraction = if decimals > 0 {
            format!(".{:0decimals$}", digits as u64)
        } else {
            String::new()
        };
        match self {
            TimeBase::Unix => match chrono::DateTime::from_timestamp(whole as i64, 0) {
                Some(utc) => {
                    let local: chrono::DateTime<chrono::Local> = utc.into();
                    format!("{}{fraction}", local.format("%H:%M:%S"))
                }
                None => format!("{seconds}"),
            },
            TimeBase::SinceBoot => {
                let whole = whole.max(0.0) as u64;
                format!(
                    "{}:{:02}:{:02}{fraction}",
                    whole / 3600,
                    whole / 60 % 60,
                    whole % 60
                )
            }
        }
    }
}

/// A window of `window` seconds shown by all views. It either follows the newest data
/// (live), or ends at a time that stays while paused and moves on at the pace of the
/// timestamps while playing.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TimeRange {
    window: f64,
    /// End of the window in seconds, or `None` to follow the newest data.
    #[serde(skip)]
    end: Option<f64>,
    /// Whether the window moves on in time, always while live.
    #[serde(skip)]
    playing: bool,
    /// When the window was last moved on while playing, in seconds as egui counts them.
    #[serde(skip)]
    played_at: Option<f64>,
    /// Times of the first and the newest collected data in seconds.
    #[serde(skip)]
    span: Option<(f64, f64)>,
    /// Time of the cursor in seconds, or `None` to keep it at the end of the window.
    #[serde(skip)]
    cursor: Option<f64>,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            window: 20.0,
            end: None,
            playing: true,
            played_at: None,
            span: None,
            cursor: None,
        }
    }
}

impl TimeRange {
    /// Updates the span of the collected data and moves the window on while playing,
    /// call once per frame before the views with the time `now` in seconds.
    pub fn update(&mut self, model: &Model, now: f64) {
        self.update_span(model);
        self.play_on(now);
    }

    fn update_span(&mut self, model: &Model) {
        self.span = model
            .time_span()
            .map(|(first, latest)| (timestamp_to_seconds(first), timestamp_to_seconds(latest)));
        if self.span.is_none() {
            self.go_live();
        }
    }

    /// Moves the window and the cursor on by the time passed since the last call while
    /// playing, and follows the newest data once the window reaches it.
    fn play_on(&mut self, now: f64) {
        let last = self.played_at.take();
        let (true, Some(end), Some((_, latest))) = (self.playing, self.end, self.span) else {
            return;
        };
        self.played_at = Some(now);
        let step = last.map_or(0.0, |last| (now - last).max(0.0));
        if end + step >= latest {
            self.go_live();
        } else {
            self.end = Some(end + step);
            self.cursor = self.cursor.map(|cursor| cursor + step);
        }
    }

    /// The time of the cursor in seconds, shown by all views and the time of the values
    /// of views that show one sample per trace: where the user put it, or else the end
    /// of the window.
    pub fn cursor(&self) -> Option<f64> {
        self.cursor.or_else(|| self.range().map(|(_, end)| end))
    }

    /// Puts the cursor at `t` seconds within the collected data. This pauses, so the
    /// cursor stays in the window.
    pub fn set_cursor(&mut self, t: f64) {
        let Some((first, latest)) = self.span else {
            return;
        };
        self.pause();
        self.cursor = Some(t.clamp(first, latest));
    }

    /// Shows all data of `model`, e.g. of a loaded recording.
    pub fn fit(&mut self, model: &Model) {
        self.update_span(model);
        if let Some((first, latest)) = self.span {
            self.window = (latest - first).clamp(MIN_WINDOW, MAX_WINDOW);
        }
        self.go_live();
    }

    /// Follows the newest data with the window and the cursor.
    pub fn go_live(&mut self) {
        self.end = None;
        self.cursor = None;
        self.playing = true;
    }

    /// Stops the window where it is, also while live.
    fn pause(&mut self) {
        if self.is_live() {
            self.end = self.latest();
        }
        self.playing = false;
    }

    /// Pauses while playing, else moves the window on from where it is.
    fn toggle_play(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.playing = true;
        }
    }

    /// Whether the window moves on, following the newest data or at the pace of the
    /// timestamps.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Time of the newest collected data in seconds.
    pub fn latest(&self) -> Option<f64> {
        self.span.map(|(_, latest)| latest)
    }

    /// Time of the first collected data in seconds, used as origin of the time axes.
    pub fn origin(&self) -> f64 {
        self.span.map_or(0.0, |(first, _)| first)
    }

    /// What the timestamps of the collected data count from.
    pub fn base(&self) -> TimeBase {
        TimeBase::guess(self.span.map_or(0.0, |(_, latest)| latest))
    }

    pub fn is_live(&self) -> bool {
        self.end.is_none()
    }

    /// The shown window `[start, end]` in seconds, or `None` if there is no data yet.
    pub fn range(&self) -> Option<(f64, f64)> {
        let (_, latest) = self.span?;
        let end = self.end.unwrap_or(latest);
        Some((end - self.window, end))
    }

    /// Applies a pan or zoom of a plot to `[start, end]` seconds. Zooming keeps
    /// following the newest data in live mode and keeps playing, panning pauses. Zooming
    /// beyond the shortest or longest window changes nothing.
    pub fn pan_zoom(&mut self, start: f64, end: f64) {
        let changed = |window: f64| (window - self.window).abs() > self.window * 1.0e-6;
        let zoomed = changed(end - start);
        let window = (end - start).clamp(MIN_WINDOW, MAX_WINDOW);
        if zoomed && !changed(window) {
            return;
        }
        self.window = window;
        if !zoomed {
            self.move_end(end);
        } else if !self.is_live() {
            let playing = self.playing;
            self.set_end(end);
            if !playing {
                // reaching the newest data stays paused there
                self.pause();
            }
        }
    }

    /// Pauses and moves the end of the window to `end` seconds, see [`Self::set_end`].
    fn move_end(&mut self, end: f64) {
        self.pause();
        self.set_end(end);
    }

    /// Moves the end of the window to `end` seconds, switches to live mode if that is
    /// at or after the newest data and the cursor is not set.
    fn set_end(&mut self, end: f64) {
        match self.span {
            Some((first, latest)) if end < latest => {
                self.end = Some(end.max(first + self.window.min(latest - first)));
            }
            Some((_, latest)) if self.cursor.is_some() => self.end = Some(latest),
            _ => self.go_live(),
        }
    }

    /// The controls of the time range: playing and pausing, the window and the
    /// timeline.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (icon, hint) = if self.playing {
                (regular::PAUSE, "Pause")
            } else {
                (regular::PLAY, "Play at the pace of the timestamps")
            };
            if ui.button(icon).on_hover_text(hint).clicked() {
                self.toggle_play();
            }
            let live_button =
                egui::Button::new(format!("{} Live", regular::BROADCAST)).selected(self.is_live());
            if ui
                .add(live_button)
                .on_hover_text("Follow the newest data")
                .clicked()
            {
                self.go_live();
            }

            ui.label("Window:");
            let speed = self.window * 0.01;
            ui.add(
                egui::DragValue::new(&mut self.window)
                    .range(MIN_WINDOW..=MAX_WINDOW)
                    .suffix(" s")
                    .speed(speed),
            );

            self.timeline_ui(ui);
        });
        if self.playing && !self.is_live() {
            ui.ctx().request_repaint();
        }
    }

    /// A track over the collected data with a thumb for the shown window and the
    /// cursor. Dragging the thumb moves the window, dragging the cursor moves the cursor,
    /// clicking the track centers the window there, and scrolling over it changes the
    /// window length.
    fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
        let id = ui.id().with("timeline");
        let cursor_grabbed = cursor_grabbed(ui, id);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                // scrolling up shows a shorter window
                self.window = (self.window * f64::from(-scroll * WINDOW_SCROLL_SPEED).exp())
                    .clamp(MIN_WINDOW, MAX_WINDOW);
            }
        }

        let Some((first, latest)) = self.span else {
            ui.painter()
                .rect_filled(rect, 3.0, theme::tokens(ui.visuals()).canvas);
            return;
        };
        let duration = (latest - first).max(MIN_WINDOW);
        let x_at = |t: f64| rect.left() + ((t - first) / duration) as f32 * rect.width();
        let thumb_at = |(start, end): (f64, f64)| {
            let thumb =
                egui::Rect::from_x_y_ranges(x_at(start.max(first))..=x_at(end), rect.y_range());
            thumb.expand2(egui::vec2(
                ((MIN_THUMB_WIDTH - thumb.width()) / 2.0).max(0.0),
                0.0,
            ))
        };

        let t_at = |x: f32| first + ((x - rect.left()) / rect.width()) as f64 * duration;
        if cursor_grabbed {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.set_cursor(t_at(pos.x));
            }
        } else if let (Some(pos), Some(range)) = (response.interact_pointer_pos(), self.range()) {
            let (_, end) = range;
            let jump =
                (response.clicked() || response.drag_started()) && !thumb_at(range).contains(pos);
            if jump {
                self.move_end(t_at(pos.x) + self.window / 2.0);
            } else if response.dragged() {
                self.move_end(end + (response.drag_delta().x / rect.width()) as f64 * duration);
            }
        }

        let visuals = ui.visuals();
        ui.painter()
            .rect_filled(rect, 3.0, theme::tokens(visuals).canvas);
        if let Some(range) = self.range() {
            let thumb_visuals = ui.style().interact(&response);
            ui.painter().rect(
                thumb_at(range),
                3.0,
                visuals.selection.bg_fill,
                thumb_visuals.fg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        let cursor_x = self.cursor().map(x_at);
        show_cursor(ui, id, rect, cursor_x, cursor_grabbed);
    }
}

/// Where the cursor line of a time plot or the timeline was shown, to grab it.
#[derive(Clone, Copy)]
struct CursorLine {
    /// The rect of the plot or the timeline.
    rect: egui::Rect,
    x: Option<f32>,
    /// Whether the line is dragged.
    grabbed: bool,
}

impl CursorLine {
    fn is_near(&self, pos: egui::Pos2) -> bool {
        self.rect.contains(pos)
            && self
                .x
                .is_some_and(|x| (pos.x - x).abs() <= CURSOR_GRAB_DISTANCE)
    }
}

/// Whether the cursor line shown with [`show_cursor`] under `id` is dragged: from a
/// press near it until the release. Call before the plot or timeline handles the
/// pointer, so it does not also pan.
pub fn cursor_grabbed(ui: &egui::Ui, id: egui::Id) -> bool {
    let Some(mut line) = ui.data(|data| data.get_temp::<CursorLine>(id)) else {
        return false;
    };
    let (pressed, down, origin) = ui.input(|input| {
        let pointer = &input.pointer;
        (
            pointer.primary_pressed(),
            pointer.primary_down(),
            pointer.press_origin(),
        )
    });
    if pressed {
        line.grabbed = origin.is_some_and(|origin| line.is_near(origin))
            // not on a popup above it
            && origin
                .and_then(|origin| ui.ctx().layer_id_at(origin))
                .is_none_or(|layer| layer == ui.layer_id());
    } else if !down {
        line.grabbed = false;
    }
    ui.data_mut(|data| data.insert_temp(id, line));
    line.grabbed
}

/// Draws the cursor line at `x` in `rect` with a handle on top, and remembers it under
/// `id` for [`cursor_grabbed`].
pub fn show_cursor(ui: &egui::Ui, id: egui::Id, rect: egui::Rect, x: Option<f32>, grabbed: bool) {
    let x = x.filter(|x| rect.x_range().contains(*x));
    let line = CursorLine { rect, x, grabbed };
    ui.data_mut(|data| data.insert_temp(id, line));
    let Some(x) = x else {
        return;
    };
    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|pos| line.is_near(pos) && ui.rect_contains_pointer(rect));
    if grabbed || hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    let color = ui.visuals().selection.stroke.color;
    let width = if grabbed || hovered { 2.0 } else { 1.5 };
    let painter = ui.painter_at(rect);
    painter.vline(x, rect.y_range(), egui::Stroke::new(width, color));
    let (half_width, height) = (CURSOR_HANDLE_SIZE.x / 2.0, CURSOR_HANDLE_SIZE.y);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x - half_width, rect.top()),
            egui::pos2(x + half_width, rect.top()),
            egui::pos2(x, rect.top() + height),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time_range(first: f64, latest: f64, window: f64) -> TimeRange {
        TimeRange {
            window,
            span: Some((first, latest)),
            ..TimeRange::default()
        }
    }

    #[test]
    fn time_base_is_guessed_from_the_value() {
        assert_eq!(TimeBase::guess(1.7e9), TimeBase::Unix);
        assert_eq!(TimeBase::guess(3600.0), TimeBase::SinceBoot);
    }

    #[test]
    fn time_since_boot_is_formatted_as_hours_minutes_seconds() {
        assert_eq!(TimeBase::SinceBoot.format(3723.4567, 3), "1:02:03.457");
        assert_eq!(TimeBase::SinceBoot.format(12.3, 1), "0:00:12.3");
        assert_eq!(TimeBase::SinceBoot.format(59.96, 1), "0:01:00.0");
        assert_eq!(TimeBase::SinceBoot.format(90000.0, 1), "25:00:00.0");
    }

    #[test]
    fn unix_time_is_formatted_as_time_of_day() {
        let text = TimeBase::Unix.format(1.7e9 + 0.25, 2);
        assert_eq!(text.len(), "hh:mm:ss.25".len());
        assert!(text.ends_with(":20.25"));
    }

    #[test]
    fn live_window_ends_at_newest_data() {
        let time = time_range(100.0, 150.0, 20.0);
        assert_eq!(time.range(), Some((130.0, 150.0)));
    }

    #[test]
    fn panning_back_leaves_live_mode_and_panning_to_the_end_returns() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(120.0, 140.0);
        assert!(!time.is_live());
        assert_eq!(time.range(), Some((120.0, 140.0)));
        time.pan_zoom(135.0, 155.0);
        assert!(time.is_live());
        assert_eq!(time.range(), Some((130.0, 150.0)));
    }

    #[test]
    fn zooming_keeps_live_mode() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(135.0, 145.0);
        assert!(time.is_live());
        assert_eq!(time.range(), Some((140.0, 150.0)));
    }

    #[test]
    fn zooming_beyond_the_window_limits_neither_pans_nor_pauses() {
        let mut time = time_range(100.0, 150.0, MIN_WINDOW);
        time.pan_zoom(149.0, 149.0 + MIN_WINDOW / 2.0);
        assert!(time.is_live());
        assert_eq!(time.range(), Some((150.0 - MIN_WINDOW, 150.0)));

        let mut time = time_range(100.0, 150.0, MAX_WINDOW);
        time.pause();
        let range = time.range();
        time.pan_zoom(120.0 - MAX_WINDOW, 130.0 + MAX_WINDOW);
        assert_eq!(time.range(), range);
    }

    #[test]
    fn cursor_is_at_the_window_end_until_set() {
        let mut time = time_range(100.0, 150.0, 20.0);
        assert_eq!(time.cursor(), Some(150.0));
        time.pan_zoom(120.0, 140.0);
        assert_eq!(time.cursor(), Some(140.0));
        time.set_cursor(125.0);
        assert_eq!(time.cursor(), Some(125.0));
        time.set_cursor(10.0);
        assert_eq!(time.cursor(), Some(100.0));
    }

    #[test]
    fn setting_the_cursor_stops_live_mode_until_going_live() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.set_cursor(140.0);
        assert!(!time.is_live());
        assert_eq!(time.range(), Some((130.0, 150.0)));
        // newer data does not move the window away from the cursor
        time.span = Some((100.0, 160.0));
        assert_eq!(time.range(), Some((130.0, 150.0)));
        assert_eq!(time.cursor(), Some(140.0));

        // until going live, also by panning to the end
        time.pan_zoom(145.0, 165.0);
        assert!(!time.is_live());
        assert_eq!(time.range(), Some((140.0, 160.0)));
        time.go_live();
        assert!(time.is_live());
        assert_eq!(time.cursor(), Some(160.0));
    }

    #[test]
    fn window_end_stays_within_data() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(50.0, 70.0);
        assert_eq!(time.range(), Some((100.0, 120.0)));
    }

    #[test]
    fn a_paused_window_stays_while_data_arrives_and_plays_on_from_there() {
        let mut time = time_range(100.0, 150.0, 20.0);
        assert!(time.playing);
        time.toggle_play();
        assert!(!time.is_live() && !time.playing);
        time.span = Some((100.0, 160.0));
        time.play_on(1.0);
        time.play_on(3.0);
        assert_eq!(time.range(), Some((130.0, 150.0)));

        time.toggle_play();
        time.play_on(4.0);
        time.play_on(6.5);
        assert!(time.playing && !time.is_live());
        assert_eq!(time.range(), Some((132.5, 152.5)));
        // the time while paused is skipped
        time.toggle_play();
        time.play_on(10.0);
        time.toggle_play();
        time.play_on(20.0);
        time.play_on(21.0);
        assert_eq!(time.range(), Some((133.5, 153.5)));
    }

    #[test]
    fn playing_moves_the_cursor_along_and_goes_live_at_the_newest_data() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(120.0, 140.0);
        time.set_cursor(130.0);
        time.toggle_play();
        time.play_on(0.0);
        time.play_on(5.0);
        assert_eq!(time.range(), Some((125.0, 145.0)));
        assert_eq!(time.cursor(), Some(135.0));
        time.play_on(10.0);
        assert!(time.is_live() && time.playing);
        assert_eq!(time.cursor(), Some(150.0));
    }

    #[test]
    fn panning_and_setting_the_cursor_pause_and_zooming_plays_on() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(120.0, 140.0);
        assert!(!time.playing);
        time.toggle_play();
        time.pan_zoom(130.0, 140.0);
        assert!(time.playing);
        assert_eq!(time.range(), Some((130.0, 140.0)));
        time.set_cursor(135.0);
        assert!(!time.playing);
        // setting the cursor while live pauses at the newest data
        let mut time = time_range(100.0, 150.0, 20.0);
        time.set_cursor(140.0);
        assert!(!time.playing);
        assert_eq!(time.range(), Some((130.0, 150.0)));
    }

    #[test]
    fn zooming_while_paused_stays_paused_also_at_the_newest_data() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(120.0, 140.0);
        time.pan_zoom(135.0, 165.0);
        assert!(!time.playing && !time.is_live());
        assert_eq!(time.range(), Some((120.0, 150.0)));
    }

    #[test]
    fn the_buttons_pause_and_go_live() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 40.0))
            .build_ui_state(
                |ui, time: &mut TimeRange| time.ui(ui),
                time_range(100.0, 150.0, 20.0),
            );
        harness.run();
        harness.get_by_label(regular::PAUSE).click();
        harness.run();
        assert!(!harness.state().playing);
        assert_eq!(harness.state().range(), Some((130.0, 150.0)));
        // playing repaints each frame
        harness.get_by_label(regular::PLAY).click();
        harness.run_steps(2);
        assert!(harness.state().playing);

        harness.get_by_label(regular::PAUSE).click();
        harness.step();
        harness.run();
        harness.get_by_label_contains("Live").click();
        harness.run();
        assert!(harness.state().is_live() && harness.state().playing);
    }
}
