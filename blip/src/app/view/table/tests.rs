use super::*;

#[test]
fn collapsed_branches_hide_their_rows() {
    let traces = [
        Trace::leaked("motor.speed", Some(5), Samples::Empty(0)),
        Trace::leaked("motor.state", Some(5), Samples::Empty(0)),
        Trace::leaked("sensor/temperature", None, Samples::Empty(0)),
    ];
    let tree = TraceTree::build(traces.iter().copied());
    let labels = |collapsed: &BTreeSet<String>| {
        let mut out = vec![];
        rows(&tree, "", 0, collapsed, &mut out);
        out.iter()
            .map(|row| format!("{}{}", " ".repeat(row.depth), row.label))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(&BTreeSet::new()),
        ["motor", " speed", " state", "sensor", " temperature"]
    );
    let collapsed = BTreeSet::from(["/motor".to_owned()]);
    assert_eq!(labels(&collapsed), ["motor", "sensor", " temperature"]);
}

#[test]
fn records_have_a_column_per_field_in_the_chosen_order() {
    let mut model = Model::default();
    for (path, fields) in [("dev/a", ["temp", "volt"]), ("dev/b", ["temp", "fw"])] {
        model.traces.append(&proto::Sample {
            timestamp: 0,
            path: path.to_owned(),
            value: proto::Data::Record(
                fields
                    .iter()
                    .map(|field| (field.to_string(), proto::Data::Uint(1)))
                    .collect(),
            ),
        });
    }
    let mut table = Table {
        layout: TableLayout::Records,
        traces: BTreeSet::from(["dev/a".to_owned(), "dev/b".to_owned(), "gone".to_owned()]),
        ..Table::default()
    };
    let paths: Vec<&str> = records(&model, &table.traces)
        .iter()
        .map(|record| record.path)
        .collect();
    assert_eq!(paths, ["dev/a", "dev/b"]);
    let columns = |table: &Table| -> Vec<String> {
        let records = records(&model, &table.traces);
        table
            .columns(&records)
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(columns(&table), ["fw", "temp", "volt"]);
    // columns missing from the order, like new fields, follow it
    table.column_order = vec!["volt".to_owned(), "gone".to_owned(), "fw".to_owned()];
    assert_eq!(columns(&table), ["volt", "fw", "temp"]);

    // the layout turns records into their fields and back
    // the layout turns records into their fields and back, other traces stay
    table.traces.remove("gone");
    table.traces.insert("cpu".to_owned());
    table.change_layout(&model, TableLayout::Traces);
    assert_eq!(
        table.traces,
        ["cpu", "dev/a.temp", "dev/a.volt", "dev/b.fw", "dev/b.temp"]
            .map(str::to_owned)
            .into()
    );
    table.traces.remove("dev/b.fw");
    table.traces.remove("dev/b.temp");
    table.change_layout(&model, TableLayout::Records);
    assert_eq!(table.traces, ["cpu", "dev/a"].map(str::to_owned).into());
}

#[test]
fn columns_move_before_or_after_the_one_dropped_on() {
    let columns = ["a", "b", "c", "d"];
    assert_eq!(moved(&columns, 0, 2), ["b", "c", "a", "d"]);
    assert_eq!(moved(&columns, 3, 1), ["a", "d", "b", "c"]);
}

#[test]
fn columns_are_hidden_and_formatted_in_the_properties() {
    use egui_kittest::kittest::Queryable;

    let mut model = Model::default();
    model.traces.append(&proto::Sample {
        timestamp: 0,
        path: "dev/a".to_owned(),
        value: proto::Data::Record(vec![
            ("temp".to_owned(), proto::Data::Uint(1)),
            ("name".to_owned(), proto::Data::String("x".to_owned())),
        ]),
    });
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "dev/a.temp".to_owned(),
        range: Some(proto::ExpectedRange {
            min: -10.0,
            max: 85.0,
        }),
        ..Default::default()
    });
    let table = Table {
        layout: TableLayout::Records,
        traces: BTreeSet::from(["dev/a".to_owned()]),
        ..Table::default()
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            move |ui, (table, formats): &mut (Table, Formats)| {
                table.columns_ui(ui, &model, formats);
            },
            (table, Formats::default()),
        );
    harness.run();
    // the range the target declares shows under the column
    harness.get_by_label("Expected -10 to 85");
    harness.get_by_label("temp").click();
    harness.run();
    assert_eq!(harness.state().0.hidden_columns, ["temp".to_owned()].into());
    // the string has no number format
    harness
        .get_by(|node| node.role() == egui::accesskit::Role::ComboBox && !node.is_disabled())
        .click();
    harness.run();
    harness.get_by_label("Hex").click();
    harness.run();
    assert_eq!(
        harness.state().1.of("*.temp"),
        crate::app::model::numbers::DisplayFormat::Hex
    );
}

#[test]
fn traces_are_listed_by_path_or_name_without_empty_ones() {
    let mut traces = crate::app::model::traces::Traces::default();
    for (seconds, path) in [(1, "cpu"), (1, "left/speed"), (3, "right/speed")] {
        traces.append(&proto::Sample {
            timestamp: seconds * 1_000_000_000,
            path: path.to_owned(),
            value: proto::Data::Float(1.0),
        });
    }
    let mut table = Table {
        traces: BTreeSet::from(["left/speed".to_owned(), "right/speed".to_owned()]),
        rows: TableRows::Paths,
        ..Table::default()
    };
    let labels = |table: &Table, t| {
        table
            .table_rows(traces.iter(), t)
            .iter()
            .map(|row| row.label.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(&table, Some(2_000_000_000)),
        ["left/speed", "right/speed"]
    );
    table.rows = TableRows::Names;
    assert_eq!(
        labels(&table, Some(2_000_000_000)),
        ["left/speed", "right/speed"]
    );
    table.show_empty = false;
    assert_eq!(labels(&table, Some(2_000_000_000)), ["speed"]);
}

#[test]
fn labelled_cells_write_the_label_until_the_default_is_chosen() {
    use crate::app::model::numbers::DisplayFormat;
    use egui_kittest::kittest::Queryable;

    let mut model = Model::default();
    model.traces.append(&proto::Sample {
        timestamp: 0,
        path: "m".to_owned(),
        value: proto::Data::Uint(0),
    });
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "m".into(),
        labels: [(0, "Idle".to_owned())].into(),
        ..Default::default()
    });
    let table = Table {
        traces: BTreeSet::from(["m".to_owned()]),
        ..Table::default()
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(300.0, 200.0))
        .build_ui_state(
            move |ui, (table, formats): &mut (Table, Formats)| {
                table.show_table(ui, &model, formats, Some(0));
            },
            (table, Formats::default()),
        );
    harness.run();
    harness.get_by_label("Idle");
    harness
        .state_mut()
        .1
        .choose("m", DisplayFormat::Default, true);
    harness.run();
    harness.get_by_label("0");
    assert!(harness.query_by_label("Idle").is_none());
}
