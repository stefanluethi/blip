use crate::app::model::Model;
use crate::app::model::traces::{Samples, Trace, element_path};
use crate::app::theme;
use crate::app::view::{Selectable, ViewType};
use egui::{RichText, Ui};
use egui_phosphor::regular::{
    BRACKETS_CURLY, BRACKETS_SQUARE, FUNNEL, HASH, LIGHTNING, MAGNIFYING_GLASS, TEXT_AA, X,
};
use std::collections::{BTreeMap, BTreeSet};

/// Arrays with up to this many elements list each element for selection.
pub const MAX_LISTED_ELEMENTS: usize = 16;

/// The icon marking an action in the tree, in place of the type of a trace.
pub const ACTION_ICON: &str = LIGHTNING;

/// What a view picks in the tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Picks {
    pub view_type: ViewType,
    /// Whether the view picks whole records instead of traces, like a table of records.
    pub records: bool,
}

impl From<ViewType> for Picks {
    fn from(view_type: ViewType) -> Self {
        Self {
            view_type,
            records: false,
        }
    }
}

/// What is picked for a view in the tree.
pub struct ViewSelection<'a> {
    /// The paths of the picked traces, array elements, records or actions.
    pub paths: &'a mut BTreeSet<String>,
    /// The keys of the branches that also pick what is added below them later. The key
    /// of a node is the segments leading to it, each after a "/", e.g. "/sensors/imu".
    pub wildcards: &'a mut BTreeSet<String>,
    pub picks: Picks,
}

/// Trace and action paths split into a tree at each "/", and trace paths also at each
/// "." between the fields of a record.
#[derive(Default)]
pub struct TraceTree<'a> {
    /// The trace if one ends at this node.
    pub trace: Option<Trace<'a>>,
    /// The path of the record if one ends at this node, i.e. its fields are below it.
    pub record: Option<&'a str>,
    /// The path of the action if one ends at this node.
    pub action: Option<&'a str>,
    /// Whether the action is shown by the view, but no outpost announces it.
    pub unannounced: bool,
    /// Children by path segment, record fields with their leading ".", e.g. ".acc".
    pub children: BTreeMap<&'a str, TraceTree<'a>>,
}

impl<'a> TraceTree<'a> {
    pub fn build(traces: impl Iterator<Item = Trace<'a>>) -> Self {
        let mut root = Self::default();
        for trace in traces {
            if let Some(record) = record_path(trace) {
                root.node_mut(segments(record)).record = Some(record);
            }
            root.node_mut(trace_segments(trace)).trace = Some(trace);
        }
        root
    }

    /// Adds the actions at `paths`, which branch at each "/" only.
    pub fn add_actions(&mut self, paths: impl Iterator<Item = &'a str>) {
        for path in paths {
            self.node_mut(segments(path)).action = Some(path);
        }
    }

    /// Adds the actions at `paths` that a view shows but no outpost announces, so they
    /// can be unchecked.
    pub fn add_unannounced_actions(&mut self, paths: impl Iterator<Item = &'a str>) {
        for path in paths {
            let node = self.node_mut(segments(path));
            node.action = Some(path);
            node.unannounced = true;
        }
    }

    /// The node at `segments` below this one, added if missing.
    fn node_mut(&mut self, segments: impl Iterator<Item = &'a str>) -> &mut Self {
        let mut node = self;
        for segment in segments {
            node = node.children.entry(segment).or_default();
        }
        node
    }

    /// The node with `key` below this one, see [`ViewSelection::wildcards`].
    fn node(&self, key: &str) -> Option<&Self> {
        key.split('/')
            .filter(|segment| !segment.is_empty())
            .try_fold(self, |node, segment| node.children.get(segment))
    }

    /// Removes the fields of the records, so that records are picked as a whole.
    fn fold_records(&mut self) {
        if self.record.is_some() {
            self.children.retain(|segment, _| !segment.starts_with('.'));
        }
        for child in self.children.values_mut() {
            child.fold_records();
        }
    }

    /// Removes the traces, records and actions of which a view picking `picks` can show
    /// nothing, and the branches left without any.
    pub fn retain_selectable(&mut self, picks: Picks) {
        if self.trace.is_some_and(|trace| {
            trace_picks(trace, picks).is_empty() && whole_array_pick(trace, picks).is_none()
        }) {
            self.trace = None;
        }
        if !picks.records {
            self.record = None;
        }
        if self
            .action
            .is_some_and(|path| action_path(path, picks.view_type).is_none())
        {
            self.action = None;
        }
        self.children.retain(|_, child| {
            child.retain_selectable(picks);
            child.trace.is_some()
                || child.record.is_some()
                || child.action.is_some()
                || !child.children.is_empty()
        });
    }

    pub fn is_record(&self) -> bool {
        self.record.is_some() || self.children.keys().any(|segment| segment.starts_with('.'))
    }

    /// The paths of all traces, records and actions at and below this node that a view
    /// picking `picks` can show.
    fn paths(&self, picks: Picks) -> Vec<String> {
        self.trace
            .into_iter()
            .flat_map(|trace| trace_picks(trace, picks))
            .chain(self.record.filter(|_| picks.records).map(str::to_owned))
            .chain(
                self.action
                    .and_then(|path| action_path(path, picks.view_type)),
            )
            .chain(self.children.values().flat_map(|child| child.paths(picks)))
            .collect()
    }
}

/// The segments of `path` between its "/".
fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|segment| !segment.is_empty())
}

