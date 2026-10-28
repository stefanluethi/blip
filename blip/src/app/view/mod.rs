pub mod actions;
pub mod heatmap;
pub mod log_table;
pub mod plot;
pub mod states;
pub mod table;
pub mod time_axis;
pub mod trace_selection;
pub mod xy;

use crate::app::model::actions::Invoker;
use crate::app::model::numbers::{DisplayFormat, NumberKind};
use crate::app::model::traces::{Samples, Trace};
use crate::app::model::{Model, Version};
use crate::app::theme;
use crate::app::time::TimeRange;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use trace_selection::{MAX_LISTED_ELEMENTS, Picks, ViewSelection};

/// What views get from the app to show their contents.
pub struct ViewContext<'a> {
    pub model: &'a Model,
    pub time: &'a mut TimeRange,
    /// Sends the invocations of actions.
    pub invoker: &'a mut Invoker,
    /// The display formats of the traces of the shown view.
    pub formats: &'a Formats,
    /// Whether the shown view has a title row, see [`Viewable::title_ui`].
    pub titled: bool,
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

    /// The paths of the traces, records or actions the view shows, chosen in the tree
    /// of the left panel, and whether it shows records as a whole. `None` if it shows
    /// nothing of the tree.
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        None
    }

    /// The paths of the traces the view draws in the colours of the traces, in the order
    /// of their colours, see [`theme::trace_color`].
    fn colored_paths(&self, _model: &Model) -> Vec<String> {
        vec![]
    }

    /// Shows what the view adds to its title row after its name, like a legend, within
    /// the width `ui` has left.
    fn title_ui(&self, _ui: &mut egui::Ui, _context: &mut ViewContext<'_>) {}
}

/// A view in the layout, shown as a pane in the center.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct View {
    pub name: String,
    pub kind: ViewKind,
    #[serde(default)]
    pub formats: Formats,
    /// The keys of the branches of the trace tree whose traces, records or actions are
    /// all shown, also those added later, see [`ViewSelection::wildcards`].
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub wildcards: BTreeSet<String>,
    /// The paths of the model and what the view picked when the wildcards were last
    /// expanded, to expand them only when either changes.
    #[serde(skip)]
    expanded: Option<(Version, Picks)>,
}

/// What a type of view is and what it can show, listed in [`views!`].
pub trait ViewInfo {
    const NAME: &'static str;
    const ICON: &'static str;
    /// Whether it shows traces or actions chosen in the tree of the left panel.
    const HAS_SELECTION: bool = true;
    /// Whether it writes the values of its traces as text, so their display formats
    /// apply.
    const WRITES_VALUES: bool = true;
    /// Whether it shows one trace only.
    const SHOWS_ONE_TRACE: bool = false;
    /// Whether it shows the actions chosen in the tree, and no traces.
    const SHOWS_ACTIONS: bool = false;

    /// What it can show of `trace`.
    fn selectable(trace: Trace<'_>) -> Selectable;

    /// Whether it shows `trace` apart from the traces it shows otherwise, i.e. only
    /// along with other traces it shows apart, see [`Selectable::Array`].
    fn shows_apart(_trace: Trace<'_>) -> bool {
        false
    }
}

/// What a view can show of a trace.
pub enum Selectable {
    /// The trace as a whole.
    Trace,
    /// Each element of an array trace on its own.
    Elements,
    /// An array trace as a whole, plotted over the indices of its elements apart from
    /// other traces, and each element on its own if `elements`.
    Array { elements: bool },
    /// Nothing, for the given reason, mostly a static text as it is asked every frame.
    No(Cow<'static, str>),
}

/// Defines the types of views, `ViewType`, and the state of a view of each,
/// `ViewKind`, from the list of the types, each the variant name and the type
/// implementing [`Viewable`], [`ViewInfo`] and `Default`. A new type of view is added
/// to the list in [`views!`] below; the variant names are saved in layouts.
macro_rules! views {
    ($($kind:ident: $view:ty),* $(,)?) => {
        #[derive(serde::Deserialize, serde::Serialize, Clone)]
        pub enum ViewKind {
            $($kind($view)),*
        }

        /// The kinds of views the user can add.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum ViewType {
            $($kind),*
        }

        impl ViewType {
            pub const ALL: [ViewType; [$(stringify!($kind)),*].len()] = [$(ViewType::$kind),*];

            pub fn name(self) -> &'static str {
                match self { $(Self::$kind => <$view>::NAME),* }
            }

            pub fn icon(self) -> &'static str {
                match self { $(Self::$kind => <$view>::ICON),* }
            }

