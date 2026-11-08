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
        model.traces.append(&proto::Value {
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
        serde_cbor_2::to_vec(&proto::Message::Value(proto::Value {
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
