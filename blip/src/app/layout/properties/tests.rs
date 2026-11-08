use super::*;
use crate::app::model::Model;
use crate::app::model::numbers::DisplayFormat;
use crate::app::view::ViewKind;
use crate::app::view::plot::Interpolation;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A harness showing the properties panel of a layout with a plot and a log view,
/// with what `selected` picks from its tab and the views in it selected.
fn harness(selected: impl FnOnce(TileId, &[TileId]) -> TileId) -> Harness<'static, Layout> {
    harness_with(Model::default(), selected)
}

/// A harness as by [`harness`] showing the traces of `model`.
fn harness_with(
    model: Model,
    selected: impl FnOnce(TileId, &[TileId]) -> TileId,
) -> Harness<'static, Layout> {
    let mut layout = Layout::with_plot_and_logs();
    let tab = layout.container(layout.root()).unwrap().children_vec()[0];
    let views = layout.container(tab).unwrap().children_vec();
    layout.select(selected(tab, &views));
    Harness::builder()
        .with_size(egui::vec2(300.0, 800.0))
        .build_ui_state(
            move |ui, layout: &mut Layout| {
                let connected = Default::default();
                let context = PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &crate::app::view::NO_PLOTS,
                };
                layout.properties_ui(ui, &context);
            },
            layout,
        )
}

#[test]
fn properties_of_the_selected_view_are_changed() {
    let mut harness = harness(|_, views| views[0]);
    harness.run();
    harness.get_by_label("Step").click();
    harness.run();
    let layout = harness.state();
    let view = layout.selected.and_then(|id| layout.view(id)).unwrap();
    let ViewKind::Plot(plot) = &view.kind else {
        panic!("the plot is not selected");
    };
    assert_eq!(plot.interpolation, Interpolation::Step);
}

/// The plot selected in `harness`.
fn selected_plot(harness: &Harness<'_, Layout>) -> crate::app::view::plot::Plot {
    let layout = harness.state();
    match &layout.selected.and_then(|id| layout.view(id)).unwrap().kind {
        ViewKind::Plot(plot) => plot.clone(),
        _ => panic!("the plot is not selected"),
    }
}

#[test]
fn stats_are_added_and_removed_in_the_properties() {
    use crate::app::view::stats::Stat;

    let mut harness = harness(|_, views| views[0]);
    harness.run();
    // without traces there is nothing to measure
    assert!(harness.query_by_label_contains("Add stat").is_none());
    let Some(ViewKind::Plot(plot)) = harness.state_mut().selected_view_mut().map(|v| &mut v.kind)
    else {
        panic!("the plot is not selected");
    };
    plot.traces.extend(["a".to_owned(), "b".to_owned()]);
    harness.run();
    let first = "a";
    harness.get_by_label_contains("Add stat").click();
    harness.run();
    harness.get_by_label_contains("Add stat").click();
    harness.run();
    let stats = selected_plot(&harness).stats;
    let added: Vec<_> = stats.iter().map(|m| (m.trace.as_str(), m.stat)).collect();
    assert_eq!(added, [(first, Stat::Mean), (first, Stat::Rms)]);
    harness
        .get_all_by_label(egui_phosphor::regular::MINUS)
        .next()
        .unwrap()
        .click();
    harness.run();
    let stats = selected_plot(&harness).stats;
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].stat, Stat::Rms);
}

#[test]
fn number_fields_keep_partial_numbers_and_are_auto_when_empty() {
    let mut harness = harness(|_, views| views[0]);
    harness.run();
    // the fields are the name, the y axis min, max and label
    fn min<'a>(harness: &'a Harness<'_, Layout>) -> egui_kittest::Node<'a> {
        let mut fields = harness.get_all_by_role(egui::accesskit::Role::TextInput);
        fields.nth(1).unwrap()
    }
    min(&harness).focus();
    min(&harness).type_text("-");
    harness.run();
    assert_eq!(selected_plot(&harness).y_min, None);
    min(&harness).type_text("2");
    harness.run();
    assert_eq!(selected_plot(&harness).y_min, Some(-2.0));
    for _ in 0..2 {
        harness.key_press(egui::Key::Backspace);
        harness.run();
    }
    assert_eq!(selected_plot(&harness).y_min, None);
}

#[test]
fn numbers_take_a_format_and_other_traces_do_not() {
    let mut model = Model::default();
    for (path, value) in [
        ("motor/errors", proto::Data::Uint(3)),
        ("motor/state", proto::Data::from("on")),
    ] {
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: path.to_owned(),
            value,
        });
    }
    model.traces.set_unit("motor/errors", "count");
    let mut harness = harness_with(model, |_, views| views[0]);
    let view = harness.state_mut().selected_view_mut().unwrap();
    let selection = view.selection_mut().unwrap().paths;
    selection.insert("motor/errors".to_owned());
    selection.insert("motor/state".to_owned());
    harness.run();
    harness.get_by_label("count");
    // a format for the number, a disabled one for the string
    let format = |disabled: bool| {
        move |node: &egui_kittest::kittest::AccessKitNode<'_>| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some("Default")
                && node.is_disabled() == disabled
        }
    };
    assert_eq!(harness.get_all_by(format(true)).count(), 1);
    harness.get_by(format(false)).click();
    harness.run();
    harness.get_by_label("Hex").click();
    harness.run();
    let view = harness.state_mut().selected_view_mut().unwrap();
    assert_eq!(view.formats.of("motor/errors"), DisplayFormat::Hex);
}

#[test]
fn floats_are_not_offered_hex_or_binary() {
    let mut model = Model::default();
    model.traces.append(&proto::Value {
        timestamp: 0,
        path: "motor/speed".to_owned(),
        value: proto::Data::Float(1.5),
    });
    let mut harness = harness_with(model, |_, views| views[0]);
    let view = harness.state_mut().selected_view_mut().unwrap();
    view.selection_mut()
        .unwrap()
        .paths
        .insert("motor/speed".to_owned());
    harness.run();
    harness
        .get_by(|node| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some("Default")
        })
        .click();
    harness.run();
    harness.get_by_label("Scientific");
    assert!(harness.query_by_label("Hex").is_none());
    assert!(harness.query_by_label("Binary").is_none());
}

#[test]
fn groups_are_renamed_while_typing_but_not_to_blank_names() {
    let mut harness = harness(|tab, _| tab);
    harness.run();
    let tab = harness.state().selected.unwrap();
    let name = harness.state().name(tab).to_owned();
    let field = harness.get_by_role(egui::accesskit::Role::TextInput);
    field.focus();
    field.type_text(" x");
    harness.run();
    assert_eq!(harness.state().name(tab), format!("{name} x"));
    for _ in 0..name.len() + 2 {
        harness.key_press(egui::Key::Backspace);
        harness.run();
    }
    assert_eq!(harness.state().name(tab), &name[..1]);
}
