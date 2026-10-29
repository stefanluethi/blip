//! Toasts: short notices stacked in a corner, e.g. why connecting failed or a file
//! could not be opened, shown until they expire or the user closes them. Below them, a
//! task under way, like opening a recording, shows how far it got.

use egui_phosphor::regular::{WARNING, WARNING_CIRCLE, X};

/// How long a toast is shown in seconds, longer while the pointer rests on it.
const DURATION: f64 = 8.0;
/// The most toasts shown at once, the oldest are dropped for new ones.
const MAX_TOASTS: usize = 5;
/// The width of the toasts, unless the space they are shown in is narrower.
const WIDTH: f32 = 320.0;
/// The space between the toasts and the corner they are shown in.
const MARGIN: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// Something did not work out completely, e.g. a recording was opened in part.
    Warning,
    /// Something failed, e.g. connecting.
    Error,
}

/// A task under way shown below the toasts, e.g. opening a recording.
pub struct Progress {
    pub text: String,
    /// How much is done, from 0 to 1.
    pub fraction: f32,
}

/// The toasts being shown, oldest first.
#[derive(Default)]
pub struct Toasts {
    toasts: Vec<Toast>,
    next_id: u64,
}

struct Toast {
    /// Tells the toasts apart while their texts come and go.
    id: u64,
    level: Level,
    text: String,
    /// How often the text was added while the toast was shown.
    count: usize,
    /// When the toast expires, in seconds as egui counts them, `None` until it is shown.
    expires: Option<f64>,
}

impl Toasts {
    /// Shows `text` as an error.
    pub fn error(&mut self, text: impl Into<String>) {
        self.add(Level::Error, text.into());
    }

    /// Shows `text` as a warning.
    pub fn warning(&mut self, text: impl Into<String>) {
        self.add(Level::Warning, text.into());
    }

    /// Adds a toast, or counts the toast shown with the same text and shows it for
    /// longer, so a repeated error does not pile up.
    fn add(&mut self, level: Level, text: String) {
        let shown = self
            .toasts
            .iter_mut()
            .find(|toast| toast.level == level && toast.text == text);
        if let Some(toast) = shown {
            toast.count += 1;
            toast.expires = None;
            return;
        }
        self.toasts.push(Toast {
            id: self.next_id,
            level,
            text,
            count: 1,
            expires: None,
        });
        self.next_id += 1;
        if self.toasts.len() > MAX_TOASTS {
            self.toasts.remove(0);
        }
    }

    /// Starts the time of the toasts shown from `now` on and drops those expired.
    fn expire(&mut self, now: f64) {
        for toast in &mut self.toasts {
            toast.expires.get_or_insert(now + DURATION);
        }
        self.toasts
            .retain(|toast| toast.expires.is_some_and(|expires| expires > now));
    }

    /// Shows the toasts stacked in the bottom right corner of `rect`, newest at the
    /// bottom, each with a button to close it, and `progress` below them, with a
    /// button to cancel it. Returns whether that was clicked.
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        rect: egui::Rect,
        progress: Option<Progress>,
    ) -> bool {
        let now = ctx.input(|input| input.time);
        self.expire(now);
        if self.toasts.is_empty() && progress.is_none() {
            return false;
        }
        let mut cancelled = false;
        let width = WIDTH.min(rect.width() - 2.0 * MARGIN).max(0.0);
        let mut closed = vec![];
        egui::Area::new(egui::Id::new("toasts"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(rect.right_bottom() - egui::Vec2::splat(MARGIN))
            .show(ctx, |ui| {
                ui.set_width(width);
                for toast in &mut self.toasts {
                    let response = ui.push_id(toast.id, |ui| toast_ui(ui, toast));
                    if response.inner {
                        closed.push(toast.id);
                    }
                    if ui.rect_contains_pointer(response.response.rect) {
                        toast.expires = Some(now + DURATION);
                    }
                }
                if let Some(progress) = &progress {
                    cancelled = progress_ui(ui, progress);
                }
            });
        self.toasts.retain(|toast| !closed.contains(&toast.id));
        let next = self
            .toasts
            .iter()
            .filter_map(|toast| toast.expires)
            .fold(f64::INFINITY, f64::min);
        if next.is_finite() {
            ctx.request_repaint_after_secs((next - now) as f32);
        }
        cancelled
    }

    /// The levels and texts of the toasts, oldest first.
    #[cfg(test)]
    pub fn shown(&self) -> Vec<(Level, &str)> {
        self.toasts
            .iter()
            .map(|toast| (toast.level, toast.text.as_str()))
            .collect()
    }
}

/// Shows `toast` with the icon of its level, its text and a close button. Returns
/// whether the button was clicked.
fn toast_ui(ui: &mut egui::Ui, toast: &Toast) -> bool {
    let (icon, color) = match toast.level {
        Level::Warning => (WARNING, ui.visuals().warn_fg_color),
        Level::Error => (WARNING_CIRCLE, ui.visuals().error_fg_color),
    };
    let text = if toast.count > 1 {
        format!("{} ({}×)", toast.text, toast.count)
    } else {
        toast.text.clone()
    };
    egui::Frame::popup(ui.style())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(egui::RichText::new(icon).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let closed = ui.small_button(X).on_hover_text("Close").clicked();
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.add(egui::Label::new(text).wrap());
                    });
                    closed
                })
                .inner
            })
            .inner
        })
        .inner
}

