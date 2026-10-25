//! The layout of the views in the center pane: a tree of groups with the views as
//! leaves. The top level groups are tabs, the groups below show their children as tabs,
//! stacked vertically or side by side.

mod drag;
mod file;
mod panel;
mod properties;

pub use file::{GroupKind, LayoutFile};

use crate::app::theme;
use crate::app::view::{View, ViewContext, ViewType, Viewable};
use egui_tiles::{Container, ContainerKind, Tile, TileId, Tree};
use std::collections::{HashMap, HashSet};

/// Where a view or group dragged in the layout panel or the center pane is dropped,
/// relative to the row, view or tab it is dropped on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drop {
    Before,
    After,
    /// Into the group dropped on, as its last child.
    Into,
    /// Into a new group together with the view dropped on.
    Group,
    /// At a side of the view dropped on in the center pane: next to it in its group if
    /// the group stacks its children across that side, otherwise into a new group with
    /// the view, stacked across that side.
    Side(Side),
}

/// A side of a view in the center pane.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The kind of group that stacks its children across this side.
    fn kind(self) -> GroupKind {
        match self {
            Side::Left | Side::Right => GroupKind::Horizontal,
            Side::Top | Side::Bottom => GroupKind::Vertical,
        }
    }

    /// Whether a view dropped at this side goes after the view dropped on.
    fn is_after(self) -> bool {
        matches!(self, Side::Right | Side::Bottom)
    }
}

