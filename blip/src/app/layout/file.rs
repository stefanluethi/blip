//! The saved form of a layout, part of a preset: the tree of groups with the views
//! and their settings as leaves.

use crate::app::view::{View, ViewId};
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
    /// group of the tabs at the top level. A view refers to another by the number of
    /// that view in the saved order.
    pub fn save(tree: &Tree<View>, root: TileId, group_names: &HashMap<TileId, String>) -> Self {
        let mut views = vec![];
        let mut group = match save_node(&tree.tiles, root, group_names, &mut views) {
            Some(Node::Group(group)) => group,
            _ => Group::tabs(vec![]),
        };
        let numbers: HashMap<ViewId, ViewId> = (views.iter().enumerate())
            .map(|(number, tile_id)| (ViewId(tile_id.0), ViewId(number as u64)))
            .collect();
        group.relink(&|id| numbers.get(&id).copied());
        group
    }

    /// Restores the tiles of the group of the tabs at the top level, returns the tree
    /// and the group names.
    pub fn load(self) -> (Tree<View>, HashMap<TileId, String>) {
        let mut tiles = Tiles::default();
        let mut group_names = HashMap::new();
        let mut views = vec![];
        let root = load_group(&mut tiles, &mut group_names, self, &mut views);
        // the numbers of the views in the saved order become their tiles
        for &tile_id in &views {
            if let Some(Tile::Pane(view)) = tiles.get_mut(tile_id) {
                view.relink(|ViewId(number)| {
                    let tile_id = views.get(usize::try_from(number).ok()?)?;
                    Some(ViewId(tile_id.0))
                });
            }
        }
        (Tree::new("layout", root, tiles), group_names)
    }

    /// Changes the views the views of the group refer to by `map`, see [`View::relink`].
    fn relink(&mut self, map: &impl Fn(ViewId) -> Option<ViewId>) {
        for child in &mut self.children {
            match child {
                Node::View(view) => view.relink(map),
                Node::Group(group) => group.relink(map),
            }
        }
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

/// The saved form of `tile_id`, the views in it added to `views` in their order.
fn save_node(
    tiles: &Tiles<View>,
    tile_id: TileId,
    group_names: &HashMap<TileId, String>,
    views: &mut Vec<TileId>,
) -> Option<Node> {
    match tiles.get(tile_id)? {
        Tile::Pane(view) => {
            views.push(tile_id);
            Some(Node::View(view.clone()))
        }
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
                    .filter_map(|&id| save_node(tiles, id, group_names, views))
                    .collect(),
                shares,
                active,
            }))
        }
    }
}

/// Inserts the tiles of `group` into `tiles`, the views in it added to `views` in their
/// order.
fn load_group(
    tiles: &mut Tiles<View>,
    group_names: &mut HashMap<TileId, String>,
    group: Group,
    views: &mut Vec<TileId>,
) -> TileId {
    let children: Vec<TileId> = group
        .children
        .into_iter()
        .map(|node| match node {
            Node::View(view) => {
                let tile_id = tiles.insert_pane(view);
                views.push(tile_id);
                tile_id
            }
            Node::Group(group) => load_group(tiles, group_names, group, views),
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
    use crate::app::view::{ViewKind, ViewType, histogram};

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
        // a histogram with all its settings changed, following the plot, view 0
        for node in &mut views {
            if let Node::View(View {
                kind: ViewKind::Histogram(histogram),
                ..
            }) = node
            {
                histogram.bins = histogram::BinMode::Width;
                histogram.bin_count = 12;
                histogram.bin_width = 0.5;
                histogram.range = Some((-1.0, 2.0));
                histogram.counts = histogram::CountScale::Log;
                histogram.orientation = histogram::Orientation::Sideways;
                histogram.axis_side = histogram::AxisSide::Right;
                histogram.follow = Some(ViewId(0));
                histogram.markers = histogram::Markers::MeanSigma;
            }
        }
        let file = Group::tabs(vec![Node::Group(Group {
            shares: vec![1.5; views.len()],
            ..Group::tabs(views)
        })]);
        let cbor = serde_cbor_2::to_vec(&file).unwrap();

        let json = serde_json::to_vec_pretty(&file).unwrap();
        let text = String::from_utf8_lossy(&json);
        for setting in [
            r#""bins": "Width""#,
            r#""axis_side": "Right""#,
            r#""follow": 0"#,
        ] {
            assert!(text.contains(setting), "{setting}");
        }
        let file: Group = serde_json::from_slice(&json).unwrap();
        assert_eq!(serde_cbor_2::to_vec(&file).unwrap(), cbor);
    }
}
