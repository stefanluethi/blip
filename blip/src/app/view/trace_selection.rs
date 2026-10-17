use crate::app::model::Model;
use crate::app::model::traces::{Samples, Trace, element_path};
use crate::app::view::{Selectable, ViewType};
use egui::{RichText, Ui};
use egui_phosphor::regular::{
    BRACKETS_CURLY, BRACKETS_SQUARE, HASH, LIGHTNING, MAGNIFYING_GLASS, TEXT_AA, X,
};
use std::collections::{BTreeMap, BTreeSet};

/// Arrays with up to this many elements list each element for selection.
pub const MAX_LISTED_ELEMENTS: usize = 16;

/// The icon marking an action in the tree, in place of the type of a trace.
pub const ACTION_ICON: &str = LIGHTNING;

/// Trace and action paths split into a tree at each "/", and trace paths also at each
/// "." between the fields of a record.
#[derive(Default)]
pub struct TraceTree<'a> {
    /// The trace if one ends at this node.
    pub trace: Option<&'a Trace>,
    /// The path of the action if one ends at this node.
    pub action: Option<&'a str>,
    /// Children by path segment, record fields with their leading ".", e.g. ".acc".
    pub children: BTreeMap<&'a str, TraceTree<'a>>,
}

impl<'a> TraceTree<'a> {
    pub fn build(traces: impl Iterator<Item = &'a Trace>) -> Self {
        let mut root = Self::default();
        for trace in traces {
            let path = trace.path.as_str();
            let (name, fields) = path.split_at(trace.fields_start.unwrap_or(path.len()));
            let field_starts = fields.match_indices('.').map(|(start, _)| start);
            let field_ends = field_starts.clone().skip(1).chain([fields.len()]);
            let field_segments = field_starts
                .zip(field_ends)
                .map(|(start, end)| &fields[start..end]);
            root.node_mut(segments(name).chain(field_segments)).trace = Some(trace);
        }
        root
    }

    /// Adds the actions at `paths`, which branch at each "/" only.
    pub fn add_actions(&mut self, paths: impl Iterator<Item = &'a str>) {
        for path in paths {
            self.node_mut(segments(path)).action = Some(path);
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

    pub fn is_record(&self) -> bool {
        self.children.keys().any(|segment| segment.starts_with('.'))
    }

    /// The paths of all traces at and below this node that a view of `view_type` can
    /// show, and of all actions if it shows actions.
    fn paths(&self, view_type: ViewType) -> Vec<String> {
        self.trace
            .into_iter()
            .flat_map(|trace| trace_paths(trace, view_type))
            .chain(self.action.and_then(|path| action_path(path, view_type)))
            .chain(
                self.children
                    .values()
                    .flat_map(|child| child.paths(view_type)),
            )
            .collect()
    }
}

/// The segments of `path` between its "/".
fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|segment| !segment.is_empty())
}

/// The icon of the type of `samples`.
pub fn type_icon(samples: &Samples) -> &'static str {
    match samples {
        Samples::Empty | Samples::Numbers { .. } => HASH,
        Samples::Strings(_) => TEXT_AA,
        Samples::Arrays { .. } => BRACKETS_SQUARE,
    }
}

/// The traces selected for a view.
struct Selection<'a> {
    paths: &'a mut BTreeSet<String>,
    view_type: ViewType,
    /// Whether only the traces matching a search are shown.
    searching: bool,
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
        if selected && selection.view_type.shows_one_trace() {
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

/// The paths of `trace` that a view of `view_type` can show: its own, those of its
/// elements, or none.
pub fn trace_paths(trace: &Trace, view_type: ViewType) -> Vec<String> {
    match (view_type.selectable(trace), &trace.samples) {
        (Selectable::Elements, Samples::Arrays { shape, .. }) => (0..shape.iter().product())
            .map(|flat_index| element_path(&trace.path, &element_indices(shape, flat_index)))
            .collect(),
        (Selectable::No(_), _) => vec![],
        _ => vec![trace.path.clone()],
    }
}

/// Shows `trace` labelled `label` with the icon of its type: a checkbox, or a
/// collapsible list of checkboxes for its elements.
fn show_trace(ui: &mut Ui, label: &str, trace: &Trace, selection: &mut Selection<'_>) {
    let icon = Some(type_icon(&trace.samples));
    let paths = trace_paths(trace, selection.view_type);
    let mut hover = format!("{}\n{}", trace.path, trace.samples.type_label());
    match selection.view_type.selectable(trace) {
        Selectable::Trace => {}
        Selectable::No(reason) => hover.push_str(&format!("\n{reason}")),
        Selectable::Elements => {
            let id = ui.make_persistent_id(("trace_array", &trace.path));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    show_checkbox(ui, label, icon, &paths, selection).on_hover_text(hover);
                })
                .body(|ui| {
                    for path in &paths {
                        let label = &path[trace.path.len()..];
                        leaf_row(ui, |ui| {
                            show_checkbox(ui, label, None, std::slice::from_ref(path), selection)
                                .on_hover_text(path);
                        });
                    }
                });
            return;
        }
    }
    leaf_row(ui, |ui| {
        show_checkbox(ui, label, icon, &paths, selection)
            .on_hover_text(&hover)
            .on_disabled_hover_text(hover);
    });
}

