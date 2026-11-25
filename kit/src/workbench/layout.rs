//! The layout of the views in the center pane: a tree of groups with the views as
//! leaves. The top level groups are tabs, the groups below show their children as tabs,
//! stacked vertically or side by side.

mod drag;
mod file;
mod picker;
mod properties;

pub use file::{Group, GroupKind};

use crate::store::Store;
use crate::widgets::theme::{self, TokensExt};
use crate::widgets::view::{
    PlotInfo, Plots, View, ViewContext, ViewId, ViewKind, ViewType, Viewable,
};
use egui_tiles::{Container, ContainerKind, Tile, TileId, Tree};
use picker::view_picker;
use std::collections::{HashMap, HashSet};

/// Where a view or group dragged in the center pane is dropped, relative to the view or
/// tab it is dropped on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drop {
    Before,
    After,
    /// On a group, such as the empty part of a tab row: into the group as its last child
    /// (a tab, if the group shows tabs). On a view, the middle of it: a new group of tabs
    /// with the view and the dragged one in the place of the view, the dragged one after
    /// it and open.
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
    /// Adds a group of tabs with one empty tab at a side of a view or tab.
    AddEmptyGroup(TileId, Side),
    /// Adds an empty tab at the end of a group of tabs, the + of its tab bar.
    AddTab(TileId),
    Rename(TileId),
    Delete(TileId),
}

/// The layout from before the last delete, to undo it.
struct Undo {
    tree: Tree<View>,
    group_names: HashMap<TileId, String>,
    /// The view or group selected after undoing: the one deleted, or the one selected
    /// before the layout was cleared.
    selected: Option<TileId>,
    /// Tells this delete apart from the others, see [`Layout::undo_number`].
    number: u64,
}

/// What [`Layout::take_deleted`] names after [`Layout::clear`].
const ALL_VIEWS: &str = "all views";

/// Shows a field of at least `min_size` to edit `name`, which ends with Enter or a click
/// elsewhere, or with Escape to cancel. Returns whether to keep the name once it ended.
fn rename_field(ui: &mut egui::Ui, name: &mut String, min_size: egui::Vec2) -> Option<bool> {
    let field = theme::field(name)
        .min_size(min_size)
        .desired_width(min_size.x);
    theme::focused_edit(ui, field)
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
    /// The layout from before the last delete, until it is undone or the layout changes
    /// otherwise.
    undo: Option<Undo>,
    /// The name of what was deleted last, until the app takes it.
    deleted: Option<String>,
    /// The number of the last delete kept to undo.
    deletes: u64,
}

/// An empty layout without tabs, which shows the view types to add the first view.
impl Default for Layout {
    fn default() -> Self {
        Self::from_saved(Group::tabs(vec![]))
    }
}

impl Layout {
    /// A layout with one tab holding a plot and a log view, for tests.
    #[cfg(any(test, feature = "test-util"))]
    pub fn with_plot_and_logs() -> Self {
        let mut layout = Self::default();
        let main = layout.add_tab();
        layout.add_view_into(main, ViewType::Plot);
        layout.add_view_into(main, ViewType::Logs);
        layout.selected = Some(main);
        layout
    }

    /// Restores a saved layout, see [`Self::to_saved`].
    pub fn from_saved(saved: Group) -> Self {
        let (tree, group_names) = saved.load();
        Self {
            tree,
            group_names,
            selected: None,
            renaming: None,
            properties_name: None,
            undo: None,
            deleted: None,
            deletes: 0,
        }
    }

    /// The saved form of the layout: the group of the tabs at the top level.
    pub fn to_saved(&self) -> Group {
        Group::save(&self.tree, self.root(), &self.group_names)
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
        self.forget_undo();
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
    #[cfg(any(test, feature = "test-util"))]
    pub fn select_view_showing_traces(&mut self) {
        let view = self.tree.tiles.iter().find_map(|(&tile_id, tile)| {
            let traces = |view: &View| {
                let view_type = view.view_type();
                view.has_selection() && !view_type.shows_actions() && !view_type.shows_logs()
            };
            matches!(tile, Tile::Pane(view) if traces(view)).then_some(tile_id)
        });
        if let Some(view) = view {
            self.select(view);
        }
    }

    /// Selects the view or group named `name` and shows the tabs it is in, for the
    /// screenshots. Returns whether there is one.
    #[cfg(any(test, feature = "test-util"))]
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
        super::unused_name(base, true, |name| names.contains(name))
    }

    /// Adds a new tab at the end of the top level, returns its id.
    fn add_tab(&mut self) -> TileId {
        self.add_tab_to(self.root())
    }

    /// Adds a new empty tab at the end of the group of tabs `group`, selects and opens it,
    /// returns its id.
    fn add_tab_to(&mut self, group: TileId) -> TileId {
        self.forget_undo();
        let name = self.unique_name("Tab");
        let tab = self.tree.tiles.insert_vertical_tile(vec![]);
        self.group_names.insert(tab, name);
        self.tree
            .move_tile_to_container(tab, group, usize::MAX, true);
        self.select(tab);
        tab
    }

