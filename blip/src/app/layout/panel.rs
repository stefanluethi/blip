//! The layout panel: the groups and views of the layout as a tree, to select, add,
//! rename and remove them.

use super::{GroupKind, Layout};
use crate::app::view::ViewType;
use egui_tiles::{Tile, TileId};

/// A change of the layout, applied after the panel is shown.
enum Action {
    Select(TileId),
    AddView(ViewType),
    AddTab,
    SetGroupKind(TileId, GroupKind),
    StartRename(TileId),
    FinishRename,
    CancelRename,
    Remove(TileId),
}

impl Layout {
    pub fn panel_ui(&mut self, ui: &mut egui::Ui) {
        let mut actions = vec![];

        ui.horizontal(|ui| {
            ui.strong("Layout");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(egui_phosphor::regular::PLUS, |ui| {
                    for view_type in ViewType::ALL {
                        if ui.button(format!("{} {}", view_type.icon(), view_type.name())).clicked() {
                            actions.push(Action::AddView(view_type));
                        }
                    }
                    ui.separator();
                    if ui.button(format!("{} Tab", GroupKind::Tabs.icon())).clicked() {
                        actions.push(Action::AddTab);
                    }
                })
                .response
                .on_hover_text("Add a view or a tab");
            });
        });

        let mut renaming = self.renaming.take();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let top_level = self.container(self.root()).map(|root| root.children_vec());
                for tile_id in top_level.unwrap_or_default() {
                    self.tile_ui(ui, tile_id, &mut renaming, &mut actions);
                }
            });
        self.renaming = renaming;

        for action in actions {
            self.apply(action);
        }
    }

    fn tile_ui(
        &self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        renaming: &mut Option<(TileId, String)>,
        actions: &mut Vec<Action>,
    ) {
        match self.tree.tiles.get(tile_id) {
            Some(Tile::Pane(view)) => {
                ui.horizontal(|ui| {
                    // align with the group rows, which start with a collapse button
                    ui.add_space(ui.spacing().icon_width + ui.spacing().item_spacing.x);
                    self.row_ui(ui, tile_id, view.view_type().icon(), None, renaming, actions);
                });
            }
            Some(Tile::Container(container)) => {
                let kind = GroupKind::of(container);
                let children = container.children_vec();
                let id = ui.make_persistent_id(("layout_group", tile_id));
                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                    .show_header(ui, |ui| {
                        self.row_ui(ui, tile_id, kind.icon(), Some(kind), renaming, actions);
                    })
                    .body(|ui| {
                        for child in children {
                            self.tile_ui(ui, child, renaming, actions);
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
        renaming: &mut Option<(TileId, String)>,
        actions: &mut Vec<Action>,
    ) {
        if let Some((_, name)) = renaming.as_mut().filter(|(id, _)| *id == tile_id) {
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
        let response = ui.selectable_label(selected, format!("{icon} {}", self.name(tile_id)));
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
            if ui.button(format!("{} Rename", egui_phosphor::regular::PENCIL_SIMPLE)).clicked() {
                actions.push(Action::StartRename(tile_id));
            }
            if ui.button(format!("{} Delete", egui_phosphor::regular::TRASH)).clicked() {
                actions.push(Action::Remove(tile_id));
            }
        });
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Select(tile_id) => self.select(tile_id),
            Action::AddView(view_type) => self.add_view(view_type),
            Action::AddTab => {
                self.add_tab();
            }
            Action::SetGroupKind(tile_id, kind) => self.set_group_kind(tile_id, kind),
            Action::StartRename(tile_id) => {
                self.renaming = Some((tile_id, self.name(tile_id).to_owned()));
            }
            Action::FinishRename => {
                if let Some((tile_id, name)) = self.renaming.take()
                    && !name.trim().is_empty()
                {
                    self.set_name(tile_id, name.trim().to_owned());
                }
            }
            Action::CancelRename => self.renaming = None,
            Action::Remove(tile_id) => self.remove(tile_id),
        }
    }
}