/// The path of the action at `path` if a view of `view_type` can show it.
fn action_path(path: &str, view_type: ViewType) -> Option<String> {
    view_type.shows_actions().then(|| path.to_owned())
}

/// Shows the action at `path` labelled `label` with the action icon: a checkbox,
/// disabled unless the view shows actions.
fn show_action(ui: &mut Ui, label: &str, path: &str, selection: &mut Selection<'_>) {
    let paths = Vec::from_iter(action_path(path, selection.view_type));
    let mut hover = format!("{path}\nAction");
    if paths.is_empty() {
        hover.push_str("\nOnly actions views show actions");
    }
    leaf_row(ui, |ui| {
        show_checkbox(ui, label, Some(ACTION_ICON), &paths, selection)
            .on_hover_text(&hover)
            .on_disabled_hover_text(hover);
    });
}

/// Shows the trace and the action ending at `node`, if any, labelled `label`.
fn show_leaves(ui: &mut Ui, label: &str, node: &TraceTree<'_>, selection: &mut Selection<'_>) {
    if let Some(trace) = node.trace {
        show_trace(ui, label, trace, selection);
    }
    if let Some(path) = node.action {
        show_action(ui, label, path, selection);
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

/// Shows the children of `node` as collapsible branches, traces and actions.
fn show_trace_tree(ui: &mut Ui, node: &TraceTree<'_>, prefix: &str, selection: &mut Selection<'_>) {
    for (segment, child) in &node.children {
        let child_prefix = format!("{prefix}/{segment}");
        let label = segment.strip_prefix('.').unwrap_or(segment);
        if child.children.is_empty() {
            show_leaves(ui, label, child, selection);
        } else {
            let icon = child.is_record().then_some(BRACKETS_CURLY);
            // while searching, branches keep a separate open state, open by default
            let id = ui.make_persistent_id(("trace_tree", &child_prefix, selection.searching));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    if selection.view_type.shows_one_trace() {
                        // a branch cannot be selected as one trace
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
                    // a trace or an action can also be the prefix of other paths
                    show_leaves(ui, label, child, selection);
                    show_trace_tree(ui, child, &child_prefix, selection);
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
/// it.
pub fn search_ui(ui: &mut Ui, search: &mut String) {
    ui.horizontal(|ui| {
        // the clear button is a square as high as the field: the text, its margins and
        // its stroke, and room is left for it
        let side = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
        let clear_width = if search.is_empty() {
            0.0
        } else {
            side + ui.spacing().item_spacing.x
        };
        // the id stays the same as the clear button comes and goes, to keep the focus
        let field = ui
            .add(
                egui::TextEdit::singleline(search)
                    .id_salt("trace_search")
                    .hint_text(format!("{MAGNIFYING_GLASS} Search paths"))
                    .desired_width(ui.available_width() - clear_width),
            )
            .on_hover_text("Show only the traces and actions whose path contains each word");
        ui.spacing_mut().button_padding.x = 0.0;
        if !search.is_empty()
            && ui
                .add(egui::Button::new(X).min_size(egui::Vec2::splat(field.rect.height())))
                .on_hover_text("Clear the search")
                .clicked()
        {
            search.clear();
        }
    });
}

/// Shows the traces and the announced actions of `model` whose path matches `search`
/// as a tree with checkboxes for the traces or actions selected for a view of the given
/// type, disabled if there is no selection to edit.
pub fn show(
    ui: &mut Ui,
    model: &Model,
    search: &str,
    selection: Option<(&mut BTreeSet<String>, ViewType)>,
) {
    let mut tree = TraceTree::build(
        model
            .traces
            .iter()
            .filter(|trace| matches(&trace.path, search)),
    );
    tree.add_actions(model.actions.paths().filter(|path| matches(path, search)));
    if tree.children.is_empty() {
        if model.traces.iter().next().is_none() && model.actions.is_empty() {
            ui.weak("No traces or actions received");
        } else {
            ui.weak("No traces or actions match the search");
        }
        return;
    }
    let mut unselectable = BTreeSet::new();
    let enabled = selection.is_some();
    let (paths, view_type) = selection.unwrap_or((&mut unselectable, ViewType::Plot));
    let searching = !search.trim().is_empty();
    let mut selection = Selection {
        paths,
        view_type,
        searching,
    };
    ui.add_enabled_ui(enabled, |ui| {
        show_trace_tree(ui, &tree, "", &mut selection);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_tree_branches_at_slashes() {
        let traces: Vec<Trace> = ["acc/x", "acc/y", "/cpu", "acc"]
            .into_iter()
            .map(|path| Trace::new(path.to_owned(), None))
            .collect();
        let tree = TraceTree::build(traces.iter());
        assert_eq!(
            tree.children.keys().copied().collect::<Vec<_>>(),
            ["acc", "cpu"]
        );
        let acc = &tree.children["acc"];
        assert_eq!(acc.trace.map(|trace| trace.path.as_str()), Some("acc"));
        assert_eq!(acc.children.keys().copied().collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(
            acc.children["x"].trace.map(|trace| trace.path.as_str()),
            Some("acc/x")
        );
        assert_eq!(
            tree.children["cpu"].trace.map(|trace| trace.path.as_str()),
            Some("/cpu")
        );
    }

    #[test]
    fn trace_tree_branches_at_record_fields() {
        let traces = [
            Trace::new("sensors/imu.acc.z".to_owned(), Some(11)),
            Trace::new("sensors/imu.ok".to_owned(), Some(11)),
            Trace::new("sensors/v1.2".to_owned(), None),
        ];
        let tree = TraceTree::build(traces.iter());
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
                .map(|trace| trace.path.as_str()),
            Some("sensors/imu.acc.z")
        );
    }

    #[test]
    fn trace_tree_paths_list_array_elements() {
        let array = |path: &str, len| {
            let mut trace = Trace::new(path.to_owned(), Some(3));
            trace.samples = Samples::Arrays {
                shape: vec![len],
                elements: Default::default(),
            };
            trace
        };
        let traces = [
            array("imu.acc", 2),
            array("imu.big", MAX_LISTED_ELEMENTS + 1),
            Trace::new("imu.ok".to_owned(), Some(3)),
        ];
        let tree = TraceTree::build(traces.iter());
        assert_eq!(
            tree.children["imu"].paths(ViewType::Plot),
            ["imu.acc[0]", "imu.acc[1]", "imu.ok"]
        );
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
                    search_ui(ui, search);
                    show(ui, &model, search, Some((paths, ViewType::Plot)));
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
        let traces = [Trace::new("motor/speed".to_owned(), None)];
        let mut tree = TraceTree::build(traces.iter());
        tree.add_actions(["motor/speed", "motor/stop", "reboot"].into_iter());
        let motor = &tree.children["motor"];
        let speed = &motor.children["speed"];
        assert_eq!(
            speed.trace.map(|trace| trace.path.as_str()),
            Some("motor/speed")
        );
        assert_eq!(speed.action, Some("motor/speed"));
        assert_eq!(motor.children["stop"].action, Some("motor/stop"));
        assert_eq!(tree.children["reboot"].action, Some("reboot"));
        // only actions views show actions, and no traces
        assert_eq!(
            motor.paths(ViewType::Actions),
            ["motor/speed", "motor/stop"]
        );
        assert_eq!(motor.paths(ViewType::Table), ["motor/speed"]);
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
                    search_ui(ui, search);
                    show(ui, &model, search, Some((paths, view_type)));
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
    fn the_clear_button_is_as_high_as_the_search_field_and_fits_beside_it() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let width = 250.0;
        // the size of egui and that of blip
        for size in [12.5, 14.0] {
            let mut harness = Harness::builder()
                .with_size(egui::vec2(width, 100.0))
                .build_ui_state(
                    move |ui, search: &mut String| {
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Body, egui::FontId::proportional(size));
                        search_ui(ui, search);
                    },
                    "motor".to_owned(),
                );
            harness.run();
            let field = harness.get_by_role(egui::accesskit::Role::TextInput).rect();
            let clear = harness.get_by_label(X).rect();
            assert_eq!(field.height(), clear.height());
            assert_eq!(field.top(), clear.top());
            assert_eq!(clear.width(), clear.height());
            assert!(field.right() < clear.left() && clear.right() <= width);
        }
    }

    #[test]
    fn element_indices_are_row_major() {
        assert_eq!(element_indices(&[2, 3], 0), [0, 0]);
        assert_eq!(element_indices(&[2, 3], 4), [1, 1]);
        assert_eq!(element_indices(&[4], 3), [3]);
    }
}
