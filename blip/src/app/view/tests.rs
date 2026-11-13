use super::*;
use egui_kittest::Harness;

#[test]
fn the_number_formats_of_a_linked_histogram_are_greyed() {
    let plots = Plots(vec![PlotInfo {
        id: ViewId(7),
        name: "Errors".to_owned(),
        y_range: None,
    }]);
    let mut view = View::new(ViewType::Distribution, "Spread".to_owned());
    if let ViewKind::Distribution(distribution) = &mut view.kind {
        distribution.traces.insert("motor/errors".to_owned());
    }
    let format_disabled = |view: View, plots: Plots| {
        use egui_kittest::kittest::{NodeT, Queryable};
        let mut model = Model::default();
        model.traces.append(&proto::Sample {
            timestamp: 0,
            path: "motor/errors".to_owned(),
            value: proto::Data::Uint(3),
        });
        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 600.0))
            .build_ui_state(
                move |ui, view: &mut View| {
                    let connected = HashSet::new();
                    let context = PropertiesContext {
                        model: &model,
                        connected: &connected,
                        plots: &plots,
                    };
                    view.properties_ui(ui, &context);
                },
                view,
            );
        harness.run();
        let format = harness.get_by(|node| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some("Default")
        });
        format.accesskit_node().is_disabled()
    };
    assert!(!format_disabled(view.clone(), plots.clone()));
    if let ViewKind::Distribution(distribution) = &mut view.kind {
        distribution.link = Some(ViewId(7));
    }
    assert!(format_disabled(view.clone(), plots));
    // a link to a plot no longer in the layout leaves them to the distribution
    assert!(!format_disabled(view, Plots::default()));
}

#[test]
fn wildcards_pick_new_paths_once_the_model_has_them() {
    let frame = |path: &str| {
        serde_cbor_2::to_vec(&proto::Message::Sample(proto::Sample {
            timestamp: 0,
            path: path.to_owned(),
            value: proto::Data::Uint(1),
        }))
        .unwrap()
    };
    let mut model = Model::default();
    model.receive("a", &frame("motor/a"));
    let mut view = View::new(ViewType::Plot, "plot".to_owned());
    view.wildcards.insert("/motor".to_owned());
    let paths = |view: &mut View| view.selection_mut().unwrap().paths.clone();
    view.expand_wildcards(&model);
    assert_eq!(paths(&mut view), ["motor/a".to_owned()].into());

    // without new paths in the model, the wildcards are not expanded again
    view.selection_mut().unwrap().paths.clear();
    model.receive("a", &frame("motor/a"));
    view.expand_wildcards(&model);
    assert!(paths(&mut view).is_empty());

    model.receive("a", &frame("motor/b"));
    model.receive("a", &frame("cpu"));
    view.expand_wildcards(&model);
    assert_eq!(
        paths(&mut view),
        ["motor/a".to_owned(), "motor/b".to_owned()].into()
    );
}

/// A harness showing a plot of a line that can be zoomed by the mouse wheel, as the
/// XY and heatmap views, in a ui that is `enabled` or not. Its state is the shown
/// bounds.
fn plot_harness(enabled: bool) -> Harness<'static, Option<egui_plot::PlotBounds>> {
    Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            move |ui, bounds: &mut Option<egui_plot::PlotBounds>| {
                ui.add_enabled_ui(enabled, |ui| {
                    let plot = egui_plot::Plot::new("plot").show(ui, |plot_ui| {
                        let points: Vec<[f64; 2]> =
                            (0..=10).map(|i| [f64::from(i), f64::from(i)]).collect();
                        plot_ui.line(egui_plot::Line::new("", points));
                    });
                    zoom_on_wheel(&plot.response);
                    *bounds = Some(*plot.transform.bounds());
                });
            },
            None,
        )
}

fn widths(bounds: &egui_plot::PlotBounds) -> [f64; 2] {
    [bounds.width(), bounds.height()]
}

