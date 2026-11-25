//! The time range shared by all views, and the controls to play and pause it.
//!
//! Timestamps are integer nanoseconds as the targets send them. The time range and the
//! views count in seconds as floats, but from the origin of the data, the first
//! timestamp received, so that they keep nanoseconds also for Unix times.

use crate::store::Store;
use crate::widgets::theme::{self, TokensExt};
use egui_phosphor::regular;

mod timeline;

pub use crate::store::time_base::{NANOS_PER_SECOND, TimeBase, seconds_since, timestamp_at};
const MIN_WINDOW: f64 = 1.0e-3;
const MAX_WINDOW: f64 = 1.0e6;
/// Relative change of the duration the timeline shows per scrolled point.
const ZOOM_SPEED: f32 = 0.005;
/// Width of the field of the window length in points, wide enough for all its values.
const WINDOW_FIELD_WIDTH: f32 = 72.0;
/// Width of the caret of the window length menu at the right end of its field.
const WINDOW_CARET_WIDTH: f32 = 20.0;
/// Distance in points from the cursor line within which it can be grabbed.
const CURSOR_GRAB_DISTANCE: f32 = 5.0;
/// Width and height of the handle on top of the cursor line in points.
const CURSOR_HANDLE_SIZE: egui::Vec2 = egui::vec2(10.0, 6.0);

/// A window of `window` seconds shown by all views. It either follows the newest data
/// (live), or ends at a time that stays while paused and moves on at the pace of the
/// timestamps while playing. Its times are seconds since [`Self::origin`].
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct TimeRange {
    window: f64,
    /// The first timestamp received in nanoseconds, from which the times count.
    #[serde(skip)]
    origin: u64,
    /// What the timestamps count from, as the store has it.
    #[serde(skip)]
    base: TimeBase,
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
    /// Whether a connection is open, for the hint of the empty timeline.
    #[serde(skip)]
    connected: bool,
    /// Whether the received samples are recorded, as the controls were last shown.
    #[serde(skip)]
    recording: bool,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            window: 20.0,
            origin: 0,
            base: TimeBase::default(),
            end: None,
            playing: true,
            played_at: None,
            span: None,
            cursor: None,
            overview: None,
            connected: false,
            recording: false,
        }
    }
}

impl TimeRange {
    /// Updates the span of the collected data and moves the window on while playing,
    /// call once per frame before the views with the time `now` in seconds.
    pub fn update(&mut self, store: &dyn Store, now: f64) {
        self.update_span(store);
        self.play_on(now);
    }

