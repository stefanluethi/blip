pub mod actions;
pub mod heatmap;
pub mod log_table;
pub mod plot;
pub mod states;
pub mod table;
pub mod time_axis;
pub mod trace_selection;
pub mod xy;

use crate::app::model::Model;
use crate::app::model::actions::Invoker;
use crate::app::model::numbers::{DisplayFormat, NumberKind};
use crate::app::model::traces::{Samples, Trace};
use crate::app::time::TimeRange;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use trace_selection::MAX_LISTED_ELEMENTS;

/// What views get from the app to show their contents.
pub struct ViewContext<'a> {
    pub model: &'a Model,
    pub time: &'a mut TimeRange,
    /// Sends the invocations of actions.
    pub invoker: &'a mut Invoker,
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

/// What views get from the app to show their properties.
pub struct PropertiesContext<'a> {
    pub model: &'a Model,
    /// The names of the open connections.
    pub connected: &'a HashSet<String>,
}

pub trait Viewable {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>);

    /// Shows the properties of the view in the properties panel, in sections.
    fn properties_ui(&mut self, _ui: &mut egui::Ui, _context: &PropertiesContext<'_>) {}
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
    Actions(actions::Actions),
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
    Actions,
}

/// What a view can show of a trace.
pub enum Selectable {
    /// The trace as a whole.
    Trace,
    /// Each element of an array trace on its own.
    Elements,
    /// Nothing, for the given reason, mostly a static text as it is asked every frame.
    No(Cow<'static, str>),
}

impl ViewType {
    pub const ALL: [ViewType; 7] = [
        ViewType::Plot,
        ViewType::States,
        ViewType::Table,
        ViewType::Xy,
        ViewType::Heatmap,
        ViewType::Logs,
        ViewType::Actions,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ViewType::Plot => "Plot",
            ViewType::States => "States",
            ViewType::Table => "Table",
            ViewType::Xy => "XY",
            ViewType::Heatmap => "Heatmap",
            ViewType::Logs => "Logs",
            ViewType::Actions => "Actions",
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
            ViewType::Actions => egui_phosphor::regular::SLIDERS_HORIZONTAL,
        }
    }

    /// What a view of this type can show of `trace`.
    pub fn selectable(self, trace: &Trace) -> Selectable {
        match (self, &trace.samples) {
            (ViewType::Logs, _) => Selectable::No("Log views show no traces".into()),
            (ViewType::Actions, _) => Selectable::No("Action views show no traces".into()),
            (ViewType::Plot | ViewType::Xy, Samples::Strings(_)) => {
                Selectable::No("Plots show numbers".into())
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
                Selectable::No("States show strings, bools and integers".into())
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
                Selectable::No("Heatmaps show arrays of one or two dimensions".into())
            }
        }
    }

    /// Whether a view of this type writes the values of its traces as text, so their
    /// display formats apply.
    pub fn writes_values(self) -> bool {
        !matches!(self, ViewType::Xy | ViewType::Logs | ViewType::Actions)
    }

    /// Whether a view of this type shows one trace only.
    pub fn shows_one_trace(self) -> bool {
        self == ViewType::Heatmap
    }

    /// Whether a view of this type shows the actions chosen in the tree, and no traces.
    pub fn shows_actions(self) -> bool {
        self == ViewType::Actions
    }
}

/// Id of the flag that a plot zoomed by the mouse wheel contained the pointer.
fn wheel_zoom_id() -> egui::Id {
    egui::Id::new("wheel zoom")
}

/// Lets the mouse wheel zoom the plot of `response` without holding Ctrl, when the
/// app passes its input through [`zoom_with_wheel`].
pub fn zoom_on_wheel(response: &egui::Response) {
    if response.contains_pointer() && response.enabled() {
        response
            .ctx
            .data_mut(|data| data.insert_temp(wheel_zoom_id(), true));
    }
}

