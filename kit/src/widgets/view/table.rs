//! A table of the values of traces at the time of the cursor, or of records with a
//! column per field.

use super::Selectable;
use super::trace_selection::{TraceTree, record_path, type_icon};
use super::{Formats, PropertiesContext, ViewContext, Viewable, short_names};
use crate::store::actions::{ActionId, Args, Invoker};
use crate::store::numbers::{Labels, SHOWN_DIGITS};
use crate::store::sorted::index_at;
use crate::store::traces::{Samples, Trace};
use crate::store::{Store, Window};
use crate::widgets::theme;
use egui_extras::Column;
use egui_phosphor::regular::{BRACKETS_CURLY, DOTS_SIX_VERTICAL};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Arrays show at most this many elements per dimension in the table, and more in the
/// tooltip.
const MAX_SHOWN_ELEMENTS: usize = 8;
const MAX_TOOLTIP_ELEMENTS: usize = 64;
/// Indentation of a level of the tree in points.
const INDENT: f32 = 14.0;

named_enum! {
    /// How the traces of a table are listed.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum TableRows {
        /// As a tree of their paths.
        #[default]
        Tree => "Tree",
        /// As a list of their paths.
        Paths => "Paths",
        /// As a list of their names, see `short_names`.
        Names => "Names",
    }
}

named_enum! {
    /// What a row of a table is.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum TableLayout {
        /// A trace, with its value in a column.
        #[default]
        Traces => "Traces",
        /// A record, with the value of each field in a column.
        Records => "Records",
    }
}

/// The values of the selected traces or records at the cursor time.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Table {
    pub layout: TableLayout,
    /// Paths of the shown traces, or of the shown records in the records layout.
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
    /// The columns the application adds after the value in the traces layout, see
    /// [`Self::column`]. They are code, so they are not saved.
    #[serde(skip)]
    custom: Vec<TableColumn>,
}

/// Shows the cell of a column of the application.
type CellUi = dyn Fn(&mut egui::Ui, &mut TableCell<'_>);

/// A column an application adds to the traces layout of a [`Table`], e.g. with a
/// button for each trace.
#[derive(Clone)]
struct TableColumn {
    title: String,
    ui: Rc<CellUi>,
}

/// What the cell of a column of the application shows: the trace of its row, with
/// the store to read from and the invoker to invoke actions with.
pub struct TableCell<'a> {
    /// The trace of the row, with its sample current at the shown time if it has one.
    pub trace: Trace<'a>,
    /// The index of the sample of [`Self::trace`] current at the shown time.
    pub sample: Option<usize>,
    pub store: &'a dyn Store,
    pub invoker: &'a mut Invoker,
    /// The time in seconds, as egui counts it.
    pub now: f64,
}

impl TableCell<'_> {
    /// Invokes the action at `action` of the source named `source` with `args`, e.g.
    /// when a button of the cell was clicked.
    pub fn invoke(&mut self, source: &str, action: &str, args: Args) {
        let action = ActionId::new(source, action);
        self.invoker.invoke(&action, None, args, self.now);
    }
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
            custom: vec![],
        }
    }
}

/// A row of the table: a branch of the tree, a trace, or both.
struct Row<'a> {
    depth: usize,
    label: &'a str,
    /// Identifies the branch, for collapsing it.
    key: String,
    trace: Option<Trace<'a>>,
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
    fields: BTreeMap<&'a str, Trace<'a>>,
}

