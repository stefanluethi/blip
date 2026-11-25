//! Toasts: short notices stacked in a corner, e.g. why connecting failed or a file
//! could not be opened, shown until they expire or the user closes them, errors until
//! they are closed. A toast may offer an action, like "Undo". Below them, a task under
//! way, like opening a recording, shows how far it got.

use crate::widgets::theme::{self, TokensExt};
use egui_phosphor::regular::{INFO, WARNING, WARNING_CIRCLE, X};

/// How long a toast is shown in seconds, longer while the pointer rests on it.
const DURATION: f64 = 8.0;
/// The most toasts shown at once, the oldest are dropped for new ones.
const MAX_TOASTS: usize = 5;
/// The width of the toasts, unless the space they are shown in is narrower.
const WIDTH: f32 = 320.0;
/// The space between the toasts and the corner they are shown in.
const MARGIN: f32 = 8.0;

/// How a toast looks and how long it stays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// Tells what happened, e.g. that a file was saved. Neutral, it expires.
    Info,
    /// Something did not work out completely. It expires.
    Warning,
    /// Something failed. It stays until the user closes it.
    Error,
}

impl From<crate::store::Level> for Level {
    fn from(level: crate::store::Level) -> Self {
        match level {
            crate::store::Level::Warning => Level::Warning,
            crate::store::Level::Error => Level::Error,
        }
    }
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
    /// The values of the actions clicked since [`Self::take_actions`].
    clicked: Vec<u64>,
}

struct Toast {
    /// Tells the toasts apart while their texts come and go.
    id: u64,
    level: Level,
    text: String,
    /// The label of the button of its action and the value it gives back.
    action: Option<(String, u64)>,
    /// How often the text was added while the toast was shown.
    count: usize,
    /// When the toast expires, in seconds as egui counts them, `None` until it is shown
    /// and for an error, which stays until it is closed.
    expires: Option<f64>,
}

/// What the user did with a toast in a frame.
enum Clicked {
    Nothing,
    Close,
    Action,
}

impl Toasts {
    /// Shows `text` as an error, until the user closes it.
    pub fn error(&mut self, text: impl Into<String>) {
        self.add(Level::Error, text.into(), None);
    }

    /// Shows `text` as a warning.
    pub fn warning(&mut self, text: impl Into<String>) {
        self.add(Level::Warning, text.into(), None);
    }

    /// Shows `text` as a neutral notice, e.g. "Saved a.blip".
    pub fn info(&mut self, text: impl Into<String>) {
        self.add(Level::Info, text.into(), None);
    }

    /// Shows `text` at `level`, e.g. a notice of the store.
    pub fn notice(&mut self, level: impl Into<Level>, text: impl Into<String>) {
        self.add(level.into(), text.into(), None);
    }

    /// Shows `text` at `level` with a button `label`, e.g. "Undo". A click on it closes
    /// the toast and gives `value` back from [`Self::take_actions`], to tell the actions
    /// of the caller apart.
    pub fn with_action(
        &mut self,
        level: Level,
        text: impl Into<String>,
        label: impl Into<String>,
        value: u64,
    ) {
        self.add(level, text.into(), Some((label.into(), value)));
    }

    /// Closes the toasts whose action gives `value`, e.g. an offer to undo that no
    /// longer can.
    pub fn dismiss(&mut self, value: u64) {
        (self.toasts).retain(|toast| toast.action.as_ref().is_none_or(|(_, v)| *v != value));
    }