    /// A new view of `view_type`, not yet in the layout.
    fn new_view(&mut self, view_type: ViewType) -> TileId {
        self.forget_undo();
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

    /// Adds a new view of `view_type` and selects it: after the selected view in its
    /// group, or into the selected group, if they are shown, else into the open tab, or
    /// into a new tab if there is none.
    pub fn add_view(&mut self, view_type: ViewType) {
        let shown = self.tree.active_tiles();
        let open_tab = match self.container(self.root()) {
            Some(Container::Tabs(tabs)) => tabs.active,
            _ => None,
        };
        let anchor = (self.selected)
            .filter(|selected| shown.contains(selected))
            .or(open_tab.filter(|tab| self.tree.tiles.get(*tab).is_some()));
        match anchor {
            Some(group) if self.container(group).is_some() => {
                self.add_view_into(group, view_type);
            }
            Some(view) => {
                let new = self.new_view(view_type);
                self.move_next_to(new, view, true);
                self.select(new);
            }
            None => {
                let tab = self.add_tab();
                self.add_view_into(tab, view_type);
            }
        }
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
        self.place_beside(view, target, side);
        Some(view)
    }

    /// Adds a new group of tabs with one empty tab at `side` of `target` and selects and
    /// opens the tab, as if it was dropped there. Returns the tab, or `None`, adding
    /// nothing, if `target` is not in a group.
    fn add_empty_group_beside(&mut self, target: TileId, side: Side) -> Option<TileId> {
        self.parent(target)?;
        self.forget_undo();
        let group_name = self.unique_name("Group");
        let group = self.tree.tiles.insert_tab_tile(vec![]);
        self.group_names.insert(group, group_name);
        let tab = self.add_tab_to(group);
        self.place_beside(group, target, side);
        self.select(tab);
        Some(tab)
    }

    /// Puts the new tile `new` at `side` of `target` and selects it. At the side of a
    /// view it is dropped there. At the side of a tab that is a group, the side is that
    /// of the whole group: the tab keeps its id, name and place.
    fn place_beside(&mut self, new: TileId, target: TileId, side: Side) {
        let Some(container) = self.container(target) else {
            self.drop_tile(new, target, Drop::Side(side));
            return;
        };
        let along = GroupKind::of(container) == side.kind();
        let small = container.kind() != ContainerKind::Tabs && container.num_children() <= 1;
        // the group keeps its place, what is in it moves down, unless it takes the new
        // tile along its own direction
        if !along && !small {
            self.wrap_content(target);
        }
        if !along {
            self.set_group_kind(target, side.kind());
        }
        let index = if side.is_after() { usize::MAX } else { 0 };
        self.tree.move_tile_to_container(new, target, index, true);
        self.select(new);
    }

    /// Moves the content of the group `group` one level down: into a new group "Group n"
    /// of the same kind, with its open tab and shares, which becomes the only child of
    /// `group`. Returns the new group.
    fn wrap_content(&mut self, group: TileId) -> TileId {
        let name = self.unique_name("Group");
        let Some(Tile::Container(content)) = self.tree.tiles.remove(group) else {
            return group;
        };
        let kind = content.kind();
        let inner = self.tree.tiles.insert_container(content);
        self.group_names.insert(inner, name);
        self.tree
            .tiles
            .insert(group, Tile::Container(Container::new(kind, vec![inner])));
        inner
    }

    fn set_group_kind(&mut self, tile_id: TileId, kind: GroupKind) {
        self.forget_undo();
        if let Some(container) = self.container_mut(tile_id) {
            container.set_kind(kind.container_kind());
        }
    }

    /// Keeps the layout as it is to undo a delete, selecting `selected` after the undo.
    fn keep_undo(&mut self, selected: Option<TileId>) {
        self.deletes += 1;
        self.undo = Some(Undo {
            tree: self.tree.clone(),
            group_names: self.group_names.clone(),
            selected,
            number: self.deletes,
        });
    }

    /// Forgets the layout kept to undo a delete, as a change after the delete would be
    /// lost by undoing it.
    fn forget_undo(&mut self) {
        self.undo = None;
    }

    /// Deletes a view or a group with everything in it, so that it can be undone, see
    /// [`Self::undo_delete`].
    fn delete(&mut self, tile_id: TileId) {
        if tile_id == self.root() || self.tree.tiles.get(tile_id).is_none() {
            return;
        }
        self.keep_undo(Some(tile_id));
        self.deleted = Some(self.name(tile_id).to_owned());
        self.remove(tile_id);
    }

    /// Deletes the selected view or group with everything in it, e.g. for the Delete
    /// key, if it shows in the tab shown. It can be undone with [`Self::undo_delete`].
    pub fn delete_selected(&mut self) {
        if let Some(selected) = self.selected
            && self.tree.active_tiles().contains(&selected)
        {
            self.delete(selected);
        }
    }

    /// Starts renaming the selected view or group in a field on its title or tab, e.g.
    /// for F2, and shows the tabs it is in.
    pub fn rename_selected(&mut self) {
        if let Some(selected) = self
            .selected
            .filter(|&selected| self.tree.tiles.get(selected).is_some())
        {
            self.select(selected);
            self.start_rename(selected);
        }
    }

    /// Deletes all views and tabs, leaving the empty layout that offers the view types
    /// to start again. It can be undone with [`Self::undo_delete`].
    pub fn clear(&mut self) {
        if self.container(self.root()).is_none_or(Container::is_empty) {
            return;
        }
        let old = std::mem::take(self);
        self.deletes = old.deletes + 1;
        self.undo = Some(Undo {
            tree: old.tree,
            group_names: old.group_names,
            selected: old.selected,
            number: self.deletes,
        });
        self.deleted = Some(ALL_VIEWS.to_owned());
    }

    /// Whether a delete can be undone with [`Self::undo_delete`]: something was deleted
    /// and the layout has not changed since, other than by the views themselves.
    pub fn can_undo_delete(&self) -> bool {
        self.undo.is_some()
    }

    /// The number of the delete that [`Self::undo_delete`] undoes, if one can be undone:
    /// each delete has its own, e.g. to tell which of the toasts offering to undo a
    /// delete still can.
    pub fn undo_number(&self) -> Option<u64> {
        self.undo.as_ref().map(|undo| undo.number)
    }

    /// Brings back the layout from before the last delete of a view or group, or before
    /// [`Self::clear`], and selects what was deleted. The views that were not deleted
    /// keep what changed in them since, e.g. their traces. Does nothing if
    /// [`Self::can_undo_delete`] is false.
    pub fn undo_delete(&mut self) {
        let Some(mut undo) = self.undo.take() else {
            return;
        };
        let kept: Vec<TileId> = undo.tree.tiles.tile_ids().collect();
        for tile_id in kept {
            if let Some(Tile::Pane(mut view)) = self.tree.tiles.remove(tile_id)
                && let Some(Tile::Pane(before)) = undo.tree.tiles.get_mut(tile_id)
            {
                // the links the delete dropped come back with the views
                view.relink_as(before);
                *before = view;
            }
        }
        self.tree = undo.tree;
        self.group_names = undo.group_names;
        self.renaming = None;
        self.properties_name = None;
        self.deleted = None;
        self.selected = None;
        if let Some(selected) = undo.selected {
            self.select(selected);
        }
    }

    /// The name of the view or group deleted since the last call, or "all views" after
    /// [`Self::clear`], for the app to tell the user, e.g. "Deleted Plot 1", and to
    /// offer [`Self::undo_delete`].
    pub fn take_deleted(&mut self) -> Option<String> {
        self.deleted.take()
    }

    /// Removes a view or a group with everything in it, and the groups it leaves empty. A
    /// tab left empty stays, at any depth, to add views to it again.
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
        // links to the removed views are dropped
        let kept: HashSet<ViewId> = self.tree.tiles.tile_ids().map(view_id).collect();
        for tile in self.tree.tiles.tiles_mut() {
            if let Tile::Pane(view) = tile {
                view.relink(|id| kept.contains(&id).then_some(id));
            }
        }
        if self
            .selected
            .is_some_and(|id| self.tree.tiles.get(id).is_none())
        {
            self.selected = None;
        }
        // a view or group removed while its name was edited is no longer renamed
        if (self.renaming.as_ref())
            .is_some_and(|renaming| self.tree.tiles.get(renaming.tile_id).is_none())
        {
            self.renaming = None;
        }
    }

