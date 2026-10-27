//! Moving views and groups by drag and drop in the center pane: a view is dragged by
//! its title bar, a view or group shown as a tab by its tab button. It is dropped at the
//! side of a view nearest to the pointer, on a tab button, or into an empty group.

use super::{Drop, Layout, Side};
use crate::app::theme;
use egui_tiles::{Container, TileId};

/// What the center pane showed in this frame, to drop on.
pub(super) struct Shown {
    /// The cards of the views.
    pub cards: Vec<(TileId, egui::Rect)>,
    /// The tab buttons.
    pub tab_buttons: Vec<(TileId, egui::Rect)>,
}

/// Where the dragged view or group would be dropped in the center pane.
struct Target {
    tile_id: TileId,
    drop: Drop,
    /// The card, tab button or group dropped on.
    rect: egui::Rect,
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
        self.paint_drop(ui, &target);
        if ui.input(|i| i.pointer.any_released()) {
            egui::DragAndDrop::clear_payload(ui.ctx());
            self.drop_tile(dragged, target.tile_id, target.drop);
        }
    }

    /// Where `dragged` would be dropped with the pointer at `pos`.
    fn drop_target(&self, dragged: TileId, shown: &Shown, pos: egui::Pos2) -> Option<Target> {
        let tab = shown.tab_buttons.iter().find_map(|&(tile_id, rect)| {
            rect.contains(pos).then(|| {
                let after = pos.x > rect.center().x;
                let drop = if after { Drop::After } else { Drop::Before };
                Target {
                    tile_id,
                    drop,
                    rect,
                }
            })
        });
        let card = || {
            shown.cards.iter().find_map(|&(tile_id, rect)| {
                rect.contains(pos).then(|| Target {
                    tile_id,
                    drop: Drop::Side(nearest_side(rect, pos)),
                    rect,
                })
            })
        };
        // the user keeps empty groups to add views to them
        let empty_group = || {
            self.tree.tiles.iter().find_map(|(&tile_id, _)| {
                let empty = self.container(tile_id).is_some_and(Container::is_empty);
                let rect = self.tree.tiles.rect(tile_id)?;
                (empty && tile_id != self.root() && rect.contains(pos)).then_some(Target {
                    tile_id,
                    drop: Drop::Into,
                    rect,
                })
            })
        };
        tab.or_else(card)
            .or_else(empty_group)
            .filter(|target| self.can_drop(dragged, target.tile_id))
    }

    /// Marks where `target` would put the dropped view or group: a line in the gap where
    /// it moves between views or tabs, or the place it takes in a new or empty group.
    fn paint_drop(&self, ui: &egui::Ui, target: &Target) {
        let accent = theme::tokens(ui.visuals()).accent;
        let painter = ui.painter();
        let rect = target.rect;
        let line = |from: egui::Pos2, to: egui::Pos2| {
            painter.line_segment([from, to], egui::Stroke::new(3.0, accent));
        };
        let area = |area: egui::Rect| {
            painter.rect(
                area,
                theme::CARD_RADIUS,
                accent.gamma_multiply(0.15),
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Inside,
            );
        };
        match target.drop {
            Drop::Before => line(rect.left_top(), rect.left_bottom()),
            Drop::After => line(rect.right_top(), rect.right_bottom()),
            Drop::Side(side) if self.reorders(target.tile_id, side) => {
                // in the middle of the gap to the next view
                let rect = rect.expand(theme::GAP / 2.0);
                match side {
                    Side::Left => line(rect.left_top(), rect.left_bottom()),
                    Side::Right => line(rect.right_top(), rect.right_bottom()),
                    Side::Top => line(rect.left_top(), rect.right_top()),
                    Side::Bottom => line(rect.left_bottom(), rect.right_bottom()),
                }
            }
            Drop::Side(side) => area(match side {
                Side::Left => rect.split_left_right_at_fraction(0.5).0,
                Side::Right => rect.split_left_right_at_fraction(0.5).1,
                Side::Top => rect.split_top_bottom_at_fraction(0.5).0,
                Side::Bottom => rect.split_top_bottom_at_fraction(0.5).1,
            }),
            Drop::Into => area(rect),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_side_is_relative_to_the_size() {
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 100.0));
        assert_eq!(nearest_side(rect, egui::pos2(30.0, 50.0)), Side::Left);
        assert_eq!(nearest_side(rect, egui::pos2(390.0, 50.0)), Side::Right);
        // 20 px from the top is nearer than 100 px from the left of the wide card
        assert_eq!(nearest_side(rect, egui::pos2(100.0, 20.0)), Side::Top);
        assert_eq!(nearest_side(rect, egui::pos2(200.0, 90.0)), Side::Bottom);
    }
}