#[test]
fn the_wheel_zooms_a_plot_along_both_axes_without_panning_it() {
    let mut harness = plot_harness(true);
    harness.run();
    let before = harness.state().unwrap();

    // scrolling up zooms in around the pointer
    scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
    let zoomed = harness.state().unwrap();
    let ([width, height], [width_before, height_before]) = (widths(&zoomed), widths(&before));
    assert!(width < width_before * 0.9, "{width} {width_before}");
    assert!(height < height_before * 0.9, "{height} {height_before}");
    // zoomed in around a point inside, not moved along y
    assert!(zoomed.min()[1] > before.min()[1] && zoomed.max()[1] < before.max()[1]);
}

#[test]
fn the_wheel_does_not_zoom_a_disabled_plot() {
    let mut harness = plot_harness(false);
    harness.run();
    let before = harness.state().unwrap();
    scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
    assert_eq!(widths(&harness.state().unwrap()), widths(&before));
}

#[test]
fn short_names_are_the_last_level() {
    assert_eq!(
        short_names(["motor/speed", "imu.acc[2]", "cpu", "/a/b.c.d"]),
        ["speed", "acc[2]", "cpu", "d"]
    );
}

#[test]
fn short_names_keep_levels_to_tell_paths_apart() {
    assert_eq!(
        short_names(["left/motor/speed", "right/motor/speed", "speed", "torque"]),
        ["left/motor/speed", "right/motor/speed", "speed", "torque"]
    );
    assert_eq!(short_names(["a/x", "b/x", "c/y"]), ["a/x", "b/x", "y"]);
}

#[test]
fn sticky_width_grows_until_the_slot_changes() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("w");
    assert_eq!(sticky_width(&ctx, id, "a".to_owned(), 10.0), 10.0);
    assert_eq!(sticky_width(&ctx, id, "a".to_owned(), 5.0), 10.0);
    assert_eq!(sticky_width(&ctx, id, "a".to_owned(), 12.0), 12.0);
    assert_eq!(sticky_width(&ctx, id, "b".to_owned(), 3.0), 3.0);
}

#[test]
fn range_moved_ignores_rounding() {
    use super::plotting::range_moved;
    assert!(!range_moved((0.0, 10.0), (0.0, 10.0)));
    assert!(!range_moved((0.0, 10.0), (5e-6, 10.0)));
    assert!(range_moved((0.0, 10.0), (2e-5, 10.0)));
    assert!(range_moved((0.0, 10.0), (0.0, 9.0)));
    assert!(!range_moved((3.0, 3.0), (3.0, 3.0)));
}

#[test]
fn the_index_under_the_pointer_is_the_nearest_element() {
    use super::plotting::index_under;
    assert_eq!(index_under(2.4, 4), Some(2));
    assert_eq!(index_under(2.6, 4), Some(3));
    assert_eq!(index_under(-0.4, 4), Some(0));
    assert_eq!(index_under(-0.6, 4), None);
    assert_eq!(index_under(3.5, 4), None);
    assert_eq!(index_under(0.0, 0), None);
}

#[test]
fn indexed_names_follow_the_order_of_the_selection() {
    let paths: BTreeSet<String> = ["b/x", "a/x", "c/y"].map(str::to_owned).into();
    let named: Vec<_> = indexed_names(&paths).collect();
    assert_eq!(
        named,
        [("a/x", "a/x", 0), ("b/x", "b/x", 1), ("c/y", "y", 2)]
    );
}

#[test]
fn set_member_inserts_and_removes() {
    let mut set: BTreeSet<String> = ["a".to_owned()].into();
    let members = |set: &BTreeSet<String>| set.iter().cloned().collect::<Vec<_>>();
    set_member(&mut set, "b", true);
    assert_eq!(members(&set), ["a", "b"]);
    set_member(&mut set, "b", true);
    assert_eq!(members(&set), ["a", "b"]);
    set_member(&mut set, "a", false);
    assert_eq!(members(&set), ["b"]);
    set_member(&mut set, "z", false);
    assert_eq!(members(&set), ["b"]);
}

#[test]
fn per_path_keeps_only_values_off_the_default() {
    let mut formats = Formats::default();
    formats.set("a", DisplayFormat::Hex);
    formats.set("b", DisplayFormat::Default);
    assert_eq!(formats.of("a"), DisplayFormat::Hex);
    assert_eq!(formats.of("b"), DisplayFormat::Default);
    assert_eq!(formats.of("z"), DisplayFormat::Default);
    formats.retain_shown(&["b".to_owned()].into());
    assert_eq!(formats.of("a"), DisplayFormat::Default);
    assert!(formats.is_empty());
}

