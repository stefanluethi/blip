//! The time range shared by all views.

use crate::app::model::Model;

const NANOS_PER_SECOND: f64 = 1.0e9;
const MIN_WINDOW: f64 = 1.0e-3;
const MAX_WINDOW: f64 = 1.0e6;
const MIN_THUMB_WIDTH: f32 = 6.0;
/// Relative change of the window length per scrolled point.
const WINDOW_SCROLL_SPEED: f32 = 0.005;

pub fn timestamp_to_seconds(timestamp: u64) -> f64 {
    timestamp as f64 / NANOS_PER_SECOND
}

/// A window of `window` seconds shown by all views. It either follows the newest data
/// (live) or ends at a fixed time.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TimeRange {
    window: f64,
    /// End of the window in seconds, or `None` to follow the newest data.
    #[serde(skip)]
    end: Option<f64>,
    /// Times of the first and the newest collected data in seconds.
    #[serde(skip)]
    span: Option<(f64, f64)>,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            window: 20.0,
            end: None,
            span: None,
        }
    }
}

impl TimeRange {
    /// Updates the span of the collected data, call once per frame before the views.
    pub fn update_span(&mut self, model: &Model) {
        self.span = model
            .time_span()
            .map(|(first, latest)| (timestamp_to_seconds(first), timestamp_to_seconds(latest)));
        if self.span.is_none() {
            self.end = None;
        }
    }

    /// Time of the first collected data in seconds, used as origin of the time axes.
    pub fn origin(&self) -> f64 {
        self.span.map_or(0.0, |(first, _)| first)
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
    /// following the newest data in live mode.
    pub fn pan_zoom(&mut self, start: f64, end: f64) {
        let window = (end - start).clamp(MIN_WINDOW, MAX_WINDOW);
        if self.is_live() && (window - self.window).abs() > self.window * 1.0e-6 {
            self.window = window;
        } else {
            self.window = window;
            self.set_end(end);
        }
    }

    /// Moves the end of the window to `end` seconds, switches to live mode if that is
    /// at or after the newest data.
    fn set_end(&mut self, end: f64) {
        self.end = match self.span {
            Some((first, latest)) if end < latest => {
                Some(end.max(first + self.window.min(latest - first)))
            }
            _ => None,
        };
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let live_button = egui::Button::new(format!("{} Live", egui_phosphor::regular::BROADCAST))
                .selected(self.is_live());
            if ui
                .add(live_button)
                .on_hover_text("Follow the newest data")
                .clicked()
            {
                self.end = if self.is_live() {
                    self.span.map(|(_, latest)| latest)
                } else {
                    None
                };
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
    }

    /// A track over the collected data with a thumb for the shown window. Dragging the
    /// thumb moves the window, clicking the track centers the window there, and
    /// scrolling over it changes the window length.
    fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
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
            ui.painter().rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
            return;
        };
        let duration = (latest - first).max(MIN_WINDOW);
        let x_at = |t: f64| rect.left() + ((t - first) / duration) as f32 * rect.width();
        let thumb_at = |(start, end): (f64, f64)| {
            let thumb = egui::Rect::from_x_y_ranges(x_at(start.max(first))..=x_at(end), rect.y_range());
            thumb.expand2(egui::vec2(((MIN_THUMB_WIDTH - thumb.width()) / 2.0).max(0.0), 0.0))
        };

        if let (Some(pos), Some(range)) = (response.interact_pointer_pos(), self.range()) {
            let (_, end) = range;
            let jump = (response.clicked() || response.drag_started()) && !thumb_at(range).contains(pos);
            if jump {
                let t = first + ((pos.x - rect.left()) / rect.width()) as f64 * duration;
                self.set_end(t + self.window / 2.0);
            } else if response.dragged() {
                self.set_end(end + (response.drag_delta().x / rect.width()) as f64 * duration);
            }
        }

        let visuals = ui.visuals();
        ui.painter().rect_filled(rect, 3.0, visuals.extreme_bg_color);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time_range(first: f64, latest: f64, window: f64) -> TimeRange {
        TimeRange {
            window,
            end: None,
            span: Some((first, latest)),
        }
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
    fn window_end_stays_within_data() {
        let mut time = time_range(100.0, 150.0, 20.0);
        time.pan_zoom(50.0, 70.0);
        assert_eq!(time.range(), Some((100.0, 120.0)));
    }
}
