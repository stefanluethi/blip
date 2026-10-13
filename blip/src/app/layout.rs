//! The layout of the views in the center pane: a tree of groups with the views as
//! leaves. The top level groups are tabs, the groups below show their children as tabs,
//! stacked vertically or side by side.

mod file;
mod panel;

pub use file::{GroupKind, LayoutFile};

use crate::app::view::{View, ViewContext, ViewType, Viewable};
use egui_tiles::{Container, Tile, TileId, Tree};
use std::collections::{HashMap, HashSet};

pub struct Layout {
    tree: Tree<View>,
    /// Names of the groups, the views carry their own names.
    group_names: HashMap<TileId, String>,
    /// The view or group selected in the layout panel or by clicking a view.
    selected: Option<TileId>,
    /// The view or group renamed in the layout panel and its new name.
    renaming: Option<(TileId, String)>,
}

impl Default for Layout {
    fn default() -> Self {
        let mut layout = Self::from_file(LayoutFile {
            version: file::VERSION,
            root: file::Group::tabs(vec![]),
        });
        let main = layout.add_tab();
        layout.add_view(ViewType::Plot);
        layout.add_view(ViewType::Logs);
        layout.selected = Some(main);
        layout
    }
}

impl Layout {
    pub fn from_file(file: LayoutFile) -> Self {
        let (tree, group_names) = file.load();
        Self {
            tree,
            group_names,
            selected: None,
            renaming: None,
        }
    }

    /// Restores a saved layout, or returns `None` if it was saved in an unknown format.
    pub fn migrate(file: LayoutFile) -> Option<Self> {
        // migrations of older versions go here
        (file.version == file::VERSION).then(|| Self::from_file(file))
    }

    pub fn to_file(&self) -> LayoutFile {
        LayoutFile::new(&self.tree, self.root(), &self.group_names)
    }

    /// The tab group at the top level.
    fn root(&self) -> TileId {
        self.tree.root().expect("the layout always has a root")
    }

    fn container(&self, tile_id: TileId) -> Option<&Container> {
        self.tree.tiles.get_container(tile_id)
    }

    fn container_mut(&mut self, tile_id: TileId) -> Option<&mut Container> {
        match self.tree.tiles.get_mut(tile_id) {
            Some(Tile::Container(container)) => Some(container),
            _ => None,
        }
    }

    fn view(&self, tile_id: TileId) -> Option<&View> {
        self.tree.tiles.get_pane(&tile_id)
    }

    fn parent(&self, tile_id: TileId) -> Option<TileId> {
        self.tree.tiles.parent_of(tile_id)
    }

    fn name(&self, tile_id: TileId) -> &str {
        match self.view(tile_id) {
            Some(view) => &view.name,
            None => self.group_names.get(&tile_id).map_or("", String::as_str),
        }
    }

    fn set_name(&mut self, tile_id: TileId, name: String) {
        match self.tree.tiles.get_mut(tile_id) {
            Some(Tile::Pane(view)) => view.name = name,
            Some(Tile::Container(_)) => {
                self.group_names.insert(tile_id, name);
            }
            None => {}
        }
    }

    /// The selected view, if a view is selected.
    pub fn selected_view_mut(&mut self) -> Option<&mut View> {
        match self.tree.tiles.get_mut(self.selected?) {
            Some(Tile::Pane(view)) => Some(view),
            _ => None,
        }
    }

    /// Selects a view or group and shows the tabs it is in.
    fn select(&mut self, tile_id: TileId) {
        self.selected = Some(tile_id);
        self.tree.make_active(|id, _| id == tile_id);
    }

    /// A name `"{base} {n}"` that no view or group has yet.
    fn unique_name(&self, base: &str) -> String {
        let names: HashSet<&str> = self
            .tree
            .tiles
            .tile_ids()
            .map(|tile_id| self.name(tile_id))
            .collect();
        (1..)
            .map(|n| format!("{base} {n}"))
            .find(|name| !names.contains(name.as_str()))
            .expect("there is always an unused name")
    }

    /// Adds a new tab at the end of the top level, returns its id.
    fn add_tab(&mut self) -> TileId {
        let name = self.unique_name("Tab");
        let tab = self.tree.tiles.insert_vertical_tile(vec![]);
        self.group_names.insert(tab, name);
        let root = self.root();
        self.tree.move_tile_to_container(tab, root, usize::MAX, true);
        self.select(tab);
        tab
    }

