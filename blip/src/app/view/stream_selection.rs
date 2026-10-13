use crate::app::model::Model;
use crate::app::model::streams::{Samples, Stream, element_path};
use crate::app::view::{Selectable, ViewType};
use egui::{RichText, Ui};
use egui_phosphor::regular::{BRACKETS_CURLY, BRACKETS_SQUARE, HASH, TEXT_AA};
use std::collections::{BTreeMap, BTreeSet};

/// Arrays with up to this many elements list each element for selection.
pub const MAX_LISTED_ELEMENTS: usize = 16;

/// Stream paths split into a tree at each "/", and at each "." between the fields of
/// a record.
#[derive(Default)]
pub struct StreamTree<'a> {
    /// The stream if one ends at this node.
    pub stream: Option<&'a Stream>,
    /// Children by path segment, record fields with their leading ".", e.g. ".acc".
    pub children: BTreeMap<&'a str, StreamTree<'a>>,
}

impl<'a> StreamTree<'a> {
    pub fn build(streams: impl Iterator<Item = &'a Stream>) -> Self {
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

    pub fn is_record(&self) -> bool {
        self.children.keys().any(|segment| segment.starts_with('.'))
    }

    /// The paths of all streams at and below this node that a view of `view_type` can
    /// show.
    fn paths(&self, view_type: ViewType) -> Vec<String> {
        self.stream
            .into_iter()
            .flat_map(|stream| stream_paths(stream, view_type))
            .chain(
                self.children
                    .values()
                    .flat_map(|child| child.paths(view_type)),
            )
            .collect()
    }
}

/// The icon of the type of `samples`.
pub fn type_icon(samples: &Samples) -> &'static str {
    match samples {
        Samples::Empty | Samples::Numbers { .. } => HASH,
        Samples::Strings(_) => TEXT_AA,
        Samples::Arrays { .. } => BRACKETS_SQUARE,
    }
}

/// The streams selected for a view.
struct Selection<'a> {
    paths: &'a mut BTreeSet<String>,
    view_type: ViewType,
}