    /// The values of the actions whose buttons were clicked since the last call, in
    /// the order they were clicked.
    pub fn take_actions(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.clicked)
    }

    /// Adds a toast, or counts the toast shown with the same text and shows it for
    /// longer, so a repeated error does not pile up. A toast with an action is not
    /// counted, as each has its own.
    fn add(&mut self, level: Level, text: String, action: Option<(String, u64)>) {
        let shown = self.toasts.iter_mut().find(|toast| {
            toast.level == level && toast.text == text && toast.action.is_none() && action.is_none()
        });
        if let Some(toast) = shown {
            toast.count += 1;
            toast.expires = None;
            return;
        }
        self.toasts.push(Toast {
            id: self.next_id,
            level,
            text,
            action,
            count: 1,
            expires: None,
        });
        self.next_id += 1;
        if self.toasts.len() > MAX_TOASTS {
            self.toasts.remove(0);
        }
    }

    /// Starts the time of the toasts shown from `now` on and drops those expired.
    /// Errors do not expire.
    fn expire(&mut self, now: f64) {
        for toast in &mut self.toasts {
            if toast.level != Level::Error {
                toast.expires.get_or_insert(now + DURATION);
            }
        }
        self.toasts.retain(|toast| {
            toast.level == Level::Error || toast.expires.is_some_and(|expires| expires > now)
        });
    }

    /// Shows the toasts stacked in the bottom right corner of `rect`, newest at the
    /// bottom, each with a button to close it and the button of its action, and
    /// `progress` below them, with a button to cancel it. Returns whether that was
    /// clicked. The clicked actions are kept for [`Self::take_actions`].
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
                    match response.inner {
                        Clicked::Nothing => {}
                        Clicked::Close => closed.push(toast.id),
                        Clicked::Action => {
                            closed.push(toast.id);
                            if let Some((_, value)) = &toast.action {
                                self.clicked.push(*value);
                            }
                        }
                    }
                    if toast.expires.is_some() && ui.rect_contains_pointer(response.response.rect) {
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
    #[cfg(any(test, feature = "test-util"))]
    pub fn shown(&self) -> Vec<(Level, &str)> {
        self.toasts
            .iter()
            .map(|toast| (toast.level, toast.text.as_str()))
            .collect()
    }
}

/// A button with only `icon`, as high as the controls, to close or cancel.
fn close_button(ui: &mut egui::Ui, hint: &str) -> bool {
    let square = egui::Vec2::splat(theme::CONTROL_HEIGHT);
    let button = theme::icon_button(X).button(ui).min_size(square);
    ui.add(button).on_hover_text(hint).clicked()
}

/// The room above a line of text that puts it in the middle of a control beside it.
fn control_pad(ui: &egui::Ui) -> f32 {
    ((theme::CONTROL_HEIGHT - ui.text_style_height(&egui::TextStyle::Body)) / 2.0).max(0.0)
}

/// Shows `toast` with the icon of its level, its text, the button of its action and a
/// close button. Returns which was clicked.
fn toast_ui(ui: &mut egui::Ui, toast: &Toast) -> Clicked {
    let (icon, color) = match toast.level {
        Level::Info => (INFO, ui.visuals().weak_text_color()),
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
            let pad = control_pad(ui);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.add_space(pad);
                    ui.label(egui::RichText::new(icon).color(color));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let closed = close_button(ui, "Close");
                    let height = egui::vec2(0.0, theme::CONTROL_HEIGHT);
                    let action = (toast.action.as_ref()).is_some_and(|(label, _)| {
                        let button = egui::Button::new(label.as_str()).min_size(height);
                        ui.add(button).clicked()
                    });
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.add_space(pad);
                        ui.add(egui::Label::new(text).wrap());
                    });
                    match (closed, action) {
                        (true, _) => Clicked::Close,
                        (false, true) => Clicked::Action,
                        (false, false) => Clicked::Nothing,
                    }
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
            let pad = control_pad(ui);
            let cancelled = ui
                .horizontal_top(|ui| {
                    let size = ui.text_style_height(&egui::TextStyle::Body);
                    ui.vertical(|ui| {
                        ui.add_space(pad);
                        ui.add(egui::Spinner::new().size(size));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let cancelled = close_button(ui, "Cancel");
                        let percent = (progress.fraction * 100.0).floor();
                        ui.vertical(|ui| {
                            ui.add_space(pad);
                            ui.weak(format!("{percent:.0} %"));
                        });
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.add_space(pad);
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
    let tokens = ui.tokens();
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
mod tests;
