//! The saved form of a layout, part of a preset: the tree of groups with the views
//! and their settings as leaves.

use crate::app::view::View;
use egui_tiles::{Container, ContainerKind, Tile, TileId, Tiles, Tree};
use std::collections::HashMap;

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub enum Node {
    Group(Group),
    View(View),
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
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

named_enum! {
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Eq, Debug)]
    pub enum GroupKind {
        Tabs => "Tabs",
        Vertical => "Vertical",
        Horizontal => "Horizontal",
    }
}

impl GroupKind {
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

impl Group {
    /// Saves the tiles below `root`, with the group names from `group_names`, as the
    /// group of the tabs at the top level.
    pub fn save(tree: &Tree<View>, root: TileId, group_names: &HashMap<TileId, String>) -> Self {
        match save_node(&tree.tiles, root, group_names) {
            Some(Node::Group(group)) => group,
            _ => Group::tabs(vec![]),
        }
    }

    /// Restores the tiles of the group of the tabs at the top level, returns the tree
    /// and the group names.
    pub fn load(self) -> (Tree<View>, HashMap<TileId, String>) {
        let mut tiles = Tiles::default();
        let mut group_names = HashMap::new();
        let root = load_group(&mut tiles, &mut group_names, self);
        (Tree::new("layout", root, tiles), group_names)
    }

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
        let file = Group {
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
        };
        let bytes = serde_cbor_2::to_vec(&file).unwrap();

        let (tree, group_names) = serde_cbor_2::from_slice::<Group>(&bytes).unwrap().load();
        let saved = Group::save(&tree, tree.root().unwrap(), &group_names);
        assert_eq!(serde_cbor_2::to_vec(&saved).unwrap(), bytes);
    }

    #[test]
    fn layouts_survive_json() {
        let mut views: Vec<Node> = ViewType::ALL
            .into_iter()
            .enumerate()
            .map(|(index, view_type)| {
                let mut view = View::new(view_type, format!("View {index}"));
                if let Some(selection) = view.selection_mut() {
                    selection.paths.insert("imu.acc[1]".to_owned());
                }
                Node::View(view)
            })
            .collect();
        if let Node::View(view) = &mut views[0] {
            view.formats
                .set("imu.acc[1]", crate::app::model::numbers::DisplayFormat::Hex);
        }
        let file = Group::tabs(vec![Node::Group(Group {
            shares: vec![1.5; views.len()],
            ..Group::tabs(views)
        })]);
        let cbor = serde_cbor_2::to_vec(&file).unwrap();

        let json = serde_json::to_vec_pretty(&file).unwrap();
        let file: Group = serde_json::from_slice(&json).unwrap();
        assert_eq!(serde_cbor_2::to_vec(&file).unwrap(), cbor);
    }
}