            /// See [`ViewInfo::HAS_SELECTION`].
            pub fn has_selection(self) -> bool {
                match self { $(Self::$kind => <$view>::HAS_SELECTION),* }
            }

            /// See [`ViewInfo::WRITES_VALUES`].
            pub fn writes_values(self) -> bool {
                match self { $(Self::$kind => <$view>::WRITES_VALUES),* }
            }

            /// See [`ViewInfo::SHOWS_ONE_TRACE`].
            pub fn shows_one_trace(self) -> bool {
                match self { $(Self::$kind => <$view>::SHOWS_ONE_TRACE),* }
            }

            /// See [`ViewInfo::SHOWS_ACTIONS`].
            pub fn shows_actions(self) -> bool {
                match self { $(Self::$kind => <$view>::SHOWS_ACTIONS),* }
            }

            /// What a view of this type can show of `trace`.
            pub fn selectable(self, trace: Trace<'_>) -> Selectable {
                match self { $(Self::$kind => <$view>::selectable(trace)),* }
            }

            /// See [`ViewInfo::shows_apart`].
            pub fn shows_apart(self, trace: Trace<'_>) -> bool {
                match self { $(Self::$kind => <$view>::shows_apart(trace)),* }
            }

            fn create(self) -> ViewKind {
                match self { $(Self::$kind => ViewKind::$kind(<$view>::default())),* }
            }
        }

        impl ViewKind {
            pub fn view_type(&self) -> ViewType {
                match self { $(Self::$kind(_) => ViewType::$kind),* }
            }

            fn viewable(&mut self) -> &mut dyn Viewable {
                match self { $(Self::$kind(view) => view),* }
            }

            fn viewable_ref(&self) -> &dyn Viewable {
                match self { $(Self::$kind(view) => view),* }
            }
        }
    };
}

views! {
    Plot: plot::Plot,
    States: states::States,
    Table: table::Table,
    Xy: xy::Xy,
    Heatmap: heatmap::Heatmap,
    Logs: log_table::LogTable,
    Actions: actions::Actions,
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
/// scroll stays a zoom instead of scrolling what the pointer moves on to. With Alt, the
/// wheel zooms along the y axis only, as with Ctrl and Alt.
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
            && (modifiers.is_none() || *modifiers == egui::Modifiers::ALT)
            && delta.y.abs() >= delta.x.abs()
        {
            *modifiers |= zoom_modifier;
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

/// The width of the labels of properties, so that their values line up.
const PROPERTY_LABEL_WIDTH: f32 = 84.0;

/// Shows a section of properties titled `title` that can be collapsed, below a line,
/// with the label and the value of each property added by `add` in a row of a grid.
pub fn properties_section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    let line = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.painter()
        .hline(ui.max_rect().x_range(), ui.cursor().top(), line);
    ui.add_space(4.0);
    egui::CollapsingHeader::new(theme::title(ui.visuals(), title))
        .icon(theme::paint_caret)
        .id_salt(("properties", title))
        .default_open(true)
        .show_unindented(ui, |ui| {
            egui::Grid::new(("properties grid", title))
                .num_columns(2)
                .min_col_width(PROPERTY_LABEL_WIDTH)
                .min_row_height(ui.spacing().interact_size.y)
                .spacing([8.0, 6.0])
                .show(ui, add);
        });
}

/// The label of a property in the first column of the grid of a properties section.
pub fn property_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let color = ui.visuals().weak_text_color();
    ui.label(egui::RichText::new(text).color(color))
}

/// Adds a row labelled `label` to the grid of a properties section to choose one of
/// `options` by their names.
pub fn choice_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    property_label(ui, label);
    theme::segmented(ui, value, options);
    ui.end_row();
}

/// Adds a row labelled `label` to the grid of a properties section with a field for
/// `value`, which is `None` while the field is empty. While the field is edited, text
/// that is no number is kept in it and leaves `value` as it was.
pub fn optional_number_row(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>) {
    property_label(ui, label);
    let id = ui.id().with(("optional number", label));
    let edited: Option<String> = ui.data(|data| data.get_temp(id));
    let mut text = edited.unwrap_or_else(|| value.map_or_else(String::new, |v| v.to_string()));
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text("auto")
            .margin(theme::FIELD_MARGIN)
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
        property_label(ui, "Min");
        ui.add(egui::DragValue::new(min).speed(speed));
        ui.end_row();
        property_label(ui, "Max");
        ui.add(egui::DragValue::new(max).speed(speed));
        ui.end_row();
    }
}

