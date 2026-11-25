//! The view picker: the types of views to add in an empty group, as buttons, as a row of
//! icons or as a menu, whichever fits the room the group has.

use crate::widgets::theme::{self, TokensExt};
use crate::widgets::view::ViewType;
use egui_tiles::TileId;

/// Size of a button of the list form of [`view_picker`].
const PICKER_BUTTON: egui::Vec2 = egui::vec2(96.0, 72.0);

/// Height of the title "Add a view" above the buttons of the list form.
const PICKER_TITLE_HEIGHT: f32 = 28.0;

/// Size of an icon button of the icons form.
const PICKER_ICON: f32 = theme::CONTROL_HEIGHT;

/// Space between the icon buttons of the icons form.
const PICKER_ICON_GAP: f32 = 4.0;

/// Height of the title "Add a view" above the icon buttons.
const PICKER_SMALL_TITLE: f32 = 20.0;

/// How [`view_picker`] shows the types, by the room it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PickerForm {
    /// A button with icon and name for each type.
    List,
    /// A row of icon buttons below a title, with the name on hover.
    Icons,
    /// One button that opens a menu of the types.
    Menu,
}

/// The form of the picker for `types` types in a rect of `size`: the list if its buttons
/// fit, else the icons if they fit, else the menu.
fn picker_form(size: egui::Vec2, types: usize) -> PickerForm {
    let gap = theme::GAP;
    let (columns, rows) = list_grid(size.x, types);
    let list_width = columns as f32 * (PICKER_BUTTON.x + gap) + gap;
    let list_height = PICKER_TITLE_HEIGHT + rows as f32 * (PICKER_BUTTON.y + gap) + gap;
    if list_width <= size.x && list_height <= size.y {
        return PickerForm::List;
    }
    let per_row = icons_per_row(size.x);
    if per_row >= 1 {
        let rows = types.div_ceil(per_row);
        if 36.0 + (PICKER_ICON + PICKER_ICON_GAP) * rows as f32 <= size.y {
            return PickerForm::Icons;
        }
    }
    PickerForm::Menu
}

/// The columns and rows of the list form in `width`: as many columns as fit with a gap
/// between and around them, at most half of the types, so that they form two rows or more.
fn list_grid(width: f32, types: usize) -> (usize, usize) {
    let gap = theme::GAP;
    let fit = ((width - gap) / (PICKER_BUTTON.x + gap)).floor().max(0.0) as usize;
    let columns = fit.clamp(1, types.div_ceil(2).max(1));
    (columns, types.div_ceil(columns))
}

/// How many icon buttons fit in a row of `width`, with a gap around them.
fn icons_per_row(width: f32) -> usize {
    ((width - 12.0) / (PICKER_ICON + PICKER_ICON_GAP))
        .floor()
        .max(0.0) as usize
}

/// Shows the types of views centered in `rect` of an empty group `group`, in the form
/// that fits `rect`, see [`picker_form`]. Returns the type clicked.
pub(super) fn view_picker(ui: &mut egui::Ui, rect: egui::Rect, group: TileId) -> Option<ViewType> {
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("view_picker", group))
            .max_rect(rect),
    );
    // a small group shows a part of the buttons, which neither paint nor take clicks
    // outside it
    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
    match picker_form(rect.size(), ViewType::all().len()) {
        PickerForm::List => list_form(&mut ui, rect),
        PickerForm::Icons => icons_form(&mut ui, rect),
        PickerForm::Menu => menu_form(&mut ui, rect),
    }
}

