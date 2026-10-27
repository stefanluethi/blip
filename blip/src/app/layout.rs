//! The layout of the views in the center pane: a tree of groups with the views as
//! leaves. The top level groups are tabs, the groups below show their children as tabs,
//! stacked vertically or side by side.

mod drag;
mod file;
mod properties;

pub use file::{GroupKind, LayoutFile};

use crate::app::theme;
use crate::app::view::{View, ViewContext, ViewType, Viewable};
use egui_tiles::{Container, ContainerKind, Tile, TileId, Tree};
use std::collections::{HashMap, HashSet};

/// Where a view or group dragged in the center pane is dropped, relative to the view or
/// tab it is dropped on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drop {
    Before,
    After,
    /// Into the group dropped on, as its last child.
    Into,
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

/// A view or group whose name is edited in a field on its title or tab, and the text of
/// the field.
struct Renaming {
    tile_id: TileId,
    name: String,
}

/// A change of the layout asked for in the center pane, applied after it is shown.
enum Action {
    /// Adds a view of a type at a side of a view.
    AddView(TileId, Side, ViewType),
    /// Adds a view of a type into an empty group.
    AddViewInto(TileId, ViewType),
    Rename(TileId),
    Remove(TileId),
}

/// Shows a field of at least `min_size` to edit `name`, which ends with Enter or a click
/// elsewhere, or with Escape to cancel. Returns whether to keep the name once it ended.
fn rename_field(ui: &mut egui::Ui, name: &mut String, min_size: egui::Vec2) -> Option<bool> {
    let field = egui::TextEdit::singleline(name)
        .min_size(min_size)
        .desired_width(min_size.x)
        .margin(theme::FIELD_MARGIN);
    let response = ui.add(field);
    if response.lost_focus() {
        Some(!ui.input(|i| i.key_pressed(egui::Key::Escape)))
    } else {
        response.request_focus();
        None
    }
}

pub struct Layout {
    tree: Tree<View>,
    /// Names of the groups, the views carry their own names.
    group_names: HashMap<TileId, String>,
    /// The view or group selected by clicking it or its tab.
    selected: Option<TileId>,
    /// The view or group renamed on its title or tab.
    renaming: Option<Renaming>,
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
        layout.add_view_into(main, ViewType::Plot);
        layout.add_view_into(main, ViewType::Logs);
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

    /// Starts renaming `tile_id` on its title or tab.
    fn start_rename(&mut self, tile_id: TileId) {
        let name = self.name(tile_id).to_owned();
        self.renaming = Some(Renaming { tile_id, name });
    }

