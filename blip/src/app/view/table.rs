//! A table of the values of traces at the time of the cursor, or of records with a
//! column per field.

use super::time_axis::index_at;
use super::trace_selection::{TraceTree, record_path, type_icon};
use super::{Formats, PropertiesContext, ViewContext, Viewable, short_names};
use crate::app::model::Model;
use crate::app::model::numbers::SHOWN_DIGITS;
use crate::app::model::traces::{Samples, Trace};
use crate::app::theme;
use egui_extras::Column;
use egui_phosphor::regular::{BRACKETS_CURLY, CARET_DOWN, CARET_RIGHT, DOTS_SIX_VERTICAL};
use std::collections::{BTreeMap, BTreeSet};

/// Arrays show at most this many elements per dimension in the table, and more in the
/// tooltip.
const MAX_SHOWN_ELEMENTS: usize = 8;
const MAX_TOOLTIP_ELEMENTS: usize = 64;
/// Indentation of a level of the tree in points.
const INDENT: f32 = 14.0;

/// How the traces of a table are listed.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum TableRows {
    /// As a tree of their paths.
    #[default]
    Tree,
    /// As a list of their paths.
    Paths,
    /// As a list of their names, see [`short_names`].
    Names,
}

/// What a row of a table is.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum TableLayout {
    /// A trace, with its value in a column.
    #[default]
    Traces,
    /// A record, with the value of each field in a column.
    Records,
}

/// The values of the selected traces or records at the cursor time.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Table {
    pub layout: TableLayout,
    /// Paths of the shown traces, or of the shown records in the records layout.
    #[serde(alias = "streams")] // the name before layout version 3
    pub traces: BTreeSet<String>,
    /// The collapsed branches, by the path segments leading to them joined with "/".
    pub collapsed: BTreeSet<String>,
    pub rows: TableRows,
    /// Whether traces without a sample at the cursor time get a row.
    pub show_empty: bool,
    /// The order of the field columns of the records layout, by the paths of the
    /// fields in their records, e.g. "acc.z". Fields not in it follow by name.
    pub column_order: Vec<String>,
    /// The field columns of the records layout that are not shown.
    pub hidden_columns: BTreeSet<String>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            layout: TableLayout::default(),
            traces: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            rows: TableRows::default(),
            show_empty: true,
            column_order: vec![],
            hidden_columns: BTreeSet::new(),
        }
    }
}

/// A row of the table: a branch of the tree, a trace, or both.
struct Row<'a> {
    depth: usize,
    label: &'a str,
    /// Identifies the branch, for collapsing it.
    key: String,
    trace: Option<&'a Trace>,
    is_branch: bool,
    is_record: bool,
}

/// The rows of the children of `node`, without those of collapsed branches.
fn rows<'a>(
    node: &TraceTree<'a>,
    prefix: &str,
    depth: usize,
    collapsed: &BTreeSet<String>,
    out: &mut Vec<Row<'a>>,
) {
    for (&segment, child) in &node.children {
        let key = format!("{prefix}/{segment}");
        out.push(Row {
            depth,
            label: segment.strip_prefix('.').unwrap_or(segment),
            key: key.clone(),
            trace: child.trace,
            is_branch: !child.children.is_empty(),
            is_record: child.is_record(),
        });
        if !collapsed.contains(&key) {
            rows(child, &key, depth + 1, collapsed, out);
        }
    }
}

/// A record of the records layout with its fields by their paths in it.
struct Record<'a> {
    path: &'a str,
    fields: BTreeMap<&'a str, &'a Trace>,
}

/// The records at `paths` in `model` with their fields, without those that are gone.
fn records<'a>(model: &'a Model, paths: &'a BTreeSet<String>) -> Vec<Record<'a>> {
    paths
        .iter()
        .map(|path| Record {
            path,
            fields: model
                .traces
                .field_paths(path)
                .filter_map(|field| model.traces.get(field))
                .map(|trace| (&trace.path[path.len() + 1..], trace))
                .collect(),
        })
        .filter(|record| !record.fields.is_empty())
        .collect()
}