    fn update_span(&mut self, store: &dyn Store) {
        self.base = store.time_base().unwrap_or_default();
        let origin = store.origin().unwrap_or(0);
        if origin != self.origin {
            // other data, e.g. of an opened recording
            self.origin = origin;
            self.go_live();
            self.overview = None;
        }
        self.span = store
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

    /// Shows all data of `store`, e.g. of a loaded recording.
    pub fn fit(&mut self, store: &dyn Store) {
        self.update_span(store);
        if let Some((first, latest)) = self.span {
            self.window = (latest - first).clamp(MIN_WINDOW, MAX_WINDOW);
        }
        self.overview = None;
        self.go_live();
    }

    /// Puts the cursor back at the end of the window, which it follows again, e.g. on
    /// Escape. The window stays where it is.
    pub fn release_cursor(&mut self) {
        self.cursor = None;
    }

    /// Tells the timeline whether a connection is open, so that its hint without data
    /// says what to do: record, or connect or open a recording.
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
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

    /// Whether the shown window moves on: following the newest data while `recording`,
    /// or playing from where it was paused. Live without recording, nothing moves.
    fn moves(&self, recording: bool) -> bool {
        self.playing && (recording || !self.is_live())
    }

    /// Does what the play and pause button does, e.g. for Space: pauses while the window
    /// moves on, else plays on from where it is. Does nothing without data or at the end
    /// of the data while not recording, when the button is disabled.
    pub fn toggle_play(&mut self) {
        if self.can_play(self.recording) {
            self.toggle_play_as(self.recording);
        }
    }

    /// Pauses while the window moves with `recording`, see [`Self::moves`], else plays
    /// on from where it is.
    fn toggle_play_as(&mut self, recording: bool) {
        if self.moves(recording) {
            self.pause();
        } else {
            self.playing = true;
        }
    }

    /// Whether there is something to play or pause: data, and the window moves or
    /// stopped before the end of the data.
    fn can_play(&self, recording: bool) -> bool {
        self.span.is_some() && (self.moves(recording) || !self.is_live())
    }

    /// Whether going live changes anything: while `recording` it follows the newest
    /// data, else it shows the end of the data and releases the cursor.
    fn can_go_live(&self, recording: bool) -> bool {
        recording || !self.is_live() || self.cursor_is_set()
    }

    /// How far the end of the window is behind the newest data in seconds, while it
    /// does not follow it.
    fn behind(&self) -> Option<f64> {
        let (end, latest) = (self.end?, self.latest()?);
        Some((latest - end).max(0.0))
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
        self.base
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

    /// The controls of the time range over the data of `store`: what `leading` adds,
    /// playing and pausing, going live while `recording`, the window and the timeline.
    /// The controls line up with the track of the timeline, under its ruler. Returns
    /// what `leading` returns.
    pub fn ui<R>(
        &mut self,
        ui: &mut egui::Ui,
        store: &dyn Store,
        recording: bool,
        leading: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        ui.horizontal(|ui| {
            let inner = ui
                .vertical(|ui| {
                    let band_size = egui::vec2(0.0, timeline::ruler_height(ui));
                    let band = ui.allocate_space(band_size).1;
                    let (inner, live) = ui
                        .horizontal(|ui| {
                            let inner = leading(ui);
                            let live = self.controls(ui, recording);
                            (inner, live)
                        })
                        .inner;
                    // in the band above the controls, so that nothing moves
                    self.paint_behind(ui, band, live.left(), recording);
                    inner
                })
                .inner;
            timeline::show(self, ui, store);
            inner
        })
        .inner
    }

    /// The timeline alone: all the data of `store` with what arrived when, the shown
    /// window to move and the cursor, for an application that lays out the controls
    /// itself. Use [`Self::ui`] for the timeline with the controls beside it.
    pub fn timeline_ui(&mut self, ui: &mut egui::Ui, store: &dyn Store) {
        timeline::show(self, ui, store);
        self.repaint_while_playing(ui);
    }

    /// Asks for the next frame while the window plays on, which moves it.
    fn repaint_while_playing(&self, ui: &egui::Ui) {
        if self.playing && !self.is_live() {
            ui.ctx().request_repaint();
        }
    }

    /// The buttons to play, pause and go live, which only follows new data while
    /// `recording`, and the length of the window.
    pub fn controls_ui(&mut self, ui: &mut egui::Ui, recording: bool) {
        self.controls(ui, recording);
    }

    /// Shows the controls, see [`Self::controls_ui`], and returns the rect of the Live
    /// button.
    fn controls(&mut self, ui: &mut egui::Ui, recording: bool) -> egui::Rect {
        self.recording = recording;
        self.repaint_while_playing(ui);
        let moves = self.moves(recording);
        let (icon, hint) = if moves {
            (regular::PAUSE, "Pause")
        } else {
            (regular::PLAY, "Play at the pace of the timestamps")
        };
        let cannot_play = if self.span.is_some() {
            "At the end of the data: move the window back to play from there"
        } else {
            "Nothing to play yet"
        };
        if ui
            .add_enabled(self.can_play(recording), theme::icon_button(icon))
            .on_hover_text(hint)
            .on_disabled_hover_text(cannot_play)
            .clicked()
        {
            self.toggle_play_as(recording);
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
        let (hint, disabled_hint) = if recording {
            ("Follow the newest data", "")
        } else if self.span.is_some() {
            (
                "Follow the end of the recording",
                "Shows the end of the recording",
            )
        } else {
            ("", "Follows the newest data while recording")
        };
        let live = ui
            .add_enabled(self.can_go_live(recording), live_button)
            .on_hover_text(hint)
            .on_disabled_hover_text(disabled_hint);
        if live.clicked() {
            self.go_live();
        }
        self.window_ui(ui);
        live.rect
    }

    /// Paints how far the window is behind the newest data in `band` from `left` on,
    /// while `recording` and not live: the cue that it does not follow.
    fn paint_behind(&self, ui: &egui::Ui, band: egui::Rect, left: f32, recording: bool) {
        let Some(behind) = self.behind().filter(|_| recording) else {
            return;
        };
        let state = if self.playing { "Playing" } else { "Paused" };
        let text = format!("{state}, {} behind", format_duration(behind));
        let font = egui::FontId::proportional(11.0);
        let color = ui.visuals().weak_text_color();
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(text, font, color);
        let pos = egui::pos2(left, band.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, color);
    }

    /// The field of the length of the window, with a menu of round lengths beside it.
    /// An entry that is not a length stays in the field, marked, until it is replaced.
    fn window_ui(&mut self, ui: &mut egui::Ui) {
        let id = ui.id().with("window length");
        let edit = ui
            .data(|data| data.get_temp::<WindowEdit>(id))
            .filter(|edit| {
                // the mark of a limited length goes once the length changes
                !matches!(edit, WindowEdit::Limited(window) if *window != self.window)
            });
        let mut text = match &edit {
            Some(WindowEdit::Editing(text, _) | WindowEdit::Rejected(text)) => text.clone(),
            _ => format_duration(self.window),
        };
        // the length the text was written from, to tell whether the user changed it
        let written = match &edit {
            Some(WindowEdit::Editing(_, written)) => *written,
            _ => self.window,
        };
        // one control of a fixed width, so that the timeline beside it stays put: the
        // field with the caret of the menu inside it at the right, as in a combo box
        let height = ui.spacing().interact_size.y;
        let size = egui::vec2(WINDOW_FIELD_WIDTH + WINDOW_CARET_WIDTH, height);
        let rect = ui.allocate_exact_size(size, egui::Sense::hover()).0;
        let mut margin = theme::FIELD_MARGIN;
        margin.right += WINDOW_CARET_WIDTH as i8;
        let field = theme::field(&mut text)
            .id(id.with("field"))
            .margin(margin)
            .horizontal_align(egui::Align::Center)
            .vertical_align(egui::Align::Center);
        let response = ui.put(rect, field);
        let edit = if response.lost_focus() {
            // text the user did not change leaves the length as it is now, e.g. zoomed
            let unchanged = text == format_duration(written);
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                None
            } else if unchanged {
                // the mark of an entry before stays
                edit.filter(|edit| !matches!(edit, WindowEdit::Editing(..)))
            } else {
                match parse_duration(&text) {
                    Some(window) => {
                        self.window = window.clamp(MIN_WINDOW, MAX_WINDOW);
                        (self.window != window).then_some(WindowEdit::Limited(self.window))
                    }
                    None => Some(WindowEdit::Rejected(text)),
                }
            }
        } else if response.has_focus() {
            Some(WindowEdit::Editing(text, written))
        } else {
            edit
        };
        let hint = match &edit {
            Some(WindowEdit::Rejected(_)) => format!(
                "Not a length: write a number with {DURATION_UNITS}, e.g. 500 ms, 20 s or 5 min"
            ),
            Some(WindowEdit::Limited(_)) => format!(
                "The window is {} at least and {} at most",
                format_duration(MIN_WINDOW),
                format_duration(MAX_WINDOW)
            ),
            _ => "Window: the length of the shown range, e.g. 500 ms, 20 s or 5 min".to_owned(),
        };
        if matches!(edit, Some(WindowEdit::Rejected(_) | WindowEdit::Limited(_))) {
            let stroke = egui::Stroke::new(1.0, ui.tokens().error);
            let radius = theme::CONTROL_RADIUS;
            let painter = ui.painter();
            painter.rect_stroke(response.rect, radius, stroke, egui::StrokeKind::Inside);
        }
        response.on_hover_text(hint);
        ui.data_mut(|data| {
            if let Some(edit) = edit {
                data.insert_temp(id, edit);
            } else {
                data.remove::<WindowEdit>(id);
            }
        });

        // the caret inside the right end of the field, within its stroke
        let caret = egui::Rect::from_min_max(
            egui::pos2(rect.right() - WINDOW_CARET_WIDTH, rect.top()),
            rect.right_bottom(),
        )
        .shrink(2.0);
        let builder =
            egui::UiBuilder::new()
                .max_rect(caret)
                .layout(egui::Layout::centered_and_justified(
                    egui::Direction::LeftToRight,
                ));
        ui.scope_builder(builder, |ui| {
            ui.spacing_mut().button_padding.x = 0.0;
            let button = theme::icon_button(regular::CARET_DOWN).button(ui);
            egui::containers::menu::MenuButton::from_button(button)
                .ui(ui, |ui| {
                    for &(seconds, label) in WINDOW_PRESETS {
                        let chosen = (self.window - seconds).abs() < seconds * 1.0e-9;
                        if ui.add(theme::selectable(chosen, label)).clicked() {
                            self.window = seconds;
                            ui.data_mut(|data| data.remove::<WindowEdit>(id));
                            ui.close();
                        }
                    }
                })
                .0
                .on_hover_text("Choose a window length");
        });
    }
}

/// What the field of the window length holds apart from the length.
#[derive(Clone)]
enum WindowEdit {
    /// The text while the field has the focus, and the length it was written from.
    Editing(String, f64),
    /// A text that is not a length, kept to correct it.
    Rejected(String),
    /// The length an entry was limited to, marked as long as it stays.
    Limited(f64),
}

/// The units [`parse_duration`] reads, for the hint of a rejected entry.
const DURATION_UNITS: &str = "µs, ms, s, min, h or d";

/// The window lengths to choose from, in steps of 1, 2 and 5.
const WINDOW_PRESETS: &[(f64, &str)] = &[
    (0.01, "10 ms"),
    (0.02, "20 ms"),
    (0.05, "50 ms"),
    (0.1, "100 ms"),
    (0.2, "200 ms"),
    (0.5, "500 ms"),
    (1.0, "1 s"),
    (2.0, "2 s"),
    (5.0, "5 s"),
    (10.0, "10 s"),
    (20.0, "20 s"),
    (60.0, "1 min"),
    (120.0, "2 min"),
    (300.0, "5 min"),
    (600.0, "10 min"),
    (1200.0, "20 min"),
    (3600.0, "1 h"),
];

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
/// for seconds, with the units of [`DURATION_UNITS`] and their usual spellings, in any
/// case. Gives `None` for anything else, and for negative durations.
fn parse_duration(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text.find(|c: char| c.is_alphabetic()).unwrap_or(text.len());
    let value: f64 = text[..split].trim().parse().ok()?;
    let scale = match text[split..].trim().to_lowercase().as_str() {
        "us" | "µs" | "μs" => 1.0e-6,
        "ms" => 1.0e-3,
        "" | "s" | "sec" => 1.0,
        "m" | "min" => 60.0,
        "h" | "hr" => 3600.0,
        "d" => 86400.0,
        _ => return None,
    };
    let seconds = value * scale;
    (seconds.is_finite() && seconds >= 0.0).then_some(seconds)
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
/// notch. Shift scrolls sideways. The timeline takes them with `timeline_wheel`.
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