/// The segments of the node of `trace` in the tree: those of its path between its "/",
/// then its record fields with their leading ".".
fn trace_segments(trace: Trace<'_>) -> impl Iterator<Item = &str> {
    let path = trace.path;
    let (name, fields) = path.split_at(trace.fields_start.unwrap_or(path.len()));
    let field_starts = fields.match_indices('.').map(|(start, _)| start);
    let field_ends = field_starts.clone().skip(1).chain([fields.len()]);
    let field_segments = field_starts
        .zip(field_ends)
        .map(|(start, end)| &fields[start..end]);
    segments(name).chain(field_segments)
}

/// The path of the record `trace` is a field of, e.g. "imu" for "imu.acc.z".
pub fn record_path(trace: Trace<'_>) -> Option<&str> {
    trace.fields_start.map(|start| &trace.path[..start])
}

/// Whether the node with `key` is below the node with `branch`.
fn key_is_below(key: &str, branch: &str) -> bool {
    key.strip_prefix(branch)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Adds the paths a view picking `selection.picks` can show below its wildcards, so
/// that what is added to a branch later is picked as well.
pub fn expand_wildcards(model: &Model, selection: ViewSelection<'_>) {
    if selection.wildcards.is_empty() {
        return;
    }
    let mut tree = TraceTree::build(model.traces.iter());
    tree.add_actions(model.actions.paths());
    for key in selection.wildcards.iter() {
        if let Some(node) = tree.node(key) {
            selection.paths.extend(node.paths(selection.picks));
        }
    }
}

/// The icon of the type of `samples`.
pub fn type_icon(samples: &Samples) -> &'static str {
    match samples {
        Samples::Empty(_) | Samples::Numbers { .. } => HASH,
        Samples::Strings(_) => TEXT_AA,
        Samples::Arrays { .. } => BRACKETS_SQUARE,
    }
}

/// What is picked for a view while the tree is shown, see [`ViewSelection`].
struct Selection<'a> {
    paths: &'a mut BTreeSet<String>,
    /// The paths of the traces the view shows apart from the others, see
    /// [`ViewType::shows_apart`].
    apart: BTreeSet<&'a str>,
    wildcards: &'a mut BTreeSet<String>,
    picks: Picks,
    /// Whether only the traces matching a search are shown.
    searching: bool,
}