#[test]
fn per_path_writes_the_bytes_of_the_old_newtypes() {
    use super::plot::{LineStyle, LineStyles};
    #[derive(serde::Serialize)]
    struct OldFormats(BTreeMap<String, DisplayFormat>);
    #[derive(serde::Serialize)]
    struct OldLineStyles(BTreeMap<String, LineStyle>);

    let mut formats = Formats::default();
    formats.set("a", DisplayFormat::Hex);
    let old = OldFormats([("a".to_owned(), DisplayFormat::Hex)].into());
    assert_eq!(serde_json::to_string(&formats).unwrap(), r#"{"a":"Hex"}"#);
    assert_eq!(
        serde_json::to_string(&formats).unwrap(),
        serde_json::to_string(&old).unwrap()
    );
    assert_eq!(
        serde_cbor_2::to_vec(&formats).unwrap(),
        serde_cbor_2::to_vec(&old).unwrap()
    );

    let mut styles = LineStyles::default();
    styles.set("a", LineStyle::Dashed);
    styles.set("b", LineStyle::Solid);
    let old = OldLineStyles([("a".to_owned(), LineStyle::Dashed)].into());
    assert_eq!(serde_json::to_string(&styles).unwrap(), r#"{"a":"Dashed"}"#);
    assert_eq!(
        serde_cbor_2::to_vec(&styles).unwrap(),
        serde_cbor_2::to_vec(&old).unwrap()
    );

    let read: Formats = serde_json::from_str(r#"{"a":"Hex","c":"Binary"}"#).unwrap();
    assert_eq!(read.of("a"), DisplayFormat::Hex);
    assert_eq!(read.of("c"), DisplayFormat::Binary);
}

#[test]
fn the_nearest_point_is_within_the_radius() {
    let transform = egui_plot::PlotTransform::new(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
        egui_plot::PlotBounds::from_min_max([0.0, 0.0], [100.0, 100.0]),
        egui::Vec2b::FALSE,
    );
    let points = [[10.0, 10.0], [50.0, 50.0], [f64::NAN, 90.0]];
    let near = |pointer: egui::Pos2| plotting::nearest_point(&points, pointer, &transform, 5.0);
    assert_eq!(near(egui::pos2(12.0, 88.0)), Some((0, 8.0)));
    assert_eq!(near(egui::pos2(52.0, 52.0)), Some((1, 8.0)));
    assert_eq!(near(egui::pos2(13.0, 94.0)), Some((0, 25.0)));
    assert_eq!(near(egui::pos2(30.0, 70.0)), None);
    let gap = [[f64::NAN, 10.0]];
    let pointer = egui::pos2(10.0, 90.0);
    assert_eq!(
        plotting::nearest_point(&gap, pointer, &transform, 5.0),
        None
    );
}

#[test]
fn labels_are_the_default_of_labelled_traces() {
    let mut formats = Formats::default();
    assert_eq!(formats.format_of("a", true), DisplayFormat::Labels);
    assert_eq!(formats.format_of("a", false), DisplayFormat::Default);
    formats.choose("a", DisplayFormat::Default, true);
    assert_eq!(
        serde_json::to_string(&formats).unwrap(),
        r#"{"a":"Default"}"#
    );
    assert_eq!(formats.format_of("a", true), DisplayFormat::Default);
    formats.choose("a", DisplayFormat::Labels, true);
    assert_eq!(serde_json::to_string(&formats).unwrap(), "{}");
    formats.choose("b", DisplayFormat::Hex, false);
    assert_eq!(serde_json::to_string(&formats).unwrap(), r#"{"b":"Hex"}"#);
}

#[test]
fn the_facts_of_a_trace_are_written_in_one_line_each() {
    assert_eq!(expected_text((0.0, 100.0)), "Expected 0 to 100");
    assert_eq!(expected_text((-2.5, 10.0)), "Expected -2.5 to 10");
    let labels: Labels = [(-1, "fault"), (0, "off"), (1, "on")]
        .into_iter()
        .map(|(number, label)| (number, label.to_owned()))
        .collect();
    assert_eq!(
        labels_text(&labels),
        (
            "-1 fault, 0 off, 1 on".to_owned(),
            "-1 fault\n0 off\n1 on".to_owned()
        )
    );
}

/// A model with the traces "a" (a float with a unit, a description and a range) and "m"
/// (integers with labels), and "p" without any facts.
fn facts_model(described: bool) -> Model {
    let mut model = Model::default();
    for (path, value) in [
        ("a", proto::Data::Float(1.0)),
        ("m", proto::Data::Uint(1)),
        ("p", proto::Data::Float(2.0)),
    ] {
        model.traces.append(&proto::Sample {
            timestamp: 0,
            path: path.to_owned(),
            value,
        });
    }
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "a".to_owned(),
        unit: "rpm".to_owned(),
        description: "Speed".to_owned(),
        range: Some(proto::ExpectedRange {
            min: 0.0,
            max: 3000.0,
        }),
        ..Default::default()
    });
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "m".to_owned(),
        description: if described { "Mode" } else { "" }.to_owned(),
        labels: [(-1, "fault"), (0, "off"), (1, "on")]
            .into_iter()
            .map(|(number, label)| (number, label.to_owned()))
            .collect(),
        ..Default::default()
    });
    model
}

