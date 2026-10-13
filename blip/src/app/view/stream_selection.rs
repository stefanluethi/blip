use crate::app::model::Model;
use egui::Ui;
use std::collections::{BTreeMap, BTreeSet};

/// Stream paths split into a tree at each "/".
#[derive(Default)]
struct StreamTree<'a> {
    /// Full stream path if a stream ends at this node.
    path: Option<&'a str>,
    children: BTreeMap<&'a str, StreamTree<'a>>,
}

impl<'a> StreamTree<'a> {
    fn build(paths: impl Iterator<Item = &'a str>) -> Self {
        let mut root = Self::default();
        for path in paths {
            let mut node = &mut root;
            for segment in path.split('/').filter(|segment| !segment.is_empty()) {
                node = node.children.entry(segment).or_default();
            }
            node.path = Some(path);
        }
        root
    }
}

fn show_stream_checkbox(ui: &mut Ui, label: &str, path: &str, selection: &mut BTreeSet<String>) {
    let mut selected = selection.contains(path);
    if ui.checkbox(&mut selected, label).on_hover_text(path).changed() {
        if selected {
            selection.insert(path.to_owned());
        } else {
            selection.remove(path);
        }
    }
}

/// Shows the children of `node` as collapsible branches and stream checkboxes.
fn show_stream_tree(
    ui: &mut Ui,
    node: &StreamTree<'_>,
    prefix: &str,
    selection: &mut BTreeSet<String>,
) {
    for (segment, child) in &node.children {
        let child_prefix = format!("{prefix}/{segment}");
        if child.children.is_empty() {
            if let Some(path) = child.path {
                show_stream_checkbox(ui, segment, path, selection);
            }
        } else {
            egui::CollapsingHeader::new(*segment)
                .id_salt(("stream_tree", &child_prefix))
                .default_open(true)
                .show(ui, |ui| {
                    // a stream can also be the prefix of other streams
                    if let Some(path) = child.path {
                        show_stream_checkbox(ui, segment, path, selection);
                    }
                    show_stream_tree(ui, child, &child_prefix, selection);
                });
        }
    }
}

/// Shows all streams of `model` as a tree with checkboxes for the streams in
/// `selection`, disabled if there is no selection to edit.
pub fn show(ui: &mut Ui, model: &Model, selection: Option<&mut BTreeSet<String>>) {
    let tree = StreamTree::build(model.streams.iter().map(|stream| stream.path.as_str()));
    if tree.children.is_empty() {
        ui.weak("No streams received");
        return;
    }
    let mut unselectable = BTreeSet::new();
    let enabled = selection.is_some();
    let selection = selection.unwrap_or(&mut unselectable);
    ui.add_enabled_ui(enabled, |ui| show_stream_tree(ui, &tree, "", selection));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_tree_branches_at_slashes() {
        let tree = StreamTree::build(["acc/x", "acc/y", "/cpu", "acc"].into_iter());
        assert_eq!(tree.children.keys().copied().collect::<Vec<_>>(), ["acc", "cpu"]);
        let acc = &tree.children["acc"];
        assert_eq!(acc.path, Some("acc"));
        assert_eq!(acc.children.keys().copied().collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(acc.children["x"].path, Some("acc/x"));
        assert_eq!(tree.children["cpu"].path, Some("/cpu"));
    }
}