/// Shows a checkbox labelled `label` with a weak `icon` after it for picking all
/// `paths` of the node with `key`: indeterminate if only some are picked, and disabled
/// if there are none. Checking a `branch` also picks what is added below it later,
/// unless only the traces matching a search are shown.
fn show_checkbox(
    ui: &mut Ui,
    label: &str,
    icon: Option<&str>,
    paths: &[String],
    key: &str,
    branch: bool,
    selection: &mut Selection<'_>,
) -> egui::Response {
    let picked_count = paths
        .iter()
        .filter(|path| selection.paths.contains(*path))
        .count();
    let all = !paths.is_empty() && picked_count == paths.len();
    let mut checked = all;
    let checkbox = match icon {
        Some(icon) => egui::Checkbox::new(&mut checked, (label, RichText::new(icon).weak())),
        None => egui::Checkbox::new(&mut checked, label),
    };
    let checkbox = checkbox.indeterminate(picked_count > 0 && picked_count < paths.len());
    let mut response = ui.add_enabled(!paths.is_empty(), checkbox);
    let wildcard = branch && !selection.searching;
    if wildcard {
        response = response.on_hover_text("Checking it also picks what is added to it later");
    }
    if response.clicked() {
        if all {
            // a wildcard at or above it would pick it again
            selection.wildcards.retain(|wildcard| {
                wildcard != key && !key_is_below(key, wildcard) && !key_is_below(wildcard, key)
            });
            for path in paths {
                selection.paths.remove(path);
            }
        } else {
            if selection.picks.view_type.shows_one_trace() {
                selection.paths.clear();
            }
            // what is shown apart and what is not are not shown together
            let apart = &selection.apart;
            let adds_apart = paths.iter().any(|path| apart.contains(path.as_str()));
            selection
                .paths
                .retain(|path| apart.contains(path.as_str()) == adds_apart);
            if adds_apart {
                // they would add what is not shown apart
                selection.wildcards.clear();
            }
            selection.paths.extend(paths.iter().cloned());
            if wildcard {
                // what was a wildcard below it is part of this one
                selection
                    .wildcards
                    .retain(|wildcard| !key_is_below(wildcard, key));
                selection.wildcards.insert(key.to_owned());
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

/// The paths of `trace` that a view of `view_type` can show along with other traces,
/// and so are picked with a branch: its own, those of its elements, or none.
pub fn trace_paths(trace: Trace<'_>, view_type: ViewType) -> Vec<String> {
    match (view_type.selectable(trace), &trace.samples) {
        (
            Selectable::Elements | Selectable::Array { elements: true },
            Samples::Arrays { shape, .. },
        ) => (0..shape.iter().product())
            .map(|flat_index| element_path(trace.path, &element_indices(shape, flat_index)))
            .collect(),
        (Selectable::No(_) | Selectable::Array { .. }, _) => vec![],
        _ => vec![trace.path.to_owned()],
    }
}

/// The path of `trace` if a view picking `picks` can show it as a whole array, see
/// [`Selectable::Array`].
fn whole_array_pick(trace: Trace<'_>, picks: Picks) -> Option<String> {
    (!picks.records && picks.view_type.shows_apart(trace)).then(|| trace.path.to_owned())
}

/// The paths of `trace` that a view picking `picks` can show, none if it picks records.
fn trace_picks(trace: Trace<'_>, picks: Picks) -> Vec<String> {
    if picks.records {
        vec![]
    } else {
        trace_paths(trace, picks.view_type)
    }
}

/// Shows `trace` labelled `label` with the icon of its type: a checkbox, or a
/// collapsible list of checkboxes for its elements below a checkbox for all of them,
/// after one for the whole array if the view shows it so.
fn show_trace(
    ui: &mut Ui,
    label: &str,
    key: &str,
    trace: Trace<'_>,
    selection: &mut Selection<'_>,
) {
    let icon = Some(type_icon(trace.samples));
    let picks = selection.picks;
    let paths = trace_picks(trace, picks);
    let mut hover = format!("{}\n{}", trace.path, trace.samples.type_label());
    // what the checkbox of the trace picks, the elements listed below it, and whether
    // the whole array is listed before them
    let (paths, element_paths, whole) = match picks.view_type.selectable(trace) {
        _ if picks.records => {
            hover.push_str("\nTables of records show records");
            (paths, None, false)
        }
        Selectable::Trace => (paths, None, false),
        Selectable::No(reason) => {
            hover.push_str(&format!("\n{reason}"));
            (paths, None, false)
        }
        Selectable::Elements => (paths.clone(), Some(paths), false),
        Selectable::Array { elements: true } => (paths.clone(), Some(paths), true),
        Selectable::Array { elements: false } => {
            hover.push_str(&format!("\n{WHOLE_ARRAY_HINT}"));
            (vec![trace.path.to_owned()], None, false)
        }
    };
    let Some(element_paths) = element_paths else {
        leaf_row(ui, |ui| {
            show_checkbox(ui, label, icon, &paths, key, false, selection)
                .on_hover_text(&hover)
                .on_disabled_hover_text(hover);
        });
        return;
    };
    let id = ui.make_persistent_id(("trace_array", &trace.path));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| {
            show_checkbox(ui, label, icon, &paths, key, false, selection).on_hover_text(hover);
        })
        .body(|ui| {
            if whole {
                leaf_row(ui, |ui| {
                    let paths = [trace.path.to_owned()];
                    show_checkbox(ui, "whole array", None, &paths, key, false, selection)
                        .on_hover_text(format!("{}\n{WHOLE_ARRAY_HINT}", trace.path));
                });
            }
            for path in &element_paths {
                let label = &path[trace.path.len()..];
                leaf_row(ui, |ui| {
                    let paths = std::slice::from_ref(path);
                    show_checkbox(ui, label, None, paths, key, false, selection)
                        .on_hover_text(path);
                });
            }
        });
}

/// What a view showing an array as a whole does with it, see [`Selectable::Array`].
const WHOLE_ARRAY_HINT: &str = "Plotted as a whole over the indices of its elements";

/// Shows the record at `path` labelled `label`: a checkbox to pick it as a whole.
fn show_record(ui: &mut Ui, label: &str, key: &str, path: &str, selection: &mut Selection<'_>) {
    let paths = [path.to_owned()];
    leaf_row(ui, |ui| {
        show_checkbox(
            ui,
            label,
            Some(BRACKETS_CURLY),
            &paths,
            key,
            false,
            selection,
        )
        .on_hover_text(format!("{path}\nRecord"));
    });
}

/// The path of the action at `path` if a view of `view_type` can show it.
fn action_path(path: &str, view_type: ViewType) -> Option<String> {
    view_type.shows_actions().then(|| path.to_owned())
}

/// Shows the action at `path` labelled `label` with the action icon: a checkbox,
/// disabled unless the view shows actions.
fn show_action(
    ui: &mut Ui,
    label: &str,
    key: &str,
    path: &str,
    unannounced: bool,
    selection: &mut Selection<'_>,
) {
    let paths = Vec::from_iter(action_path(path, selection.picks.view_type));
    let mut hover = format!("{path}\nAction");
    if unannounced {
        hover.push_str(", not announced by any outpost");
    }
    if paths.is_empty() {
        hover.push_str("\nOnly actions views show actions");
    }
    leaf_row(ui, |ui| {
        show_checkbox(ui, label, Some(ACTION_ICON), &paths, key, false, selection)
            .on_hover_text(&hover)
            .on_disabled_hover_text(hover);
    });
}

/// Shows the trace, the record and the action ending at `node` with `key`, if any,
/// labelled `label`.
fn show_leaves(
    ui: &mut Ui,
    label: &str,
    key: &str,
    node: &TraceTree<'_>,
    selection: &mut Selection<'_>,
) {
    if let Some(trace) = node.trace {
        show_trace(ui, label, key, trace, selection);
    }
    if let Some(path) = node.record.filter(|_| selection.picks.records) {
        show_record(ui, label, key, path, selection);
    }
    if let Some(path) = node.action {
        show_action(ui, label, key, path, node.unannounced, selection);
    }
}

/// Shows the row added by `add` indented by the width of the collapse button of a
/// branch, so its checkbox lines up with those of the branches next to it.
fn leaf_row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().indent);
        add(ui);
    });
}

