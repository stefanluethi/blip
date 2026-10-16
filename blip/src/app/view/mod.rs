pub mod heatmap;
pub mod log_table;
pub mod plot;
pub mod states;
pub mod table;
pub mod time_axis;
pub mod trace_selection;
pub mod xy;

use crate::app::model::Model;
use crate::app::model::numbers::{DisplayFormat, NumberKind};
use crate::app::model::traces::{Samples, Trace};
use crate::app::time::TimeRange;
use std::collections::{BTreeMap, BTreeSet};
use trace_selection::MAX_LISTED_ELEMENTS;

/// What views get from the app to show their contents.
pub struct ViewContext<'a> {
    pub model: &'a Model,
    pub time: &'a mut TimeRange,
    /// The display formats of the traces of the shown view.
    pub formats: &'a Formats,
}

/// The display formats of the traces of a view by their paths, the default format for
/// traces without one.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct Formats(BTreeMap<String, DisplayFormat>);

/// The formats of a view whose traces all have the default format.
pub static NO_FORMATS: Formats = Formats(BTreeMap::new());

impl Formats {
    pub fn of(&self, path: &str) -> DisplayFormat {
        self.0.get(path).copied().unwrap_or_default()
    }

    pub fn set(&mut self, path: &str, format: DisplayFormat) {
        if format == DisplayFormat::Default {
            self.0.remove(path);
        } else {
            self.0.insert(path.to_owned(), format);
        }
    }
}

pub trait Viewable {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>);

    /// Shows the properties of the view in the properties panel, in sections.
    fn properties_ui(&mut self, _ui: &mut egui::Ui, _model: &Model) {}
}

/// A view in the layout, shown as a pane in the center.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct View {
    pub name: String,
    pub kind: ViewKind,
    #[serde(default)]
    pub formats: Formats,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub enum ViewKind {
    Plot(plot::Plot),
    States(states::States),
    Table(table::Table),
    Xy(xy::Xy),
    Heatmap(heatmap::Heatmap),
    Logs(log_table::LogTable),
}

/// The kinds of views the user can add.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewType {
    Plot,
    States,
    Table,
    Xy,
    Heatmap,
    Logs,
}

/// What a view can show of a trace.
pub enum Selectable {
    /// The trace as a whole.
    Trace,
    /// Each element of an array trace on its own.
    Elements,
    /// Nothing, for the given reason.
    No(String),
}

impl ViewType {
    pub const ALL: [ViewType; 6] = [
        ViewType::Plot,
        ViewType::States,
        ViewType::Table,
        ViewType::Xy,
        ViewType::Heatmap,
        ViewType::Logs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ViewType::Plot => "Plot",
            ViewType::States => "States",
            ViewType::Table => "Table",
            ViewType::Xy => "XY",
            ViewType::Heatmap => "Heatmap",
            ViewType::Logs => "Logs",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ViewType::Plot => egui_phosphor::regular::CHART_LINE,
            ViewType::States => egui_phosphor::regular::CHART_BAR_HORIZONTAL,
            ViewType::Table => egui_phosphor::regular::TABLE,
            ViewType::Xy => egui_phosphor::regular::CHART_SCATTER,
            ViewType::Heatmap => egui_phosphor::regular::GRID_NINE,
            ViewType::Logs => egui_phosphor::regular::LIST_BULLETS,
        }
    }

    /// What a view of this type can show of `trace`.
    pub fn selectable(self, trace: &Trace) -> Selectable {
        match (self, &trace.samples) {
            (ViewType::Logs, _) => Selectable::No("Log views show no traces".to_owned()),
            (ViewType::Plot | ViewType::Xy, Samples::Strings(_)) => {
                Selectable::No("Plots show numbers".to_owned())
            }
            (ViewType::Plot | ViewType::Xy, Samples::Arrays { shape, .. }) => elements(shape),
            (ViewType::Plot | ViewType::Xy, _) => Selectable::Trace,
            (
                ViewType::States,
                Samples::Numbers(numbers)
                | Samples::Arrays {
                    elements: numbers, ..
                },
            ) if numbers.kind() == Some(NumberKind::Float) => {
                Selectable::No("States show strings, bools and integers".to_owned())
            }
            (ViewType::States, Samples::Arrays { shape, .. }) => elements(shape),
            (ViewType::States, _) => Selectable::Trace,
            (ViewType::Table, _) => Selectable::Trace,
            (ViewType::Heatmap, Samples::Arrays { shape, .. })
                if (1..=2).contains(&shape.len()) =>
            {
                Selectable::Trace
            }
            (ViewType::Heatmap, _) => {
                Selectable::No("Heatmaps show arrays of one or two dimensions".to_owned())
            }
        }
    }

    /// Whether a view of this type writes the values of its traces as text, so their
    /// display formats apply.
    pub fn writes_values(self) -> bool {
        !matches!(self, ViewType::Xy | ViewType::Logs)
    }

    /// Whether a view of this type shows one trace only.
    pub fn shows_one_trace(self) -> bool {
        self == ViewType::Heatmap
    }
}

