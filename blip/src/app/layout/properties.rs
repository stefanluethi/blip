//! The properties panel: the properties of the selected view or group.

use super::{GroupKind, Layout};
use crate::app::model::Model;
use crate::app::view::{Viewable, properties_section};
use egui_tiles::{Tile, TileId};

impl Layout {
    pub fn properties_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        ui.strong("Properties");
        let Some(tile_id) = self
            .selected
            .filter(|&id| self.tree.tiles.get(id).is_some())
        else {
            ui.weak("Select a view or a group to see its properties");
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt("properties")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.container(tile_id).map(GroupKind::of) {
                Some(kind) => self.group_ui(ui, tile_id, kind),
                None => self.view_ui(ui, tile_id, model),
            });
    }

    fn group_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, current: GroupKind) {
        properties_section(ui, "Group", |ui| {
            ui.label("Name");
            self.name_ui(ui, tile_id);
            ui.end_row();
            ui.label("Arrange");
            let label = |kind: GroupKind| format!("{} {}", kind.icon(), kind.name());
            egui::ComboBox::from_id_salt("group_kind")
                .selected_text(label(current))
                .show_ui(ui, |ui| {
                    for kind in GroupKind::ALL {
                        if ui.selectable_label(current == kind, label(kind)).clicked() {
                            self.set_group_kind(tile_id, kind);
                        }
                    }
                });
            ui.end_row();
        });
    }

    fn view_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, model: &Model) {
        properties_section(ui, "View", |ui| {
            ui.label("Name");
            self.name_ui(ui, tile_id);
            ui.end_row();
            if let Some(view) = self.view(tile_id) {
                let view_type = view.view_type();
                ui.label("Type");
                ui.label(format!("{} {}", view_type.icon(), view_type.name()));
                ui.end_row();
            }
        });
        if let Some(Tile::Pane(view)) = self.tree.tiles.get_mut(tile_id) {
            view.properties_ui(ui, model);
        }
    }

    /// Shows a field to rename `tile_id`, renaming it while typing unless the name is
    /// blank.
    fn name_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId) {
        let mut name = match &self.properties_name {
            Some((id, name)) if *id == tile_id => name.clone(),
            _ => self.name(tile_id).to_owned(),
        };
        let response = ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
        if response.changed() && !name.trim().is_empty() {
            self.set_name(tile_id, name.trim().to_owned());
        }
        self.properties_name = response.has_focus().then_some((tile_id, name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view::ViewKind;
    use crate::app::view::plot::Interpolation;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// A harness showing the properties panel of the default layout with what
    /// `selected` picks from its tab and the views in it selected.
    fn harness(selected: impl FnOnce(TileId, &[TileId]) -> TileId) -> Harness<'static, Layout> {
        let mut layout = Layout::default();
        let tab = layout.container(layout.root()).unwrap().children_vec()[0];
        let views = layout.container(tab).unwrap().children_vec();
        layout.select(selected(tab, &views));
        let model = Model::default();
        Harness::builder()
            .with_size(egui::vec2(300.0, 800.0))
            .build_ui_state(
                move |ui, layout: &mut Layout| layout.properties_ui(ui, &model),
                layout,
            )
    }

    #[test]
    fn properties_of_the_selected_view_are_changed() {
        let mut harness = harness(|_, views| views[0]);
        harness.run();
        harness.get_by_label("Step").click();
        harness.run();
        let layout = harness.state();
        let view = layout.selected.and_then(|id| layout.view(id)).unwrap();
        let ViewKind::Plot(plot) = &view.kind else {
            panic!("the plot is not selected");
        };
        assert_eq!(plot.interpolation, Interpolation::Step);
    }

    #[test]
    fn groups_are_renamed_while_typing_but_not_to_blank_names() {
        let mut harness = harness(|tab, _| tab);
        harness.run();
        let tab = harness.state().selected.unwrap();
        let name = harness.state().name(tab).to_owned();
        let field = harness.get_by_role(egui::accesskit::Role::TextInput);
        field.focus();
        field.type_text(" x");
        harness.run();
        assert_eq!(harness.state().name(tab), format!("{name} x"));
        for _ in 0..name.len() + 2 {
            harness.key_press(egui::Key::Backspace);
            harness.run();
        }
        assert_eq!(harness.state().name(tab), &name[..1]);
    }
}