/// Shows the children of `node` with the key `prefix` as collapsible branches, traces,
/// records and actions.
fn show_trace_tree(ui: &mut Ui, node: &TraceTree<'_>, prefix: &str, selection: &mut Selection<'_>) {
    for (segment, child) in &node.children {
        let key = format!("{prefix}/{segment}");
        let label = segment.strip_prefix('.').unwrap_or(segment);
        if child.children.is_empty() {
            show_leaves(ui, label, &key, child, selection);
        } else {
            let icon = child.is_record().then_some(BRACKETS_CURLY);
            // while searching, branches keep a separate open state, open by default
            let id = ui.make_persistent_id(("trace_tree", &key, selection.searching));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    if selection.picks.view_type.shows_one_trace() {
                        // a branch cannot be selected as one trace
                        match icon {
                            Some(icon) => ui.label(format!("{label} {icon}")),
                            None => ui.label(label),
                        };
                    } else {
                        let paths = child.paths(selection.picks);
                        show_checkbox(ui, label, icon, &paths, &key, true, selection);
                    }
                })
                .body(|ui| {
                    // a trace or an action can also be the prefix of other paths
                    show_leaves(ui, label, &key, child, selection);
                    show_trace_tree(ui, child, &key, selection);
                });
        }
    }
}

/// Whether `path` contains each word of `search`, ignoring case.
fn matches(path: &str, search: &str) -> bool {
    let path = path.to_lowercase();
    search
        .to_lowercase()
        .split_whitespace()
        .all(|word| path.contains(word))
}

/// Shows a field to search the traces and actions by `search`, with a button to clear
/// it, and a toggle to show only those the selected view can show, `compatible_only`,
/// disabled and shown off unless the view is `filterable`, i.e. it shows any.
pub fn search_ui(ui: &mut Ui, search: &mut String, compatible_only: &mut bool, filterable: bool) {
    ui.horizontal(|ui| {
        // the buttons are squares as high as the field: the text, its margins and its
        // stroke, and room is left for them
        let margins = theme::FIELD_MARGIN.sum().y;
        let side = ui.text_style_height(&egui::TextStyle::Body) + margins + 2.0;
        let button_width = side + ui.spacing().item_spacing.x;
        let buttons = if search.is_empty() { 1.0 } else { 2.0 };
        // the id stays the same as the clear button comes and goes, to keep the focus
        let field = ui
            .add(
                egui::TextEdit::singleline(search)
                    .id_salt("trace_search")
                    .margin(theme::FIELD_MARGIN)
                    .hint_text(format!("{MAGNIFYING_GLASS} Search paths"))
                    .desired_width(ui.available_width() - buttons * button_width),
            )
            .on_hover_text("Show only the traces and actions whose path contains each word");
        ui.spacing_mut().button_padding.x = 0.0;
        let square = egui::Vec2::splat(field.rect.height());
        if !search.is_empty()
            && ui
                .add(egui::Button::new(X).min_size(square))
                .on_hover_text("Clear the search")
                .clicked()
        {
            search.clear();
        }
        let hint = if *compatible_only {
            "Showing only the traces and actions the selected view can show, click to show all"
        } else {
            "Click to show only the traces and actions the selected view can show"
        };
        let selected = *compatible_only && filterable;
        let filter = theme::hover_frame(egui::Button::new(FUNNEL).min_size(square));
        let filter = filter.selected(selected);
        if ui
            .add_enabled(filterable, filter)
            .on_hover_text(hint)
            .on_disabled_hover_text("Select a view that shows traces or actions to filter by it")
            .clicked()
        {
            *compatible_only = !*compatible_only;
        }
    });
}