#[test]
fn the_traces_section_lists_the_facts() {
    use egui_kittest::kittest::Queryable;

    let model = facts_model(true);
    let mut view = View::new(ViewType::Plot, "Motor".to_owned());
    if let ViewKind::Plot(plot) = &mut view.kind {
        plot.traces.extend(["a", "m", "p"].map(str::to_owned));
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 600.0))
        .build_ui_state(
            move |ui, view: &mut View| {
                let connected = HashSet::new();
                let plots = Plots::default();
                let context = PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &plots,
                };
                view.properties_ui(ui, &context);
            },
            view,
        );
    harness.run();
    for label in [
        "TRACES",
        "Speed",
        "Expected 0 to 3000",
        "-1 fault, 0 off, 1 on",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    assert!(harness.query_by_label("NUMBER FORMAT").is_none());
    // "p" has no facts and adds no row
    assert_eq!(harness.query_all_by_label_contains("Expected ").count(), 1);
}

#[test]
fn the_hover_of_a_trace_has_its_path_type_and_description() {
    let model = facts_model(true);
    let trace = model.traces.get("m").expect("the trace");
    assert_eq!(trace_hover(&model, trace), "m\nint\nMode");
    let model = facts_model(false);
    let trace = model.traces.get("m").expect("the trace");
    assert_eq!(trace_hover(&model, trace), "m\nint");
}

#[test]
fn fit_to_and_formats_survive_json() {
    use super::plot::Plot;
    use super::xy::Xy;
    let plot = Plot {
        fit: Fit::Values,
        ..Plot::default()
    };
    let mut view = View::new(ViewType::Plot, "P".to_owned());
    view.kind = ViewKind::Plot(plot);
    view.formats.choose("m", DisplayFormat::Labels, false);
    view.formats.choose("a", DisplayFormat::Default, true);
    let read: View = serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
    assert!(matches!(&read.kind, ViewKind::Plot(plot) if plot.fit == Fit::Values));
    assert_eq!(
        serde_json::to_string(&read.formats).unwrap(),
        serde_json::to_string(&view.formats).unwrap()
    );
    assert_eq!(read.formats.format_of("m", false), DisplayFormat::Labels);
    assert_eq!(read.formats.format_of("a", true), DisplayFormat::Default);

    let xy = Xy {
        fit: Fit::Values,
        ..Xy::default()
    };
    let json = serde_json::to_string(&xy).unwrap();
    assert_eq!(serde_json::from_str::<Xy>(&json).unwrap().fit, Fit::Values);

    let old = r#"{"name": "P", "kind": {"Plot": {"traces": ["a"]}}, "formats": {"a": "Hex"}}"#;
    let old: View = serde_json::from_str(old).unwrap();
    assert!(matches!(&old.kind, ViewKind::Plot(plot) if plot.fit == Fit::Expected));
    assert_eq!(old.formats.format_of("a", false), DisplayFormat::Hex);
}
