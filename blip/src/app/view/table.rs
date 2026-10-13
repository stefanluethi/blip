//! A table of the values of streams at one time: the hovered time, or else the end of
//! the shown window.

use super::stream_selection::{StreamTree, type_icon};
use super::time_axis::index_at;
use super::{ViewContext, Viewable};
use crate::app::model::Model;
use crate::app::model::streams::{Samples, Stream};
use crate::app::time::{TimeBase, timestamp_to_seconds};
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular::{BRACKETS_CURLY, CARET_DOWN, CARET_RIGHT};
use std::collections::BTreeSet;

/// Arrays show at most this many elements per dimension in the table, and more in the
/// tooltip.
const MAX_SHOWN_ELEMENTS: usize = 8;
/// Floats show at most this many significant digits in the table, and all in the
/// tooltip.
const SHOWN_DIGITS: usize = 6;
const MAX_TOOLTIP_ELEMENTS: usize = 64;
/// Indentation of a level of the tree in points.
const INDENT: f32 = 14.0;

/// The values of the selected streams at the cursor time.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
#[serde(default)]
pub struct Table {
    /// Paths of the shown streams.
    pub streams: BTreeSet<String>,
    /// The collapsed branches, by the path segments leading to them joined with "/".
    pub collapsed: BTreeSet<String>,
}

/// A row of the table: a branch of the tree, a stream, or both.
struct Row<'a> {
    depth: usize,
    label: &'a str,
    /// Identifies the branch, for collapsing it.
    key: String,
    stream: Option<&'a Stream>,
    is_branch: bool,
    is_record: bool,
}

/// The rows of the children of `node`, without those of collapsed branches.
fn rows<'a>(
    node: &StreamTree<'a>,
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
            stream: child.stream,
            is_branch: !child.children.is_empty(),
            is_record: child.is_record(),
        });
        if !collapsed.contains(&key) {
            rows(child, &key, depth + 1, collapsed, out);
        }
    }
}

/// Shows `value` of `stream` without wrapping, in monospace for an array so its
/// columns line up.
fn value_label(ui: &mut egui::Ui, stream: &Stream, value: &str) -> egui::Response {
    let text = match stream.samples {
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
        let icon = match row.stream {
            Some(stream) => Some(type_icon(&stream.samples)),
            None => row.is_record.then_some(BRACKETS_CURLY),
        };
        let response = ui.label(row.label);
        if let Some(icon) = icon {
            ui.weak(icon);
        }
        if let Some(stream) = row.stream {
            response.on_hover_text(&stream.path);
        }
    }

    fn show_table(&mut self, ui: &mut egui::Ui, model: &Model, t: Option<f64>, base: TimeBase) {
        let selected = model
            .streams
            .iter()
            .filter(|stream| self.streams.contains(&stream.path));
        let tree = StreamTree::build(selected);
        let mut table_rows = vec![];
        rows(&tree, "", 0, &self.collapsed, &mut table_rows);

        // the sample of each row current at t, with its value written as text
        let samples: Vec<Option<(&Stream, usize, String)>> = table_rows
            .iter()
            .map(|row| {
                let stream = row.stream?;
                let index = index_at(&stream.timestamps, t?)?;
                let value = stream
                    .samples
                    .format(index, MAX_SHOWN_ELEMENTS, Some(SHOWN_DIGITS));
                Some((stream, index, value))
            })
            .collect();
        let row_height = ui.spacing().interact_size.y;
        let line_height = ui.text_style_height(&egui::TextStyle::Monospace);
        // arrays take a line per row
        let heights = samples.iter().map(|sample| {
            let lines = sample
                .as_ref()
                .map_or(1, |(_, _, value)| value.lines().count());
            row_height.max(lines as f32 * line_height)
        });

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(200.0).at_least(80.0).clip(true))
            .column(Column::initial(300.0).at_least(60.0).clip(true))
            .column(Column::remainder().at_least(60.0))
            .header(row_height, |mut header| {
                for title in ["Stream", "Value", "Time"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.heterogeneous_rows(heights, |mut table_row| {
                    let row = &table_rows[table_row.index()];
                    let sample = &samples[table_row.index()];
                    table_row.col(|ui| self.name_ui(ui, row));
                    table_row.col(|ui| {
                        if let Some((stream, index, value)) = sample {
                            value_label(ui, stream, value).on_hover_ui(|ui| {
                                let value =
                                    stream.samples.format(*index, MAX_TOOLTIP_ELEMENTS, None);
                                value_label(ui, stream, &value);
                            });
                        }
                    });
                    table_row.col(|ui| {
                        if let Some((stream, index, _)) = sample {
                            let time = timestamp_to_seconds(stream.timestamps[*index]);
                            ui.weak(base.format(time, 3));
                        }
                    });
                });
            });
    }
}

impl Viewable for Table {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.streams.is_empty() {
            ui.weak("Select the streams to show in the streams panel");
            return;
        }
        let time = &context.time;
        if let Some(label) = time.cursor_label() {
            ui.weak(format!("Values at {label}"));
        }
        self.show_table(ui, context.model, time.cursor(), time.base());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_branches_hide_their_rows() {
        let streams = [
            Stream::new("motor.speed".to_owned(), Some(5)),
            Stream::new("motor.state".to_owned(), Some(5)),
            Stream::new("sensor/temperature".to_owned(), None),
        ];
        let tree = StreamTree::build(streams.iter());
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
}