/// The buttons with icon and name, in columns below a title.
fn list_form(ui: &mut egui::Ui, rect: egui::Rect) -> Option<ViewType> {
    let tokens = ui.tokens();
    let gap = theme::GAP;
    let types = ViewType::all();
    let (columns, rows) = list_grid(rect.width(), types.len());
    let size = egui::vec2(
        columns as f32 * (PICKER_BUTTON.x + gap) - gap,
        PICKER_TITLE_HEIGHT + rows as f32 * (PICKER_BUTTON.y + gap) - gap,
    );
    let top_left = (rect.center() - size / 2.0).max(rect.left_top() + egui::vec2(gap, gap));

    ui.painter().text(
        top_left + egui::vec2(size.x / 2.0, 0.0),
        egui::Align2::CENTER_TOP,
        "Add a view",
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
    let mut clicked = None;
    for (index, view_type) in types.into_iter().enumerate() {
        let (row, column) = (index / columns, index % columns);
        let min = top_left
            + egui::vec2(
                column as f32 * (PICKER_BUTTON.x + gap),
                PICKER_TITLE_HEIGHT + row as f32 * (PICKER_BUTTON.y + gap),
            );
        let button = egui::Rect::from_min_size(min, PICKER_BUTTON);
        let response = ui
            .interact(button, ui.id().with(index), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, view_type.name())
        });
        let stroke = if response.hovered() {
            tokens.accent
        } else {
            tokens.border
        };
        let painter = ui.painter();
        painter.rect(
            button,
            theme::CARD_RADIUS,
            tokens.card,
            egui::Stroke::new(1.0, stroke),
            egui::StrokeKind::Inside,
        );
        let text_color = if response.hovered() {
            ui.visuals().strong_text_color()
        } else {
            ui.visuals().text_color()
        };
        painter.text(
            button.center() - egui::vec2(0.0, 10.0),
            egui::Align2::CENTER_CENTER,
            view_type.icon(),
            egui::FontId::proportional(22.0),
            text_color,
        );
        painter.text(
            button.center() + egui::vec2(0.0, 18.0),
            egui::Align2::CENTER_CENTER,
            view_type.name(),
            egui::TextStyle::Body.resolve(ui.style()),
            text_color,
        );
        if response.clicked() {
            clicked = Some(view_type);
        }
    }
    clicked
}

/// A title above a wrapping row of icon buttons, the name of a type on hover.
fn icons_form(ui: &mut egui::Ui, rect: egui::Rect) -> Option<ViewType> {
    let gap = theme::GAP;
    let types = ViewType::all();
    let per_row = icons_per_row(rect.width()).clamp(1, types.len().max(1));
    let rows = types.len().div_ceil(per_row);
    let step = PICKER_ICON + PICKER_ICON_GAP;
    let size = egui::vec2(
        per_row.min(types.len()) as f32 * step - PICKER_ICON_GAP,
        PICKER_SMALL_TITLE + rows as f32 * step,
    );
    let top_left = (rect.center() - size / 2.0).max(rect.left_top() + egui::vec2(gap, gap));

    ui.painter().text(
        top_left + egui::vec2(size.x / 2.0, 0.0),
        egui::Align2::CENTER_TOP,
        "Add a view",
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().weak_text_color(),
    );
    let mut clicked = None;
    for (index, view_type) in types.into_iter().enumerate() {
        let (row, column) = (index / per_row, index % per_row);
        let min =
            top_left + egui::vec2(column as f32 * step, PICKER_SMALL_TITLE + row as f32 * step);
        let button = egui::Rect::from_min_size(min, egui::Vec2::splat(PICKER_ICON));
        let response = ui
            .put(button, theme::icon_button(view_type.icon()))
            .on_hover_text(view_type.name());
        if response.clicked() {
            clicked = Some(view_type);
        }
    }
    clicked
}

/// One button in the middle of `rect`, which opens a menu of the types.
fn menu_form(ui: &mut egui::Ui, rect: egui::Rect) -> Option<ViewType> {
    let mut clicked = None;
    let layout = egui::Layout::top_down(egui::Align::Center).with_main_align(egui::Align::Center);
    let builder = egui::UiBuilder::new().max_rect(rect).layout(layout);
    ui.scope_builder(builder, |ui| {
        let text = format!("Add a view {}", egui_phosphor::regular::CARET_DOWN);
        ui.menu_button(text, |ui| {
            for view_type in ViewType::all() {
                if ui
                    .button(format!("{} {}", view_type.icon(), view_type.name()))
                    .clicked()
                {
                    clicked = Some(view_type);
                }
            }
        });
    });
    clicked
}

#[cfg(test)]
mod tests;