    /// Adds a new view next to the selected view, or into the selected group, or into the
    /// shown tab.
    fn add_view(&mut self, view_type: ViewType) {
        let (container, index) = match self.selected.filter(|&id| self.tree.tiles.get(id).is_some()) {
            Some(selected) if self.container(selected).is_some() => (selected, usize::MAX),
            Some(selected) => {
                let parent = self.parent(selected).unwrap_or(self.root());
                let index = self
                    .container(parent)
                    .and_then(|container| container.children().position(|&id| id == selected))
                    .map_or(usize::MAX, |index| index + 1);
                (parent, index)
            }
            None => {
                let shown_tab = match self.container(self.root()) {
                    Some(Container::Tabs(tabs)) => tabs.active,
                    _ => None,
                };
                match shown_tab.filter(|&tab| self.container(tab).is_some()) {
                    Some(tab) => (tab, usize::MAX),
                    None => (self.add_tab(), usize::MAX),
                }
            }
        };

        let name = self.unique_name(view_type.name());
        let view = self.tree.tiles.insert_pane(View::new(view_type, name));
        self.tree.move_tile_to_container(view, container, index, true);
        self.select(view);
    }

    fn set_group_kind(&mut self, tile_id: TileId, kind: GroupKind) {
        if let Some(container) = self.container_mut(tile_id) {
            container.set_kind(kind.container_kind());
        }
    }

    /// Removes a view or a group with everything in it.
    fn remove(&mut self, tile_id: TileId) {
        if tile_id == self.root() {
            return;
        }
        self.tree.remove_recursively(tile_id);
        self.group_names.retain(|&id, _| self.tree.tiles.get(id).is_some());
        if self.selected.is_some_and(|id| self.tree.tiles.get(id).is_none()) {
            self.selected = None;
        }
        self.remove_empty_groups();
    }

    /// Removes groups left without children, except the top level.
    fn remove_empty_groups(&mut self) {
        let root = self.root();
        let empty = self.tree.tiles.iter().find_map(|(&id, tile)| match tile {
            Tile::Container(container) if id != root && container.is_empty() => Some(id),
            _ => None,
        });
        if let Some(empty) = empty {
            // removing it removes further groups left empty
            self.remove(empty);
        }
    }

    /// Shows the views in the center pane.
    pub fn ui(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.container(self.root()).is_none_or(Container::is_empty) {
            ui.centered_and_justified(|ui| {
                ui.weak("Add a view in the layout panel");
            });
            return;
        }

        let panes_in_tabs = self
            .tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Container(Container::Tabs(tabs)) => Some(tabs.children.iter().copied()),
                _ => None,
            })
            .flatten()
            .collect();
        let mut behavior = Behavior {
            context,
            group_names: &self.group_names,
            selected: &mut self.selected,
            panes_in_tabs,
        };
        self.tree.ui(&mut behavior, ui);
    }
}

/// Shows the views of the layout in the center pane.
struct Behavior<'a, 'b> {
    context: &'a mut ViewContext<'b>,
    group_names: &'a HashMap<TileId, String>,
    selected: &'a mut Option<TileId>,
    /// Views shown as a tab, they get no title bar.
    panes_in_tabs: HashSet<TileId>,
}

impl egui_tiles::Behavior<View> for Behavior<'_, '_> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, view: &mut View) -> egui_tiles::UiResponse {
        // select the view on any click into it, also on clicks handled by its contents
        if ui.input(|i| i.pointer.primary_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            *self.selected = Some(tile_id);
        }

        if !self.panes_in_tabs.contains(&tile_id) {
            let title_bar_color = if ui.visuals().dark_mode {
                ui.visuals().extreme_bg_color
            } else {
                ui.visuals().widgets.inactive.bg_fill
            };
            egui::Frame::NONE
                .fill(title_bar_color)
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("{} {}", view.view_type().icon(), view.name));
                        ui.allocate_space(ui.available_size())
                    })
                });
        }

        egui::Frame::NONE
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.push_id(tile_id, |ui| view.view(ui, self.context));
            });

        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, view: &View) -> egui::WidgetText {
        view.name.as_str().into()
    }

    fn tab_title_for_tile(&mut self, tiles: &egui_tiles::Tiles<View>, tile_id: TileId) -> egui::WidgetText {
        match tiles.get(tile_id) {
            Some(Tile::Pane(view)) => self.tab_title_for_pane(view),
            _ => self.group_names.get(&tile_id).map_or("", String::as_str).into(),
        }
    }

    fn paint_on_top_of_tile(&self, painter: &egui::Painter, style: &egui::Style, tile_id: TileId, rect: egui::Rect) {
        if *self.selected == Some(tile_id) {
            painter.rect_stroke(rect, 0.0, style.visuals.selection.stroke, egui::StrokeKind::Inside);
        }
    }

    fn is_tile_draggable(&self, _tiles: &egui_tiles::Tiles<View>, _tile_id: TileId) -> bool {
        // the layout is edited in the layout panel
        false
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        // the layout keeps the groups as the user made them
        egui_tiles::SimplificationOptions::OFF
    }
}