/// A text colour readable on `background`.
pub fn text_color_on(background: egui::Color32) -> egui::Color32 {
    let luminance = 0.299 * f32::from(background.r())
        + 0.587 * f32::from(background.g())
        + 0.114 * f32::from(background.b());
    if luminance > 140.0 {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    }
}

/// Names of the traces at `paths` for legends and axes: the last level of each path,
/// after its last "/" or, for a record field, its last ".". Paths whose last levels are
/// the same keep as many levels as needed to tell them apart.
pub fn short_names<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let paths: Vec<&str> = paths.into_iter().collect();
    let name = |path: &'a str, levels: usize| -> &'a str {
        let start = path
            .rmatch_indices(['/', '.'])
            .nth(levels - 1)
            .map_or(0, |(index, _)| index + 1);
        &path[start..]
    };
    let mut levels = vec![1; paths.len()];
    loop {
        let names: Vec<&str> = paths
            .iter()
            .zip(&levels)
            .map(|(path, &levels)| name(path, levels))
            .collect();
        let mut changed = false;
        for (index, path) in paths.iter().enumerate() {
            let shared = names
                .iter()
                .enumerate()
                .any(|(other, &name)| other != index && name == names[index]);
            if shared && names[index].len() < path.len() {
                levels[index] += 1;
                changed = true;
            }
        }
        if !changed {
            return names;
        }
    }
}

/// Shows a section of properties titled `title` that can be collapsed, with the label
/// and the value of each property added by `add` in a row of a grid.
pub fn properties_section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .id_salt(("properties", title))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new(("properties grid", title))
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, add);
        });
}

/// Adds a row labelled `label` to the grid of a properties section to choose one of
/// `options` by their names.
pub fn choice_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    ui.label(label);
    ui.horizontal_wrapped(|ui| {
        for &(option, name) in options {
            ui.selectable_value(value, option, name);
        }
    });
    ui.end_row();
}

/// Adds a row labelled `label` to the grid of a properties section with a field for
/// `value`, which is `None` while the field is empty. While the field is edited, text
/// that is no number is kept in it and leaves `value` as it was.
pub fn optional_number_row(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>) {
    ui.label(label);
    let id = ui.id().with(("optional number", label));
    let edited: Option<String> = ui.data(|data| data.get_temp(id));
    let mut text = edited.unwrap_or_else(|| value.map_or_else(String::new, |v| v.to_string()));
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text("auto")
            .desired_width(f32::INFINITY),
    );
    if response.changed() {
        let text = text.trim();
        if text.is_empty() {
            *value = None;
        } else if let Ok(number) = text.parse() {
            *value = Some(number);
        }
    }
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
    } else {
        ui.data_mut(|data| data.remove::<String>(id));
    }
    ui.end_row();
}

/// Adds rows to the grid of a properties section to choose whether `range` fits the
/// shown values (`None`) or is fixed, and to set the ends of a fixed range. A range
/// that becomes fixed starts at `fitted`, the range fitted to the shown values.
pub fn range_rows(ui: &mut egui::Ui, range: &mut Option<(f64, f64)>, fitted: (f64, f64)) {
    let mut fixed = range.is_some();
    choice_row(ui, "Range", &mut fixed, &[(false, "Auto"), (true, "Fixed")]);
    if fixed != range.is_some() {
        *range = fixed.then_some(fitted);
    }
    if let Some((min, max)) = range {
        let speed = ((*max - *min).abs() * 0.01).max(1.0e-6);
        ui.label("Min");
        ui.add(egui::DragValue::new(min).speed(speed));
        ui.end_row();
        ui.label("Max");
        ui.add(egui::DragValue::new(max).speed(speed));
        ui.end_row();
    }
}

/// The elements of an array of `shape` if they are few enough to be listed.
fn elements(shape: &[usize]) -> Selectable {
    if shape.iter().product::<usize>() > MAX_LISTED_ELEMENTS {
        Selectable::No(format!(
            "Arrays of more than {MAX_LISTED_ELEMENTS} elements cannot be shown per element"
        ))
    } else {
        Selectable::Elements
    }
}

impl View {
    pub fn new(view_type: ViewType, name: String) -> Self {
        let kind = match view_type {
            ViewType::Plot => ViewKind::Plot(plot::Plot::new()),
            ViewType::States => ViewKind::States(states::States::new()),
            ViewType::Table => ViewKind::Table(table::Table::new()),
            ViewType::Xy => ViewKind::Xy(xy::Xy::new()),
            ViewType::Heatmap => ViewKind::Heatmap(heatmap::Heatmap::new()),
            ViewType::Logs => ViewKind::Logs(log_table::LogTable::new()),
        };
        Self {
            name,
            kind,
            formats: Formats::default(),
        }
    }