    /// Ends renaming, giving the view or group its new name if `keep` and the name is
    /// not blank.
    fn end_rename(&mut self, keep: bool) {
        if let Some(Renaming { tile_id, name, .. }) = self.renaming.take()
            && keep
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

    /// Selects the view or group named `name` and shows the tabs it is in, for the
    /// screenshots. Returns whether there is one.
    #[cfg(test)]
    pub fn select_named(&mut self, name: &str) -> bool {
        let found = (self.tree.tiles.iter())
            .map(|(&tile_id, _)| tile_id)
            .find(|&tile_id| self.name(tile_id) == name);
        if let Some(tile_id) = found {
            self.select(tile_id);
        }
        found.is_some()
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

    /// A new view of `view_type`, not yet in the layout.
    fn new_view(&mut self, view_type: ViewType) -> TileId {
        let name = self.unique_name(view_type.name());
        self.tree.tiles.insert_pane(View::new(view_type, name))
    }

    /// Adds a new view of `view_type` at the end of `group` and selects it, returns its
    /// id.
    fn add_view_into(&mut self, group: TileId, view_type: ViewType) -> TileId {
        let view = self.new_view(view_type);
        self.tree
            .move_tile_to_container(view, group, usize::MAX, true);
        self.select(view);
        view
    }

    /// Adds a new view of `view_type` at `side` of `target` and selects it, as if it was
    /// dropped there. Returns its id, or `None` if `target` is not in a group.
    fn add_view_beside(
        &mut self,
        target: TileId,
        side: Side,
        view_type: ViewType,
    ) -> Option<TileId> {
        // a new view can always be dropped next to a view in a group
        self.parent(target)?;
        let view = self.new_view(view_type);
        self.drop_tile(view, target, Drop::Side(side));
        Some(view)
    }

    fn set_group_kind(&mut self, tile_id: TileId, kind: GroupKind) {
        if let Some(container) = self.container_mut(tile_id) {
            container.set_kind(kind.container_kind());
        }
    }

    /// Removes a view or a group with everything in it. A tab left empty stays, to add
    /// views to it again.
    fn remove(&mut self, tile_id: TileId) {
        let parent = self.parent(tile_id);
        self.remove_tile(tile_id);
        if let Some(parent) = parent {
            self.remove_if_empty(parent, true);
        }
    }

    /// Removes a view or a group with everything in it, but not the groups it leaves
    /// empty.
    fn remove_tile(&mut self, tile_id: TileId) {
        if tile_id == self.root() {
            return;
        }
        self.tree.remove_recursively(tile_id);
        self.group_names
            .retain(|&id, _| self.tree.tiles.get(id).is_some());
        if self
            .selected
            .is_some_and(|id| self.tree.tiles.get(id).is_none())
        {
            self.selected = None;
        }
    }

    /// Removes `group` if a move or removal left it without children, and its parent if
    /// that is left empty too, but not the top level, nor a tab if `keep_tab`.
    fn remove_if_empty(&mut self, group: TileId, keep_tab: bool) {
        let is_tab = self.parent(group) == Some(self.root());
        if group != self.root()
            && !(keep_tab && is_tab)
            && self.container(group).is_some_and(Container::is_empty)
        {
            let parent = self.parent(group);
            self.remove_tile(group);
            if let Some(parent) = parent {
                self.remove_if_empty(parent, keep_tab);
            }
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
            self.remove_if_empty(old_parent, false);
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
            if let Some(view_type) = view_picker(ui, ui.max_rect(), self.root()) {
                let tab = self.add_tab();
                self.add_view_into(tab, view_type);
            }
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
        // a view or group dragged in the center
        let dragged = egui::DragAndDrop::payload::<TileId>(ui.ctx())
            .map(|dragged| *dragged)
            .filter(|&dragged| self.tree.tiles.get(dragged).is_some());
        let mut behavior = Behavior {
            context,
            root: self.root(),
            // the center pane is clipped to its edges, without the gap around the views
            center_x: ui.clip_rect().x_range(),
            add_tab_clicked: false,
            group_names: &self.group_names,
            selected: &mut self.selected,
            panes_in_tabs,
            open_tabs,
            renaming: &mut self.renaming,
            rename_ended: None,
            tab_bar_bottoms: Vec::new(),
            below_tab_bar: HashSet::new(),
            dragged,
            cards: Vec::new(),
            tab_buttons: Vec::new(),
            actions: Vec::new(),
        };
        self.tree.ui(&mut behavior, ui);
        let add_tab_clicked = behavior.add_tab_clicked;
        let shown = drag::Shown {
            cards: std::mem::take(&mut behavior.cards),
            tab_buttons: std::mem::take(&mut behavior.tab_buttons),
        };
        let rename_ended = behavior.rename_ended;
        let mut actions = std::mem::take(&mut behavior.actions);
        actions.extend(self.empty_groups_ui(ui));
        if let Some(keep) = rename_ended {
            self.end_rename(keep);
        }
        for action in actions {
            self.apply(action);
        }
        if add_tab_clicked {
            self.add_tab();
        }
        if let Some(dragged) = dragged {
            self.center_drop_ui(ui, dragged, &shown);
        }
    }

    /// Shows the types of views to add in the empty groups shown, like a new tab.
    fn empty_groups_ui(&self, ui: &mut egui::Ui) -> Vec<Action> {
        let mut actions = vec![];
        for tile_id in self.tree.active_tiles() {
            if tile_id == self.root() || !self.container(tile_id).is_some_and(Container::is_empty) {
                continue;
            }
            if let Some(rect) = self.tree.tiles.rect(tile_id)
                && let Some(view_type) = view_picker(ui, rect, tile_id)
            {
                actions.push(Action::AddViewInto(tile_id, view_type));
            }
        }
        actions
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::AddView(target, side, view_type) => {
                self.add_view_beside(target, side, view_type);
            }
            Action::AddViewInto(group, view_type) => {
                self.add_view_into(group, view_type);
            }
            Action::Rename(tile_id) => self.start_rename(tile_id),
            Action::Remove(tile_id) => self.remove(tile_id),
        }
    }
}

/// Size of a button of [`view_picker`].
const PICKER_BUTTON: egui::Vec2 = egui::vec2(96.0, 72.0);

/// Shows the types of views centered in `rect` of an empty group `group`, as buttons
/// with their icon above their name. Returns the type clicked.
fn view_picker(ui: &mut egui::Ui, rect: egui::Rect, group: TileId) -> Option<ViewType> {
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("view_picker", group))
            .max_rect(rect),
    );
    // a small group shows a part of the buttons, which neither paint nor take clicks
    // outside it
    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
    let tokens = theme::tokens(ui.visuals());
    let gap = theme::GAP;
    let types = ViewType::ALL;
    // as many columns as fit with a gap between and around them, n·(button + gap) + gap
    // ≤ width, at most half of the types, so that they form two rows or more
    let fit = ((rect.width() - gap) / (PICKER_BUTTON.x + gap)).floor() as usize;
    let columns = fit.clamp(1, types.len().div_ceil(2));
    let rows = types.len().div_ceil(columns);
    let title_height = 28.0;
    let size = egui::vec2(
        columns as f32 * (PICKER_BUTTON.x + gap) - gap,
        title_height + rows as f32 * (PICKER_BUTTON.y + gap) - gap,
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
                title_height + row as f32 * (PICKER_BUTTON.y + gap),
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

/// The menu of a view or group, opened by a right click on its title or tab: views can
/// be added at its sides, and it can be renamed or deleted.
fn tile_menu(ui: &mut egui::Ui, tile_id: TileId, is_view: bool, actions: &mut Vec<Action>) {
    if is_view {
        let sides = [
            (egui_phosphor::regular::COLUMNS, "Add right", Side::Right),
            (egui_phosphor::regular::ROWS, "Add below", Side::Bottom),
        ];
        for (icon, text, side) in sides {
            ui.menu_button(format!("{icon} {text}"), |ui| {
                for view_type in ViewType::ALL {
                    if ui
                        .button(format!("{} {}", view_type.icon(), view_type.name()))
                        .clicked()
                    {
                        actions.push(Action::AddView(tile_id, side, view_type));
                    }
                }
            });
        }
        ui.separator();
    }
    if ui
        .button(format!("{} Rename", egui_phosphor::regular::PENCIL_SIMPLE))
        .clicked()
    {
        actions.push(Action::Rename(tile_id));
    }
    if ui
        .button(format!("{} Delete", egui_phosphor::regular::TRASH))
        .clicked()
    {
        actions.push(Action::Remove(tile_id));
    }
}

/// Shows the views of the layout in the center pane.
struct Behavior<'a, 'b> {
    context: &'a mut ViewContext<'b>,
    /// The tab group at the top level, whose tab bar has a button to add a tab.
    root: TileId,
    /// The center pane from edge to edge, which the line under the top tab bar spans.
    center_x: egui::Rangef,
    /// Set when the button to add a tab is clicked.
    add_tab_clicked: bool,
    group_names: &'a HashMap<TileId, String>,
    selected: &'a mut Option<TileId>,
    /// Views shown as a tab, they get no title bar.
    panes_in_tabs: HashSet<TileId>,
    /// The tabs shown in their group.
    open_tabs: HashSet<TileId>,
    /// The view or group renamed, see [`Layout::renaming`].
    renaming: &'a mut Option<Renaming>,
    /// Set when editing the title ends, whether to keep the name.
    rename_ended: Option<bool>,
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
    /// The changes chosen in the menus of the views and tabs.
    actions: Vec<Action>,
}

/// Height of a tab bar, the line under it at its bottom.
const TAB_BAR_HEIGHT: f32 = 28.0;

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
        let Some(renaming) = self
            .renaming
            .as_mut()
            .filter(|renaming| renaming.tile_id == tile_id)
        else {
            return false;
        };
        if let Some(keep) = rename_field(ui, &mut renaming.name, min_size) {
            self.rename_ended = Some(keep);
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
            let bar = ui
                .interact(
                    bar,
                    ui.id().with("title_bar"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            bar.dnd_set_drag_payload(tile_id);
            let title = egui::Frame::NONE
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
                        if self.rename_ui(ui, tile_id, field_size) {
                            return None;
                        }
                        let title = egui::Label::new(egui::RichText::new(&view.name).strong())
                            .sense(egui::Sense::click_and_drag());
                        let title = ui
                            .add(title)
                            .on_hover_cursor(egui::CursorIcon::Text)
                            .on_hover_text("Click to rename, drag to move, right-click for more");
                        title.dnd_set_drag_payload(tile_id);
                        if title.clicked() {
                            *self.renaming = Some(Renaming {
                                tile_id,
                                name: view.name.clone(),
                            });
                        }
                        ui.allocate_space(ui.available_size());
                        Some(title)
                    })
                    .inner
                })
                .inner;
            // the title lies on the bar and takes its right clicks there
            let bar = match title {
                Some(title) => bar.union(title),
                None => bar,
            };
            bar.context_menu(|ui| tile_menu(ui, tile_id, true, &mut self.actions));
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

