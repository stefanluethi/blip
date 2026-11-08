//! The time range shared by all views, and the controls to play and pause it.
//!
//! Timestamps are integer nanoseconds as the targets send them. The time range and the
//! views count in seconds as floats, but from the origin of the data, the first
//! timestamp received, so that they keep nanoseconds also for Unix times.

use crate::app::model::Model;
use crate::app::theme::{self, TokensExt};
use egui_phosphor::regular;

mod timeline;

const NANOS_PER_SECOND: f64 = 1.0e9;
const MIN_WINDOW: f64 = 1.0e-3;
const MAX_WINDOW: f64 = 1.0e6;
/// Relative change of the duration the timeline shows per scrolled point.
const ZOOM_SPEED: f32 = 0.005;
/// Width of the field of the window length in points, wide enough for all its values.
const WINDOW_FIELD_WIDTH: f32 = 72.0;
/// Distance in points from the cursor line within which it can be grabbed.
const CURSOR_GRAB_DISTANCE: f32 = 5.0;
/// Width and height of the handle on top of the cursor line in points.
const CURSOR_HANDLE_SIZE: egui::Vec2 = egui::vec2(10.0, 6.0);

/// Seconds from `origin` to `timestamp`, both in nanoseconds, negative before it.
pub fn seconds_since(origin: u64, timestamp: u64) -> f64 {
    timestamp.wrapping_sub(origin) as i64 as f64 / NANOS_PER_SECOND
}

/// The timestamp `seconds` after `origin` in nanoseconds, 0 before the epoch.
pub fn timestamp_at(origin: u64, seconds: f64) -> u64 {
    origin.saturating_add_signed((seconds * NANOS_PER_SECOND).round() as i64)
}

/// Timestamps from this one on are taken as Unix time (after 2001-09-09), earlier ones
/// as time since the device booted.
const UNIX_TIME_THRESHOLD: u64 = 1_000_000_000 * 1_000_000_000;

/// What the timestamps count from, guessed from their value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeBase {
    /// Seconds since the Unix epoch, shown as local time.
    Unix,
    /// Seconds since the device booted.
    SinceBoot,
}

impl TimeBase {
    /// The time base of `timestamp` in nanoseconds.
    pub fn guess(timestamp: u64) -> Self {
        if timestamp >= UNIX_TIME_THRESHOLD {
            TimeBase::Unix
        } else {
            TimeBase::SinceBoot
        }
    }

    /// How many seconds the times that [`Self::format`] shows for `timestamp` are
    /// ahead of them: the offset of the local time zone from UTC for Unix time, else 0.
    pub fn display_offset(self, timestamp: u64) -> f64 {
        match self {
            TimeBase::Unix => {
                let seconds = (timestamp / 1_000_000_000) as i64;
                chrono::DateTime::from_timestamp(seconds, 0).map_or(0.0, |utc| {
                    let local: chrono::DateTime<chrono::Local> = utc.into();
                    f64::from(local.offset().local_minus_utc())
                })
            }
            TimeBase::SinceBoot => 0.0,
        }
    }

    /// Formats `timestamp` in nanoseconds as local time of day, or as hours, minutes
    /// and seconds since boot, with `decimals` decimals of the seconds, rounded.
    pub fn format(self, timestamp: u64, decimals: usize) -> String {
        self.format_as(timestamp, decimals, "%H:%M:%S")
    }

    /// Formats `timestamp` like [`Self::format`], local time with the date before it.
    pub fn format_dated(self, timestamp: u64, decimals: usize) -> String {
        self.format_as(timestamp, decimals, "%Y-%m-%d %H:%M:%S")
    }

