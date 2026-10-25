//! The properties panel: the properties of the selected view or group.

use super::{GroupKind, Layout};
use crate::app::theme;
use crate::app::view::{PropertiesContext, Viewable, properties_section, property_label};
use egui_tiles::{Tile, TileId};

impl Layout {
    pub fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        theme::section_header(ui, "Properties", |_| {});
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
                None => self.view_ui(ui, tile_id, context),
            });
    }

    fn group_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, current: GroupKind) {
        title_row(ui, current.icon(), self.name(tile_id), "Group");
        properties_section(ui, "Group", |ui| {
            property_label(ui, "Name");
            self.name_ui(ui, tile_id);
            ui.end_row();
            property_label(ui, "Arrange");
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

    fn view_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, context: &PropertiesContext<'_>) {
        if let Some(view) = self.view(tile_id) {
            let view_type = view.view_type();
            title_row(ui, view_type.icon(), &view.name, view_type.name());
        }
        properties_section(ui, "View", |ui| {
            property_label(ui, "Name");
            self.name_ui(ui, tile_id);
            ui.end_row();
        });
        if let Some(Tile::Pane(view)) = self.tree.tiles.get_mut(tile_id) {
            view.properties_ui(ui, context);
        }
    }

    /// Shows a field to rename `tile_id`, renaming it while typing unless the name is
    /// blank.
    fn name_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId) {
        let mut name = match &self.properties_name {
            Some((id, name)) if *id == tile_id => name.clone(),
            _ => self.name(tile_id).to_owned(),
        };
        let field = egui::TextEdit::singleline(&mut name)
            .margin(theme::FIELD_MARGIN)
            .desired_width(f32::INFINITY);
        let response = ui.add(field);
        if response.changed() && !name.trim().is_empty() {
            self.set_name(tile_id, name.trim().to_owned());
        }
        self.properties_name = response.has_focus().then_some((tile_id, name));
    }
}