/// Shows the traces and the announced actions of `model` whose path matches `search`
/// as a tree with checkboxes for what is picked for a view, disabled if there is no
/// selection to edit. If `compatible_only`, only what the view can show is in the tree,
/// or all without a view.
pub fn show(
    ui: &mut Ui,
    model: &Model,
    search: &str,
    compatible_only: bool,
    selection: Option<ViewSelection<'_>>,
) {
    let picks = selection.as_ref().map(|selection| selection.picks);
    let filter = picks.filter(|_| compatible_only);
    let mut tree = TraceTree::build(
        model
            .traces
            .iter()
            .filter(|trace| matches(trace.path, search)),
    );
    tree.add_actions(model.actions.paths().filter(|path| matches(path, search)));
    // shown actions no outpost announces, copied apart from the selection edited below
    let unannounced: Vec<String> = match &selection {
        Some(selection) if selection.picks.view_type.shows_actions() => selection
            .paths
            .iter()
            .filter(|path| model.actions.announcers(path).next().is_none())
            .filter(|path| matches(path, search))
            .cloned()
            .collect(),
        _ => vec![],
    };
    tree.add_unannounced_actions(unannounced.iter().map(String::as_str));
    if picks.is_some_and(|picks| picks.records) {
        tree.fold_records();
    }
    if let Some(picks) = filter {
        tree.retain_selectable(picks);
    }
    let searching = !search.trim().is_empty();
    if tree.children.is_empty() {
        ui.weak(match (filter, searching) {
            _ if model.traces.iter().next().is_none() && model.actions.is_empty() => {
                "No traces or actions received"
            }
            (Some(_), true) => "No traces or actions the selected view can show match the search",
            (Some(_), false) => "The selected view can show none of the traces and actions",
            (None, _) => "No traces or actions match the search",
        });
        return;
    }
    let enabled = selection.is_some();
    let (mut no_paths, mut no_wildcards) = (BTreeSet::new(), BTreeSet::new());
    let view = selection.unwrap_or(ViewSelection {
        paths: &mut no_paths,
        wildcards: &mut no_wildcards,
        picks: ViewType::Plot.into(),
    });
    let apart = (model.traces.iter())
        .filter(|trace| view.picks.view_type.shows_apart(*trace))
        .map(|trace| trace.path)
        .collect();
    let mut selection = Selection {
        apart,
        paths: view.paths,
        wildcards: view.wildcards,
        picks: view.picks,
        searching,
    };
    ui.add_enabled_ui(enabled, |ui| {
        show_trace_tree(ui, &tree, "", &mut selection);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is picked for a view of `view_type` as `paths`, without wildcards.
    fn selection(paths: &mut BTreeSet<String>, view_type: ViewType) -> ViewSelection<'_> {
        // a wildcard is never set by these tests, so leaking an empty set is fine
        ViewSelection {
            paths,
            wildcards: Box::leak(Box::default()),
            picks: view_type.into(),
        }
    }

    #[test]
    fn trace_tree_branches_at_slashes() {
        let traces: Vec<Trace<'_>> = ["acc/x", "acc/y", "/cpu", "acc"]
            .into_iter()
            .map(|path| Trace::leaked(path, None, Samples::Empty(0)))
            .collect();
        let tree = TraceTree::build(traces.iter().copied());
        assert_eq!(
            tree.children.keys().copied().collect::<Vec<_>>(),
            ["acc", "cpu"]
        );
        let acc = &tree.children["acc"];
        assert_eq!(acc.trace.map(|trace| trace.path), Some("acc"));
        assert_eq!(acc.children.keys().copied().collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(
            acc.children["x"].trace.map(|trace| trace.path),
            Some("acc/x")
        );
        assert_eq!(
            tree.children["cpu"].trace.map(|trace| trace.path),
            Some("/cpu")
        );
    }

    #[test]
    fn trace_tree_branches_at_record_fields() {
        let traces = [
            Trace::leaked("sensors/imu.acc.z", Some(11), Samples::Empty(0)),
            Trace::leaked("sensors/imu.ok", Some(11), Samples::Empty(0)),
            Trace::leaked("sensors/v1.2", None, Samples::Empty(0)),
        ];
        let tree = TraceTree::build(traces.iter().copied());
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
                .trace
                .map(|trace| trace.path),
            Some("sensors/imu.acc.z")
        );
    }

    #[test]
    fn trace_tree_paths_list_array_elements() {
        let array = |path: &str, len| {
            let samples = Samples::Arrays {
                shape: vec![len],
                elements: Default::default(),
            };
            Trace::leaked(path, Some(3), samples)
        };
        let traces = [
            array("imu.acc", 2),
            array("imu.big", MAX_LISTED_ELEMENTS + 1),
            Trace::leaked("imu.ok", Some(3), Samples::Empty(0)),
        ];
        let tree = TraceTree::build(traces.iter().copied());
        assert_eq!(
            tree.children["imu"].paths(ViewType::Plot.into()),
            ["imu.acc[0]", "imu.acc[1]", "imu.ok"]
        );
    }

    #[test]
    fn pruning_keeps_only_what_the_view_can_show() {
        let array = |path: &str, shape: Vec<usize>| {
            let samples = Samples::Arrays {
                shape,
                elements: Default::default(),
            };
            Trace::leaked(path, None, samples)
        };
        let text = Trace::leaked("motor/mode", None, Samples::Strings(Default::default()));
        let traces = [
            Trace::leaked("motor/rpm", None, Samples::Empty(0)),
            text,
            array("imu/acc", vec![3]),
            // arrays without elements have none to select
            array("imu/none", vec![0]),
            array("imu/big", vec![MAX_LISTED_ELEMENTS + 1]),
        ];
        let mut tree = TraceTree::build(traces.iter().copied());
        tree.add_actions(["reboot"].into_iter());
        tree.retain_selectable(ViewType::Plot.into());
        assert_eq!(
            tree.children.keys().copied().collect::<Vec<_>>(),
            ["imu", "motor"]
        );
        assert_eq!(
            tree.children["imu"]
                .children
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            ["acc"]
        );
        assert_eq!(
            tree.children["motor"]
                .children
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            ["rpm"]
        );

        let mut tree = TraceTree::build(traces.iter().copied());
        tree.add_actions(["motor/rpm", "reboot"].into_iter());
        tree.retain_selectable(ViewType::Actions.into());
        assert_eq!(
            tree.children.keys().copied().collect::<Vec<_>>(),
            ["motor", "reboot"]
        );
        let rpm = &tree.children["motor"].children["rpm"];
        assert!(rpm.trace.is_none());
        assert_eq!(rpm.action, Some("motor/rpm"));
    }

    #[test]
    fn searches_match_each_word_ignoring_case() {
        assert!(matches("motor/left.Speed", ""));
        assert!(matches("motor/left.Speed", "  LEFT speed "));
        assert!(matches("motor/left.Speed", "speed motor"));
        assert!(!matches("motor/left.Speed", "right speed"));
    }

    #[test]
    fn searching_shows_and_selects_only_the_matching_traces() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut model = Model::default();
        for path in ["motor/speed", "motor/current", "sensor/temp"] {
            model.traces.append(&proto::Value {
                timestamp: 0,
                path: path.to_owned(),
                value: proto::Data::Uint(1),
            });
        }
        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 400.0))
            .build_ui_state(
                move |ui, (search, paths): &mut (String, BTreeSet<String>)| {
                    search_ui(ui, search, &mut false, true);
                    show(
                        ui,
                        &model,
                        search,
                        false,
                        Some(selection(paths, ViewType::Plot)),
                    );
                },
                (String::new(), BTreeSet::new()),
            );
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("MOTOR spe");
        harness.run();
        assert!(harness.query_by_label_contains("speed").is_some());
        assert!(harness.query_by_label_contains("current").is_none());
        assert!(harness.query_by_label_contains("temp").is_none());

        harness.get_by_label_contains("motor").click();
        harness.run();
        assert_eq!(
            harness.state().1,
            BTreeSet::from(["motor/speed".to_owned()])
        );

        harness.get_by_label(X).click();
        harness.run();
        assert!(harness.state().0.is_empty());
        assert!(harness.query_by_label_contains("temp").is_some());
    }

    #[test]
    fn actions_join_the_traces_in_the_tree() {
        let traces = [Trace::leaked("motor/speed", None, Samples::Empty(0))];
        let mut tree = TraceTree::build(traces.iter().copied());
        tree.add_actions(["motor/speed", "motor/stop", "reboot"].into_iter());
        let motor = &tree.children["motor"];
        let speed = &motor.children["speed"];
        assert_eq!(speed.trace.map(|trace| trace.path), Some("motor/speed"));
        assert_eq!(speed.action, Some("motor/speed"));
        assert_eq!(motor.children["stop"].action, Some("motor/stop"));
        assert_eq!(tree.children["reboot"].action, Some("reboot"));
        // only actions views show actions, and no traces
        assert_eq!(
            motor.paths(ViewType::Actions.into()),
            ["motor/speed", "motor/stop"]
        );
        assert_eq!(motor.paths(ViewType::Table.into()), ["motor/speed"]);
    }

    /// A harness showing the tree of the trace "motor/rpm" and the action
    /// "motor/speed" for a view of `view_type`, with the paths selected for it.
    fn tree_harness(
        view_type: ViewType,
    ) -> egui_kittest::Harness<'static, (String, BTreeSet<String>)> {
        let mut model = Model::default();
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: "motor/rpm".to_owned(),
            value: proto::Data::Uint(1),
        });
        model.actions.set(
            "a",
            vec![proto::ActionDescriptor {
                path: "motor/speed".to_owned(),
                description: String::new(),
                params: vec![],
                readback: None,
                confirm: false,
                enabled: true,
            }],
        );
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 400.0))
            .build_ui_state(
                move |ui, (search, paths): &mut (String, BTreeSet<String>)| {
                    search_ui(ui, search, &mut false, true);
                    show(ui, &model, search, false, Some(selection(paths, view_type)));
                },
                (String::new(), BTreeSet::new()),
            );
        harness.run();
        harness
    }

    #[test]
    fn actions_views_check_the_actions_in_the_tree() {
        use egui_kittest::kittest::{NodeT, Queryable};

        let mut harness = tree_harness(ViewType::Actions);
        assert!(
            harness
                .get_by_label_contains("rpm")
                .accesskit_node()
                .is_disabled()
        );
        let speed = harness.get_by_label_contains("speed");
        assert!(
            speed
                .accesskit_node()
                .label()
                .unwrap()
                .contains(ACTION_ICON)
        );
        speed.click();
        harness.run();
        assert_eq!(
            harness.state().1,
            BTreeSet::from(["motor/speed".to_owned()])
        );
        // a branch checks the actions below it
        harness.get_by_label_contains("speed").click();
        harness.run();
        harness.get_by_label_contains("motor").click();
        harness.run();
        assert_eq!(
            harness.state().1,
            BTreeSet::from(["motor/speed".to_owned()])
        );

        // the search applies to actions too
        harness.state_mut().0 = "rpm".to_owned();
        harness.run();
        assert!(harness.query_by_label_contains("speed").is_none());
    }

    #[test]
    fn shown_actions_no_outpost_announces_can_be_unchecked() {
        use egui_kittest::kittest::Queryable;

        let mut harness = tree_harness(ViewType::Actions);
        harness.state_mut().1 = BTreeSet::from(["gone/reset".to_owned()]);
        harness.run();
        harness.get_by_label_contains("reset").click();
        harness.run();
        assert!(harness.state().1.is_empty());
        // once unchecked, it is gone from the tree
        harness.run();
        assert!(harness.query_by_label_contains("reset").is_none());
    }

    #[test]
    fn actions_are_disabled_for_other_views() {
        use egui_kittest::kittest::{NodeT, Queryable};

        let mut harness = tree_harness(ViewType::Plot);
        assert!(
            harness
                .get_by_label_contains("speed")
                .accesskit_node()
                .is_disabled()
        );
        harness.get_by_label_contains("motor").click();
        harness.run();
        assert_eq!(harness.state().1, BTreeSet::from(["motor/rpm".to_owned()]));
    }

    #[test]
    fn the_filter_shows_only_what_the_view_can_show() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::{NodeT, Queryable};

        let mut model = Model::default();
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: "motor/rpm".to_owned(),
            value: proto::Data::Uint(1),
        });
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: "motor/mode".to_owned(),
            value: proto::Data::String("idle".to_owned()),
        });
        model.actions.set(
            "a",
            vec![proto::ActionDescriptor {
                path: "motor/speed".to_owned(),
                description: String::new(),
                params: vec![],
                readback: None,
                confirm: false,
                enabled: true,
            }],
        );
        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 400.0))
            .build_ui_state(
                move |ui, (search, filter, view_type): &mut (String, bool, Option<ViewType>)| {
                    search_ui(ui, search, filter, view_type.is_some());
                    let mut paths = BTreeSet::new();
                    let selection = view_type.map(|view_type| selection(&mut paths, view_type));
                    show(ui, &model, search, *filter, selection);
                },
                (String::new(), false, Some(ViewType::Plot)),
            );
        harness.run();
        let shown = |harness: &Harness<'_, _>| {
            ["rpm", "mode", "speed"]
                .into_iter()
                .filter(|label| harness.query_by_label_contains(label).is_some())
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&harness), ["rpm", "mode", "speed"]);

        harness.get_by_label(FUNNEL).click();
        harness.run();
        assert!(harness.state().1);
        // plots show numbers, and no actions
        assert_eq!(shown(&harness), ["rpm"]);

        harness.state_mut().2 = Some(ViewType::Actions);
        harness.run();
        assert_eq!(shown(&harness), ["speed"]);

        // without a view, nothing is filtered out and the filter cannot be toggled
        harness.state_mut().2 = None;
        harness.run();
        assert_eq!(shown(&harness), ["rpm", "mode", "speed"]);
        assert!(harness.get_by_label(FUNNEL).accesskit_node().is_disabled());

        // heatmaps show arrays only, and the message tells the search apart
        harness.state_mut().2 = Some(ViewType::Heatmap);
        harness.run();
        assert!(
            harness
                .query_by_label("The selected view can show none of the traces and actions")
                .is_some()
        );
        harness.state_mut().0 = "rpm".to_owned();
        harness.run();
        assert!(
            harness
                .query_by_label("No traces or actions the selected view can show match the search")
                .is_some()
        );
        harness.state_mut().0.clear();

        harness.get_by_label(FUNNEL).click();
        harness.run();
        assert!(!harness.state().1);
        assert_eq!(shown(&harness), ["rpm", "mode", "speed"]);
    }

    #[test]
    fn the_buttons_are_as_high_as_the_search_field_and_fit_beside_it() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let width = 250.0;
        // the size of egui and that of blip, without and with the clear button
        for (size, search) in [12.5, 14.0]
            .into_iter()
            .flat_map(|size| ["", "motor"].map(|search| (size, search)))
        {
            let mut harness = Harness::builder()
                .with_size(egui::vec2(width, 100.0))
                .build_ui_state(
                    move |ui, search: &mut String| {
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Body, egui::FontId::proportional(size));
                        search_ui(ui, search, &mut false, true);
                    },
                    search.to_owned(),
                );
            harness.run();
            let field = harness.get_by_role(egui::accesskit::Role::TextInput).rect();
            let clear = harness.query_by_label(X).map(|clear| clear.rect());
            let filter = harness.get_by_label(FUNNEL).rect();
            assert_eq!(clear.is_some(), !search.is_empty());
            for button in clear.iter().chain([&filter]) {
                assert_eq!(field.height(), button.height());
                assert_eq!(field.top(), button.top());
                assert_eq!(button.width(), button.height());
            }
            let before_filter = clear.map_or(field.right(), |clear| {
                assert!(field.right() < clear.left());
                clear.right()
            });
            assert!(before_filter < filter.left() && filter.right() <= width);
        }
    }

    /// Adds a record with the fields "temp" and "volt" at `path` to `model`.
    fn add_record(model: &mut Model, path: &str) {
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: path.to_owned(),
            value: proto::Data::Record(vec![
                ("temp".to_owned(), proto::Data::Float(1.0)),
                ("volt".to_owned(), proto::Data::Uint(3)),
            ]),
        });
    }

    fn add_trace(model: &mut Model, path: &str) {
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: path.to_owned(),
            value: proto::Data::Uint(1),
        });
    }

    type PickState = (Model, BTreeSet<String>, BTreeSet<String>, String);

    /// A harness showing the search field and the tree of `model` for a view picking
    /// `picks`, with the picked paths and the wildcards, which are expanded in each
    /// frame.
    fn pick_harness(model: Model, picks: Picks) -> egui_kittest::Harness<'static, PickState> {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 400.0))
            .build_ui_state(
                move |ui, (model, paths, wildcards, search): &mut PickState| {
                    search_ui(ui, search, &mut false, true);
                    let selection = ViewSelection {
                        paths,
                        wildcards,
                        picks,
                    };
                    expand_wildcards(model, selection);
                    let selection = ViewSelection {
                        paths,
                        wildcards,
                        picks,
                    };
                    show(ui, model, search, false, Some(selection));
                },
                (model, BTreeSet::new(), BTreeSet::new(), String::new()),
            );
        harness.run();
        harness
    }

    fn set(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|path| path.to_string()).collect()
    }

    #[test]
    fn checking_a_branch_also_picks_what_is_added_to_it_later() {
        use egui_kittest::kittest::Queryable;

        let mut model = Model::default();
        for path in ["motor/a", "motor/b", "cpu"] {
            add_trace(&mut model, path);
        }
        let mut harness = pick_harness(model, ViewType::Plot.into());
        let click = |harness: &mut egui_kittest::Harness<'_, PickState>, label: &str| {
            harness.get_by_label(label).click();
            harness.run();
        };
        click(&mut harness, "motor");
        assert_eq!(harness.state().1, set(&["motor/a", "motor/b"]));
        assert_eq!(harness.state().2, set(&["/motor"]));
        add_trace(&mut harness.state_mut().0, "motor/c");
        add_trace(&mut harness.state_mut().0, "other/d");
        harness.run();
        assert_eq!(harness.state().1, set(&["motor/a", "motor/b", "motor/c"]));

        // unchecking a trace below the wildcard ends the wildcard
        click(&mut harness, &format!("a {HASH}"));
        assert_eq!(harness.state().1, set(&["motor/b", "motor/c"]));
        assert!(harness.state().2.is_empty());

        // checking the branch again picks all, unchecking it none
        click(&mut harness, "motor");
        assert_eq!(harness.state().2, set(&["/motor"]));
        click(&mut harness, "motor");
        assert!(harness.state().1.is_empty());
        assert!(harness.state().2.is_empty());

        // while searching, a branch picks only what matches, without a wildcard
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("b");
        harness.run();
        click(&mut harness, "motor");
        assert_eq!(harness.state().1, set(&["motor/b"]));
        assert!(harness.state().2.is_empty());
    }

    /// Adds an array of `len` floats at `path` to `model`.
    fn add_array(model: &mut Model, path: &str, len: usize) {
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: path.to_owned(),
            value: proto::Data::Array(vec![proto::Data::Float(1.0); len]),
        });
    }

    #[test]
    fn xy_views_check_arrays_as_a_whole_apart_from_other_traces() {
        use egui_kittest::kittest::Queryable;

        let mut model = Model::default();
        add_trace(&mut model, "sensors/rpm");
        add_array(&mut model, "sensors/adc", MAX_LISTED_ELEMENTS + 1);
        add_array(&mut model, "sensors/wave", MAX_LISTED_ELEMENTS + 1);
        add_array(&mut model, "sensors/pos", 2);
        add_array(&mut model, "sensors/grid", 4);
        let mut tree = TraceTree::build(model.traces.iter());
        // a branch picks the elements of small arrays, never whole arrays
        assert_eq!(
            tree.children["sensors"].paths(ViewType::Xy.into()),
            [
                "sensors/grid[0]",
                "sensors/grid[1]",
                "sensors/grid[2]",
                "sensors/grid[3]",
                "sensors/pos[0]",
                "sensors/pos[1]",
                "sensors/rpm"
            ]
        );
        // a big array can only be shown as a whole
        tree.retain_selectable(ViewType::Xy.into());
        assert!(tree.children["sensors"].children.contains_key("adc"));

        let mut harness = pick_harness(model, ViewType::Xy.into());
        let click = |harness: &mut egui_kittest::Harness<'_, PickState>, label: &str| {
            harness.get_by_label(label).click();
            harness.run();
        };
        let array = |name: &str| format!("{name} {BRACKETS_SQUARE}");
        click(&mut harness, &format!("rpm {HASH}"));
        assert_eq!(harness.state().1, set(&["sensors/rpm"]));
        click(&mut harness, &array("adc"));
        assert_eq!(harness.state().1, set(&["sensors/adc"]));
        click(&mut harness, &array("wave"));
        assert_eq!(harness.state().1, set(&["sensors/adc", "sensors/wave"]));

        // checking other traces unchecks the arrays, and the other way round; a small
        // array checks its elements, as in other views
        click(&mut harness, &array("pos"));
        assert_eq!(
            harness.state().1,
            set(&["sensors/pos[0]", "sensors/pos[1]"])
        );
        click(&mut harness, &array("adc"));
        click(&mut harness, "sensors");
        assert!(harness.state().1.contains("sensors/pos[1]"));
        assert!(!harness.state().1.contains("sensors/adc"));
        assert_eq!(harness.state().2, set(&["/sensors"]));
        click(&mut harness, &array("adc"));
        assert_eq!(harness.state().1, set(&["sensors/adc"]));
        assert!(harness.state().2.is_empty());
    }

    #[test]
    fn tables_of_records_pick_records_and_folders_of_records() {
        let mut model = Model::default();
        add_record(&mut model, "devices/x");
        add_record(&mut model, "devices/y");
        add_trace(&mut model, "cpu");
        let picks = Picks {
            view_type: ViewType::Table,
            records: true,
        };
        let mut tree = TraceTree::build(model.traces.iter());
        tree.fold_records();
        let devices = &tree.children["devices"];
        assert!(devices.children["x"].children.is_empty());
        assert_eq!(devices.paths(picks), ["devices/x", "devices/y"]);
        assert!(tree.children["cpu"].paths(picks).is_empty());
        tree.retain_selectable(picks);
        assert_eq!(
            tree.children.keys().copied().collect::<Vec<_>>(),
            ["devices"]
        );

        // a wildcard picks the records added later, not their fields
        let (mut paths, mut wildcards) = (set(&["devices/x"]), set(&["/devices"]));
        add_record(&mut model, "devices/z");
        add_record(&mut model, "elsewhere/w");
        let selection = ViewSelection {
            paths: &mut paths,
            wildcards: &mut wildcards,
            picks,
        };
        expand_wildcards(&model, selection);
        assert_eq!(paths, set(&["devices/x", "devices/y", "devices/z"]));
    }

    #[test]
    fn records_are_checked_as_a_whole_in_tables_of_records() {
        use egui_kittest::kittest::{NodeT, Queryable};

        let mut model = Model::default();
        add_record(&mut model, "devices/x");
        add_trace(&mut model, "cpu");
        let picks = Picks {
            view_type: ViewType::Table,
            records: true,
        };
        let mut harness = pick_harness(model, picks);
        assert!(harness.query_by_label_contains("temp").is_none());
        assert!(
            harness
                .get_by_label_contains("cpu")
                .accesskit_node()
                .is_disabled()
        );
        harness.get_by_label_contains("x").click();
        harness.run();
        assert_eq!(harness.state().1, set(&["devices/x"]));
    }

    #[test]
    fn element_indices_are_row_major() {
        assert_eq!(element_indices(&[2, 3], 0), [0, 0]);
        assert_eq!(element_indices(&[2, 3], 4), [1, 1]);
        assert_eq!(element_indices(&[4], 3), [3]);
    }
}
