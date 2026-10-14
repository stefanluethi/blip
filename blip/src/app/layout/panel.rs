//! The layout panel: the groups and views of the layout as a tree, to select, add,
//! rename, move and remove them.

use super::{Drop, GroupKind, Layout};
use crate::app::view::ViewType;
use egui_tiles::{Tile, TileId};

/// A change of the layout, applied after the panel is shown.
enum Action {
    Select(TileId),
    AddView(ViewType),
    AddTab,
    AddGroup(GroupKind),
    SetGroupKind(TileId, GroupKind),
    StartRename(TileId),
    FinishRename,
    CancelRename,
    Remove(TileId),
    Drop(TileId, TileId, Drop),
}

/// Fraction of the row height at the top and bottom of a row to drop before or after it.
const DROP_EDGE_FRACTION: f32 = 0.25;

/// What is collected while showing the rows.
struct RowsState {
    renaming: Option<(TileId, String)>,
    actions: Vec<Action>,
    /// The dragged view or group.
    dragged: Option<TileId>,
    /// The row the dragged view or group is over and where it would be dropped.
    drop_target: Option<(TileId, Drop)>,
}

impl Layout {
    pub fn panel_ui(&mut self, ui: &mut egui::Ui) {
        let mut actions = vec![];

        ui.horizontal(|ui| {
            ui.strong("Layout");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(egui_phosphor::regular::PLUS, |ui| {
                    for view_type in ViewType::ALL {
                        if ui
                            .button(format!("{} {}", view_type.icon(), view_type.name()))
                            .clicked()
                        {
                            actions.push(Action::AddView(view_type));
                        }
                    }
                    ui.separator();
                    if ui
                        .button(format!("{} Tab", GroupKind::Tabs.icon()))
                        .clicked()
                    {
                        actions.push(Action::AddTab);
                    }
                    for kind in GroupKind::ALL {
                        if ui
                            .button(format!("{} {} group", kind.icon(), kind.name()))
                            .clicked()
                        {
                            actions.push(Action::AddGroup(kind));
                        }
                    }
                })
                .response
                .on_hover_text("Add a view, a tab or a group");
            });
        });

        let mut rows = RowsState {
            renaming: self.renaming.take(),
            actions,
            dragged: egui::DragAndDrop::payload::<TileId>(ui.ctx()).map(|dragged| *dragged),
            drop_target: None,
        };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let top_level = self.container(self.root()).map(|root| root.children_vec());
                for tile_id in top_level.unwrap_or_default() {
                    self.tile_ui(ui, tile_id, &mut rows);
                }
            });
        self.renaming = rows.renaming;
        let mut actions = rows.actions;

        if let Some(dragged) = rows.dragged {
            self.dragged_ui(ui, dragged);
            if let Some((target, drop)) = rows.drop_target
                && ui.input(|i| i.pointer.any_released())
            {
                egui::DragAndDrop::clear_payload(ui.ctx());
                actions.push(Action::Drop(dragged, target, drop));
            }
        }

        for action in actions {
            self.apply(action);
        }
    }

    /// Shows the name of the dragged view or group at the pointer.
    fn dragged_ui(&self, ui: &egui::Ui, dragged: TileId) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let Some(pos) = ui.ctx().pointer_interact_pos() else {
            return;
        };
        egui::Area::new(egui::Id::new("layout_dragged"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos + egui::vec2(12.0, 0.0))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(self.name(dragged));
                });
            });
    }

    fn tile_ui(&self, ui: &mut egui::Ui, tile_id: TileId, rows: &mut RowsState) {
        match self.tree.tiles.get(tile_id) {
            Some(Tile::Pane(view)) => {
                ui.horizontal(|ui| {
                    // align with the group rows, which start with a collapse button
                    ui.add_space(ui.spacing().icon_width + ui.spacing().item_spacing.x);
                    self.row_ui(ui, tile_id, view.view_type().icon(), None, rows);
                });
            }
            Some(Tile::Container(container)) => {
                let kind = GroupKind::of(container);
                let children = container.children_vec();
                let id = ui.make_persistent_id(("layout_group", tile_id));
                egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    id,
                    true,
                )
                .show_header(ui, |ui| {
                    self.row_ui(ui, tile_id, kind.icon(), Some(kind), rows);
                })
                .body(|ui| {
                    for child in children {
                        self.tile_ui(ui, child, rows);
                    }
                });
            }
            None => {}
        }
    }

    /// A row of a view, or of a group of `group_kind`.
    fn row_ui(
        &self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        icon: &str,
        group_kind: Option<GroupKind>,
        rows: &mut RowsState,
    ) {
        let actions = &mut rows.actions;
        if let Some((_, name)) = rows.renaming.as_mut().filter(|(id, _)| *id == tile_id) {
            ui.label(icon);
            let response = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
            if response.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    actions.push(Action::CancelRename);
                } else {
                    actions.push(Action::FinishRename);
                }
            } else {
                response.request_focus();
            }
            return;
        }

        let selected = self.selected == Some(tile_id);
        let label = egui::Button::selectable(selected, format!("{icon} {}", self.name(tile_id)))
            .sense(egui::Sense::click_and_drag());
        let response = ui.add(label);
        response.dnd_set_drag_payload(tile_id);
        if let Some(dragged) = rows.dragged {
            let row = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), response.rect.y_range());
            if let Some(drop) = self.drop_at(ui, row, dragged, tile_id, group_kind.is_some()) {
                rows.drop_target = Some((tile_id, drop));
            }
        }
        if response.clicked() {
            actions.push(Action::Select(tile_id));
        }
        if response.double_clicked() {
            actions.push(Action::StartRename(tile_id));
        }
        response.context_menu(|ui| {
            if let Some(current) = group_kind {
                for kind in GroupKind::ALL {
                    if ui
                        .radio(current == kind, format!("{} {}", kind.icon(), kind.name()))
                        .clicked()
                    {
                        actions.push(Action::SetGroupKind(tile_id, kind));
                    }
                }
                ui.separator();
            }
            if ui
                .button(format!("{} Rename", egui_phosphor::regular::PENCIL_SIMPLE))
                .clicked()
            {
                actions.push(Action::StartRename(tile_id));
            }
            if ui
                .button(format!("{} Delete", egui_phosphor::regular::TRASH))
                .clicked()
            {
                actions.push(Action::Remove(tile_id));
            }
        });
    }

    /// Where `dragged` would be dropped on the row `row` of `target` with the pointer at
    /// its current position, and marks that in the row.
    fn drop_at(
        &self,
        ui: &egui::Ui,
        row: egui::Rect,
        dragged: TileId,
        target: TileId,
        is_group: bool,
    ) -> Option<Drop> {
        let pos = ui
            .ctx()
            .pointer_interact_pos()
            .filter(|&pos| row.contains(pos))?;
        if !self.can_drop(dragged, target) {
            return None;
        }

        let fraction = (pos.y - row.top()) / row.height();
        let drop = if fraction < DROP_EDGE_FRACTION {
            Drop::Before
        } else if fraction > 1.0 - DROP_EDGE_FRACTION {
            Drop::After
        } else if is_group {
            Drop::Into
        } else {
            Drop::Group
        };

        let stroke = egui::Stroke::new(2.0, ui.visuals().selection.stroke.color);
        let painter = ui.painter();
        match drop {
            Drop::Before => painter.hline(row.x_range(), row.top(), stroke),
            Drop::After => painter.hline(row.x_range(), row.bottom(), stroke),
            Drop::Into | Drop::Group => {
                painter.rect_stroke(row, 2.0, stroke, egui::StrokeKind::Inside)
            }
        };
        Some(drop)
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Select(tile_id) => self.select(tile_id),
            Action::AddView(view_type) => self.add_view(view_type),
            Action::AddTab => {
                self.add_tab();
            }
            Action::AddGroup(kind) => {
                self.add_group(kind);
            }
            Action::SetGroupKind(tile_id, kind) => self.set_group_kind(tile_id, kind),
            Action::StartRename(tile_id) => {
                self.renaming = Some((tile_id, self.name(tile_id).to_owned()));
            }
            Action::FinishRename => {
                let renamed = self.renaming.take();
                self.finish_rename(renamed);
            }
            Action::CancelRename => self.renaming = None,
            Action::Remove(tile_id) => self.remove(tile_id),
            Action::Drop(dragged, target, drop) => self.drop_tile(dragged, target, drop),
        }
    }
}
