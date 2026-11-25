use super::*;
use crate::store::MemoryStore;
use crate::store::testing::append;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// What is picked for a view of `view_type` as `paths`, without wildcards.
fn selection(paths: &mut BTreeSet<String>, view_type: ViewType) -> ViewSelection<'_> {
    // a wildcard is never set by these tests, so leaking an empty set is fine
    ViewSelection {
        paths,
        wildcards: Box::leak(Box::default()),
        picks: view_type.into(),
    }
}

/// The filter of the tree with `search`, showing only what the view can show if
/// `compatible_only`.
fn filter(search: &str, compatible_only: bool) -> TreeFilter<'_> {
    TreeFilter {
        search,
        compatible_only,
        checked_only: false,
    }
}

/// The names of the children of `tree`.
fn keys<'a>(tree: &TraceTree<'a>) -> Vec<&'a str> {
    tree.children.keys().copied().collect()
}

#[test]
fn wildcard_folders_are_marked_and_tell_their_state() {
    let mut model = MemoryStore::default();
    add_trace(&mut model, "motor/rpm");
    add_trace(&mut model, "imu/x");
    let mut harness = pick_harness(model, ViewType::Plot.into());
    assert!(harness.query_by_label(WILDCARD_MARK).is_none());
    harness.get_by_label("motor").hover();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("Check to also show traces added later")
            .is_some()
    );
    harness.get_by_label("motor").click();
    harness.run();
    assert_eq!(harness.state().2, set(&["/motor"]));
    // only the wildcard folder is marked
    assert_eq!(harness.query_all_by_label(WILDCARD_MARK).count(), 1);
    harness.get_by_label("imu").hover();
    harness.run();
    harness.get_by_label("motor").hover();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("Also shows traces added here later")
            .is_some()
    );
}

#[test]
fn branches_and_wildcards_below_others_are_known() {
    let mut paths = BTreeSet::new();
    let mut wildcards = set(&["/motor"]);
    let selection = Selection {
        paths: &mut paths,
        apart: &BTreeSet::new(),
        wildcards: &mut wildcards,
        picks: ViewType::Plot.into(),
        searching: false,
        colored: &[],
        editable: true,
    };
    assert!(selection.is_wildcard("/motor"));
    assert!(selection.is_wildcard("/motor/left"));
    assert!(!selection.is_wildcard("/motors"));
    assert_eq!(branch_path("/imu/.acc"), "imu.acc");
}

#[test]
fn one_trace_views_pick_with_radio_buttons() {
    let mut model = MemoryStore::default();
    add_array(&mut model, "a", 3);
    add_array(&mut model, "b", 3);
    let mut harness = pick_harness(model, ViewType::Heatmap.into());
    let radios = || egui::accesskit::Role::RadioButton;
    assert_eq!(harness.query_all_by_role(radios()).count(), 2);
    assert_eq!(
        harness
            .query_all_by_role(egui::accesskit::Role::CheckBox)
            .count(),
        0
    );
    harness.get_by_label("a").click();
    harness.run();
    harness.get_by_label("b").click();
    harness.run();
    assert_eq!(harness.state().1, set(&["b"]));
}

#[test]
fn without_a_view_the_folders_still_open_and_close() {
    let mut model = MemoryStore::default();
    add_trace(&mut model, "motor/rpm");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui(move |ui| show(ui, &model, filter("", false), None, &[], true));
    harness.run();
    assert!(
        harness
            .get_by_label_contains("rpm")
            .accesskit_node()
            .is_disabled()
    );
    let caret = harness.get_by_label("Collapse");
    assert!(!caret.accesskit_node().is_disabled());
    caret.click();
    harness.run();
    assert!(harness.query_by_label_contains("rpm").is_none());
}