/// The name of the selected view or group with its `icon`, and its `kind` in a chip on
/// the right.
fn title_row(ui: &mut egui::Ui, icon: &str, name: &str, kind: &str) {
    // both ends are centered in one row as high as the controls
    let size = egui::vec2(ui.available_width(), theme::CONTROL_HEIGHT);
    let (row, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let mut end = |align: egui::Align| {
        let layout = match align {
            egui::Align::Max => egui::Layout::right_to_left(egui::Align::Center),
            _ => egui::Layout::left_to_right(egui::Align::Center),
        };
        ui.new_child(egui::UiBuilder::new().max_rect(row).layout(layout))
    };
    let mut left = end(egui::Align::Min);
    let accent = theme::tokens(left.visuals()).accent;
    left.label(egui::RichText::new(icon).size(15.0).color(accent));
    left.label(egui::RichText::new(name).size(14.0).strong());
    let mut right = end(egui::Align::Max);
    let visuals = right.visuals();
    egui::Frame::NONE
        .fill(visuals.faint_bg_color)
        .stroke(visuals.widgets.noninteractive.bg_stroke)
        .corner_radius(theme::CONTROL_RADIUS)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(&mut right, |ui| {
            let color = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(kind).size(11.0).color(color));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::Model;
    use crate::app::model::numbers::DisplayFormat;
    use crate::app::view::ViewKind;
    use crate::app::view::plot::Interpolation;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// A harness showing the properties panel of the default layout with what
    /// `selected` picks from its tab and the views in it selected.
    fn harness(selected: impl FnOnce(TileId, &[TileId]) -> TileId) -> Harness<'static, Layout> {
        harness_with(Model::default(), selected)
    }

    /// A harness as by [`harness`] showing the traces of `model`.
    fn harness_with(
        model: Model,
        selected: impl FnOnce(TileId, &[TileId]) -> TileId,
    ) -> Harness<'static, Layout> {
        let mut layout = Layout::default();
        let tab = layout.container(layout.root()).unwrap().children_vec()[0];
        let views = layout.container(tab).unwrap().children_vec();
        layout.select(selected(tab, &views));
        Harness::builder()
            .with_size(egui::vec2(300.0, 800.0))
            .build_ui_state(
                move |ui, layout: &mut Layout| {
                    let connected = Default::default();
                    let context = PropertiesContext {
                        model: &model,
                        connected: &connected,
                    };
                    layout.properties_ui(ui, &context);
                },
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

    /// The plot selected in `harness`.
    fn selected_plot(harness: &Harness<'_, Layout>) -> crate::app::view::plot::Plot {
        let layout = harness.state();
        match &layout.selected.and_then(|id| layout.view(id)).unwrap().kind {
            ViewKind::Plot(plot) => plot.clone(),
            _ => panic!("the plot is not selected"),
        }
    }

    #[test]
    fn number_fields_keep_partial_numbers_and_are_auto_when_empty() {
        let mut harness = harness(|_, views| views[0]);
        harness.run();
        // the fields are the name, the y axis min, max and label
        fn min<'a>(harness: &'a Harness<'_, Layout>) -> egui_kittest::Node<'a> {
            let mut fields = harness.get_all_by_role(egui::accesskit::Role::TextInput);
            fields.nth(1).unwrap()
        }
        min(&harness).focus();
        min(&harness).type_text("-");
        harness.run();
        assert_eq!(selected_plot(&harness).y_min, None);
        min(&harness).type_text("2");
        harness.run();
        assert_eq!(selected_plot(&harness).y_min, Some(-2.0));
        for _ in 0..2 {
            harness.key_press(egui::Key::Backspace);
            harness.run();
        }
        assert_eq!(selected_plot(&harness).y_min, None);
    }

    #[test]
    fn numbers_take_a_format_and_other_traces_do_not() {
        let mut model = Model::default();
        for (path, value) in [
            ("motor/errors", proto::Data::Uint(3)),
            ("motor/state", proto::Data::from("on")),
        ] {
            model.traces.append(&proto::Value {
                timestamp: 0,
                path: path.to_owned(),
                value,
            });
        }
        model.traces.set_unit("motor/errors", "count");
        let mut harness = harness_with(model, |_, views| views[0]);
        let view = harness.state_mut().selected_view_mut().unwrap();
        let selection = view.selection_mut().unwrap().0;
        selection.insert("motor/errors".to_owned());
        selection.insert("motor/state".to_owned());
        harness.run();
        harness.get_by_label("count");
        // a format for the number, a disabled one for the string
        let format = |disabled: bool| {
            move |node: &egui_kittest::kittest::AccessKitNode<'_>| {
                node.role() == egui::accesskit::Role::ComboBox
                    && node.value().as_deref() == Some("Default")
                    && node.is_disabled() == disabled
            }
        };
        assert_eq!(harness.get_all_by(format(true)).count(), 1);
        harness.get_by(format(false)).click();
        harness.run();
        harness.get_by_label("Hex").click();
        harness.run();
        let view = harness.state_mut().selected_view_mut().unwrap();
        assert_eq!(view.formats.of("motor/errors"), DisplayFormat::Hex);
    }

    #[test]
    fn floats_are_not_offered_hex_or_binary() {
        let mut model = Model::default();
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: "motor/speed".to_owned(),
            value: proto::Data::Float(1.5),
        });
        let mut harness = harness_with(model, |_, views| views[0]);
        let view = harness.state_mut().selected_view_mut().unwrap();
        view.selection_mut()
            .unwrap()
            .0
            .insert("motor/speed".to_owned());
        harness.run();
        harness
            .get_by(|node| {
                node.role() == egui::accesskit::Role::ComboBox
                    && node.value().as_deref() == Some("Default")
            })
            .click();
        harness.run();
        harness.get_by_label("Scientific");
        assert!(harness.query_by_label("Hex").is_none());
        assert!(harness.query_by_label("Binary").is_none());
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