        // a click on an inactive tab opens it and keeps the selection, a click on the open
        // tab selects and renames it
        if response.clicked() && self.open_tabs.contains(&tile_id) {
            let name = self.tab_title_for_tile(tiles, tile_id).text().to_owned();
            *self.renaming = Some(Renaming { tile_id, name });
            *self.selected = Some(tile_id);
        }
        let is_view = matches!(tiles.get(tile_id), Some(Tile::Pane(_)));
        response.context_menu(|ui| tile_menu(ui, tile_id, is_view, &mut self.actions));

        if self
            .renaming
            .as_ref()
            .is_some_and(|renaming| renaming.tile_id == tile_id)
        {
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
        TAB_BAR_HEIGHT
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<View>,
        ui: &mut egui::Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        // the line under the tabs, painted before them so that the underline of the open
        // tab lies on it, under the top tabs from edge to edge of the center pane
        let bar = ui.max_rect();
        let x = if tile_id == self.root {
            self.center_x
        } else {
            bar.x_range()
        };
        let line = egui::Rect::from_x_y_ranges(x, bar.bottom() - 1.0..=bar.bottom());
        // egui_tiles clips the tab bar to the tiles, which leaves out the gap around them
        let mut painter = ui.painter().clone();
        painter.set_clip_rect(line);
        painter.rect_filled(line, 0.0, theme::tokens(ui.visuals()).border);
    }