#[test]
fn alt_click_and_the_menu_open_and_close_all_below() {
    let mut model = MemoryStore::default();
    add_trace(&mut model, "a/b/c");
    add_trace(&mut model, "a/d/e");
    let mut harness = pick_harness(model, ViewType::Plot.into());
    let shown = |harness: &egui_kittest::Harness<'_, PickState>| {
        ["b", "c", "d", "e"]
            .into_iter()
            .filter(|label| harness.query_by_label(label).is_some())
            .count()
    };
    assert_eq!(shown(&harness), 4);
    harness.get_by_label(DOTS_THREE_VERTICAL).click();
    harness.run();
    harness.get_by_label("Collapse all").click();
    harness.run_steps(2);
    assert_eq!(shown(&harness), 0);
    // opening the top branch alone leaves the ones below closed
    harness.get_by_label("Expand").click();
    harness.run_steps(2);
    assert_eq!(shown(&harness), 2);
    // with Alt, it closes and opens all below it
    harness.get_by_label("Collapse").click();
    harness.run_steps(2);
    harness
        .get_by_label("Expand")
        .click_modifiers(egui::Modifiers::ALT);
    harness.run_steps(2);
    assert_eq!(shown(&harness), 4);
}

#[test]
fn checked_only_shows_the_checked_and_hidden_ones_are_counted() {
    let mut model = MemoryStore::default();
    add_trace(&mut model, "motor/rpm");
    add_trace(&mut model, "motor/temp");
    add_array(&mut model, "arr", 3);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(
            move |ui, (search, checked_only, paths): &mut (String, bool, BTreeSet<String>)| {
                search_ui(ui, search, &mut false, checked_only, true);
                let filter = TreeFilter {
                    search,
                    compatible_only: false,
                    checked_only: *checked_only,
                };
                show(
                    ui,
                    &model,
                    filter,
                    Some(selection(paths, ViewType::Plot)),
                    &[],
                    true,
                );
            },
            (String::new(), false, set(&["motor/rpm", "arr[1]"])),
        );
    harness.run();
    assert!(harness.query_by_label_contains("temp").is_some());
    harness.get_by_label(CHECK_SQUARE).click();
    harness.run_steps(2);
    assert!(harness.state().1);
    assert!(harness.query_by_label_contains("temp").is_none());
    assert!(harness.query_by_label_contains("rpm").is_some());
    assert!(harness.query_by_label_contains("arr").is_some());
    // the search hides a checked trace, which is counted
    harness.state_mut().0 = "rpm".to_owned();
    harness.run_steps(2);
    assert!(
        harness
            .query_by_label("1 checked trace is hidden by the search or the filter")
            .is_some()
    );
    assert!(has_trace_or_element(&set(&["arr[1]"]), "arr"));
    assert!(!has_trace_or_element(&set(&["arr2"]), "arr"));
}

#[test]
fn ctrl_f_focuses_the_search() {
    let mut harness = tree_harness(ViewType::Plot);
    request_search_focus(&harness.ctx);
    harness.run_steps(2);
    let field = harness.get_by_role(egui::accesskit::Role::TextInput);
    assert!(field.is_focused());
}

#[test]
fn the_arrow_keys_move_the_focus_and_open_and_close_folders() {
    let mut model = MemoryStore::default();
    add_trace(&mut model, "motor/rpm");
    add_trace(&mut model, "motor/temp");
    let mut harness = pick_harness(model, ViewType::Plot.into());
    harness.get_by_label_contains("motor").focus();
    harness.run_steps(2);
    harness.key_press(egui::Key::ArrowLeft);
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("rpm").is_none());
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("rpm").is_some());
    // Space checks the focused row
    harness.key_press(egui::Key::Space);
    harness.run_steps(2);
    assert_eq!(harness.state().1, set(&["motor/rpm", "motor/temp"]));
}

