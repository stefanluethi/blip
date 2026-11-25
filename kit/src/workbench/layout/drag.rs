//! Moving views and groups by drag and drop in the center pane: a view is dragged by
//! its title bar, a view or group shown as a tab by its tab button. It is dropped at the
//! side of a view nearest to the pointer, on the middle of a view to stack them as tabs,
//! on a tab button, on the empty part of a tab row to be its last tab, or into an empty
//! group.

use super::{Drop, Layout, Side};
use crate::widgets::theme::{self, TokensExt};
use egui_tiles::{Container, TileId};

/// What the center pane showed in this frame, to drop on.
pub(super) struct Shown {
    /// The cards of the views.
    pub cards: Vec<(TileId, egui::Rect)>,
    /// The tab buttons.
    pub tab_buttons: Vec<(TileId, egui::Rect)>,
    /// The empty parts of the tab rows after the last tab.
    pub tab_rows: Vec<(TileId, egui::Rect)>,
}

/// Where the dragged view or group would be dropped in the center pane.
struct Target {
    tile_id: TileId,
    drop: Drop,
    /// The card, tab button, empty part of a tab row or group dropped on.
    rect: egui::Rect,
    /// Whether `rect` is the empty part of a tab row, where the dropped one becomes the
    /// last tab.
    on_tab_row: bool,
}

impl Target {
    /// A drop at `rect`, not on the empty part of a tab row.
    fn new(tile_id: TileId, drop: Drop, rect: egui::Rect) -> Self {
        Self {
            tile_id,
            drop,
            rect,
            on_tab_row: false,
        }
    }
}

/// The smallest side of the middle of a card, see [`middle`].
const MIN_MIDDLE: f32 = 40.0;

/// The middle of `card`, where a drop stacks the dragged one with the view as tabs: half
/// of each side, at least [`MIN_MIDDLE`] unless the card is smaller.
fn middle(card: egui::Rect) -> egui::Rect {
    let side = |length: f32| (length / 2.0).max(MIN_MIDDLE.min(length));
    egui::Rect::from_center_size(
        card.center(),
        egui::vec2(side(card.width()), side(card.height())),
    )
}

/// How a drop at `pos` on `card` goes: into the middle, else at the nearest side.
fn drop_on_card(card: egui::Rect, pos: egui::Pos2) -> Drop {
    if middle(card).contains(pos) {
        Drop::Into
    } else {
        Drop::Side(nearest_side(card, pos))
    }
}

/// Puts a button over the tab button `tab` of `tile_id` that moves the tile by drag and
/// drop. Returns the responses of both, the button on top takes the clicks of the pointer
/// but screen readers click the tab button.
pub(super) fn tab_drag_button(tab: &egui::Response, tile_id: TileId) -> egui::Response {
    let id = tab.id.with("drag");
    let mut ui = egui::Ui::new(
        tab.ctx.clone(),
        id.with("ui"),
        egui::UiBuilder::new()
            .layer_id(tab.layer_id)
            .max_rect(tab.rect),
    );
    // tabs scrolled out of the tab bar are clipped
    ui.set_clip_rect(tab.interact_rect);
    let response = ui.interact(tab.rect, id, egui::Sense::click_and_drag());
    response.dnd_set_drag_payload(tile_id);
    tab.union(response)
}

/// The side of `rect` nearest to `pos`, relative to the size of `rect`.
fn nearest_side(rect: egui::Rect, pos: egui::Pos2) -> Side {
    let x = (pos.x - rect.left()) / rect.width();
    let y = (pos.y - rect.top()) / rect.height();
    [
        (x, Side::Left),
        (1.0 - x, Side::Right),
        (y, Side::Top),
        (1.0 - y, Side::Bottom),
    ]
    .into_iter()
    .min_by(|(a, _), (b, _)| a.total_cmp(b))
    .map(|(_, side)| side)
    .expect("there are four sides")
}