/// Shows a checkbox labelled `label` with a weak `icon` after it for selecting all
/// `paths`: indeterminate if only some are selected, and disabled if there are none.
fn show_checkbox(
    ui: &mut Ui,
    label: &str,
    icon: Option<&str>,
    paths: &[String],
    selection: &mut Selection<'_>,
) -> egui::Response {
    let selected_count = paths
        .iter()
        .filter(|path| selection.paths.contains(*path))
        .count();
    let mut selected = !paths.is_empty() && selected_count == paths.len();
    let checkbox = match icon {
        Some(icon) => egui::Checkbox::new(&mut selected, (label, RichText::new(icon).weak())),
        None => egui::Checkbox::new(&mut selected, label),
    };
    let checkbox = checkbox.indeterminate(selected_count > 0 && selected_count < paths.len());
    let response = ui.add_enabled(!paths.is_empty(), checkbox);
    if response.changed() {
        if selected && selection.view_type.shows_one_stream() {
            selection.paths.clear();
        }
        for path in paths {
            if selected {
                selection.paths.insert(path.clone());
            } else {
                selection.paths.remove(path);
            }
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

/// The paths of `stream` that a view of `view_type` can show: its own, those of its
/// elements, or none.
pub fn stream_paths(stream: &Stream, view_type: ViewType) -> Vec<String> {
    match (view_type.selectable(stream), &stream.samples) {
        (Selectable::Elements, Samples::Arrays { shape, .. }) => (0..shape.iter().product())
            .map(|flat_index| element_path(&stream.path, &element_indices(shape, flat_index)))
            .collect(),
        (Selectable::No(_), _) => vec![],
        _ => vec![stream.path.clone()],
    }
}

/// Shows `stream` labelled `label` with the icon of its type: a checkbox, or a
/// collapsible list of checkboxes for its elements.
fn show_stream(ui: &mut Ui, label: &str, stream: &Stream, selection: &mut Selection<'_>) {
    let icon = Some(type_icon(&stream.samples));
    let paths = stream_paths(stream, selection.view_type);
    let mut hover = format!("{}\n{}", stream.path, stream.samples.type_label());
    if let Some(remark) = selection.view_type.remark(stream) {
        hover.push_str(&format!("\n{remark}"));
    }
    match selection.view_type.selectable(stream) {
        Selectable::Stream => {}
        Selectable::No(reason) => hover.push_str(&format!("\n{reason}")),
        Selectable::Elements => {
            let id = ui.make_persistent_id(("stream_array", &stream.path));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    show_checkbox(ui, label, icon, &paths, selection).on_hover_text(hover);
                })
                .body(|ui| {
                    for path in &paths {
                        let label = &path[stream.path.len()..];
                        show_checkbox(ui, label, None, std::slice::from_ref(path), selection)
                            .on_hover_text(path);
                    }
                });
            return;
        }
    }
    show_checkbox(ui, label, icon, &paths, selection)
        .on_hover_text(&hover)
        .on_disabled_hover_text(hover);
}

/// Shows the children of `node` as collapsible branches and streams.
fn show_stream_tree(
    ui: &mut Ui,
    node: &StreamTree<'_>,
    prefix: &str,
    selection: &mut Selection<'_>,
) {
    for (segment, child) in &node.children {
        let child_prefix = format!("{prefix}/{segment}");
        let label = segment.strip_prefix('.').unwrap_or(segment);
        if child.children.is_empty() {
            if let Some(stream) = child.stream {
                show_stream(ui, label, stream, selection);
            }
        } else {
            let icon = child.is_record().then_some(BRACKETS_CURLY);
            let id = ui.make_persistent_id(("stream_tree", &child_prefix));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    if selection.view_type.shows_one_stream() {
                        // a branch cannot be selected as one stream
                        match icon {
                            Some(icon) => ui.label(format!("{label} {icon}")),
                            None => ui.label(label),
                        };
                    } else {
                        let paths = child.paths(selection.view_type);
                        show_checkbox(ui, label, icon, &paths, selection);
                    }
                })
                .body(|ui| {
                    // a stream can also be the prefix of other streams
                    if let Some(stream) = child.stream {
                        show_stream(ui, label, stream, selection);
                    }
                    show_stream_tree(ui, child, &child_prefix, selection);
                });
        }
    }
}

/// Shows all streams of `model` as a tree with checkboxes for the streams selected for
/// a view of the given type, disabled if there is no selection to edit.
pub fn show(ui: &mut Ui, model: &Model, selection: Option<(&mut BTreeSet<String>, ViewType)>) {
    let tree = StreamTree::build(model.streams.iter());
    if tree.children.is_empty() {
        ui.weak("No streams received");
        return;
    }
    let mut unselectable = BTreeSet::new();
    let enabled = selection.is_some();
    let (paths, view_type) = selection.unwrap_or((&mut unselectable, ViewType::Plot));
    let mut selection = Selection { paths, view_type };
    ui.add_enabled_ui(enabled, |ui| {
        show_stream_tree(ui, &tree, "", &mut selection);
    });
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
    fn stream_tree_paths_list_array_elements() {
        let array = |path: &str, len| {
            let mut stream = Stream::new(path.to_owned(), Some(3));
            stream.samples = Samples::Arrays {
                shape: vec![len],
                kind: None,
                values: vec![],
            };
            stream
        };
        let streams = [
            array("imu.acc", 2),
            array("imu.big", MAX_LISTED_ELEMENTS + 1),
            Stream::new("imu.ok".to_owned(), Some(3)),
        ];
        let tree = StreamTree::build(streams.iter());
        assert_eq!(
            tree.children["imu"].paths(ViewType::Plot),
            ["imu.acc[0]", "imu.acc[1]", "imu.ok"]
        );
    }

    #[test]
    fn element_indices_are_row_major() {
        assert_eq!(element_indices(&[2, 3], 0), [0, 0]);
        assert_eq!(element_indices(&[2, 3], 4), [1, 1]);
        assert_eq!(element_indices(&[4], 3), [3]);
    }
}