#[test]
fn trace_tree_branches_at_slashes() {
    let traces: Vec<Trace<'_>> = ["acc/x", "acc/y", "/cpu", "acc"]
        .into_iter()
        .map(|path| Trace::leaked(path, None, Samples::Empty(0)))
        .collect();
    let tree = TraceTree::build(traces.iter().copied());
    assert_eq!(keys(&tree), ["acc", "cpu"]);
    let acc = &tree.children["acc"];
    assert_eq!(acc.trace.map(|trace| trace.path), Some("acc"));
    assert_eq!(keys(acc), ["x", "y"]);
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
    assert_eq!(keys(sensors), ["imu", "v1.2"]);
    let imu = &sensors.children["imu"];
    assert!(imu.is_record());
    assert!(!sensors.is_record());
    assert_eq!(keys(imu), [".acc", ".ok"]);
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
    assert_eq!(keys(&tree), ["imu", "motor"]);
    assert_eq!(keys(&tree.children["imu"]), ["acc"]);
    assert_eq!(keys(&tree.children["motor"]), ["rpm"]);

    let mut tree = TraceTree::build(traces.iter().copied());
    tree.add_actions(["motor/rpm", "reboot"].into_iter());
    tree.retain_selectable(ViewType::Actions.into());
    assert_eq!(keys(&tree), ["motor", "reboot"]);
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
    let mut model = MemoryStore::default();
    for path in ["motor/speed", "motor/current", "sensor/temp"] {
        append(&mut model.traces, 0, path, proto::Data::Uint(1));
    }
    let mut harness = tree_harness_of(model, ViewType::Plot);
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

#[test]
fn logs_join_the_traces_in_the_tree() {
    let traces = [Trace::leaked("motor/speed", None, Samples::Empty(0))];
    let mut tree = TraceTree::build(traces.iter().copied());
    tree.add_logs(["log", "motor"].into_iter());
    let motor = &tree.children["motor"];
    assert_eq!(motor.log, Some("motor"));
    assert_eq!(tree.children["log"].log, Some("log"));
    // only log views show logs, and no traces
    assert_eq!(tree.paths(ViewType::Logs.into()), ["log", "motor"]);
    assert_eq!(motor.paths(ViewType::Table.into()), ["motor/speed"]);
    tree.retain_selectable(ViewType::Logs.into());
    assert!(tree.children["motor"].children.is_empty());
}

/// A harness showing the tree of the trace "motor/rpm" and the action
/// "motor/speed" for a view of `view_type`, with the paths selected for it.
/// Measures how long showing the tree of 2000 traces in 20 branches takes per
/// frame.
#[test]
#[ignore = "a benchmark, run with `cargo test --release -p blip tree_frame -- --ignored --nocapture`"]
fn tree_frame_time() {
    let mut model = MemoryStore::default();
    for branch in 0..20 {
        for leaf in 0..100 {
            let path = format!("node{branch}/sensor{leaf}");
            append(&mut model.traces, 0, &path, proto::Data::Float(1.0));
        }
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 800.0))
        .build_ui_state(
            move |ui, paths: &mut BTreeSet<String>| {
                let start = std::time::Instant::now();
                show(
                    ui,
                    &model,
                    filter("", true),
                    Some(selection(paths, ViewType::Plot)),
                    &[],
                    true,
                );
                ui.data_mut(|data| {
                    *data.get_temp_mut_or_default::<f64>(egui::Id::new("spent")) +=
                        start.elapsed().as_secs_f64();
                });
            },
            BTreeSet::new(),
        );
    harness.run();
    harness
        .ctx
        .data_mut(|data| data.insert_temp(egui::Id::new("spent"), 0.0f64));
    let frames = 50;
    for _ in 0..frames {
        harness.step();
    }
    let spent: f64 = harness
        .ctx
        .data(|data| data.get_temp(egui::Id::new("spent")))
        .unwrap();
    println!("{:.3} ms per frame", spent / f64::from(frames) * 1.0e3);
}

/// Adds the action "motor/speed" to `model`.
fn add_action(model: &mut MemoryStore) {
    model.actions.set(
        "a",
        vec![proto::ActionDescriptor {
            path: "motor/speed".to_owned(),
            description: String::new(),
            params: vec![],
            readback: None,
            confirm: false,
            enabled: true,
            disabled_reason: None,
        }],
    );
}

/// A harness showing the search field and the tree of `model` for a view of
/// `view_type`.
fn tree_harness_of(
    model: MemoryStore,
    view_type: ViewType,
) -> Harness<'static, (String, BTreeSet<String>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(
            move |ui, (search, paths): &mut (String, BTreeSet<String>)| {
                search_ui(ui, search, &mut false, &mut false, true);
                show(
                    ui,
                    &model,
                    filter(search, false),
                    Some(selection(paths, view_type)),
                    &[],
                    true,
                );
            },
            (String::new(), BTreeSet::new()),
        );
    harness.run();
    harness
}