/// The elements of an array of `shape` if they are few enough to be listed.
pub fn elements(shape: &[usize]) -> Selectable {
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
        Self {
            name,
            kind: view_type.create(),
            formats: Formats::default(),
            wildcards: BTreeSet::new(),
            expanded: None,
        }
    }

    pub fn view_type(&self) -> ViewType {
        self.kind.view_type()
    }

    /// Whether the view shows traces or actions chosen in the tree of the left panel.
    pub fn has_selection(&self) -> bool {
        self.view_type().has_selection()
    }

    /// What the view shows of the trace tree: traces, records, or the chosen actions
    /// for an actions view, or `None` if it shows nothing of it.
    pub fn selection_mut(&mut self) -> Option<ViewSelection<'_>> {
        let view_type = self.view_type();
        let (paths, records) = self.kind.viewable().selection()?;
        Some(ViewSelection {
            paths,
            wildcards: &mut self.wildcards,
            picks: Picks { view_type, records },
        })
    }

    /// Adds what was added to the model below the wildcards of the view.
    fn expand_wildcards(&mut self, model: &Model) {
        let Some(picks) = self.selection_mut().map(|selection| selection.picks) else {
            return;
        };
        let expanded = Some((model.paths_version(), picks));
        if self.expanded != expanded
            && let Some(selection) = self.selection_mut()
        {
            trace_selection::expand_wildcards(model, selection);
            self.expanded = expanded;
        }
    }
}

impl Viewable for View {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.expand_wildcards(context.model);
        let context = &mut ViewContext {
            model: context.model,
            time: context.time,
            invoker: context.invoker,
            formats: &self.formats,
            titled: context.titled,
        };
        self.kind.viewable().view(ui, context);
    }

    fn colored_paths(&self, model: &Model) -> Vec<String> {
        self.kind.viewable_ref().colored_paths(model)
    }

    fn title_ui(&self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let context = &mut ViewContext {
            model: context.model,
            time: context.time,
            invoker: context.invoker,
            formats: &self.formats,
            titled: context.titled,
        };
        self.kind.viewable_ref().title_ui(ui, context);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        self.kind.viewable().properties_ui(ui, context);
        match &mut self.kind {
            ViewKind::Table(table) if table.layout == table::TableLayout::Records => {
                table.columns_ui(ui, context.model, &mut self.formats);
            }
            _ => self.number_format_ui(ui, context.model),
        }
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
            Some(selection) if !selection.paths.is_empty() => {
                selection.paths.iter().cloned().collect()
            }
            _ => return,
        };
        let names = short_names(paths.iter().map(String::as_str));
        properties_section(ui, "Number format", |ui| {
            for (path, name) in paths.iter().zip(names) {
                property_label(ui, name).on_hover_text(path);
                let kind = number_kind(model, path);
                number_format_ui(ui, &mut self.formats, path, kind, model.traces.unit(path));
                ui.end_row();
            }
        });
    }
}

/// Shows a choice of the format in `formats` under `key` for numbers of `kind`,
/// disabled if there are none, followed by `unit`.
fn number_format_ui(
    ui: &mut egui::Ui,
    formats: &mut Formats,
    key: &str,
    kind: Option<NumberKind>,
    unit: Option<&str>,
) {
    ui.horizontal(|ui| {
        let current = formats.of(key);
        let choices = kind.map_or(&[][..], DisplayFormat::of_kind);
        ui.add_enabled_ui(!choices.is_empty(), |ui| {
            egui::ComboBox::from_id_salt(("number format", key))
                .selected_text(current.name())
                .width(90.0)
                .show_ui(ui, |ui| {
                    for &format in choices {
                        if ui
                            .selectable_label(current == format, format.name())
                            .clicked()
                        {
                            formats.set(key, format);
                        }
                    }
                })
                .response
                .on_hover_text("How the numbers are written")
                .on_disabled_hover_text("Only numbers have a format");
        });
        if let Some(unit) = unit {
            ui.weak(unit);
        }
    });
}

/// The kind of the numbers of the trace at `path`, or of its elements if it is an
/// array trace, or `None` if it has no numbers.
fn number_kind(model: &Model, path: &str) -> Option<NumberKind> {
    match model.traces.series(path) {
        Some(series) => Some(series.kind),
        None => match model.traces.get(path).map(|trace| trace.samples) {
            Some(Samples::Arrays { elements, .. }) => elements.kind(),
            _ => None,
        },
    }
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
}