    /// Removes `group` if a move or removal left it without children, and its parent if
    /// that is left empty too, up the tree. Not the top level, and not, if `keep_tab`, a
    /// tab that is not a group of tabs, at any depth. A group of tabs below the top
    /// level that is left without tabs is removed either way.
    fn remove_if_empty(&mut self, group: TileId, keep_tab: bool) {
        let parent = self.parent(group);
        let is_tab = parent
            .and_then(|parent| self.container(parent))
            .is_some_and(|parent| parent.kind() == ContainerKind::Tabs)
            && self
                .container(group)
                .is_some_and(|group| group.kind() != ContainerKind::Tabs);
        if group != self.root()
            && !(keep_tab && is_tab)
            && self.container(group).is_some_and(Container::is_empty)
        {
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
        let into_group = drop == Drop::Into && self.container(target).is_some();
        // only the drops next to or onto a view need the group of the target
        let parent = self.parent(target);
        if parent.is_none() && !into_group {
            return;
        }
        self.forget_undo();
        let old_parent = self.parent(dragged);
        match (drop, parent) {
            (Drop::Into, _) if into_group => {
                self.tree
                    .move_tile_to_container(dragged, target, usize::MAX, true);
            }
            (_, None) => {}
            (Drop::Before | Drop::After, Some(_)) => {
                self.move_next_to(dragged, target, drop == Drop::After);
            }
            (Drop::Into, Some(_)) => self.stack_with(dragged, target),
            (Drop::Side(side), Some(_)) if self.reorders(target, side) => {
                self.move_next_to(dragged, target, side.is_after());
            }
            (Drop::Side(side), Some(parent)) if self.are_only_children(dragged, target) => {
                // turn the group instead of nesting a new one in it
                self.set_group_kind(parent, side.kind());
                self.move_next_to(dragged, target, side.is_after());
            }
            (Drop::Side(side), Some(_)) => {
                self.group_with(dragged, target, side.kind(), side.is_after());
            }
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

    /// Puts `target`, a view, and `dragged` into a group of tabs in the place of `target`,
    /// `dragged` after it. If they are the only children of a vertical or horizontal
    /// group, that group turns into the tabs instead.
    fn stack_with(&mut self, dragged: TileId, target: TileId) {
        if self.are_only_children(dragged, target)
            && let Some(parent) = self.parent(target)
        {
            self.set_group_kind(parent, GroupKind::Tabs);
            self.move_next_to(dragged, target, true);
        } else {
            self.group_with(dragged, target, GroupKind::Tabs, true);
        }
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

    /// The plot views of the layout in its order, with the y ranges they show.
    pub fn plots(&self, store: &dyn Store) -> Plots {
        let mut plots = vec![];
        let mut below = vec![self.root()];
        while let Some(tile_id) = below.pop() {
            match self.tree.tiles.get(tile_id) {
                Some(Tile::Pane(View {
                    name,
                    kind: ViewKind::Plot(plot),
                    ..
                })) => plots.push(PlotInfo {
                    id: view_id(tile_id),
                    name: name.clone(),
                    y_range: plot.shown_y(store),
                }),
                Some(Tile::Container(container)) => {
                    below.extend(container.children_vec().into_iter().rev());
                }
                _ => {}
            }
        }
        Plots(plots)
    }

    /// Gives each distribution linked to a plot the traces of the plot, with its wildcards
    /// and number formats, so that it shows what the plot shows. After unlinking, the
    /// distribution keeps them.
    pub fn take_linked_traces(&mut self) {
        let links: Vec<(TileId, TileId)> = (self.tree.tiles.iter())
            .filter_map(|(&tile_id, tile)| match tile {
                Tile::Pane(View {
                    kind: ViewKind::Distribution(distribution),
                    ..
                }) => Some((tile_id, tile_of(distribution.link?))),
                _ => None,
            })
            .collect();
        for (distribution, plot) in links {
            let Some(View {
                kind: ViewKind::Plot(plot),
                wildcards,
                formats,
                ..
            }) = self.view(plot)
            else {
                continue;
            };
            let taken = (plot.traces.clone(), wildcards.clone(), formats.clone());
            if let Some(Tile::Pane(view)) = self.tree.tiles.get_mut(distribution)
                && let ViewKind::Distribution(distribution) = &mut view.kind
            {
                (distribution.traces, view.wildcards, view.formats) = taken;
            }
        }
    }

    /// The name of the plot the selected view takes its traces from, if it is a
    /// distribution linked to a plot.
    pub fn selected_link(&self) -> Option<&str> {
        let Some(View {
            kind: ViewKind::Distribution(distribution),
            ..
        }) = self.view(self.selected?)
        else {
            return None;
        };
        match self.view(tile_of(distribution.link?))? {
            View {
                name,
                kind: ViewKind::Plot(_),
                ..
            } => Some(name),
            _ => None,
        }
    }

    /// Shows the views in the center pane.
    pub fn ui(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.take_linked_traces();
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
            .filter_map(|(&id, tile)| match tile {
                Tile::Container(Container::Tabs(tabs)) => Some((id, tabs)),
                _ => None,
            })
            .collect();
        let panes_in_tabs = tab_groups
            .iter()
            .flat_map(|(_, tabs)| tabs.children.iter().copied())
            .collect();
        // the tabs of the groups of tabs below the top level, which are cards
        let below_top: Vec<_> = tab_groups
            .iter()
            .filter(|(id, _)| *id != self.root())
            .map(|(_, tabs)| *tabs)
            .collect();
        let tabs_in_cards = below_top
            .iter()
            .flat_map(|tabs| tabs.children.iter().copied())
            .collect();
        // the open tabs of those groups that are a view or an empty group, which share
        // one card with their tab row
        let joined_cards = below_top
            .iter()
            .filter_map(|tabs| tabs.next_active(&self.tree.tiles))
            .filter(|&active| match self.tree.tiles.get(active) {
                Some(Tile::Pane(_)) => true,
                Some(Tile::Container(container)) => container.is_empty(),
                None => false,
            })
            .collect();
        // egui_tiles takes a group out of the tree while showing its tabs, so which tabs
        // are open is looked up before
        let open_tabs = tab_groups
            .iter()
            .filter_map(|(_, tabs)| tabs.next_active(&self.tree.tiles))
            .collect();
        // a view or group dragged in the center
        let dragged = egui::DragAndDrop::payload::<TileId>(ui.ctx())
            .map(|dragged| *dragged)
            .filter(|&dragged| self.tree.tiles.get(dragged).is_some());
        let store = context.store;
        let plots = self.plots(store);
        let context = &mut ViewContext {
            store,
            time: &mut *context.time,
            invoker: &mut *context.invoker,
            formats: context.formats,
            titled: context.titled,
            plots: &plots,
        };
        let mut behavior = Behavior {
            context,
            root: self.root(),
            // the center pane is clipped to its edges, without the gap around the views
            center_x: ui.clip_rect().x_range(),
            group_names: &self.group_names,
            selected: &mut self.selected,
            panes_in_tabs,
            tabs_in_cards,
            joined_cards,
            open_tabs,
            renaming: &mut self.renaming,
            rename_ended: None,
            dragged,
            cards: Vec::new(),
            tab_buttons: Vec::new(),
            tab_rows: Vec::new(),
            actions: Vec::new(),
        };
        self.tree.ui(&mut behavior, ui);
        let shown = drag::Shown {
            cards: std::mem::take(&mut behavior.cards),
            tab_buttons: std::mem::take(&mut behavior.tab_buttons),
            tab_rows: std::mem::take(&mut behavior.tab_rows),
        };
        let rename_ended = behavior.rename_ended;
        let mut actions = std::mem::take(&mut behavior.actions);
        if self.plots(store) != plots {
            // the views that follow a plot shown before it show its range in the next frame
            ui.ctx().request_repaint();
        }
        actions.extend(self.empty_groups_ui(ui));
        if let Some(keep) = rename_ended {
            self.end_rename(keep);
        }
        for action in actions {
            self.apply(action);
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
            Action::AddEmptyGroup(target, side) => {
                self.add_empty_group_beside(target, side);
            }
            Action::AddTab(group) => {
                self.add_tab_to(group);
            }
            Action::Rename(tile_id) => self.start_rename(tile_id),
            Action::Delete(tile_id) => self.delete(tile_id),
        }
    }
}

/// The id by which views refer to the view at `tile_id`.
fn view_id(tile_id: TileId) -> ViewId {
    ViewId(tile_id.0)
}

/// The tile of the view that `view` refers to, see [`view_id`].
fn tile_of(view: ViewId) -> TileId {
    TileId::from_u64(view.0)
}

/// The menu of a view or group, opened by a right click on its title or tab: views and
/// empty groups can be added at its sides if `can_add`, and it can be renamed or deleted.
fn tile_menu(ui: &mut egui::Ui, tile_id: TileId, can_add: bool, actions: &mut Vec<Action>) {
    if can_add {
        let sides = [
            (egui_phosphor::regular::COLUMNS, "Add right", Side::Right),
            (egui_phosphor::regular::ROWS, "Add below", Side::Bottom),
        ];
        for (icon, text, side) in sides {
            ui.menu_button(format!("{icon} {text}"), |ui| {
                for view_type in ViewType::all() {
                    if ui
                        .button(format!("{} {}", view_type.icon(), view_type.name()))
                        .clicked()
                    {
                        actions.push(Action::AddView(tile_id, side, view_type));
                    }
                }
                ui.separator();
                let group = format!("{} Empty group", GroupKind::Tabs.icon());
                if ui.button(group).clicked() {
                    actions.push(Action::AddEmptyGroup(tile_id, side));
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
        actions.push(Action::Delete(tile_id));
    }
}

/// Shows the views of the layout in the center pane.
struct Behavior<'a, 'b> {
    context: &'a mut ViewContext<'b>,
    /// The tab group at the top level.
    root: TileId,
    /// The center pane from edge to edge, which the line under the top tab bar spans.
    center_x: egui::Rangef,
    group_names: &'a HashMap<TileId, String>,
    selected: &'a mut Option<TileId>,
    /// Views shown as a tab, they get no title bar.
    panes_in_tabs: HashSet<TileId>,
    /// The tabs of the groups of tabs below the top level, shown as cards.
    tabs_in_cards: HashSet<TileId>,
    /// The open tabs of those groups that are a view or an empty group, whose body lies in
    /// the same card as the tab row.
    joined_cards: HashSet<TileId>,
    /// The tabs shown in their group.
    open_tabs: HashSet<TileId>,
    /// The view or group renamed, see [`Layout::renaming`].
    renaming: &'a mut Option<Renaming>,
    /// Set when editing the title ends, whether to keep the name.
    rename_ended: Option<bool>,
    /// The view or group being dragged, shown dimmed.
    dragged: Option<TileId>,
    /// The cards of the views shown, to drop on.
    cards: Vec<(TileId, egui::Rect)>,
    /// The tab buttons shown, to drop on.
    tab_buttons: Vec<(TileId, egui::Rect)>,
    /// The empty parts of the tab rows, after the last tab, to drop on.
    tab_rows: Vec<(TileId, egui::Rect)>,
    /// The changes chosen in the menus of the views and tabs.
    actions: Vec<Action>,
}

/// Height of a tab bar, the line under it at its bottom.
const TAB_BAR_HEIGHT: f32 = 28.0;

/// Height of the title bar of a view not shown as a tab, which moves it by drag and drop.
const TITLE_BAR_HEIGHT: f32 = 28.0;

/// Size of the menu button at the right end of the title bar of a view.
const MENU_BUTTON_SIZE: f32 = 22.0;

/// Width of the dots of the menu button, which line up with the content of the view.
const MENU_DOTS_WIDTH: f32 = 12.0;

/// Size of the button that deletes a tab, at the right end of the tab.
const TAB_CLOSE_SIZE: f32 = 20.0;

/// Space between the title of a tab and the button that deletes it, and after it.
const TAB_CLOSE_GAP: f32 = 4.0;

/// The least width left in a title row for what the view adds to it, like a legend.
const MIN_TITLE_EXTRA_WIDTH: f32 = 24.0;

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
        // a view in a tabbed card lies in the card of its group, which is painted with
        // the tab row; the gap under the row, which every tab bar leaves, belongs to the
        // card, so that the view is joined to the row
        let in_card = self.tabs_in_cards.contains(&tile_id);
        let tokens = ui.tokens();
        let mut card = ui.max_rect();
        if in_card {
            card.min.y -= theme::GAP;
        }
        self.cards.push((tile_id, card));
        if !in_card {
            ui.painter().rect(
                card,
                theme::CARD_RADIUS,
                tokens.card,
                egui::Stroke::new(1.0, tokens.border),
                egui::StrokeKind::Inside,
            );
        }

        let titled = !self.panes_in_tabs.contains(&tile_id);
        if titled {
            // the title bar moves the view by drag and drop, it is registered before the
            // title so that clicks on the title still rename the view
            let bar = card.with_max_y(card.top() + TITLE_BAR_HEIGHT);
            // the title bar of the selected view is raised, so that the selection does not
            // take the blue of the accent and the first trace
            if *self.selected == Some(tile_id) {
                let radius = egui::CornerRadius {
                    nw: theme::CARD_RADIUS,
                    ne: theme::CARD_RADIUS,
                    sw: 0,
                    se: 0,
                };
                ui.painter().rect_filled(bar, radius, tokens.hover);
            }
            let bar = ui
                .interact(
                    bar,
                    ui.id().with("title_bar"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            bar.dnd_set_drag_payload(tile_id);
            // the icon, the title and the menu button in a row, centred on the bar; the
            // button's frame reaches into the margin, so that its dots line up with the
            // content below
            let inset = ((MENU_BUTTON_SIZE - MENU_DOTS_WIDTH) / 2.0) as i8;
            let title = egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 10,
                    right: 10 - inset,
                    top: ((TITLE_BAR_HEIGHT - MENU_BUTTON_SIZE) / 2.0) as i8,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    let row = egui::vec2(ui.available_width(), MENU_BUTTON_SIZE);
                    let right_to_left = egui::Layout::right_to_left(egui::Align::Center);
                    ui.allocate_ui_with_layout(row, right_to_left, |ui| {
                        // little padding, so that the dots are centred in the button
                        ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
                        let icon =
                            egui::RichText::new(egui_phosphor::regular::DOTS_THREE).size(16.0);
                        let button =
                            egui::Button::new(icon).min_size(egui::Vec2::splat(MENU_BUTTON_SIZE));
                        let button = theme::hover_frame(button).button(ui);
                        egui::containers::menu::MenuButton::from_button(button)
                            .ui(ui, |ui| tile_menu(ui, tile_id, true, &mut self.actions))
                            .0
                            .on_hover_text("Add a view next to this one, rename or delete it");
                        let left_to_right = egui::Layout::left_to_right(egui::Align::Center);
                        let rest = egui::vec2(ui.available_width(), MENU_BUTTON_SIZE);
                        ui.allocate_ui_with_layout(rest, left_to_right, |ui| {
                            ui.label(
                                egui::RichText::new(view.view_type().icon())
                                    .color(ui.visuals().weak_text_color()),
                            );
                            let field_size = egui::vec2(ui.available_width(), 0.0);
                            if self.rename_ui(ui, tile_id, field_size) {
                                return None;
                            }
                            let title = egui::Label::new(egui::RichText::new(&view.name).strong())
                                .truncate()
                                .sense(egui::Sense::click_and_drag());
                            let title = ui
                                .add(title)
                                .on_hover_cursor(egui::CursorIcon::Text)
                                .on_hover_text(
                                    "Click to rename, drag to move, right-click for more",
                                );
                            title.dnd_set_drag_payload(tile_id);
                            if title.clicked() {
                                *self.renaming = Some(Renaming {
                                    tile_id,
                                    name: view.name.clone(),
                                });
                            }
                            // what the view adds, like a legend, cut off at the menu button,
                            // unless a long title leaves no room for it
                            let rest = ui
                                .available_rect_before_wrap()
                                .shrink2(egui::vec2(4.0, 0.0));
                            if rest.width() >= MIN_TITLE_EXTRA_WIDTH {
                                ui.scope_builder(egui::UiBuilder::new().max_rect(rest), |ui| {
                                    ui.set_clip_rect(rest.intersect(ui.clip_rect()));
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    self.context.titled = true;
                                    view.title_ui(ui, self.context);
                                });
                            }
                            Some(title)
                        })
                        .inner
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

        let top = match (titled, in_card) {
            (true, _) => 4,
            (false, false) => 10,
            (false, true) => 10 - theme::GAP as i8,
        };
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top,
                bottom: 10,
            })
            .show(ui, |ui| {
                self.context.titled = titled;
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

    fn tab_ui(
        &mut self,
        tiles: &mut egui_tiles::Tiles<View>,
        ui: &mut egui::Ui,
        id: egui::Id,
        tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Response {
        // like the tab of egui_tiles, with room after the title for the button that
        // deletes the tab, so that the tab keeps its width when the button shows
        let text = self.tab_title_for_tile(tiles, tile_id);
        let font_id = egui::TextStyle::Button.resolve(ui.style());
        let galley = text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, font_id);
        let margin = self.tab_title_spacing(ui.visuals());
        let width = margin + galley.size().x + TAB_CLOSE_GAP + TAB_CLOSE_SIZE + TAB_CLOSE_GAP;
        let (_, rect) = ui.allocate_space(egui::vec2(width, ui.available_height()));
        // the row is at the top of the bar, the gap below it belongs to what is below
        let rect = rect.with_max_y(rect.top() + TAB_BAR_HEIGHT);
        let response = ui.interact(rect, id, egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                state.active,
                galley.text(),
            )
        });
        if ui.is_rect_visible(rect) {
            let color = self.tab_text_color(ui.visuals(), tiles, tile_id, state);
            if state.active && self.tabs_in_cards.contains(&tile_id) {
                // the open tab of a card is raised, as a selected title bar
                let radius = egui::CornerRadius {
                    nw: theme::CARD_RADIUS,
                    ne: theme::CARD_RADIUS,
                    sw: 0,
                    se: 0,
                };
                let hover = ui.visuals().tokens().hover;
                ui.painter().rect_filled(rect, radius, hover);
            }
            let title = rect.with_min_x(rect.left() + margin);
            let pos = egui::Align2::LEFT_CENTER
                .align_size_within_rect(galley.size(), title)
                .min;
            ui.painter().galley(pos, galley, color);
        }
        let response = self.on_tab_button(tiles, tile_id, response);

        // the button that deletes the tab shows while the pointer is on the tab, after the
        // button that drags the tab, so that it takes the clicks on it
        let renamed = (self.renaming.as_ref()).is_some_and(|renaming| renaming.tile_id == tile_id);
        if !renamed && ui.rect_contains_pointer(rect) {
            let close = egui::Align2::RIGHT_CENTER.align_size_within_rect(
                egui::Vec2::splat(TAB_CLOSE_SIZE),
                rect.shrink2(egui::vec2(TAB_CLOSE_GAP, 0.0)),
            );
            let layout = egui::Layout::centered_and_justified(egui::Direction::LeftToRight);
            let mut ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("close tab", tile_id))
                    .max_rect(close)
                    .layout(layout),
            );
            let what = if matches!(tiles.get(tile_id), Some(Tile::Pane(_))) {
                "Delete this view"
            } else {
                "Delete this tab and its views"
            };
            ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
            let button = ui
                .add(theme::icon_button(egui_phosphor::regular::X))
                .on_hover_text(what);
            if button.clicked() {
                self.actions.push(Action::Delete(tile_id));
            }
        }
        response
    }

    fn on_tab_button(
        &mut self,
        tiles: &mut egui_tiles::Tiles<View>,
        tile_id: TileId,
        response: egui::Response,
    ) -> egui::Response {
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
        // tab selects it, a double click renames it
        if response.double_clicked() {
            let name = self.tab_title_for_tile(tiles, tile_id).text().to_owned();
            *self.renaming = Some(Renaming { tile_id, name });
            *self.selected = Some(tile_id);
        } else if response.clicked() && self.open_tabs.contains(&tile_id) {
            *self.selected = Some(tile_id);
        }
        let can_add = matches!(tiles.get(tile_id), Some(Tile::Pane(_)))
            || self.tabs_in_cards.contains(&tile_id);
        let response =
            response.on_hover_text("Double-click to rename, drag to move, right-click for more");
        response.context_menu(|ui| tile_menu(ui, tile_id, can_add, &mut self.actions));

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
            let veil = style.visuals.tokens().canvas.gamma_multiply(0.6);
            painter.rect_filled(rect, theme::CARD_RADIUS, veil);
        }
        if *self.selected == Some(tile_id) {
            let stroke = egui::Stroke::new(1.0, style.visuals.tokens().text_faint);
            let mut rect = rect;
            if self.joined_cards.contains(&tile_id) {
                // the frame spans the whole card, tab row included
                rect.min.y -= TAB_BAR_HEIGHT + theme::GAP;
            }
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
        // the row, then a gap: the views below the top tabs lie a gap under them, and a
        // tabbed card starts at its row, as high as the card of a view beside it
        TAB_BAR_HEIGHT + theme::GAP
    }

    fn top_bar_right_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<View>,
        ui: &mut egui::Ui,
        tile_id: TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let bar = ui.max_rect();
        if tile_id != self.root {
            // a tabbed card: the row and the body of the open tab, if it is a view or
            // empty, in one card, else the row alone
            let row = bar.with_max_y(bar.top() + TAB_BAR_HEIGHT);
            // a group in the open tab lies in the same card, a gap under the row
            let body = tabs.next_active(tiles).and_then(|_| tiles.rect(tile_id));
            let joined = tabs
                .next_active(tiles)
                .is_some_and(|open| self.joined_cards.contains(&open));
            paint_tabbed_card(ui.painter(), row, body, joined);
            return;
        }
        // the line under the tabs, painted before them so that the underline of the open
        // tab lies on it, under the top tabs from edge to edge of the center pane
        let line = egui::Rect::from_x_y_ranges(
            self.center_x,
            bar.top() + TAB_BAR_HEIGHT - 1.0..=bar.top() + TAB_BAR_HEIGHT,
        );
        // egui_tiles clips the tab bar to the tiles, which leaves out the gap around them
        let mut painter = ui.painter().clone();
        painter.set_clip_rect(line);
        painter.rect_filled(line, 0.0, ui.tokens().border);
    }

    fn tab_bar_trailing_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<View>,
        ui: &mut egui::Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
    ) {
        // the rest of the row after the last tab takes drops, nothing else
        let top = tiles
            .rect(tile_id)
            .map_or(ui.max_rect().top(), |group| group.top());
        let row = egui::Rect::from_x_y_ranges(
            ui.cursor().min.x..=ui.clip_rect().right(),
            top..=top + TAB_BAR_HEIGHT,
        );
        if row.width() > 0.0 {
            self.tab_rows.push((tile_id, row));
        }
        // the button is centred in the row, not in the bar with the gap below it
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        let add = ui
            .scope_builder(egui::UiBuilder::new().max_rect(row).layout(layout), |ui| {
                ui.add_space(4.0);
                ui.add(theme::icon_button(egui_phosphor::regular::PLUS))
                    .on_hover_text("Add a tab")
            })
            .inner;
        if add.clicked() {
            self.actions.push(Action::AddTab(tile_id));
        }
    }

    fn tab_bar_color(&self, visuals: &egui::Visuals) -> egui::Color32 {
        visuals.tokens().canvas
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

/// The card of a tabbed card: `row` alone, or with `body`, the rect of the whole group,
/// below it, then with a line under the row if the body is `joined` to it, else the open
/// tab holds a group, which lies a gap under the row. Painted before the tabs.
fn paint_tabbed_card(
    painter: &egui::Painter,
    row: egui::Rect,
    body: Option<egui::Rect>,
    joined: bool,
) {
    let tokens = painter.ctx().global_style().visuals.tokens();
    let card = body.unwrap_or(row);
    let mut painter = painter.clone();
    painter.set_clip_rect(card);
    painter.rect(
        card,
        theme::CARD_RADIUS,
        tokens.card,
        egui::Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );
    if joined {
        let line = egui::Rect::from_x_y_ranges(row.x_range(), row.bottom() - 1.0..=row.bottom());
        painter.rect_filled(line, 0.0, tokens.border);
    }
}

/// The underline of the open tab at the bottom of `rect`, in the accent and inset like
/// its title.
fn paint_tab_underline(painter: &egui::Painter, rect: egui::Rect) {
    let accent = painter.ctx().global_style().visuals.tokens().accent;
    let inset = 4.0;
    let line = egui::Rect::from_x_y_ranges(
        rect.left() + inset..=rect.right() - inset,
        rect.bottom() - 2.0..=rect.bottom(),
    );
    painter.rect_filled(line, 1.0, accent);
}

#[cfg(test)]
mod tests;