/// The records at `paths` in `store` with their fields, without those that are gone.
fn records<'a>(
    store: &'a dyn Store,
    paths: &'a BTreeSet<String>,
    window: Window,
) -> Vec<Record<'a>> {
    paths
        .iter()
        .map(|path| Record {
            path,
            fields: store
                .field_paths(path)
                .filter_map(|field| store.trace(field, window).data)
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
type Cell<'a> = (Trace<'a>, usize, String);

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
fn value_label(ui: &mut egui::Ui, trace: Trace<'_>, value: &str) -> egui::Response {
    let text = match trace.samples {
        Samples::Arrays { .. } => egui::RichText::new(value).monospace(),
        _ => egui::RichText::new(value),
    };
    ui.add(egui::Label::new(text).extend())
}

impl Table {
    /// Shows the name of `row`, with a button to collapse or expand it for a branch.
    fn name_ui(&mut self, ui: &mut egui::Ui, store: &dyn Store, row: &Row<'_>) {
        ui.add_space(row.depth as f32 * INDENT);
        if row.is_branch {
            let collapsed = self.collapsed.contains(&row.key);
            // the caret of the tree in the traces panel
            let size = egui::vec2(ui.spacing().icon_width, ui.spacing().interact_size.y);
            let (_, response) = ui.allocate_exact_size(size, egui::Sense::click());
            let openness = if collapsed { 0.0 } else { 1.0 };
            theme::paint_caret(ui, openness, &response);
            let name = if collapsed { "Expand" } else { "Collapse" };
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name)
            });
            if response.clicked() {
                super::set_member(&mut self.collapsed, &row.key, !collapsed);
            }
        } else {
            ui.add_space(ui.spacing().icon_width);
        }
        let icon = match row.trace {
            Some(trace) => Some(type_icon(trace.samples)),
            None => row.is_record.then_some(BRACKETS_CURLY),
        };
        let response = ui.label(row.label);
        if let Some(icon) = icon {
            ui.weak(icon);
        }
        if let Some(trace) = row.trace {
            response.on_hover_text(super::trace_hover(store, trace));
        }
    }

    /// The rows of the shown traces of `traces` as listed in the properties, without
    /// those without a sample at `t` nanoseconds unless empty traces are shown.
    fn table_rows<'a>(
        &self,
        traces: impl Iterator<Item = Trace<'a>>,
        t: Option<u64>,
    ) -> Vec<Row<'a>> {
        let selected: Vec<Trace<'_>> = traces
            .filter(|trace| self.traces.contains(trace.path))
            .filter(|trace| {
                self.show_empty || t.and_then(|t| index_at(trace.timestamps, t)).is_some()
            })
            .collect();
        let mut table_rows = vec![];
        let paths = selected.iter().map(|trace| trace.path);
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
                key: trace.path.to_owned(),
                trace: Some(*trace),
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
    fn change_layout(&mut self, store: &dyn Store, layout: TableLayout) {
        if layout == self.layout {
            return;
        }
        for trace in store.traces() {
            let Some(record) = record_path(trace) else {
                continue;
            };
            match layout {
                TableLayout::Records if self.traces.remove(trace.path) => {
                    self.traces.insert(record.to_owned());
                }
                TableLayout::Traces if self.traces.contains(record) => {
                    self.traces.insert(trace.path.to_owned());
                }
                _ => {}
            }
        }
        if layout == TableLayout::Traces {
            for record in store.traces().filter_map(record_path) {
                self.traces.remove(record);
            }
        }
        self.layout = layout;
    }

    /// Shows a row per shown record with the value of each shown field at `t` in
    /// nanoseconds.
    fn show_records(
        &self,
        ui: &mut egui::Ui,
        store: &dyn Store,
        formats: &Formats,
        t: Option<u64>,
    ) {
        // without a time to show, what the records are is enough
        let records = records(store, &self.traces, t.map_or(Window::NONE, Window::at));
        let columns: Vec<&str> = self
            .columns(&records)
            .into_iter()
            .filter(|column| !self.hidden_columns.contains(*column))
            .collect();
        // the labels of each column are those of its first trace
        let labels: Vec<Option<&Labels>> = columns
            .iter()
            .map(|column| {
                records
                    .iter()
                    .find_map(|record| record.fields.get(column))
                    .and_then(|trace| store.labels(trace.path))
            })
            .collect();
        let formats: Vec<_> = columns
            .iter()
            .zip(&labels)
            .map(|(column, labels)| formats.format_of(&column_format_key(column), labels.is_some()))
            .collect();
        // the `index`th sample of `trace` in the format of its column and with its unit,
        // to `digits` significant digits if given
        let write = |trace: Trace<'_>,
                     column: usize,
                     index: usize,
                     max_len: usize,
                     digits: Option<usize>| {
            let value =
                trace
                    .samples
                    .format(index, max_len, digits, formats[column], labels[column]);
            with_unit_of(store, trace, index, value)
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
                        let index = index_at(trace.timestamps, t?)?;
                        Some((
                            *trace,
                            index,
                            write(
                                *trace,
                                column,
                                index,
                                MAX_SHOWN_ELEMENTS,
                                Some(SHOWN_DIGITS),
                            ),
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
            // a right click on a cell opens its menu, not one of the text
            ui.style_mut().interaction.selectable_labels = false;
            let mut table = theme::table(ui).resizable(true).sense(egui::Sense::click());
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
                            let response = ui.strong(*column);
                            let first = records
                                .iter()
                                .find_map(|record| record.fields.get(column).copied());
                            if let Some(trace) = first {
                                response.on_hover_text(super::trace_hover(store, trace));
                            }
                        });
                    }
                })
                .body(|mut body| {
                    let spacing = body.ui_mut().spacing().item_spacing.x;
                    let lines = lines.in_body(body.ui_mut()).across(body.widths(), spacing);
                    body.heterogeneous_rows(heights.into_iter(), |mut table_row| {
                        let index = table_row.index();
                        // a value in full, with all its digits
                        let full = |column: usize, (trace, sample, _): &Cell<'_>| {
                            write(*trace, column, *sample, MAX_TOOLTIP_ELEMENTS, None)
                        };
                        let copy_row = |ui: &mut egui::Ui| {
                            if ui.button("Copy row").clicked() {
                                let values =
                                    cells[index].iter().enumerate().map(|(column, cell)| {
                                        cell.as_ref()
                                            .map_or_else(String::new, |cell| full(column, cell))
                                    });
                                let text = std::iter::once(labels[index].to_owned())
                                    .chain(values)
                                    .collect::<Vec<_>>()
                                    .join("\t");
                                ui.ctx().copy_text(text);
                            }
                        };
                        let (_, name) = table_row.col(|ui| {
                            lines.above(ui, index);
                            ui.add(egui::Label::new(labels[index]).truncate())
                                .on_hover_text(records[index].path);
                        });
                        name.context_menu(copy_row);
                        for (column, cell) in cells[index].iter().enumerate() {
                            let (_, response) = table_row.col(|ui| {
                                if let Some(cell @ (trace, _, value)) = cell {
                                    value_label(ui, *trace, value).on_hover_ui(|ui| {
                                        value_label(ui, *trace, &full(column, cell));
                                    });
                                }
                            });
                            response.context_menu(|ui| {
                                if let Some(cell) = cell
                                    && ui.button("Copy value").clicked()
                                {
                                    ui.ctx().copy_text(full(column, cell));
                                }
                                copy_row(ui);
                            });
                        }
                    });
                });
        });
    }

    /// Shows the field columns of the records layout to show, hide and reorder them by
    /// drag and drop, with the display format of each in `formats`.
    pub(crate) fn columns_ui(
        &mut self,
        ui: &mut egui::Ui,
        store: &dyn Store,
        formats: &mut Formats,
    ) {
        let records = records(store, &self.traces, Window::NONE);
        let columns = self.columns(&records);
        if columns.is_empty() {
            return;
        }
        // a trace of each column, for its kind of numbers and its unit
        let samples: Vec<Trace<'_>> = columns
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
                        super::set_member(&mut self.hidden_columns, *column, !shown);
                    }
                });
                let key = column_format_key(column);
                let kind = super::number_kind(store, trace.path);
                super::number_format_ui(
                    ui,
                    formats,
                    &key,
                    kind,
                    store.labels(trace.path).is_some(),
                    store.unit(trace.path),
                );
                ui.end_row();
                super::trace_facts_rows(ui, store, trace.path);
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

    fn show_table(
        &mut self,
        ui: &mut egui::Ui,
        store: &dyn Store,
        formats: &Formats,
        t: Option<u64>,
        invoker: &mut Invoker,
    ) {
        let now = ui.input(|input| input.time);
        let custom = self.custom.clone();
        // in the order of the store, each with its sample at the shown time
        let window = t.map_or(Window::NONE, Window::at);
        let shown = (store.traces())
            .filter(|trace| self.traces.contains(trace.path))
            .filter_map(|trace| store.trace(trace.path, window).data);
        let table_rows = self.table_rows(shown, t);
        // the `index`th sample of `trace` in its format and with its unit
        let write = |trace: Trace<'_>, index: usize, max_len: usize, digits: Option<usize>| {
            let labels = store.labels(trace.path);
            let value = trace.samples.format(
                index,
                max_len,
                digits,
                formats.format_of(trace.path, labels.is_some()),
                labels,
            );
            with_unit_of(store, trace, index, value)
        };

        // the sample of each row current at t, with its value written as text
        let samples: Vec<Option<(Trace<'_>, usize, String)>> = table_rows
            .iter()
            .map(|row| {
                let trace = row.trace?;
                let index = index_at(trace.timestamps, t?)?;
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
        // a right click on a cell opens its menu, not one of the text
        ui.style_mut().interaction.selectable_labels = false;
        let mut table = theme::table(ui)
            .resizable(true)
            .sense(egui::Sense::click())
            .column(Column::initial(200.0).at_least(80.0).clip(true));
        // the value takes the room that is left, before the columns of the application
        table = if custom.is_empty() {
            table.column(Column::remainder().at_least(60.0).clip(true))
        } else {
            table.column(Column::initial(120.0).at_least(60.0).clip(true))
        };
        for (index, _) in custom.iter().enumerate() {
            let column = if index + 1 == custom.len() {
                Column::remainder().at_least(40.0)
            } else {
                Column::auto().at_least(40.0)
            };
            table = table.column(column.clip(true));
        }
        let titles = (["Trace", "Value"].into_iter())
            .chain(custom.iter().map(|column| column.title.as_str()));
        table
            .header(row_height, |mut header| {
                for title in titles {
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
                    // the value in full, with all its digits
                    let full = |(trace, index, _): &(Trace<'_>, usize, String)| {
                        write(*trace, *index, MAX_TOOLTIP_ELEMENTS, None)
                    };
                    let copy_row = |ui: &mut egui::Ui| {
                        if ui.button("Copy row").clicked() {
                            let text = match sample {
                                Some(sample) => format!("{}\t{}", row.label, full(sample)),
                                None => row.label.to_owned(),
                            };
                            ui.ctx().copy_text(text);
                        }
                    };
                    let (_, name) = table_row.col(|ui| {
                        lines.above(ui, index);
                        self.name_ui(ui, store, row);
                    });
                    name.context_menu(copy_row);
                    let (_, cell) = table_row.col(|ui| {
                        if let Some(sample @ (trace, _, value)) = sample {
                            value_label(ui, *trace, value).on_hover_ui(|ui| {
                                value_label(ui, *trace, &full(sample));
                            });
                        }
                    });
                    cell.context_menu(|ui| {
                        if let Some(sample) = sample
                            && ui.button("Copy value").clicked()
                        {
                            ui.ctx().copy_text(full(sample));
                        }
                        copy_row(ui);
                    });
                    for column in &custom {
                        table_row.col(|ui| {
                            if let Some(trace) = row.trace {
                                let mut cell = TableCell {
                                    trace,
                                    sample: sample.as_ref().map(|(_, index, _)| *index),
                                    store,
                                    invoker: &mut *invoker,
                                    now,
                                };
                                (column.ui)(ui, &mut cell);
                            }
                        });
                    }
                });
            });
    }

    /// Adds a column titled `title` after the value in the traces layout. `ui` shows
    /// the cell of each row with a trace, e.g. a button that invokes an action with
    /// [`TableCell::invoke`]. The columns are not saved with the table: add them
    /// whenever the table is made.
    pub fn column(
        mut self,
        title: impl Into<String>,
        ui: impl Fn(&mut egui::Ui, &mut TableCell<'_>) + 'static,
    ) -> Self {
        self.custom.push(TableColumn {
            title: title.into(),
            ui: Rc::new(ui),
        });
        self
    }
}

impl super::ViewInfo for Table {
    const NAME: &'static str = "Table";
    const ICON: &'static str = egui_phosphor::regular::TABLE;

    fn selectable(_trace: Trace<'_>) -> Selectable {
        Selectable::Trace
    }
}

impl Viewable for Table {
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        let records = self.layout == TableLayout::Records;
        Some((&mut self.traces, records))
    }

    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.traces.is_empty() {
            let text = match self.layout {
                TableLayout::Traces => super::CHECK_TRACES,
                TableLayout::Records => "Check the records to show in the tree on the left",
            };
            theme::hint(ui, text);
            return;
        }
        let time = &context.time;
        let t = time.cursor_timestamp();
        let (store, formats) = (context.store, context.formats);
        let invoker = &mut *context.invoker;
        // the table changes the look of the ui
        ui.scope(|ui| match self.layout {
            TableLayout::Traces => self.show_table(ui, store, formats, t, invoker),
            TableLayout::Records => self.show_records(ui, store, formats, t),
        });
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        super::properties_section(ui, "Rows", |ui| {
            let mut layout = self.layout;
            super::choice_row(ui, "Layout", &mut layout, &TableLayout::CHOICES);
            self.change_layout(context.store, layout);
            if self.layout == TableLayout::Traces {
                super::choice_row(ui, "Traces", &mut self.rows, &TableRows::CHOICES);
                super::checkbox_row(
                    ui,
                    "Show empty traces",
                    &mut self.show_empty,
                    "Show traces without a value at the cursor time",
                );
            }
        });
    }
}

/// `value`, the `index`th sample of `trace` written as text, followed by the unit of
/// the trace, unless the sample is null.
fn with_unit_of(store: &dyn Store, trace: Trace<'_>, index: usize, value: String) -> String {
    let unit = store.unit(trace.path);
    super::with_unit(value, unit.filter(|_| !trace.samples.is_null(index)))
}

#[cfg(test)]
mod tests;