    pub fn view_type(&self) -> ViewType {
        match self.kind {
            ViewKind::Plot(_) => ViewType::Plot,
            ViewKind::States(_) => ViewType::States,
            ViewKind::Table(_) => ViewType::Table,
            ViewKind::Xy(_) => ViewType::Xy,
            ViewKind::Heatmap(_) => ViewType::Heatmap,
            ViewKind::Logs(_) => ViewType::Logs,
        }
    }

    pub fn shows_traces(&self) -> bool {
        !matches!(self.kind, ViewKind::Logs(_))
    }

    /// The paths of the traces the view shows and its type, or `None` if it does not
    /// show traces.
    pub fn selection_mut(&mut self) -> Option<(&mut BTreeSet<String>, ViewType)> {
        let view_type = self.view_type();
        let paths = match &mut self.kind {
            ViewKind::Plot(plot) => &mut plot.traces,
            ViewKind::States(states) => &mut states.traces,
            ViewKind::Table(table) => &mut table.traces,
            ViewKind::Xy(xy) => &mut xy.traces,
            ViewKind::Heatmap(heatmap) => &mut heatmap.traces,
            ViewKind::Logs(_) => return None,
        };
        Some((paths, view_type))
    }
}

impl Viewable for View {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let context = &mut ViewContext {
            model: context.model,
            time: context.time,
            formats: &self.formats,
        };
        match &mut self.kind {
            ViewKind::Plot(plot) => plot.view(ui, context),
            ViewKind::States(states) => states.view(ui, context),
            ViewKind::Table(table) => table.view(ui, context),
            ViewKind::Xy(xy) => xy.view(ui, context),
            ViewKind::Heatmap(heatmap) => heatmap.view(ui, context),
            ViewKind::Logs(logs) => logs.view(ui, context),
        }
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        match &mut self.kind {
            ViewKind::Plot(plot) => plot.properties_ui(ui, model),
            ViewKind::States(states) => states.properties_ui(ui, model),
            ViewKind::Table(table) => table.properties_ui(ui, model),
            ViewKind::Xy(xy) => xy.properties_ui(ui, model),
            ViewKind::Heatmap(heatmap) => heatmap.properties_ui(ui, model),
            ViewKind::Logs(logs) => logs.properties_ui(ui, model),
        }
        self.number_format_ui(ui, model);
    }
}

impl View {
    /// Shows the number format of each shown trace with its unit, if the view writes
    /// the values of its traces.
    fn number_format_ui(&mut self, ui: &mut egui::Ui, model: &Model) {
        if !self.view_type().writes_values() {
            return;
        }
        let paths: Vec<String> = match self.selection_mut() {
            Some((paths, _)) if !paths.is_empty() => paths.iter().cloned().collect(),
            _ => return,
        };
        let names = short_names(paths.iter().map(String::as_str));
        properties_section(ui, "Number format", |ui| {
            for (path, name) in paths.iter().zip(names) {
                ui.label(name).on_hover_text(path);
                ui.horizontal(|ui| {
                    let current = self.formats.of(path);
                    ui.add_enabled_ui(has_numbers(model, path), |ui| {
                        egui::ComboBox::from_id_salt(("number format", path))
                            .selected_text(current.name())
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for format in DisplayFormat::ALL {
                                    if ui
                                        .selectable_label(current == format, format.name())
                                        .clicked()
                                    {
                                        self.formats.set(path, format);
                                    }
                                }
                            })
                            .response
                            .on_hover_text("How the numbers of the trace are written")
                            .on_disabled_hover_text("Only numbers have a format");
                    });
                    if let Some(unit) = model.traces.unit(path) {
                        ui.weak(unit);
                    }
                });
                ui.end_row();
            }
        });
    }
}

/// Whether the trace at `path` has numbers other than bools, which display formats
/// apply to.
fn has_numbers(model: &Model, path: &str) -> bool {
    let kind = match model.traces.series(path) {
        Some(series) => Some(series.kind),
        None => match model.traces.get(path).map(|trace| &trace.samples) {
            Some(Samples::Arrays { elements, .. }) => elements.kind(),
            _ => None,
        },
    };
    kind.is_some_and(|kind| kind != NumberKind::Bool)
}

/// The unit of the traces at `paths` if they all have the same.
pub fn common_unit<'a, 'b>(
    model: &'a Model,
    paths: impl IntoIterator<Item = &'b str>,
) -> Option<&'a str> {
    let mut units = paths.into_iter().map(|path| model.traces.unit(path));
    let first = units.next()??;
    units.all(|unit| unit == Some(first)).then_some(first)
}

/// `value` followed by `unit` if there is one.
pub fn with_unit(value: String, unit: Option<&str>) -> String {
    match unit {
        Some(unit) => format!("{value} {unit}"),
        None => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