/// Shows `progress` with its text, a bar of how far it got and a button to cancel it.
/// Returns whether the button was clicked.
fn progress_ui(ui: &mut egui::Ui, progress: &Progress) -> bool {
    egui::Frame::popup(ui.style())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let cancelled = ui
                .horizontal_top(|ui| {
                    let size = ui.text_style_height(&egui::TextStyle::Body);
                    ui.add(egui::Spinner::new().size(size));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let cancelled = ui.small_button(X).on_hover_text("Cancel").clicked();
                        let percent = (progress.fraction * 100.0).floor();
                        ui.weak(format!("{percent:.0} %"));
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.add(egui::Label::new(progress.text.as_str()).wrap());
                        });
                        cancelled
                    })
                    .inner
                })
                .inner;
            bar_ui(ui, progress.fraction);
            cancelled
        })
        .inner
}

/// A thin bar across `ui`, filled `fraction` of the way in the accent.
fn bar_ui(ui: &mut egui::Ui, fraction: f32) {
    let tokens = super::theme::tokens(ui.visuals());
    let size = egui::vec2(ui.available_width(), 4.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, tokens.border);
    let done = rect.with_max_x(
        rect.lerp_inside(egui::vec2(fraction.clamp(0.0, 1.0), 0.0))
            .x,
    );
    painter.rect_filled(done, 2.0, tokens.accent);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn toasts_expire_after_they_are_shown() {
        let mut toasts = Toasts::default();
        toasts.error("refused");
        toasts.expire(100.0);
        assert_eq!(toasts.toasts.len(), 1);
        toasts.warning("cut off");
        toasts.expire(100.0 + DURATION / 2.0);
        assert_eq!(toasts.toasts.len(), 2);
        toasts.expire(100.0 + DURATION);
        assert_eq!(toasts.toasts.len(), 1);
        assert_eq!(toasts.toasts[0].text, "cut off");
    }

    #[test]
    fn a_repeated_toast_is_counted_and_shown_for_longer() {
        let mut toasts = Toasts::default();
        toasts.error("refused");
        toasts.expire(0.0);
        toasts.error("refused");
        toasts.warning("refused");
        toasts.expire(DURATION / 2.0);
        toasts.expire(DURATION);
        assert_eq!(toasts.toasts.len(), 2);
        assert_eq!(toasts.toasts[0].count, 2);

        for n in 0..MAX_TOASTS + 2 {
            toasts.error(n.to_string());
        }
        assert_eq!(toasts.toasts.len(), MAX_TOASTS);
        assert_eq!(toasts.toasts[0].text, "2");
    }

    #[test]
    fn a_toast_is_shown_until_it_is_closed() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 300.0))
            .build_ui_state(
                |ui, toasts: &mut Toasts| {
                    toasts.ui(ui.ctx(), ui.max_rect(), None);
                },
                Toasts::default(),
            );
        harness.state_mut().error("ws://a cannot be connected");
        harness.run();
        assert!(
            harness
                .query_by_label_contains("cannot be connected")
                .is_some()
        );
        harness.get_by_label(X).click();
        harness.run();
        assert!(harness.state().toasts.is_empty());
        assert!(
            harness
                .query_by_label_contains("cannot be connected")
                .is_none()
        );
    }
}
