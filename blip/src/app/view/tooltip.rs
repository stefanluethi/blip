//! The tooltips of the plot-like views, alike in all of them: a header, then rows of a
//! name and values right aligned in columns as wide as their widest text, in groups
//! split by a rule. A view builds a [`Tooltip`] and shows it at the pointer.

use super::{LEGEND_DOT_SIZE, legend_dot, text_size, with_unit};
use crate::app::model::numbers::{DisplayFormat, Number};
use crate::app::theme::{self, TokensExt};

/// A value that is not there, e.g. of a trace not received yet.
pub const MISSING: &str = "—";
/// The room between the columns and between the rows.
const COLUMN_SPACING: f32 = 12.0;
const ROW_SPACING: f32 = 2.0;
/// The height of the rule between two groups of rows.
const RULE_HEIGHT: f32 = 7.0;

/// A trace as a tooltip names it: its dot and short name, its path on hover.
#[derive(Clone, PartialEq, Debug)]
pub struct TraceName {
    pub name: String,
    pub path: String,
    /// Picks its colour.
    pub index: usize,
}

impl TraceName {
    pub fn new(name: &str, path: &str, index: usize) -> Self {
        Self {
            name: name.to_owned(),
            path: path.to_owned(),
            index,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Header {
    /// A time, a range of values or an element index, in weak text.
    Text(String),
    Trace(TraceName),
}

/// The left of a row.
#[derive(Clone, PartialEq, Debug)]
pub enum Label {
    Trace(TraceName),
    /// A name in weak text.
    Weak(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Strong,
    Text,
    Weak,
    Faint,
}

impl Tone {
    /// The colour of the tone in the theme of `ui`.
    pub fn color(self, ui: &egui::Ui) -> egui::Color32 {
        match self {
            Self::Strong => ui.visuals().strong_text_color(),
            Self::Text => ui.visuals().text_color(),
            Self::Weak => ui.visuals().weak_text_color(),
            Self::Faint => ui.tokens().text_faint,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Value {
    pub text: String,
    pub tone: Tone,
}

impl Value {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }

    /// In the text colour.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Text)
    }

    pub fn faint(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Faint)
    }

    /// [`MISSING`], faint.
    pub fn missing() -> Self {
        Self::faint(MISSING)
    }

    /// Faint if `faint`, else as it is.
    pub fn faint_if(self, faint: bool) -> Self {
        match faint {
            true => Self::faint(self.text),
            false => self,
        }
    }

    /// `number` with `digits` significant digits in `format` and its `unit`, in the
    /// text colour; a null is "null", faint and without the unit.
    pub fn number(
        number: Number,
        digits: usize,
        format: DisplayFormat,
        unit: Option<&str>,
    ) -> Self {
        let unit = unit.filter(|_| !number.is_null());
        Self::text(with_unit(number.format(Some(digits), format), unit)).faint_if(number.is_null())
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub label: Label,
    pub values: Vec<Value>,
    /// Whether the label is faint, e.g. of a trace with no samples in a bin.
    pub faint: bool,
}

impl Row {
    pub fn new(label: Label, values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            label,
            values: values.into_iter().collect(),
            faint: false,
        }
    }

    /// The row with its label faint.
    pub fn faint(self) -> Self {
        Self {
            faint: true,
            ..self
        }
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Tooltip {
    pub header: Option<Header>,
    /// A line in faint text under the header, e.g. for a trace without samples.
    pub note: Option<String>,
    /// The groups of rows, a rule between two of them.
    pub groups: Vec<Vec<Row>>,
}

impl Tooltip {
    pub fn new(header: Header) -> Self {
        Self {
            header: Some(header),
            ..Self::default()
        }
    }

    /// Adds a group of `rows`; an empty one is left out.
    pub fn group(mut self, rows: Vec<Row>) -> Self {
        if !rows.is_empty() {
            self.groups.push(rows);
        }
        self
    }

    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.note = Some(text.into());
        self
    }

    /// The widths of the columns: the widest label, then the widest value of each
    /// column, over the rows of all groups, so that the groups line up.
    pub fn widths(&self, ui: &egui::Ui) -> Vec<f32> {
        let body = |text: &str| text_size(ui, text, egui::TextStyle::Body).x;
        let rows = || self.groups.iter().flatten();
        let columns = 1 + rows().map(|row| row.values.len()).max().unwrap_or(0);
        let mut widths = vec![0.0_f32; columns];
        for row in rows() {
            let label = match &row.label {
                Label::Trace(trace) => {
                    LEGEND_DOT_SIZE + ui.spacing().item_spacing.x + body(&trace.name)
                }
                Label::Weak(name) => body(name),
            };
            widths[0] = widths[0].max(label);
            for (column, value) in row.values.iter().enumerate() {
                widths[column + 1] = widths[column + 1].max(body(&value.text));
            }
        }
        widths
    }

    /// Shows the header, the note and the groups with a rule between them.
    pub fn ui(&self, ui: &mut egui::Ui) {
        let faint = ui.tokens().text_faint;
        let (weak, text) = (ui.visuals().weak_text_color(), ui.visuals().text_color());
        match &self.header {
            Some(Header::Text(header)) => {
                ui.label(egui::RichText::new(header).color(weak));
            }
            Some(Header::Trace(trace)) => {
                ui.horizontal(|ui| {
                    legend_dot(ui, theme::trace_color(ui.visuals(), trace.index));
                    ui.label(&trace.name).on_hover_text(&trace.path);
                });
            }
            None => {}
        }
        if let Some(note) = &self.note {
            ui.label(egui::RichText::new(note).color(faint));
        }
        let widths = self.widths(ui);
        let height = ui.spacing().interact_size.y;
        for (index, group) in self.groups.iter().enumerate() {
            if index > 0 {
                let width = widths.iter().sum::<f32>() + COLUMN_SPACING * (widths.len() - 1) as f32;
                let (rule, _) =
                    ui.allocate_exact_size(egui::vec2(width, RULE_HEIGHT), egui::Sense::hover());
                let line = ui.visuals().widgets.noninteractive.bg_stroke;
                ui.painter().hline(rule.x_range(), rule.center().y, line);
            }
            egui::Grid::new(("tooltip", index))
                .num_columns(widths.len())
                .spacing([COLUMN_SPACING, ROW_SPACING])
                .show(ui, |ui| {
                    for row in group {
                        let color = if row.faint { faint } else { text };
                        let left = egui::Layout::left_to_right(egui::Align::Center);
                        ui.allocate_ui_with_layout(egui::vec2(widths[0], height), left, |ui| {
                            // as wide as the widest label, also in the other groups
                            ui.set_min_width(widths[0]);
                            match &row.label {
                                Label::Trace(trace) => {
                                    legend_dot(ui, theme::trace_color(ui.visuals(), trace.index));
                                    let name = egui::RichText::new(&trace.name).color(color);
                                    ui.label(name).on_hover_text(&trace.path);
                                }
                                Label::Weak(name) => {
                                    let color = if row.faint { faint } else { weak };
                                    ui.label(egui::RichText::new(name).color(color));
                                }
                            }
                        });
                        // every value column has a fixed width, so that the tooltip is
                        // as wide as its texts wherever it opens: a right to left layout
                        // fills the width it is given, and the grid gives the last
                        // column all of the tooltip's, the widest a tooltip can be
                        for (value, &width) in row.values.iter().zip(&widths[1..]) {
                            let right = egui::Layout::right_to_left(egui::Align::Center);
                            ui.allocate_ui_with_layout(egui::vec2(width, height), right, |ui| {
                                ui.label(
                                    egui::RichText::new(&value.text).color(value.tone.color(ui)),
                                );
                            });
                        }
                        ui.end_row();
                    }
                });
        }
    }
}

/// The rect of the tooltip shown in `ctx`, if one is.
#[cfg(test)]
pub fn shown_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    let layers = ctx.memory(|memory| memory.areas().visible_layer_ids());
    let tooltip = layers
        .into_iter()
        .find(|layer| layer.order == egui::Order::Tooltip)?;
    egui::AreaState::load(ctx, tooltip.id).map(|state| state.rect())
}

/// The texts of the labels in the tooltip shown in `harness`, with their rects, from
/// the top; panics without a tooltip.
#[cfg(test)]
pub fn shown_texts<State>(harness: &egui_kittest::Harness<'_, State>) -> Vec<(String, egui::Rect)> {
    use egui_kittest::kittest::{NodeT, Queryable};
    let tooltip = shown_rect(&harness.ctx).expect("a tooltip");
    let mut texts: Vec<(String, egui::Rect)> = harness
        .query_all_by(|node| node.value().is_some())
        .filter_map(|node| {
            let rect = node.rect();
            let text = node.accesskit_node().value()?;
            tooltip.contains_rect(rect).then_some((text, rect))
        })
        .collect();
    texts.sort_by(|a, b| {
        (a.1.top(), a.1.left())
            .partial_cmp(&(b.1.top(), b.1.left()))
            .expect("finite")
    });
    texts
}

#[cfg(test)]
mod tests;