    /// Formats `timestamp` like [`Self::format`], local time as `pattern` has it.
    fn format_as(self, timestamp: u64, decimals: usize, pattern: &str) -> String {
        let decimals = decimals.min(9);
        let unit = 10u64.pow(9 - decimals as u32);
        let rounded = timestamp.saturating_add(unit / 2) / unit * unit;
        let whole = rounded / 1_000_000_000;
        let fraction = if decimals > 0 {
            let digits = rounded % 1_000_000_000 / unit;
            format!(".{digits:0decimals$}")
        } else {
            String::new()
        };
        match self {
            TimeBase::Unix => match chrono::DateTime::from_timestamp(whole as i64, 0) {
                Some(utc) => {
                    let local: chrono::DateTime<chrono::Local> = utc.into();
                    format!("{}{fraction}", local.format(pattern))
                }
                None => format!("{timestamp}"),
            },
            TimeBase::SinceBoot => {
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
/// timestamps while playing. Its times are seconds since [`Self::origin`].
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TimeRange {
    window: f64,
    /// The first timestamp received in nanoseconds, from which the times count.
    #[serde(skip)]
    origin: u64,
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
    /// What the timeline shows when zoomed in, `None` to show all data.
    #[serde(skip)]
    overview: Option<timeline::Overview>,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            window: 20.0,
            origin: 0,
            end: None,
            playing: true,
            played_at: None,
            span: None,
            cursor: None,
            overview: None,
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
        let origin = model.origin().unwrap_or(0);
        if origin != self.origin {
            // other data, e.g. of an opened recording
            self.origin = origin;
            self.go_live();
            self.overview = None;
        }
        self.span = model
            .time_span()
            .map(|(first, latest)| (self.seconds(first), self.seconds(latest)));
        if self.span.is_none() {
            self.go_live();
            self.overview = None;
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

    /// The shown window in nanoseconds.
    pub fn timestamps(&self) -> Option<(u64, u64)> {
        self.range()
            .map(|(start, end)| (self.timestamp(start), self.timestamp(end)))
    }

    /// The time of the cursor in nanoseconds, see [`TimeRange::cursor`].
    pub fn cursor_timestamp(&self) -> Option<u64> {
        self.cursor().map(|t| self.timestamp(t))
    }

    /// Whether the user put the cursor somewhere, instead of it following the end of the
    /// window.
    pub fn cursor_is_set(&self) -> bool {
        self.cursor.is_some()
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

    /// Puts the cursor at `t` seconds, and the window around it unless it shows `t`.
    pub fn go_to(&mut self, t: f64) {
        if self
            .range()
            .is_some_and(|(start, end)| !(start..=end).contains(&t))
        {
            self.move_end(t + self.window / 2.0);
        }
        self.set_cursor(t);
    }

    /// Shows all data of `model`, e.g. of a loaded recording.
    pub fn fit(&mut self, model: &Model) {
        self.update_span(model);
        if let Some((first, latest)) = self.span {
            self.window = (latest - first).clamp(MIN_WINDOW, MAX_WINDOW);
        }
        self.overview = None;
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

    /// The first timestamp received in nanoseconds, from which the times count.
    pub fn origin(&self) -> u64 {
        self.origin
    }

    /// The time in seconds of `timestamp` in nanoseconds.
    pub fn seconds(&self, timestamp: u64) -> f64 {
        seconds_since(self.origin, timestamp)
    }

    /// The timestamp in nanoseconds of the time `seconds`.
    pub fn timestamp(&self, seconds: f64) -> u64 {
        timestamp_at(self.origin, seconds)
    }

    /// What the timestamps of the collected data count from.
    pub fn base(&self) -> TimeBase {
        TimeBase::guess(self.timestamp(self.latest().unwrap_or(0.0)))
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

    /// The controls of the time range over the data of `model`: what `leading` adds,
    /// playing and pausing, going live while `recording`, the window and the timeline.
    /// The controls line up with the track of the timeline, under its ruler. Returns
    /// what `leading` returns.
    pub fn ui<R>(
        &mut self,
        ui: &mut egui::Ui,
        model: &Model,
        recording: bool,
        leading: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let inner = ui
            .horizontal(|ui| {
                let inner = ui
                    .vertical(|ui| {
                        ui.add_space(timeline::ruler_height(ui));
                        ui.horizontal(|ui| {
                            let inner = leading(ui);
                            self.controls_ui(ui, recording);
                            inner
                        })
                        .inner
                    })
                    .inner;
                timeline::show(self, ui, model);
                inner
            })
            .inner;
        if self.playing && !self.is_live() {
            ui.ctx().request_repaint();
        }
        inner
    }

    /// The buttons to play, pause and go live, which only follows new data while
    /// `recording`, and the length of the window.
    fn controls_ui(&mut self, ui: &mut egui::Ui, recording: bool) {
        let (icon, hint) = if self.playing {
            (regular::PAUSE, "Pause")
        } else {
            (regular::PLAY, "Play at the pace of the timestamps")
        };
        if ui.button(icon).on_hover_text(hint).clicked() {
            self.toggle_play();
        }
        // filled with the accent while following the newest data
        let live = format!("{} Live", regular::BROADCAST);
        let following = recording && self.is_live();
        let live_button = if following {
            let tokens = ui.tokens();
            egui::Button::new(egui::RichText::new(live).color(tokens.on_accent))
                .fill(tokens.accent)
                .stroke(egui::Stroke::NONE)
        } else {
            egui::Button::new(live)
        }
        .selected(following);
        if ui
            .add_enabled(recording, live_button)
            .on_hover_text("Follow the newest data")
            .on_disabled_hover_text("Follows the newest data while recording")
            .clicked()
        {
            self.go_live();
        }

        let speed = self.window * 0.01;
        let window = egui::DragValue::new(&mut self.window)
            .range(MIN_WINDOW..=MAX_WINDOW)
            .speed(speed)
            .custom_formatter(|seconds, _| format_duration(seconds))
            .custom_parser(parse_duration);
        // a fixed width, so that the timeline beside it stays put
        let size = egui::vec2(WINDOW_FIELD_WIDTH, ui.spacing().interact_size.y);
        ui.add_sized(size, window)
            .on_hover_text("Window: the length of the shown range, e.g. 500 ms, 20 s or 5 min");
    }
}

/// Formats `seconds` with three significant digits in ms, s, min or h.
fn format_duration(seconds: f64) -> String {
    let (value, unit) = if seconds < 1.0 {
        (seconds * 1.0e3, "ms")
    } else if seconds < 60.0 {
        (seconds, "s")
    } else if seconds < 3600.0 {
        (seconds / 60.0, "min")
    } else {
        (seconds / 3600.0, "h")
    };
    let decimals = if value < 10.0 {
        2
    } else if value < 100.0 {
        1
    } else {
        0
    };
    format!("{value:.decimals$} {unit}")
}

/// Reads a duration in seconds as [`format_duration`] writes it, also without a unit
/// for seconds.
fn parse_duration(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text.find(|c: char| c.is_alphabetic()).unwrap_or(text.len());
    let value: f64 = text[..split].trim().parse().ok()?;
    let scale = match text[split..].trim() {
        "ms" => 1.0e-3,
        "" | "s" => 1.0,
        "m" | "min" => 60.0,
        "h" => 3600.0,
        _ => return None,
    };
    Some(value * scale)
}

/// Id of the flag that the timeline contained the pointer in the last pass.
fn timeline_hovered_id() -> egui::Id {
    egui::Id::new("timeline hovered")
}

/// Id of the mouse wheel scrolled over the timeline in points, see [`take_wheel`].
fn timeline_wheel_id() -> egui::Id {
    egui::Id::new("timeline wheel")
}

/// Takes the mouse wheel events of `raw_input` without modifiers or with Shift for
/// the timeline, if it contained the pointer in the last pass, before egui smooths
/// them: so that what the pointer moves on to does not scroll with the rest of a
/// notch. Shift scrolls sideways. The timeline takes them with [`timeline_wheel`].
pub fn take_wheel(ctx: &egui::Context, raw_input: &mut egui::RawInput) {
    let hovered = ctx.data_mut(|data| data.remove_temp::<bool>(timeline_hovered_id()));
    if hovered != Some(true) {
        return;
    }
    let line = ctx.options(|options| options.input_options.line_scroll_speed);
    let page = raw_input.screen_rect.map_or(500.0, |rect| rect.height());
    let mut wheel = egui::Vec2::ZERO;
    raw_input.events.retain(|event| {
        let egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        else {
            return true;
        };
        let shift = *modifiers == egui::Modifiers::SHIFT;
        if !modifiers.is_none() && !shift {
            return true;
        }
        let delta = match unit {
            egui::MouseWheelUnit::Point => *delta,
            egui::MouseWheelUnit::Line => *delta * line,
            egui::MouseWheelUnit::Page => *delta * page,
        };
        wheel += if shift {
            egui::vec2(delta.x + delta.y, 0.0)
        } else {
            delta
        };
        false
    });
    if wheel != egui::Vec2::ZERO {
        ctx.data_mut(|data| {
            *data.get_temp_mut_or_default::<egui::Vec2>(timeline_wheel_id()) += wheel;
        });
    }
}

/// Marks that the timeline contains the pointer, for [`take_wheel`] to take the
/// wheel for it, and returns what it took since the last call.
fn timeline_wheel(ui: &egui::Ui) -> egui::Vec2 {
    ui.data_mut(|data| {
        data.insert_temp(timeline_hovered_id(), true);
        data.remove_temp::<egui::Vec2>(timeline_wheel_id())
            .unwrap_or_default()
    })
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

/// How the cursor line of a view is drawn.
#[derive(Clone, Copy)]
pub struct CursorLook {
    /// Whether it is being dragged.
    pub grabbed: bool,
    /// Whether the user put it somewhere, see [`TimeRange::cursor_is_set`]. At the end
    /// of the window it is faint, as it only marks the time of the shown values.
    pub set: bool,
}

/// Draws the cursor line at `x` in `rect` with a handle on top, and remembers it under
/// `id` for [`cursor_grabbed`].
pub fn show_cursor(
    ui: &egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    x: Option<f32>,
    look: CursorLook,
) {
    let grabbed = look.grabbed;
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

    let width = if grabbed || hovered { 2.0 } else { 1.5 };
    let opacity = if look.set || grabbed || hovered {
        1.0
    } else {
        0.35
    };
    let mut painter = ui.painter_at(rect);
    painter.multiply_opacity(opacity);
    paint_cursor(&painter, x, rect.y_range(), width, ui.visuals());
}

/// Paints the cursor line at `x` over `y` with `width` and a handle on top.
fn paint_cursor(
    painter: &egui::Painter,
    x: f32,
    y: egui::Rangef,
    width: f32,
    visuals: &egui::Visuals,
) {
    let color = theme::cursor_color(visuals);
    painter.vline(x, y, egui::Stroke::new(width, color));
    let (half_width, height) = (CURSOR_HANDLE_SIZE.x / 2.0, CURSOR_HANDLE_SIZE.y);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x - half_width, y.min),
            egui::pos2(x + half_width, y.min),
            egui::pos2(x, y.min + height),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests;