/// Turns the vertical mouse wheel events of `raw_input` without modifiers into zooming,
/// as with Ctrl, if a plot passed to [`zoom_on_wheel`] contained the pointer in the last
/// pass. The plot then zooms along its zoomable axes, and the smoothed rest of the
/// scroll stays a zoom instead of scrolling what the pointer moves on to.
pub fn zoom_with_wheel(ctx: &egui::Context, raw_input: &mut egui::RawInput) {
    let over_plot = ctx.data_mut(|data| data.remove_temp::<bool>(wheel_zoom_id()));
    if over_plot != Some(true) {
        return;
    }
    let zoom_modifier = ctx.options(|options| options.input_options.zoom_modifier);
    for event in &mut raw_input.events {
        if let egui::Event::MouseWheel {
            delta, modifiers, ..
        } = event
            && modifiers.is_none()
            && delta.y.abs() >= delta.x.abs()
        {
            *modifiers = zoom_modifier;
        }
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
        Selectable::No(
            format!(
                "Arrays of more than {MAX_LISTED_ELEMENTS} elements cannot be shown per element"
            )
            .into(),
        )
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
            ViewType::Actions => ViewKind::Actions(actions::Actions::new()),
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
            ViewKind::Actions(_) => ViewType::Actions,
        }
    }

    /// Whether the view shows traces or actions chosen in the tree of the left panel.
    pub fn has_selection(&self) -> bool {
        !matches!(self.kind, ViewKind::Logs(_))
    }

    /// The paths of the traces the view shows, or of the chosen actions for an actions
    /// view, and its type, or `None` if it shows neither.
    pub fn selection_mut(&mut self) -> Option<(&mut BTreeSet<String>, ViewType)> {
        let view_type = self.view_type();
        let paths = match &mut self.kind {
            ViewKind::Plot(plot) => &mut plot.traces,
            ViewKind::States(states) => &mut states.traces,
            ViewKind::Table(table) => &mut table.traces,
            ViewKind::Xy(xy) => &mut xy.traces,
            ViewKind::Heatmap(heatmap) => &mut heatmap.traces,
            ViewKind::Actions(actions) => &mut actions.shown,
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
            invoker: context.invoker,
            formats: &self.formats,
        };
        match &mut self.kind {
            ViewKind::Plot(plot) => plot.view(ui, context),
            ViewKind::States(states) => states.view(ui, context),
            ViewKind::Table(table) => table.view(ui, context),
            ViewKind::Xy(xy) => xy.view(ui, context),
            ViewKind::Heatmap(heatmap) => heatmap.view(ui, context),
            ViewKind::Logs(logs) => logs.view(ui, context),
            ViewKind::Actions(actions) => actions.view(ui, context),
        }
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        match &mut self.kind {
            ViewKind::Plot(plot) => plot.properties_ui(ui, context),
            ViewKind::States(states) => states.properties_ui(ui, context),
            ViewKind::Table(table) => table.properties_ui(ui, context),
            ViewKind::Xy(xy) => xy.properties_ui(ui, context),
            ViewKind::Heatmap(heatmap) => heatmap.properties_ui(ui, context),
            ViewKind::Logs(logs) => logs.properties_ui(ui, context),
            ViewKind::Actions(actions) => actions.properties_ui(ui, context),
        }
        self.number_format_ui(ui, context.model);
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

/// Scrolls the mouse wheel by `delta` points at `pos` without modifiers, with the input
/// passed through [`zoom_with_wheel`] as by the app, and runs until the smoothed scroll
/// has been applied.
#[cfg(test)]
pub fn scroll_wheel<State>(
    harness: &mut egui_kittest::Harness<'_, State>,
    pos: egui::Pos2,
    delta: f32,
) {
    harness.hover_at(pos);
    harness.run();
    let ctx = harness.ctx.clone();
    let input = harness.input_mut();
    input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, delta),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    zoom_with_wheel(&ctx, input);
    // the scroll is smoothed over several frames
    harness.run_steps(60);
    harness.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

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
}