/// A tree with the described trace "motor/rpm" and the action "motor/speed".
fn tree_harness(view_type: ViewType) -> Harness<'static, (String, BTreeSet<String>)> {
    let mut model = MemoryStore::default();
    append(&mut model.traces, 0, "motor/rpm", proto::Data::Uint(1));
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "motor/rpm".to_owned(),
        description: "Speed of the shaft".to_owned(),
        ..Default::default()
    });
    add_action(&mut model);
    tree_harness_of(model, view_type)
}

#[test]
fn actions_views_check_the_actions_in_the_tree() {
    let mut harness = tree_harness(ViewType::Actions);
    assert!(
        harness
            .get_by_label_contains("rpm")
            .accesskit_node()
            .is_disabled()
    );
    // the action is marked by its icon at the end of its row
    assert!(harness.query_by_label(ACTION_ICON).is_some());
    harness.get_by_label_contains("speed").click();
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
    let mut model = MemoryStore::default();
    append(&mut model.traces, 0, "motor/rpm", proto::Data::Uint(1));
    append(
        &mut model.traces,
        0,
        "motor/mode",
        proto::Data::String("idle".to_owned()),
    );
    add_action(&mut model);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(
            move |ui, (search, compatible, view_type): &mut (String, bool, Option<ViewType>)| {
                search_ui(ui, search, compatible, &mut false, view_type.is_some());
                let mut paths = BTreeSet::new();
                let selection = view_type.map(|view_type| selection(&mut paths, view_type));
                show(
                    ui,
                    &model,
                    filter(search, *compatible),
                    selection,
                    &[],
                    true,
                );
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
            .query_by_label("The selected view can show none of the traces, actions and logs")
            .is_some()
    );
    harness.state_mut().0 = "rpm".to_owned();
    harness.run();
    assert!(
        harness
            .query_by_label("Nothing the selected view can show matches the search")
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
                    search_ui(ui, search, &mut false, &mut false, true);
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
fn add_record(model: &mut MemoryStore, path: &str) {
    append(
        &mut model.traces,
        0,
        path,
        proto::Data::Record(vec![
            ("temp".to_owned(), proto::Data::Float(1.0)),
            ("volt".to_owned(), proto::Data::Uint(3)),
        ]),
    );
}

fn add_trace(model: &mut MemoryStore, path: &str) {
    append(&mut model.traces, 0, path, proto::Data::Uint(1));
}

/// Clicks the label and runs the frames after it.
fn click(harness: &mut Harness<'_, PickState>, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

type PickState = (MemoryStore, BTreeSet<String>, BTreeSet<String>, String);

/// A harness showing the search field and the tree of `model` for a view picking
/// `picks`, with the picked paths and the wildcards, which are expanded in each
/// frame.
fn pick_harness(model: MemoryStore, picks: Picks) -> Harness<'static, PickState> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(
            move |ui, (model, paths, wildcards, search): &mut PickState| {
                search_ui(ui, search, &mut false, &mut false, true);
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
                show(ui, model, filter(search, false), Some(selection), &[], true);
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
    let mut model = MemoryStore::default();
    for path in ["motor/a", "motor/b", "cpu"] {
        add_trace(&mut model, path);
    }
    let mut harness = pick_harness(model, ViewType::Plot.into());
    click(&mut harness, "motor");
    assert_eq!(harness.state().1, set(&["motor/a", "motor/b"]));
    assert_eq!(harness.state().2, set(&["/motor"]));
    add_trace(&mut harness.state_mut().0, "motor/c");
    add_trace(&mut harness.state_mut().0, "other/d");
    harness.run();
    assert_eq!(harness.state().1, set(&["motor/a", "motor/b", "motor/c"]));

    // unchecking a trace below the wildcard ends the wildcard
    click(&mut harness, "a");
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

/// Adds a log record at `path` to `model`.
fn add_log(model: &mut MemoryStore, path: &str) {
    let record = proto::Message::Log(proto::LogRecord {
        timestamp: 1,
        path: path.to_owned(),
        level: 2,
        message: "hi".to_owned(),
    });
    crate::store::testing::receive(model, "", &record);
}

#[test]
fn new_log_views_check_the_logs_of_all_paths_also_later_ones() {
    let mut model = MemoryStore::default();
    add_trace(&mut model, "motor/rpm");
    add_log(&mut model, "motor");
    let mut harness = pick_harness(model, ViewType::Logs.into());
    let view = crate::widgets::view::View::new(ViewType::Logs, String::new());
    harness.state_mut().2 = view.wildcards;
    harness.run();
    assert_eq!(harness.state().1, set(&["motor"]));
    add_log(&mut harness.state_mut().0, "robot/arm");
    // without a path, of the target as a whole
    add_log(&mut harness.state_mut().0, "");
    harness.run();
    assert_eq!(
        harness.state().1,
        set(&["log/default", "motor", "robot/arm"])
    );

    // unchecking the logs of a path ends the wildcard
    harness.get_by_label("arm").click();
    harness.run();
    assert_eq!(harness.state().1, set(&["log/default", "motor"]));
    assert!(harness.state().2.is_empty());
}

#[test]
fn checking_the_log_folder_picks_the_log_paths_below_it_also_later_ones() {
    let mut model = MemoryStore::default();
    for path in ["", "log/imu", "motor"] {
        add_log(&mut model, path);
    }
    let mut harness = pick_harness(model, ViewType::Logs.into());
    harness.get_by_label("log").click();
    harness.run();
    assert_eq!(harness.state().1, set(&["log/default", "log/imu"]));
    assert_eq!(harness.state().2, set(&["/log"]));
    add_log(&mut harness.state_mut().0, "log/motor/left");
    add_log(&mut harness.state_mut().0, "other");
    harness.run();
    assert_eq!(
        harness.state().1,
        set(&["log/default", "log/imu", "log/motor/left"])
    );
}

#[test]
fn shown_logs_of_paths_without_records_can_be_unchecked() {
    let mut model = MemoryStore::default();
    add_log(&mut model, "motor");
    let mut harness = pick_harness(model, ViewType::Logs.into());
    harness.state_mut().1 = set(&["motor", "robot/arm"]);
    harness.run();
    harness.get_by_label("arm").click();
    harness.run();
    assert_eq!(harness.state().1, set(&["motor"]));
}

/// Adds an array of `len` floats at `path` to `model`.
fn add_array(model: &mut MemoryStore, path: &str, len: usize) {
    append(
        &mut model.traces,
        0,
        path,
        proto::Data::Array(vec![proto::Data::Float(1.0); len]),
    );
}

#[test]
fn xy_views_check_arrays_as_a_whole_apart_from_other_traces() {
    let mut model = MemoryStore::default();
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
    click(&mut harness, "rpm");
    assert_eq!(harness.state().1, set(&["sensors/rpm"]));
    click(&mut harness, "adc");
    assert_eq!(harness.state().1, set(&["sensors/adc"]));
    click(&mut harness, "wave");
    assert_eq!(harness.state().1, set(&["sensors/adc", "sensors/wave"]));

    // checking other traces unchecks the arrays, and the other way round; a small
    // array checks its elements, as in other views
    click(&mut harness, "pos");
    assert_eq!(
        harness.state().1,
        set(&["sensors/pos[0]", "sensors/pos[1]"])
    );
    click(&mut harness, "adc");
    click(&mut harness, "sensors");
    assert!(harness.state().1.contains("sensors/pos[1]"));
    assert!(!harness.state().1.contains("sensors/adc"));
    assert_eq!(harness.state().2, set(&["/sensors"]));
    click(&mut harness, "adc");
    assert_eq!(harness.state().1, set(&["sensors/adc"]));
    assert!(harness.state().2.is_empty());
}

#[test]
fn tables_of_records_pick_records_and_folders_of_records() {
    let mut model = MemoryStore::default();
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
    assert_eq!(keys(&tree), ["devices"]);

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
    let mut model = MemoryStore::default();
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

#[test]
fn the_tree_row_of_a_described_trace_has_its_description_on_hover() {
    let mut harness = tree_harness(ViewType::Plot);
    harness.get_by_label(HASH).hover();
    harness.run();
    assert!(harness.query_by_label_contains("motor/rpm").is_some());
    assert!(
        harness
            .query_by_label_contains("Speed of the shaft")
            .is_some()
    );
}
