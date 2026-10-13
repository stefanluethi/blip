//! The saved form of a layout. It is versioned, so layouts saved by older versions of
//! blip can be migrated when the format changes.

use crate::app::view::View;
use egui_tiles::{Container, ContainerKind, Tile, TileId, Tiles, Tree};
use std::collections::HashMap;

/// Version of the format written by [`LayoutFile::new`].
pub const VERSION: u32 = 1;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LayoutFile {
    pub version: u32,
    /// The tabs at the top level.
    pub root: Group,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum Node {
    Group(Group),
    View(View),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Group {
    pub name: String,
    pub kind: GroupKind,
    pub children: Vec<Node>,
    /// Relative sizes of the children of a vertical or horizontal group.
    #[serde(default)]
    pub shares: Vec<f32>,
    /// Index of the shown child of a tab group.
    #[serde(default)]
    pub active: Option<usize>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupKind {
    Tabs,
    Vertical,
    Horizontal,
}

impl GroupKind {
    pub const ALL: [GroupKind; 3] = [GroupKind::Tabs, GroupKind::Vertical, GroupKind::Horizontal];

    pub fn name(self) -> &'static str {
        match self {
            GroupKind::Tabs => "Tabs",
            GroupKind::Vertical => "Vertical",
            GroupKind::Horizontal => "Horizontal",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            GroupKind::Tabs => egui_phosphor::regular::TABS,
            GroupKind::Vertical => egui_phosphor::regular::ROWS,
            GroupKind::Horizontal => egui_phosphor::regular::COLUMNS,
        }
    }

    pub fn of(container: &Container) -> Self {
        match container.kind() {
            ContainerKind::Tabs => GroupKind::Tabs,
            ContainerKind::Horizontal => GroupKind::Horizontal,
            // grids are not created by blip
            ContainerKind::Vertical | ContainerKind::Grid => GroupKind::Vertical,
        }
    }

    pub fn container_kind(self) -> ContainerKind {
        match self {
            GroupKind::Tabs => ContainerKind::Tabs,
            GroupKind::Vertical => ContainerKind::Vertical,
            GroupKind::Horizontal => ContainerKind::Horizontal,
        }
    }
}

impl LayoutFile {
    /// Saves the tiles below `root`, with the group names from `group_names`.
    pub fn new(tree: &Tree<View>, root: TileId, group_names: &HashMap<TileId, String>) -> Self {
        let root = match save_node(&tree.tiles, root, group_names) {
            Some(Node::Group(group)) => group,
            _ => Group::tabs(vec![]),
        };
        Self {
            version: VERSION,
            root,
        }
    }

    /// Restores the tiles, returns the tree and the group names.
    pub fn load(self) -> (Tree<View>, HashMap<TileId, String>) {
        let mut tiles = Tiles::default();
        let mut group_names = HashMap::new();
        let root = load_group(&mut tiles, &mut group_names, self.root);
        (Tree::new("layout", root, tiles), group_names)
    }
}

impl Group {
    pub fn tabs(children: Vec<Node>) -> Self {
        Self {
            name: String::new(),
            kind: GroupKind::Tabs,
            children,
            shares: vec![],
            active: None,
        }
    }
}

fn save_node(
    tiles: &Tiles<View>,
    tile_id: TileId,
    group_names: &HashMap<TileId, String>,
) -> Option<Node> {
    match tiles.get(tile_id)? {
        Tile::Pane(view) => Some(Node::View(view.clone())),
        Tile::Container(container) => {
            let child_ids = container.children_vec();
            let (shares, active) = match container {
                Container::Linear(linear) => (
                    child_ids.iter().map(|&id| linear.shares[id]).collect(),
                    None,
                ),
                Container::Tabs(tabs) => (
                    vec![],
                    child_ids.iter().position(|&id| Some(id) == tabs.active),
                ),
                Container::Grid(_) => (vec![], None),
            };
            Some(Node::Group(Group {
                name: group_names.get(&tile_id).cloned().unwrap_or_default(),
                kind: GroupKind::of(container),
                children: child_ids
                    .iter()
                    .filter_map(|&id| save_node(tiles, id, group_names))
                    .collect(),
                shares,
                active,
            }))
        }
    }
}

fn load_group(
    tiles: &mut Tiles<View>,
    group_names: &mut HashMap<TileId, String>,
    group: Group,
) -> TileId {
    let children: Vec<TileId> = group
        .children
        .into_iter()
        .map(|node| match node {
            Node::View(view) => tiles.insert_pane(view),
            Node::Group(group) => load_group(tiles, group_names, group),
        })
        .collect();

    let mut container = Container::new(group.kind.container_kind(), children.clone());
    match &mut container {
        Container::Linear(linear) if group.shares.len() == children.len() => {
            for (&id, &share) in children.iter().zip(&group.shares) {
                linear.shares.set_share(id, share);
            }
        }
        Container::Tabs(tabs) => {
            if let Some(&active) = group.active.and_then(|index| children.get(index)) {
                tabs.set_active(active);
            }
        }
        _ => {}
    }
    let tile_id = tiles.insert_container(container);
    group_names.insert(tile_id, group.name);
    tile_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view::ViewType;

    #[test]
    fn layout_survives_save_and_load() {
        let file = LayoutFile {
            version: VERSION,
            root: Group {
                active: Some(0),
                ..Group::tabs(vec![Node::Group(Group {
                    name: "Main".to_owned(),
                    kind: GroupKind::Vertical,
                    children: vec![
                        Node::View(View::new(ViewType::Plot, "Plot 1".to_owned())),
                        Node::Group(Group {
                            name: "Side by side".to_owned(),
                            kind: GroupKind::Horizontal,
                            children: vec![
                                Node::View(View::new(ViewType::Plot, "Plot 2".to_owned())),
                                Node::View(View::new(ViewType::Logs, "Logs".to_owned())),
                            ],
                            shares: vec![2.0, 1.0],
                            active: None,
                        }),
                    ],
                    shares: vec![1.0, 1.0],
                    active: None,
                })])
            },
        };
        let bytes = serde_cbor_2::to_vec(&file).unwrap();

        let (tree, group_names) = serde_cbor_2::from_slice::<LayoutFile>(&bytes)
            .unwrap()
            .load();
        let saved = LayoutFile::new(&tree, tree.root().unwrap(), &group_names);
        assert_eq!(serde_cbor_2::to_vec(&saved).unwrap(), bytes);
    }
}
