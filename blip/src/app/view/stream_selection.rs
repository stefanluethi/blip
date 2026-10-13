use crate::app::model::Model;
use crate::app::model::streams::{Samples, Stream, element_path};
use egui::Ui;
use std::collections::{BTreeMap, BTreeSet};

/// Arrays with up to this many elements list each element for selection.
const MAX_LISTED_ELEMENTS: usize = 16;

/// Stream paths split into a tree at each "/", and at each "." between the fields of
/// a record.
#[derive(Default)]
struct StreamTree<'a> {
    /// The stream if one ends at this node.
    stream: Option<&'a Stream>,
    /// Children by path segment, record fields with their leading ".", e.g. ".acc".
    children: BTreeMap<&'a str, StreamTree<'a>>,
}

impl<'a> StreamTree<'a> {
    fn build(streams: impl Iterator<Item = &'a Stream>) -> Self {
        let mut root = Self::default();
        for stream in streams {
            let path = stream.path.as_str();
            let (name, fields) = path.split_at(stream.fields_start.unwrap_or(path.len()));
            let field_starts = fields.match_indices('.').map(|(start, _)| start);
            let field_ends = field_starts.clone().skip(1).chain([fields.len()]);
            let field_segments = field_starts
                .zip(field_ends)
                .map(|(start, end)| &fields[start..end]);

            let mut node = &mut root;
            let segments = name.split('/').filter(|segment| !segment.is_empty());
            for segment in segments.chain(field_segments) {
                node = node.children.entry(segment).or_default();
            }
            node.stream = Some(stream);
        }
        root
    }

    fn is_record(&self) -> bool {
        self.children.keys().any(|segment| segment.starts_with('.'))
    }
}

fn show_checkbox(
    ui: &mut Ui,
    label: &str,
    path: &str,
    selection: &mut BTreeSet<String>,
) -> egui::Response {
    let mut selected = selection.contains(path);
    let response = ui.checkbox(&mut selected, label);
    if response.changed() {
        if selected {
            selection.insert(path.to_owned());
        } else {
            selection.remove(path);
        }
    }
    response
}

/// The indices of the element at `flat_index` of an array of `shape`, in row-major
/// order.
fn element_indices(shape: &[usize], mut flat_index: usize) -> Vec<usize> {
    let mut indices = vec![0; shape.len()];
    for (index, len) in indices.iter_mut().zip(shape).rev() {
        *index = flat_index % len;
        flat_index /= len;
    }
    indices
}

/// Shows `stream` labelled `label` with its type: a checkbox, or for an array a
/// collapsible list of checkboxes for its elements.
fn show_stream(ui: &mut Ui, label: &str, stream: &Stream, selection: &mut BTreeSet<String>) {
    let type_label = stream.samples.type_label();
    let Samples::Arrays { shape, .. } = &stream.samples else {
        ui.horizontal(|ui| {
            let response = show_checkbox(ui, label, &stream.path, selection);
            ui.weak(type_label);
            let hover = match stream.samples {
                Samples::Strings(_) => format!(
                    "{}\nStrings are shown in the plot tooltip only",
                    stream.path
                ),
                _ => stream.path.clone(),
            };
            response.on_hover_text(hover);
        });
        return;
    };

    let element_count: usize = shape.iter().product();
    if element_count > MAX_LISTED_ELEMENTS {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.weak(type_label);
        })
        .response
        .on_hover_text(format!(
            "{}\nArrays of more than {MAX_LISTED_ELEMENTS} elements cannot be plotted yet",
            stream.path
        ));
        return;
    }
    let id = ui.make_persistent_id(("stream_array", &stream.path));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| {
            ui.label(label).on_hover_text(&stream.path);
            ui.weak(type_label);
        })
        .body(|ui| {
            for flat_index in 0..element_count {
                let indices = element_indices(shape, flat_index);
                let path = element_path(&stream.path, &indices);
                let label = element_path("", &indices);
                show_checkbox(ui, &label, &path, selection).on_hover_text(&path);
            }
        });
}

/// Shows the children of `node` as collapsible branches and streams.
fn show_stream_tree(
    ui: &mut Ui,
    node: &StreamTree<'_>,
    prefix: &str,
    selection: &mut BTreeSet<String>,
) {
    for (segment, child) in &node.children {
        let child_prefix = format!("{prefix}/{segment}");
        if child.children.is_empty() {
            if let Some(stream) = child.stream {
                show_stream(ui, segment, stream, selection);
            }
        } else {
            let id = ui.make_persistent_id(("stream_tree", &child_prefix));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    ui.label(*segment);
                    if child.is_record() {
                        ui.weak("record");
                    }
                })
                .body(|ui| {
                    // a stream can also be the prefix of other streams
                    if let Some(stream) = child.stream {
                        show_stream(ui, segment, stream, selection);
                    }
                    show_stream_tree(ui, child, &child_prefix, selection);
                });
        }
    }
}

/// Shows all streams of `model` as a tree with checkboxes for the streams in
/// `selection`, disabled if there is no selection to edit.
pub fn show(ui: &mut Ui, model: &Model, selection: Option<&mut BTreeSet<String>>) {
    let tree = StreamTree::build(model.streams.iter());
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
        let streams: Vec<Stream> = ["acc/x", "acc/y", "/cpu", "acc"]
            .into_iter()
            .map(|path| Stream::new(path.to_owned(), None))
            .collect();
        let tree = StreamTree::build(streams.iter());
        assert_eq!(
            tree.children.keys().copied().collect::<Vec<_>>(),
            ["acc", "cpu"]
        );
        let acc = &tree.children["acc"];
        assert_eq!(acc.stream.map(|stream| stream.path.as_str()), Some("acc"));
        assert_eq!(acc.children.keys().copied().collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(
            acc.children["x"].stream.map(|stream| stream.path.as_str()),
            Some("acc/x")
        );
        assert_eq!(
            tree.children["cpu"]
                .stream
                .map(|stream| stream.path.as_str()),
            Some("/cpu")
        );
    }

    #[test]
    fn stream_tree_branches_at_record_fields() {
        let streams = [
            Stream::new("sensors/imu.acc.z".to_owned(), Some(11)),
            Stream::new("sensors/imu.ok".to_owned(), Some(11)),
            Stream::new("sensors/v1.2".to_owned(), None),
        ];
        let tree = StreamTree::build(streams.iter());
        let sensors = &tree.children["sensors"];
        assert_eq!(
            sensors.children.keys().copied().collect::<Vec<_>>(),
            ["imu", "v1.2"]
        );
        let imu = &sensors.children["imu"];
        assert!(imu.is_record());
        assert!(!sensors.is_record());
        assert_eq!(
            imu.children.keys().copied().collect::<Vec<_>>(),
            [".acc", ".ok"]
        );
        assert_eq!(
            imu.children[".acc"].children[".z"]
                .stream
                .map(|stream| stream.path.as_str()),
            Some("sensors/imu.acc.z")
        );
    }

    #[test]
    fn element_indices_are_row_major() {
        assert_eq!(element_indices(&[2, 3], 0), [0, 0]);
        assert_eq!(element_indices(&[2, 3], 4), [1, 1]);
        assert_eq!(element_indices(&[4], 3), [3]);
    }
}