    fn tab_bar_trailing_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<View>,
        ui: &mut egui::Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
    ) {
        if tile_id != self.root {
            return;
        }
        ui.add_space(4.0);
        let add = ui
            .add(theme::icon_button(egui_phosphor::regular::PLUS))
            .on_hover_text("Add a tab");
        if add.clicked() {
            self.add_tab_clicked = true;
        }
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
        // blip moves the views itself, see `drag`
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
    fn a_view_added_at_a_side_across_the_group_is_grouped_with_the_view() {
        let (mut layout, tab, plot, logs) = default_layout();
        let table = layout
            .add_view_beside(plot, Side::Right, ViewType::Table)
            .unwrap();
        let group = children(&layout, tab)[0];
        assert_eq!(children(&layout, tab), [group, logs]);
        assert_eq!(kind(&layout, group), GroupKind::Horizontal);
        assert_eq!(children(&layout, group), [plot, table]);
        assert_eq!(layout.view(table).unwrap().view_type(), ViewType::Table);
        assert_eq!(layout.selected, Some(table));
    }

    #[test]
    fn a_view_added_at_a_side_along_the_group_goes_next_to_the_view() {
        let (mut layout, tab, plot, logs) = default_layout();
        let states = layout
            .add_view_beside(plot, Side::Bottom, ViewType::States)
            .unwrap();
        assert_eq!(children(&layout, tab), [plot, states, logs]);
        assert_eq!(layout.name(states), "States 1");
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
        let states = layout.add_view_into(tab, ViewType::States);
        layout.drop_tile(logs, plot, Drop::Side(Side::Left));
        let group = children(&layout, tab)[0];
        assert_eq!(children(&layout, tab), [group, states]);
        assert_eq!(kind(&layout, group), GroupKind::Horizontal);
        assert_eq!(children(&layout, group), [logs, plot]);
        assert_eq!(layout.name(group), "Group 1");

        // the side decides the direction, not the level
        let table = layout.add_view_into(group, ViewType::Table);
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
    fn deleting_the_last_view_of_a_tab_keeps_the_tab() {
        let (mut layout, tab, plot, logs) = default_layout();
        let table = layout
            .add_view_beside(plot, Side::Right, ViewType::Table)
            .unwrap();
        let group = children(&layout, tab)[0];
        layout.remove(table);
        layout.remove(plot);
        assert!(layout.tree.tiles.get(group).is_none());
        layout.remove(logs);
        assert_eq!(children(&layout, layout.root()), [tab]);
        assert!(children(&layout, tab).is_empty());
    }

    #[test]
    fn a_group_cannot_be_dropped_into_itself() {
        let (mut layout, tab, plot, logs) = default_layout();
        layout.drop_tile(tab, plot, Drop::Side(Side::Left));
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
            *renamed.borrow_mut() = layout.renaming.as_ref().map(|renaming| renaming.tile_id);
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
    fn the_plus_after_the_tabs_adds_a_tab_and_opens_it() {
        let (layout, tab, _, _) = default_layout();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label(egui_phosphor::regular::PLUS).click();
        harness.run();

        let layout = layout.borrow();
        let tabs = children(&layout, layout.root());
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0], tab);
        assert_eq!(layout.name(tabs[1]), "Tab 2");
        assert_eq!(layout.selected, Some(tabs[1]));
    }

    #[test]
    fn a_view_type_clicked_in_an_empty_tab_is_added_to_it() {
        let (mut layout, _, _, _) = default_layout();
        let tab = layout.add_tab();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Heatmap").click();
        harness.run();

        let layout = layout.borrow();
        let views = children(&layout, tab);
        assert_eq!(views.len(), 1);
        assert_eq!(
            layout.view(views[0]).unwrap().view_type(),
            ViewType::Heatmap
        );
        assert_eq!(layout.selected, Some(views[0]));
    }

    #[test]
    fn a_view_type_clicked_without_tabs_is_added_in_a_new_tab() {
        let (mut layout, tab, _, _) = default_layout();
        layout.remove_tile(tab);
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Plot").click();
        harness.run();

        let layout = layout.borrow();
        let tabs = children(&layout, layout.root());
        assert_eq!(tabs.len(), 1);
        assert_eq!(children(&layout, tabs[0]).len(), 1);
    }

    #[test]
    fn the_menu_of_a_title_adds_a_view_below() {
        let (layout, tab, plot, logs) = default_layout();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Plot 1").click_secondary();
        harness.run();
        harness.get_by_label_contains("Add below").hover();
        harness.run();
        harness.get_by_label_contains("XY").click();
        harness.run();

        let layout = layout.borrow();
        let views = children(&layout, tab);
        assert_eq!(views.len(), 3);
        assert_eq!((views[0], views[2]), (plot, logs));
        assert_eq!(layout.view(views[1]).unwrap().view_type(), ViewType::Xy);
    }

    #[test]
    fn the_menu_of_a_title_deletes_the_view() {
        let (layout, tab, plot, logs) = default_layout();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Logs 1").click_secondary();
        harness.run();
        harness.get_by_label_contains("Delete").click();
        harness.run();

        let layout = layout.borrow();
        assert_eq!(children(&layout, tab), [plot]);
        assert!(layout.view(logs).is_none());
    }

    #[test]
    fn a_view_is_not_added_beside_the_top_level() {
        let (mut layout, _, _, _) = default_layout();
        let tiles = layout.tree.tiles.len();
        let root = layout.root();
        assert_eq!(
            layout.add_view_beside(root, Side::Right, ViewType::Plot),
            None
        );
        assert_eq!(layout.tree.tiles.len(), tiles);
    }

    #[test]
    fn the_menu_of_a_tab_deletes_the_tab_with_its_views() {
        let (mut layout, tab, plot, _) = default_layout();
        let second = layout.add_tab();
        layout.add_view_into(second, ViewType::Table);
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Tab 1").click_secondary();
        harness.run();
        harness.get_by_label_contains("Delete").click();
        harness.run();

        let layout = layout.borrow();
        assert_eq!(children(&layout, layout.root()), [second]);
        assert!(layout.tree.tiles.get(tab).is_none());
        assert!(layout.view(plot).is_none());
    }

    #[test]
    fn the_menu_of_a_tab_renames_it() {
        let (layout, tab, _, _) = default_layout();
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        harness.get_by_label("Tab 1").click_secondary();
        harness.run();
        harness.get_by_label_contains("Rename").click();
        harness.run();

        let layout = layout.borrow();
        assert_eq!(
            layout.renaming.as_ref().map(|renaming| renaming.tile_id),
            Some(tab)
        );
    }

    #[test]
    fn switching_tabs_keeps_the_selected_view_and_the_open_tab_is_selected_by_a_click() {
        let (mut layout, tab, plot, _) = default_layout();
        let second = layout.add_tab();
        layout.add_view_into(second, ViewType::Table);
        layout.select(plot);
        let layout = Rc::new(RefCell::new(layout));
        let mut harness = center_harness(layout.clone());
        harness.run();

        harness.get_by_label("Tab 2").click();
        harness.run();
        assert_eq!(layout.borrow().selected, Some(plot));
        harness.get_by_label("Tab 1").click();
        harness.run();
        assert_eq!(layout.borrow().selected, Some(plot));
        harness.get_by_label("Tab 1").click();
        harness.run();
        assert_eq!(layout.borrow().selected, Some(tab));
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
            layout.renaming.as_ref().map(|renaming| renaming.tile_id),
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