impl Layout {
    /// Shows where `dragged` would be dropped in the center pane and drops it there when
    /// the pointer is released. Hovering a tab button opens its tab, to drop into it.
    pub(super) fn center_drop_ui(&mut self, ui: &egui::Ui, dragged: TileId, shown: &Shown) {
        let Some(pos) = ui.ctx().pointer_interact_pos() else {
            return;
        };
        if let Some(&(tab, _)) = shown
            .tab_buttons
            .iter()
            .find(|(_, rect)| rect.contains(pos))
        {
            self.tree.make_active(|id, _| id == tab);
        }
        let Some(target) = self.drop_target(dragged, shown, pos) else {
            return;
        };
        paint_drop(ui, &target);
        if ui.input(|i| i.pointer.any_released()) {
            egui::DragAndDrop::clear_payload(ui.ctx());
            self.drop_tile(dragged, target.tile_id, target.drop);
        }
    }

    /// Where `dragged` would be dropped with the pointer at `pos`.
    fn drop_target(&self, dragged: TileId, shown: &Shown, pos: egui::Pos2) -> Option<Target> {
        let tab = shown.tab_buttons.iter().find_map(|&(tile_id, rect)| {
            rect.contains(pos).then(|| {
                let drop = if pos.x > rect.center().x {
                    Drop::After
                } else {
                    Drop::Before
                };
                Target::new(tile_id, drop, rect)
            })
        });
        let tab_row = || {
            shown.tab_rows.iter().find_map(|&(tile_id, rect)| {
                rect.contains(pos).then_some(Target {
                    tile_id,
                    drop: Drop::Into,
                    rect,
                    on_tab_row: true,
                })
            })
        };
        let card = || {
            shown.cards.iter().find_map(|&(tile_id, rect)| {
                rect.contains(pos)
                    .then(|| Target::new(tile_id, drop_on_card(rect, pos), rect))
            })
        };
        // the user keeps empty groups to add views to them
        let empty_group = || {
            self.tree.tiles.iter().find_map(|(&tile_id, _)| {
                let empty = self.container(tile_id).is_some_and(Container::is_empty);
                let rect = self.tree.tiles.rect(tile_id)?;
                (empty && tile_id != self.root() && rect.contains(pos))
                    .then(|| Target::new(tile_id, Drop::Into, rect))
            })
        };
        tab.or_else(tab_row)
            .or_else(card)
            .or_else(empty_group)
            .filter(|target| self.can_drop(dragged, target.tile_id))
    }
}

/// The area `target` marks: the half of the view at the side it is dropped at, the whole
/// of a view dropped on its middle, of an empty group or of the empty part of a tab row.
/// None for a drop on a tab button, which is marked by a line.
fn marked_area(target: &Target) -> Option<egui::Rect> {
    let rect = target.rect;
    match target.drop {
        Drop::Before | Drop::After => None,
        // the same for a move next to the view and for a new group of both, the user
        // need not know how the views are grouped to read it
        Drop::Side(side) => Some(match side {
            Side::Left => rect.split_left_right_at_fraction(0.5).0,
            Side::Right => rect.split_left_right_at_fraction(0.5).1,
            Side::Top => rect.split_top_bottom_at_fraction(0.5).0,
            Side::Bottom => rect.split_top_bottom_at_fraction(0.5).1,
        }),
        Drop::Into => Some(rect),
    }
}

/// The line `target` marks: at the side of a tab button the dropped one moves next to,
/// at the left edge of the empty part of a tab row, where the new tab goes.
fn marked_line(target: &Target) -> Option<[egui::Pos2; 2]> {
    let rect = target.rect;
    match target.drop {
        Drop::Before => Some([rect.left_top(), rect.left_bottom()]),
        Drop::After => Some([rect.right_top(), rect.right_bottom()]),
        Drop::Into if target.on_tab_row => Some([rect.left_top(), rect.left_bottom()]),
        Drop::Into | Drop::Side(_) => None,
    }
}

/// Marks where `target` would put the dropped view or group, see [`marked_area`] and
/// [`marked_line`].
fn paint_drop(ui: &egui::Ui, target: &Target) {
    let accent = ui.tokens().accent;
    let painter = ui.painter();
    if let Some(area) = marked_area(target) {
        painter.rect(
            area,
            theme::CARD_RADIUS,
            accent.gamma_multiply(0.15),
            egui::Stroke::new(1.0, accent),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(line) = marked_line(target) {
        painter.line_segment(line, egui::Stroke::new(3.0, accent));
    }
}

#[cfg(test)]
mod tests;