pub struct Layout {
    tree: Tree<View>,
    /// Names of the groups, the views carry their own names.
    group_names: HashMap<TileId, String>,
    /// The view or group selected in the layout panel or by clicking a view.
    selected: Option<TileId>,
    /// The view or group renamed in the layout panel and its new name.
    renaming: Option<(TileId, String)>,
    /// The view or group renamed by clicking its title in the center and its new name.
    renaming_title: Option<(TileId, String)>,
    /// The view or group whose name is edited in the properties panel and the text of
    /// the field, which may be blank while the name is not.
    properties_name: Option<(TileId, String)>,
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
            renaming_title: None,
            properties_name: None,
        }
    }

    /// Restores a saved layout, or returns `None` if it was saved in an unknown format.
    pub fn migrate(file: LayoutFile) -> Option<Self> {
        file.migrate().map(Self::from_file)
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

    /// Gives a view or group the name of `renamed`, unless the name is blank.
    fn finish_rename(&mut self, renamed: Option<(TileId, String)>) {
        if let Some((tile_id, name)) = renamed
            && !name.trim().is_empty()
        {
            self.set_name(tile_id, name.trim().to_owned());
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

    /// Selects a view that shows traces, for tests of the traces panel.
    #[cfg(test)]
    pub fn select_view_showing_traces(&mut self) {
        let view = self.tree.tiles.iter().find_map(|(&tile_id, tile)| {
            let traces = |view: &View| view.has_selection() && !view.view_type().shows_actions();
            matches!(tile, Tile::Pane(view) if traces(view)).then_some(tile_id)
        });
        if let Some(view) = view {
            self.select(view);
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
        self.tree
            .move_tile_to_container(tab, root, usize::MAX, true);
        self.select(tab);
        tab
    }

    /// Where a new view or group is added: next to the selected view, or into the
    /// selected group, or into the shown tab. Returns the group and the index in it.
    fn insertion_point(&mut self) -> (TileId, usize) {
        match self
            .selected
            .filter(|&id| self.tree.tiles.get(id).is_some())
        {
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
        }
    }

    /// Adds a new view at the [insertion point](Self::insertion_point).
    fn add_view(&mut self, view_type: ViewType) {
        let (container, index) = self.insertion_point();
        let name = self.unique_name(view_type.name());
        let view = self.tree.tiles.insert_pane(View::new(view_type, name));
        self.tree
            .move_tile_to_container(view, container, index, true);
        self.select(view);
    }

    /// Adds a new empty group of `kind` at the [insertion point](Self::insertion_point),
    /// returns its id.
    fn add_group(&mut self, kind: GroupKind) -> TileId {
        let (container, index) = self.insertion_point();
        let name = self.unique_name("Group");
        let group = self
            .tree
            .tiles
            .insert_container(Container::new(kind.container_kind(), vec![]));
        self.group_names.insert(group, name);
        self.tree
            .move_tile_to_container(group, container, index, true);
        self.select(group);
        group
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
        let parent = self.parent(tile_id);
        self.tree.remove_recursively(tile_id);
        self.group_names
            .retain(|&id, _| self.tree.tiles.get(id).is_some());
        if self
            .selected
            .is_some_and(|id| self.tree.tiles.get(id).is_none())
        {
            self.selected = None;
        }
        if let Some(parent) = parent {
            self.remove_if_empty(parent);
        }
    }

    /// Removes `group` if a move or removal left it without children, except the top
    /// level. Groups made empty by the user stay, to add views to them.
    fn remove_if_empty(&mut self, group: TileId) {
        if group != self.root() && self.container(group).is_some_and(Container::is_empty) {
            // removing it removes its parent if that is left empty too
            self.remove(group);
        }
    }

    /// Whether `ancestor` is `tile_id` or contains it.
    fn contains(&self, ancestor: TileId, tile_id: TileId) -> bool {
        std::iter::successors(Some(tile_id), |&id| self.parent(id)).any(|id| id == ancestor)
    }

    /// Whether `dragged` can be dropped on `target`, i.e. not into itself.
    fn can_drop(&self, dragged: TileId, target: TileId) -> bool {
        !self.contains(dragged, target)
    }

    /// Moves `dragged` relative to `target`.
    fn drop_tile(&mut self, dragged: TileId, target: TileId, drop: Drop) {
        if !self.can_drop(dragged, target) {
            return;
        }
        let Some(parent) = self.parent(target) else {
            return;
        };
        let old_parent = self.parent(dragged);
        match drop {
            Drop::Before | Drop::After => self.move_next_to(dragged, target, drop == Drop::After),
            Drop::Into => self
                .tree
                .move_tile_to_container(dragged, target, usize::MAX, true),
            Drop::Group => {
                // alternate the direction at each level, the groups at the top level are
                // tabs stacking their views vertically
                let kind = match self.container(parent).map(GroupKind::of) {
                    Some(GroupKind::Vertical) => GroupKind::Horizontal,
                    _ => GroupKind::Vertical,
                };
                self.group_with(dragged, target, kind, true);
            }
            Drop::Side(side) if self.reorders(target, side) => {
                self.move_next_to(dragged, target, side.is_after());
            }
            Drop::Side(side) if self.are_only_children(dragged, target) => {
                // turn the group instead of nesting a new one in it
                self.set_group_kind(parent, side.kind());
                self.move_next_to(dragged, target, side.is_after());
            }
            Drop::Side(side) => self.group_with(dragged, target, side.kind(), side.is_after()),
        }
        if let Some(old_parent) = old_parent {
            self.remove_if_empty(old_parent);
        }
        self.select(dragged);
    }

    /// Whether dropping at `side` of `target` moves the dropped view next to `target` in
    /// its group, instead of grouping them.
    fn reorders(&self, target: TileId, side: Side) -> bool {
        self.parent(target)
            .and_then(|parent| self.container(parent))
            .is_some_and(|parent| GroupKind::of(parent) == side.kind())
    }

    /// Whether `dragged` and `target` are the only children of a vertical or horizontal
    /// group.
    fn are_only_children(&self, dragged: TileId, target: TileId) -> bool {
        let parent = self.parent(target);
        parent == self.parent(dragged)
            && parent
                .and_then(|parent| self.container(parent))
                .is_some_and(|parent| {
                    parent.kind() != ContainerKind::Tabs && parent.num_children() == 2
                })
    }

    /// Moves `dragged` before or after `target` in the group of `target`.
    fn move_next_to(&mut self, dragged: TileId, target: TileId, after: bool) {
        let Some(parent) = self.parent(target) else {
            return;
        };
        let index = self
            .container(parent)
            .and_then(|container| container.children().position(|&id| id == target))
            .map_or(usize::MAX, |index| index + usize::from(after));
        self.tree
            .move_tile_to_container(dragged, parent, index, true);
    }

    /// Puts `target` and `dragged` into a new group of `kind` in place of `target`,
    /// `dragged` before or after it.
    fn group_with(&mut self, dragged: TileId, target: TileId, kind: GroupKind, after: bool) {
        let Some(parent) = self.parent(target) else {
            return;
        };
        if let Some(old_parent) = self.parent(dragged)
            && let Some(container) = self.container_mut(old_parent)
        {
            container.remove_child(dragged);
        }
        let name = self.unique_name(if parent == self.root() {
            "Tab"
        } else {
            "Group"
        });
        let children = if after {
            vec![target, dragged]
        } else {
            vec![dragged, target]
        };
        let group = self
            .tree
            .tiles
            .insert_container(Container::new(kind.container_kind(), children));
        self.group_names.insert(group, name);
        if let Some(container) = self.container_mut(parent) {
            // `target` is a child of `parent`, so it is always replaced
            let _ = container.replace_child(target, group);
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

        let tab_groups: Vec<_> = self
            .tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Container(Container::Tabs(tabs)) => Some(tabs),
                _ => None,
            })
            .collect();
        let panes_in_tabs = tab_groups
            .iter()
            .flat_map(|tabs| tabs.children.iter().copied())
            .collect();
        // egui_tiles takes a group out of the tree while showing its tabs, so which tabs
        // are open is looked up before
        let open_tabs = tab_groups.iter().filter_map(|tabs| tabs.active).collect();
        // a view or group dragged in the center or from the layout panel
        let dragged = egui::DragAndDrop::payload::<TileId>(ui.ctx())
            .map(|dragged| *dragged)
            .filter(|&dragged| self.tree.tiles.get(dragged).is_some());
        let mut behavior = Behavior {
            context,
            group_names: &self.group_names,
            selected: &mut self.selected,
            panes_in_tabs,
            open_tabs,
            renaming: &mut self.renaming_title,
            rename_ended: None,
            tab_bar_bottoms: Vec::new(),
            below_tab_bar: HashSet::new(),
            dragged,
            cards: Vec::new(),
            tab_buttons: Vec::new(),
        };
        self.tree.ui(&mut behavior, ui);
        let shown = drag::Shown {
            cards: std::mem::take(&mut behavior.cards),
            tab_buttons: std::mem::take(&mut behavior.tab_buttons),
        };
        match behavior.rename_ended {
            Some(RenameEnd::Keep) => {
                let renamed = self.renaming_title.take();
                self.finish_rename(renamed);
            }
            Some(RenameEnd::Cancel) => self.renaming_title = None,
            None => {}
        }
        if let Some(dragged) = dragged {
            self.center_drop_ui(ui, dragged, &shown);
        }
    }
}

/// How editing a title ended.
#[derive(Clone, Copy)]
enum RenameEnd {
    Keep,
    Cancel,
}

/// Shows the views of the layout in the center pane.
struct Behavior<'a, 'b> {
    context: &'a mut ViewContext<'b>,
    group_names: &'a HashMap<TileId, String>,
    selected: &'a mut Option<TileId>,
    /// Views shown as a tab, they get no title bar.
    panes_in_tabs: HashSet<TileId>,
    /// The tabs shown in their group.
    open_tabs: HashSet<TileId>,
    /// The view or group whose title is edited and its new name.
    renaming: &'a mut Option<(TileId, String)>,
    /// Set when editing the title ends.
    rename_ended: Option<RenameEnd>,
    /// The bottoms of the tab bars shown so far, which egui_tiles shows before the
    /// tiles below them.
    tab_bar_bottoms: Vec<f32>,
    /// The views right below a tab bar, whose cards leave a gap to it.
    below_tab_bar: HashSet<TileId>,
    /// The view or group being dragged, shown dimmed.
    dragged: Option<TileId>,
    /// The cards of the views shown, to drop on.
    cards: Vec<(TileId, egui::Rect)>,
    /// The tab buttons shown, to drop on.
    tab_buttons: Vec<(TileId, egui::Rect)>,
}

/// Height of the title bar of a view not shown as a tab, which moves it by drag and drop.
const TITLE_BAR_HEIGHT: f32 = 28.0;

/// The card of a view in `rect`, leaving a gap to a tab bar right above it.
fn card_rect(rect: egui::Rect, below_tab_bar: bool) -> egui::Rect {
    if below_tab_bar {
        rect.with_min_y(rect.top() + theme::GAP)
    } else {
        rect
    }
}

impl Behavior<'_, '_> {
    /// Shows a field of at least `min_size` to edit the title of `tile_id` if it is
    /// being edited, ending with Enter or a click elsewhere, or Escape to cancel.
    /// Returns whether the field is shown.
    fn rename_ui(&mut self, ui: &mut egui::Ui, tile_id: TileId, min_size: egui::Vec2) -> bool {
        let Some((_, name)) = self.renaming.as_mut().filter(|(id, _)| *id == tile_id) else {
            return false;
        };
        let field = egui::TextEdit::singleline(name)
            .min_size(min_size)
            .desired_width(min_size.x)
            .margin(egui::Margin::symmetric(4, 0));
        let response = ui.add(field);
        if response.lost_focus() {
            self.rename_ended = Some(if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                RenameEnd::Cancel
            } else {
                RenameEnd::Keep
            });
        } else {
            response.request_focus();
        }
        true
    }
}

impl egui_tiles::Behavior<View> for Behavior<'_, '_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        view: &mut View,
    ) -> egui_tiles::UiResponse {
        // select the view on any click into it, also on clicks handled by its contents
        if ui.input(|i| i.pointer.primary_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
            *self.selected = Some(tile_id);
        }
        let top = ui.max_rect().top();
        let below_tab_bar = self
            .tab_bar_bottoms
            .iter()
            .any(|bottom| (top - bottom).abs() < 1.0);
        if below_tab_bar {
            self.below_tab_bar.insert(tile_id);
            ui.add_space(theme::GAP);
        }
        // each view is a card on the canvas
        let tokens = theme::tokens(ui.visuals());
        let card = card_rect(ui.max_rect(), below_tab_bar);
        self.cards.push((tile_id, card));
        ui.painter().rect(
            card,
            theme::CARD_RADIUS,
            tokens.card,
            egui::Stroke::new(1.0, tokens.border),
            egui::StrokeKind::Inside,
        );

        let titled = !self.panes_in_tabs.contains(&tile_id);
        if titled {
            // the title bar moves the view by drag and drop, it is registered before the
            // title so that clicks on the title still rename the view
            let bar = card.with_max_y(card.top() + TITLE_BAR_HEIGHT);
            ui.interact(
                bar,
                ui.id().with("title_bar"),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab)
            .dnd_set_drag_payload(tile_id);
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 10,
                    right: 10,
                    top: 6,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(view.view_type().icon())
                                .color(ui.visuals().weak_text_color()),
                        );
                        let field_size = egui::vec2(ui.available_width(), 0.0);
                        if !self.rename_ui(ui, tile_id, field_size) {
                            let title = egui::Label::new(egui::RichText::new(&view.name).strong())
                                .sense(egui::Sense::click_and_drag());
                            let title = ui
                                .add(title)
                                .on_hover_cursor(egui::CursorIcon::Text)
                                .on_hover_text("Click to rename, drag to move");
                            title.dnd_set_drag_payload(tile_id);
                            if title.clicked() {
                                *self.renaming = Some((tile_id, view.name.clone()));
                            }
                        }
                        ui.allocate_space(ui.available_size())
                    })
                });
        }

        let top = if titled { 4 } else { 10 };
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top,
                bottom: 10,
            })
            .show(ui, |ui| {
                ui.push_id(tile_id, |ui| view.view(ui, self.context));
            });

        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, view: &View) -> egui::WidgetText {
        view.name.as_str().into()
    }

    fn tab_title_for_tile(
        &mut self,
        tiles: &egui_tiles::Tiles<View>,
        tile_id: TileId,
    ) -> egui::WidgetText {
        match tiles.get(tile_id) {
            Some(Tile::Pane(view)) => self.tab_title_for_pane(view),
            _ => self
                .group_names
                .get(&tile_id)
                .map_or("", String::as_str)
                .into(),
        }
    }

    fn on_tab_button(
        &mut self,
        tiles: &mut egui_tiles::Tiles<View>,
        tile_id: TileId,
        response: egui::Response,
    ) -> egui::Response {
        self.tab_bar_bottoms.push(response.rect.bottom());
        self.tab_buttons.push((tile_id, response.interact_rect));
        // egui_tiles takes a drag of its tab button as a drag of its own, so the tab is
        // moved by a button of blip on top of it
        let response = drag::tab_drag_button(&response, tile_id);
        if self.open_tabs.contains(&tile_id) {
            paint_tab_underline(
                &response.ctx.layer_painter(response.layer_id),
                response.rect,
            );
        }

        // a click on an inactive tab opens it, a click on the open tab renames it
        if response.clicked() && self.open_tabs.contains(&tile_id) {
            let name = self.tab_title_for_tile(tiles, tile_id).text().to_owned();
            *self.renaming = Some((tile_id, name));
        }

        if self.renaming.as_ref().is_some_and(|(id, _)| *id == tile_id) {
            // over the tab, which egui_tiles draws as a whole
            let rect = response.rect;
            egui::Area::new(response.id.with("rename"))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.left_top())
                .show(&response.ctx, |ui| {
                    let min_size = egui::vec2(rect.width().max(120.0), rect.height());
                    self.rename_ui(ui, tile_id, min_size);
                });
        }
        response
    }

    fn paint_on_top_of_tile(
        &self,
        painter: &egui::Painter,
        style: &egui::Style,
        tile_id: TileId,
        rect: egui::Rect,
    ) {
        if self.dragged == Some(tile_id) {
            let rect = card_rect(rect, self.below_tab_bar.contains(&tile_id));
            let veil = theme::tokens(&style.visuals).canvas.gamma_multiply(0.6);
            painter.rect_filled(rect, theme::CARD_RADIUS, veil);
        }
        if *self.selected == Some(tile_id) {
            let stroke = egui::Stroke::new(1.0, theme::tokens(&style.visuals).accent);
            let rect = card_rect(rect, self.below_tab_bar.contains(&tile_id));
            painter.rect_stroke(rect, theme::CARD_RADIUS, stroke, egui::StrokeKind::Inside);
        }
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        theme::GAP
    }

    fn resize_stroke(
        &self,
        style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        match resize_state {
            // the gap between the cards shows the canvas
            egui_tiles::ResizeState::Idle => egui::Stroke::NONE,
            egui_tiles::ResizeState::Hovering => style.visuals.widgets.hovered.fg_stroke,
            egui_tiles::ResizeState::Dragging => style.visuals.widgets.active.fg_stroke,
        }
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        30.0
    }

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        theme::tokens(visuals).canvas
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<View>,
        _tile_id: TileId,
        _state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        // the open tab is marked by its underline, see `on_tab_button`
        egui::Color32::TRANSPARENT
    }

    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<View>,
        _tile_id: TileId,
        _state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        egui::Stroke::NONE
    }

    fn tab_text_color(
        &self,
        visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<View>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            visuals.strong_text_color()
        } else {
            visuals.weak_text_color()
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

/// The underline of the open tab at the bottom of `rect`, in the accent and inset like
/// its title.
fn paint_tab_underline(painter: &egui::Painter, rect: egui::Rect) {
    let accent = theme::tokens(&painter.ctx().global_style().visuals).accent;
    let inset = 4.0;
    let line = egui::Rect::from_x_y_ranges(
        rect.left() + inset..=rect.right() - inset,
        rect.bottom() - 2.0..=rect.bottom(),
    );
    painter.rect_filled(line, 1.0, accent);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::Model;
    use crate::app::time::TimeRange;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn children(layout: &Layout, tile_id: TileId) -> Vec<TileId> {
        layout.container(tile_id).unwrap().children_vec()
    }

    fn kind(layout: &Layout, tile_id: TileId) -> GroupKind {
        GroupKind::of(layout.container(tile_id).unwrap())
    }

    /// The default layout with its tab, plot and log view.
    fn default_layout() -> (Layout, TileId, TileId, TileId) {
        let layout = Layout::default();
        let tab = children(&layout, layout.root())[0];
        let views = children(&layout, tab);
        (layout, tab, views[0], views[1])
    }

    #[test]
    fn default_layout_has_a_tab_with_a_plot_and_logs() {
        let (layout, tab, plot, logs) = default_layout();
        assert_eq!(kind(&layout, tab), GroupKind::Vertical);
        assert_eq!(layout.view(plot).unwrap().view_type(), ViewType::Plot);
        assert_eq!(layout.view(logs).unwrap().view_type(), ViewType::Logs);
    }

    #[test]
    fn dropping_on_a_view_groups_them_across_the_direction_of_the_parent() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(logs, plot, Drop::Group);
        let group = children(&layout, tab)[0];
        assert_eq!(children(&layout, tab), [group]);
        assert_eq!(kind(&layout, group), GroupKind::Horizontal);
        assert_eq!(children(&layout, group), [plot, logs]);

        layout.add_view(ViewType::Plot);
        let new_plot = children(&layout, group)[2];
        layout.drop_tile(new_plot, logs, Drop::Group);
        let inner = children(&layout, group)[1];
        assert_eq!(kind(&layout, inner), GroupKind::Vertical);
        assert_eq!(children(&layout, inner), [logs, new_plot]);
    }

    #[test]
    fn dropping_before_and_after_reorders() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(logs, plot, Drop::Before);
        assert_eq!(children(&layout, tab), [logs, plot]);
        layout.drop_tile(logs, plot, Drop::After);
        assert_eq!(children(&layout, tab), [plot, logs]);
    }

    #[test]
    fn dropping_at_a_side_along_the_group_reorders() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(logs, plot, Drop::Side(Side::Top));
        assert_eq!(children(&layout, tab), [logs, plot]);
        layout.drop_tile(logs, plot, Drop::Side(Side::Bottom));
        assert_eq!(children(&layout, tab), [plot, logs]);
    }

    #[test]
    fn dropping_at_a_side_across_a_group_of_just_the_two_turns_the_group() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(logs, plot, Drop::Side(Side::Left));
        assert_eq!(children(&layout, layout.root()), [tab]);
        assert_eq!(kind(&layout, tab), GroupKind::Horizontal);
        assert_eq!(children(&layout, tab), [logs, plot]);
    }

    #[test]
    fn dropping_at_a_side_across_the_group_groups_in_that_direction() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.select(logs);
        layout.add_view(ViewType::States);
        let states = children(&layout, tab)[2];
        layout.drop_tile(logs, plot, Drop::Side(Side::Left));
        let group = children(&layout, tab)[0];
        assert_eq!(children(&layout, tab), [group, states]);
        assert_eq!(kind(&layout, group), GroupKind::Horizontal);
        assert_eq!(children(&layout, group), [logs, plot]);
        assert_eq!(layout.name(group), "Group 1");

        // the side decides the direction, not the level
        layout.select(plot);
        layout.add_view(ViewType::Table);
        let table = children(&layout, group)[2];
        layout.drop_tile(table, logs, Drop::Side(Side::Left));
        assert_eq!(children(&layout, group), [table, logs, plot]);
        layout.drop_tile(table, plot, Drop::Side(Side::Bottom));
        let inner = children(&layout, group)[1];
        assert_eq!(children(&layout, group), [logs, inner]);
        assert_eq!(kind(&layout, inner), GroupKind::Vertical);
        assert_eq!(children(&layout, inner), [plot, table]);
    }

    #[test]
    fn dropping_at_a_side_of_a_tab_groups_it_in_its_place() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(plot, tab, Drop::Before);
        assert_eq!(children(&layout, layout.root()), [plot, tab]);
        layout.drop_tile(logs, plot, Drop::Side(Side::Bottom));
        let new_tab = children(&layout, layout.root())[0];
        assert_eq!(kind(&layout, new_tab), GroupKind::Vertical);
        assert_eq!(children(&layout, new_tab), [plot, logs]);
        assert!(layout.name(new_tab).starts_with("Tab "));
        // the old tab was left empty by the move
        assert_eq!(children(&layout, layout.root()), [new_tab]);
    }

    #[test]
    fn moving_the_last_view_out_of_a_group_removes_the_group() {
        let (mut layout, tab, plot, logs) = default_layout();
        let new_tab = layout.add_tab();
        layout.drop_tile(plot, new_tab, Drop::Into);
        layout.drop_tile(logs, new_tab, Drop::Into);
        assert_eq!(children(&layout, layout.root()), [new_tab]);
        assert!(layout.tree.tiles.get(tab).is_none());
    }

    #[test]
    fn a_group_is_added_next_to_the_selected_view_and_kept_while_empty() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.select(plot);
        let group = layout.add_group(GroupKind::Horizontal);
        assert_eq!(children(&layout, tab), [plot, group, logs]);
        assert_eq!(kind(&layout, group), GroupKind::Horizontal);

        // moving other views around keeps the new group, adding a view puts it inside
        layout.drop_tile(logs, plot, Drop::Before);
        assert_eq!(children(&layout, tab), [logs, plot, group]);
        layout.select(group);
        layout.add_view(ViewType::Table);
        assert_eq!(children(&layout, group).len(), 1);
    }

    #[test]
    fn a_group_cannot_be_dropped_into_itself() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(tab, plot, Drop::Group);
        layout.drop_tile(tab, tab, Drop::Into);
        assert_eq!(children(&layout, layout.root()), [tab]);
        assert_eq!(children(&layout, tab), [plot, logs]);
    }

    #[test]
    fn clicking_the_open_tab_renames_it() {
        let mut layout = Layout::default();
        let main = children(&layout, layout.root())[0];
        layout.add_tab();
        let renaming = Rc::new(RefCell::new(None));
        let renamed = renaming.clone();
        let model = Model::default();
        let mut time = TimeRange::default();
        let mut invoker = crate::app::model::actions::Invoker::default();
        let mut harness = Harness::builder().build_ui(move |ui| {
            let mut context = ViewContext {
                model: &model,
                time: &mut time,
                invoker: &mut invoker,
                formats: &crate::app::view::NO_FORMATS,
            };
            layout.ui(ui, &mut context);
            *renamed.borrow_mut() = layout.renaming_title.as_ref().map(|(id, _)| *id);
        });
        harness.run();

        harness.get_by_label("Tab 1").click();
        harness.run();
        assert_eq!(*renaming.borrow(), None);
        harness.get_by_label("Tab 1").click();
        harness.run();
        assert_eq!(*renaming.borrow(), Some(main));
    }

    /// A harness showing `layout` in the center pane.
    fn center_harness(layout: Rc<RefCell<Layout>>) -> Harness<'static> {
        let model = Model::default();
        let mut time = TimeRange::default();
        let mut invoker = crate::app::model::actions::Invoker::default();
        Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(move |ui| {
                let mut context = ViewContext {
                    model: &model,
                    time: &mut time,
                    invoker: &mut invoker,
                    formats: &crate::app::view::NO_FORMATS,
                };
                layout.borrow_mut().ui(ui, &mut context);
            })
    }

    #[test]
    fn dragging_a_view_by_its_title_to_the_side_of_another_moves_it_there() {
        let (layout, tab, plot, logs) = default_layout();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();

        let title = harness.get_by_label("Plot 1").rect().center();
        let logs_title = harness.get_by_label("Logs 1").rect().center();
        // the right side of the log view, below its title
        let target = egui::pos2(780.0, logs_title.y + 80.0);
        harness.drag_at(title);
        harness.run();
        harness.hover_at(title + egui::vec2(40.0, 40.0));
        harness.run();
        harness.hover_at(target);
        harness.run();
        harness.drop_at(target);
        harness.run();

        // the tab held just the two views, so it turns
        let layout = layout.borrow();
        assert_eq!(kind(&layout, tab), GroupKind::Horizontal);
        assert_eq!(children(&layout, tab), [logs, plot]);
        assert_eq!(layout.selected, Some(plot));
    }

    #[test]
    fn dragging_a_tab_onto_another_tab_reorders_the_tabs() {
        let (mut layout, tab, _, _) = default_layout();
        let second = layout.add_tab();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();

        let from = harness.get_by_label("Tab 2").rect().center();
        let to = harness.get_by_label("Tab 1").rect().left_center() + egui::vec2(4.0, 0.0);
        harness.drag_at(from);
        harness.run();
        harness.hover_at(from + egui::vec2(-20.0, 0.0));
        harness.run();
        harness.hover_at(to);
        harness.run();
        harness.drop_at(to);
        harness.run();

        let layout = layout.borrow();
        assert_eq!(children(&layout, layout.root()), [second, tab]);
    }

    #[test]
    fn clicking_a_title_renames_the_view_without_moving_it() {
        let (layout, tab, plot, logs) = default_layout();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Plot 1").click();
        harness.run();
        let layout = layout.borrow();
        assert_eq!(children(&layout, tab), [plot, logs]);
        assert_eq!(
            layout.renaming_title.as_ref().map(|(id, _)| *id),
            Some(plot)
        );
    }

    #[test]
    fn removing_a_group_removes_its_views() {
        let (mut layout, tab, plot, _) = default_layout();
        layout.remove(tab);
        assert!(children(&layout, layout.root()).is_empty());
        assert!(layout.view(plot).is_none());
    }
}