/// The key of the display format of a column of the records layout in the formats of
/// the view, as it applies to the field in all records.
fn column_format_key(column: &str) -> String {
    format!("*.{column}")
}

/// A sample of a table: its trace, its index and its value written as text.
type Cell<'a> = (&'a Trace, usize, String);

/// `columns` with the one at `from` moved to `to`.
fn moved(columns: &[&str], from: usize, to: usize) -> Vec<String> {
    let mut order: Vec<String> = columns.iter().map(|&column| column.to_owned()).collect();
    let column = order.remove(from);
    order.insert(to, column);
    order
}

/// A column being dragged to another place in the column list of the properties.
struct ColumnDrag(usize);

/// Shows `value` of `trace` without wrapping, in monospace for an array so its
/// columns line up.
fn value_label(ui: &mut egui::Ui, trace: &Trace, value: &str) -> egui::Response {
    let text = match trace.samples {
        Samples::Arrays { .. } => egui::RichText::new(value).monospace(),
        _ => egui::RichText::new(value),
    };
    ui.add(egui::Label::new(text).extend())
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows the name of `row`, with a button to collapse or expand it for a branch.
    fn name_ui(&mut self, ui: &mut egui::Ui, row: &Row<'_>) {
        ui.add_space(row.depth as f32 * INDENT);
        if row.is_branch {
            let collapsed = self.collapsed.contains(&row.key);
            let caret = if collapsed { CARET_RIGHT } else { CARET_DOWN };
            if ui.add(egui::Button::new(caret).frame(false)).clicked() {
                if collapsed {
                    self.collapsed.remove(&row.key);
                } else {
                    self.collapsed.insert(row.key.clone());
                }
            }
        } else {
            ui.add_space(ui.spacing().icon_width);
        }
        let icon = match row.trace {
            Some(trace) => Some(type_icon(&trace.samples)),
            None => row.is_record.then_some(BRACKETS_CURLY),
        };
        let response = ui.label(row.label);
        if let Some(icon) = icon {
            ui.weak(icon);
        }
        if let Some(trace) = row.trace {
            response.on_hover_text(&trace.path);
        }
    }

    /// The rows of the shown traces of `traces` as listed in the properties, without
    /// those without a sample at `t` seconds unless empty traces are shown.
    fn table_rows<'a>(
        &self,
        traces: impl Iterator<Item = &'a Trace>,
        t: Option<f64>,
    ) -> Vec<Row<'a>> {
        let selected: Vec<&Trace> = traces
            .filter(|trace| self.traces.contains(&trace.path))
            .filter(|trace| {
                self.show_empty || t.and_then(|t| index_at(&trace.timestamps, t)).is_some()
            })
            .collect();
        let mut table_rows = vec![];
        let paths = selected.iter().map(|trace| trace.path.as_str());
        let labels: Vec<&str> = match self.rows {
            TableRows::Tree => {
                let tree = TraceTree::build(selected.iter().copied());
                rows(&tree, "", 0, &self.collapsed, &mut table_rows);
                vec![]
            }
            TableRows::Paths => paths.collect(),
            TableRows::Names => short_names(paths),
        };
        for (trace, label) in selected.iter().zip(labels) {
            table_rows.push(Row {
                depth: 0,
                label,
                key: trace.path.clone(),
                trace: Some(trace),
                is_branch: false,
                is_record: false,
            });
        }
        table_rows
    }

    /// The field columns of `records`: those in the column order first, then the others
    /// by name, the hidden ones too.
    fn columns<'a>(&self, records: &[Record<'a>]) -> Vec<&'a str> {
        let fields: BTreeSet<&str> = records
            .iter()
            .flat_map(|record| record.fields.keys().copied())
            .collect();
        let ordered = self
            .column_order
            .iter()
            .filter_map(|column| fields.get(column.as_str()).copied());
        let others = fields
            .iter()
            .copied()
            .filter(|field| !self.column_order.iter().any(|column| column == field));
        ordered.chain(others).collect()
    }

    /// Turns the shown fields into their records, or the shown records into their
    /// fields, for the table to show the same in the new `layout`. Other traces stay
    /// for when the layout changes back, a records table shows none.
    fn change_layout(&mut self, model: &Model, layout: TableLayout) {
        if layout == self.layout {
            return;
        }
        for trace in model.traces.iter() {
            let Some(record) = record_path(trace) else {
                continue;
            };
            match layout {
                TableLayout::Records if self.traces.remove(&trace.path) => {
                    self.traces.insert(record.to_owned());
                }
                TableLayout::Traces if self.traces.contains(record) => {
                    self.traces.insert(trace.path.clone());
                }
                _ => {}
            }
        }
        if layout == TableLayout::Traces {
            for record in model.traces.iter().filter_map(record_path) {
                self.traces.remove(record);
            }
        }
        self.layout = layout;
    }

    /// Shows a row per shown record with the value of each shown field at `t` seconds.
    fn show_records(&self, ui: &mut egui::Ui, model: &Model, formats: &Formats, t: Option<f64>) {
        let records = records(model, &self.traces);
        let columns: Vec<&str> = self
            .columns(&records)
            .into_iter()
            .filter(|column| !self.hidden_columns.contains(*column))
            .collect();
        let formats: Vec<_> = columns
            .iter()
            .map(|column| formats.of(&column_format_key(column)))
            .collect();
        // the `index`th sample of `trace` in the format of its column and with its unit
        let write = |trace: &Trace, column: usize, index: usize, max_len: usize| {
            let value = trace
                .samples
                .format(index, max_len, Some(SHOWN_DIGITS), formats[column]);
            match model.traces.unit(&trace.path) {
                Some(unit) if value != "null" => format!("{value} {unit}"),
                _ => value,
            }
        };
        // the sample of each field current at t, with its value written as text
        let cells: Vec<Vec<Option<Cell<'_>>>> = records
            .iter()
            .map(|record| {
                columns
                    .iter()
                    .enumerate()
                    .map(|(column, name)| {
                        let trace = record.fields.get(name)?;
                        let index = index_at(&trace.timestamps, t?)?;
                        Some((
                            *trace,
                            index,
                            write(trace, column, index, MAX_SHOWN_ELEMENTS),
                        ))
                    })
                    .collect()
            })
            .collect();
        let row_height = theme::table_row_height(ui, &egui::TextStyle::Body, 1);
        // arrays take a line per row
        let heights: Vec<f32> = cells
            .iter()
            .map(|row| {
                let lines = row
                    .iter()
                    .flatten()
                    .map(|(_, _, value)| value.lines().count())
                    .max()
                    .unwrap_or(1);
                if lines > 1 {
                    theme::table_row_height(ui, &egui::TextStyle::Monospace, lines)
                } else {
                    row_height
                }
            })
            .collect();
        let labels = short_names(records.iter().map(|record| record.path));

        egui::ScrollArea::horizontal().show(ui, |ui| {
            let lines = theme::TableLines::new(ui, true);
            let mut table = theme::table(ui).resizable(true);
            if columns.is_empty() {
                table = table.column(Column::remainder().at_least(80.0).clip(true));
            } else {
                table = table.column(Column::initial(140.0).at_least(60.0).clip(true));
                for _ in 1..columns.len() {
                    table = table.column(Column::initial(100.0).at_least(40.0).clip(true));
                }
                table = table.column(Column::remainder().at_least(60.0).clip(true));
            }
            table
                .header(row_height, |mut header| {
                    header.col(|ui| {
                        ui.strong("Record");
                    });
                    for column in &columns {
                        header.col(|ui| {
                            ui.strong(*column);
                        });
                    }
                })
                .body(|mut body| {
                    let spacing = body.ui_mut().spacing().item_spacing.x;
                    let lines = lines.in_body(body.ui_mut()).across(body.widths(), spacing);
                    body.heterogeneous_rows(heights.into_iter(), |mut table_row| {
                        let index = table_row.index();
                        table_row.col(|ui| {
                            lines.above(ui, index);
                            ui.add(egui::Label::new(labels[index]).truncate())
                                .on_hover_text(records[index].path);
                        });
                        for (column, cell) in cells[index].iter().enumerate() {
                            table_row.col(|ui| {
                                if let Some((trace, sample, value)) = cell {
                                    value_label(ui, trace, value).on_hover_ui(|ui| {
                                        let value =
                                            write(trace, column, *sample, MAX_TOOLTIP_ELEMENTS);
                                        value_label(ui, trace, &value);
                                    });
                                }
                            });
                        }
                    });
                });
        });
    }

    /// Shows the field columns of the records layout to show, hide and reorder them by
    /// drag and drop, with the display format of each in `formats`.
    pub fn columns_ui(&mut self, ui: &mut egui::Ui, model: &Model, formats: &mut Formats) {
        let records = records(model, &self.traces);
        let columns = self.columns(&records);
        if columns.is_empty() {
            return;
        }
        // a trace of each column, for its kind of numbers and its unit
        let samples: Vec<&Trace> = columns
            .iter()
            .map(|column| {
                records
                    .iter()
                    .find_map(|record| record.fields.get(column).copied())
                    .expect("each column is the field of a record")
            })
            .collect();
        let mut dropped = None;
        super::properties_section(ui, "Columns", |ui| {
            for (index, (column, trace)) in columns.iter().zip(&samples).enumerate() {
                let row = ui.horizontal(|ui| {
                    let handle = egui::Id::new(("table column", *column));
                    ui.dnd_drag_source(handle, ColumnDrag(index), |ui| {
                        ui.weak(DOTS_SIX_VERTICAL);
                    })
                    .response
                    .on_hover_cursor(egui::CursorIcon::Grab)
                    .on_hover_text("Drag to move the column");
                    let mut shown = !self.hidden_columns.contains(*column);
                    if ui.checkbox(&mut shown, *column).changed() {
                        if shown {
                            self.hidden_columns.remove(*column);
                        } else {
                            self.hidden_columns.insert((*column).to_owned());
                        }
                    }
                });
                let key = column_format_key(column);
                let kind = super::number_kind(model, &trace.path);
                super::number_format_ui(ui, formats, &key, kind, model.traces.unit(&trace.path));
                ui.end_row();
                // a line where the dragged column would go
                let row = row.response;
                if let Some(from) = row.dnd_hover_payload::<ColumnDrag>() {
                    let y = if from.0 < index {
                        row.rect.bottom() + 3.0
                    } else {
                        row.rect.top() - 3.0
                    };
                    let stroke = ui.visuals().selection.stroke;
                    ui.painter().hline(ui.max_rect().x_range(), y, stroke);
                }
                if let Some(from) = row.dnd_release_payload::<ColumnDrag>() {
                    dropped = Some((from.0, index));
                }
            }
        });
        if let Some((from, to)) = dropped {
            self.column_order = moved(&columns, from, to);
        }
    }

    fn show_table(&mut self, ui: &mut egui::Ui, model: &Model, formats: &Formats, t: Option<f64>) {
        let table_rows = self.table_rows(model.traces.iter(), t);
        // the `index`th sample of `trace` in its format and with its unit
        let write = |trace: &Trace, index: usize, max_len: usize, digits: Option<usize>| {
            let value = trace
                .samples
                .format(index, max_len, digits, formats.of(&trace.path));
            match model.traces.unit(&trace.path) {
                Some(unit) if value != "null" => format!("{value} {unit}"),
                _ => value,
            }
        };

        // the sample of each row current at t, with its value written as text
        let samples: Vec<Option<(&Trace, usize, String)>> = table_rows
            .iter()
            .map(|row| {
                let trace = row.trace?;
                let index = index_at(&trace.timestamps, t?)?;
                let value = write(trace, index, MAX_SHOWN_ELEMENTS, Some(SHOWN_DIGITS));
                Some((trace, index, value))
            })
            .collect();
        let row_height = theme::table_row_height(ui, &egui::TextStyle::Body, 1);
        // arrays take a line per row
        let heights: Vec<f32> = samples
            .iter()
            .map(|sample| match sample {
                Some((_, _, value)) if value.contains('\n') => {
                    let lines = value.lines().count();
                    theme::table_row_height(ui, &egui::TextStyle::Monospace, lines)
                }
                _ => row_height,
            })
            .collect();

        let lines = theme::TableLines::new(ui, true);
        theme::table(ui)
            .resizable(true)
            .column(Column::initial(200.0).at_least(80.0).clip(true))
            .column(Column::remainder().at_least(60.0).clip(true))
            .header(row_height, |mut header| {
                for title in ["Trace", "Value"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|mut body| {
                let lines = lines.in_body(body.ui_mut());
                body.heterogeneous_rows(heights.into_iter(), |mut table_row| {
                    let index = table_row.index();
                    let row = &table_rows[index];
                    let sample = &samples[index];
                    table_row.col(|ui| {
                        lines.above(ui, index);
                        self.name_ui(ui, row);
                    });
                    table_row.col(|ui| {
                        if let Some((trace, index, value)) = sample {
                            value_label(ui, trace, value).on_hover_ui(|ui| {
                                let value =
                                    write(trace, *index, MAX_TOOLTIP_ELEMENTS, Some(SHOWN_DIGITS));
                                value_label(ui, trace, &value);
                            });
                        }
                    });
                });
            });
    }
}

impl Viewable for Table {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.traces.is_empty() {
            ui.weak(match self.layout {
                TableLayout::Traces => "Select the traces to show in the traces panel",
                TableLayout::Records => "Select the records to show in the traces panel",
            });
            return;
        }
        let (model, formats, t) = (context.model, context.formats, context.time.cursor());
        // the table changes the look of the ui
        ui.scope(|ui| match self.layout {
            TableLayout::Traces => self.show_table(ui, model, formats, t),
            TableLayout::Records => self.show_records(ui, model, formats, t),
        });
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        super::properties_section(ui, "Rows", |ui| {
            let mut layout = self.layout;
            super::choice_row(
                ui,
                "Layout",
                &mut layout,
                &[
                    (TableLayout::Traces, "Traces"),
                    (TableLayout::Records, "Records"),
                ],
            );
            self.change_layout(context.model, layout);
            if self.layout == TableLayout::Traces {
                super::choice_row(
                    ui,
                    "Traces",
                    &mut self.rows,
                    &[
                        (TableRows::Tree, "Tree"),
                        (TableRows::Paths, "Paths"),
                        (TableRows::Names, "Names"),
                    ],
                );
                super::property_label(ui, "Empty");
                ui.checkbox(&mut self.show_empty, "")
                    .on_hover_text("Show traces without a value at the cursor time");
                ui.end_row();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_branches_hide_their_rows() {
        let traces = [
            Trace::new("motor.speed".to_owned(), Some(5)),
            Trace::new("motor.state".to_owned(), Some(5)),
            Trace::new("sensor/temperature".to_owned(), None),
        ];
        let tree = TraceTree::build(traces.iter());
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
            model.traces.append(&proto::Value {
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
        model.traces.append(&proto::Value {
            timestamp: 0,
            path: "dev/a".to_owned(),
            value: proto::Data::Record(vec![
                ("temp".to_owned(), proto::Data::Uint(1)),
                ("name".to_owned(), proto::Data::String("x".to_owned())),
            ]),
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
        let mut traces = crate::app::model::traces::Traces::new();
        for (seconds, path) in [(1, "cpu"), (1, "left/speed"), (3, "right/speed")] {
            traces.append(&proto::Value {
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
        assert_eq!(labels(&table, Some(2.0)), ["left/speed", "right/speed"]);
        table.rows = TableRows::Names;
        assert_eq!(labels(&table, Some(2.0)), ["left/speed", "right/speed"]);
        table.show_empty = false;
        assert_eq!(labels(&table, Some(2.0)), ["speed"]);
    }
}
