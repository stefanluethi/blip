//! The properties panel: the properties of the selected view or group.

use super::{GroupKind, Layout};
use crate::widgets::theme::{self, TokensExt};
use crate::widgets::view::{PropertiesContext, Viewable, properties_section, property_label};
use egui_tiles::{Tile, TileId};

impl Layout {
    pub fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        let plots = self.plots(context.store);
        let context = &PropertiesContext {
            plots: &plots,
            ..*context
        };
        theme::section_header(ui, "Properties", |_| {});
        let Some(tile_id) = self
            .selected
            .filter(|&id| self.tree.tiles.get(id).is_some())
        else {
            theme::panel_hint(ui, "Select a view or a group to see its properties");
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
            let options = GroupKind::ALL.map(|kind| (kind, kind.name()));
            let mut kind = current;
            if theme::segmented(ui, &mut kind, &options) {
                self.set_group_kind(tile_id, kind);
            }
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
    /// blank. A blank field shows the name kept as its hint, and shows it again once the
    /// field loses the focus.
    fn name_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId) {
        let mut name = match &self.properties_name {
            Some((id, name)) if *id == tile_id => name.clone(),
            _ => self.name(tile_id).to_owned(),
        };
        let kept = self.name(tile_id).to_owned();
        let field = theme::wide_field(&mut name).hint_text(kept);
        let response = ui.add(field);
        if response.changed() && !name.trim().is_empty() {
            self.set_name(tile_id, name.trim().to_owned());
        }
        self.properties_name = response.has_focus().then_some((tile_id, name));
    }
}

/// The name of the selected view or group with its `icon`, and its `kind` in a chip on
/// the right. A long name is cut off before the chip.
fn title_row(ui: &mut egui::Ui, icon: &str, name: &str, kind: &str) {
    // both ends are centered in one row as high as the controls
    let size = egui::vec2(ui.available_width(), theme::CONTROL_HEIGHT);
    let (row, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let end = |ui: &mut egui::Ui, rect: egui::Rect, layout: egui::Layout| {
        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout))
    };
    // the chip first, so that the name takes the room it leaves; a tinted patch without
    // the stroke of a button, as it cannot be clicked
    let mut right = end(ui, row, egui::Layout::right_to_left(egui::Align::Center));
    let chip = egui::Frame::NONE
        .fill(right.visuals().faint_bg_color)
        .corner_radius(theme::CONTROL_RADIUS)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(&mut right, |ui| {
            let color = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(kind).small().color(color));
        })
        .response
        .rect;
    let rest = row.with_max_x((chip.left() - theme::GAP).max(row.left()));
    let mut left = end(ui, rest, egui::Layout::left_to_right(egui::Align::Center));
    left.set_clip_rect(rest.intersect(left.clip_rect()));
    let accent = left.tokens().accent;
    left.label(egui::RichText::new(icon).color(accent));
    left.add(egui::Label::new(egui::RichText::new(name).strong()).truncate());
}

#[cfg(test)]
mod tests;
