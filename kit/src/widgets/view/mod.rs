pub mod actions;
pub mod custom;
pub mod distribution;
pub mod heatmap;
pub mod log_table;
pub mod plot;
pub mod plotting;
pub mod states;
pub mod stats;
pub mod table;
pub mod time_axis;
pub mod tooltip;
pub mod trace_selection;
pub mod xy;

use crate::store::actions::Invoker;
use crate::store::numbers::{DisplayFormat, Labels, NumberKind, SHOWN_DIGITS, format_significant};
use crate::store::traces::{Samples, Trace};
use crate::store::{Store, Version, Window};
use crate::widgets::theme;
use crate::widgets::time::TimeRange;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use trace_selection::{MAX_LISTED_ELEMENTS, Picks, ROOT_KEY, ViewSelection};

/// What views get from the app to show their contents.
pub struct ViewContext<'a> {
    pub store: &'a dyn Store,
    pub time: &'a mut TimeRange,
    /// Sends the invocations of actions.
    pub invoker: &'a mut Invoker,
    /// The display formats of the traces of the shown view.
    pub formats: &'a Formats,
    /// Whether the shown view has a title row, see [`Viewable::title_ui`].
    pub titled: bool,
    /// The plot views of the layout, which a distribution can be linked to.
    pub plots: &'a Plots,
}

impl<'a> ViewContext<'a> {
    /// A context with the default formats, no title row and no plots.
    pub fn new(store: &'a dyn Store, time: &'a mut TimeRange, invoker: &'a mut Invoker) -> Self {
        Self {
            store,
            time,
            invoker,
            formats: &NO_FORMATS,
            titled: false,
            plots: &NO_PLOTS,
        }
    }

    /// The same context with the formats of another view.
    pub fn with_formats<'b>(&'b mut self, formats: &'b Formats) -> ViewContext<'b> {
        ViewContext {
            store: self.store,
            time: &mut *self.time,
            invoker: &mut *self.invoker,
            formats,
            titled: self.titled,
            plots: self.plots,
        }
    }
}

/// A value for each trace of a view by its path, the default for traces without one.
#[derive(serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct PerPath<T>(BTreeMap<String, T>);

/// The display formats of the traces of a view by their paths, the default format for
/// traces without one.
pub type Formats = PerPath<DisplayFormat>;

/// The formats of a view whose traces all have the default format.
pub static NO_FORMATS: Formats = PerPath(BTreeMap::new());

impl<T: Copy + Default + PartialEq> PerPath<T> {
    pub fn of(&self, path: &str) -> T {
        self.0.get(path).copied().unwrap_or_default()
    }

    pub fn set(&mut self, path: &str, value: T) {
        if value == T::default() {
            self.0.remove(path);
        } else {
            self.0.insert(path.to_owned(), value);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Forgets the values of the traces that are not among `shown`, so layouts do not
    /// keep them and traces shown again start with the default.
    pub fn retain_shown(&mut self, shown: &BTreeSet<String>) {
        self.0.retain(|path, _| shown.contains(path));
    }
}

impl PerPath<DisplayFormat> {
    /// The format of the trace at `key`: the one chosen, else Labels if `labelled`, else
    /// Default.
    pub fn format_of(&self, key: &str, labelled: bool) -> DisplayFormat {
        self.0.get(key).copied().unwrap_or(if labelled {
            DisplayFormat::Labels
        } else {
            DisplayFormat::Default
        })
    }

    /// Chooses `format` for `key`, kept only when it is not the default of
    /// [`Self::format_of`].
    pub fn choose(&mut self, key: &str, format: DisplayFormat, labelled: bool) {
        let default = if labelled {
            DisplayFormat::Labels
        } else {
            DisplayFormat::Default
        };
        if format == default {
            self.0.remove(key);
        } else {
            self.0.insert(key.to_owned(), format);
        }
    }
}

/// What views get from the app to show their properties.
pub struct PropertiesContext<'a> {
    pub store: &'a dyn Store,
    /// The names of the open connections.
    pub connected: &'a HashSet<String>,
    /// The plot views of the layout, which a distribution can be linked to.
    pub plots: &'a Plots,
}

/// A view of the layout that another view refers to, by its tile in the tree of the
/// layout. In a saved layout it is the number of the view in the saved order, see
/// `layout::file`.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ViewId(pub u64);

/// A plot view of the layout, as a distribution linked to it sees it.
#[derive(Clone, PartialEq, Debug)]
pub struct PlotInfo {
    pub id: ViewId,
    pub name: String,
    /// The y range it shows, see [`plot::Plot::shown_y`].
    pub y_range: Option<(f64, f64)>,
}

/// The plot views of the layout, in the order of the layout. A distribution can be
/// linked to one of them, taking its traces and its y range, the only link between
/// views.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Plots(pub Vec<PlotInfo>);

/// The plots of a layout without plots.
pub static NO_PLOTS: Plots = Plots(Vec::new());

impl Plots {
    pub fn get(&self, id: ViewId) -> Option<&PlotInfo> {
        self.0.iter().find(|plot| plot.id == id)
    }
}

pub trait Viewable {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>);

    /// Shows the properties of the view in the properties panel, in sections.
    fn properties_ui(&mut self, _ui: &mut egui::Ui, _context: &PropertiesContext<'_>) {}

    /// The paths of the traces, records, actions or logs the view shows, chosen in the
    /// tree of the left panel, and whether it shows records as a whole. `None` if it shows
    /// nothing of the tree.
    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        None
    }

    /// The paths of the traces the view draws in the colours of the traces, in the order
    /// of their colours, see [`theme::trace_color`].
    fn colored_paths(&self, _store: &dyn Store) -> Vec<String> {
        vec![]
    }

    /// Shows what the view adds to its title row after its name, like a legend, within
    /// the width `ui` has left.
    fn title_ui(&self, _ui: &mut egui::Ui, _context: &mut ViewContext<'_>) {}
}

/// A view of the type `V` showing the traces, records, actions or logs at `paths`, for
/// an application that makes its views in code, e.g.
/// `showing::<plot::Plot>(["motor/rpm"])`. A view that shows nothing of the tree is
/// returned as it is by default.
pub fn showing<V: Viewable + Default>(paths: impl IntoIterator<Item = impl Into<String>>) -> V {
    let mut view = V::default();
    if let Some((shown, _)) = view.selection() {
        shown.extend(paths.into_iter().map(Into::into));
    }
    view
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
    /// The paths of the store and what the view picked when the wildcards were last
    /// expanded, to expand them only when either changes.
    #[serde(skip)]
    expanded: Option<(Version, Picks)>,
}

/// What a type of view is and what it can show, listed in `views!`.
pub trait ViewInfo {
    const NAME: &'static str;
    const ICON: &'static str;
    /// Whether it shows traces, actions or logs chosen in the tree of the left panel.
    const HAS_SELECTION: bool = true;
    /// Whether it writes the values of its traces as text, so their display formats
    /// apply.
    const WRITES_VALUES: bool = true;
    /// Whether it shows one trace only.
    const SHOWS_ONE_TRACE: bool = false;
    /// Whether it shows the actions chosen in the tree, and no traces.
    const SHOWS_ACTIONS: bool = false;
    /// Whether it shows the logs chosen in the tree, and no traces.
    const SHOWS_LOGS: bool = false;

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
/// to the list in [`views!`] below; the variant names are saved in layouts. Attributes
/// before a line go to its variant of `ViewKind`, e.g. a serde alias for an old name.
macro_rules! views {
    ($($(#[$meta:meta])* $kind:ident: $view:ty),* $(,)?) => {
        #[derive(serde::Deserialize, serde::Serialize, Clone)]
        pub enum ViewKind {
            $($(#[$meta])* $kind($view),)*
            /// A view of a type the application registered, see [`custom::register`].
            Custom(custom::CustomView),
        }

        /// The kinds of views the user can add.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum ViewType {
            $($kind,)*
            /// A type the application registered, see [`custom::register`].
            Custom(&'static custom::CustomType),
        }

        impl ViewType {
            /// The types of the library.
            pub const BUILT_IN: [ViewType; [$(stringify!($kind)),*].len()] =
                [$(ViewType::$kind),*];

            /// All types the user can add: those of the library, then those the
            /// application registered.
            pub fn all() -> Vec<ViewType> {
                (Self::BUILT_IN.into_iter())
                    .chain(custom::registered().into_iter().map(ViewType::Custom))
                    .collect()
            }

            pub fn name(self) -> &'static str {
                match self { $(Self::$kind => <$view>::NAME,)* Self::Custom(custom) => custom.name }
            }

            pub fn icon(self) -> &'static str {
                match self { $(Self::$kind => <$view>::ICON,)* Self::Custom(custom) => custom.icon }
            }

            /// See [`ViewInfo::HAS_SELECTION`].
            pub fn has_selection(self) -> bool {
                match self { $(Self::$kind => <$view>::HAS_SELECTION,)* Self::Custom(custom) => custom.has_selection }
            }

            /// See [`ViewInfo::WRITES_VALUES`].
            pub fn writes_values(self) -> bool {
                match self { $(Self::$kind => <$view>::WRITES_VALUES,)* Self::Custom(custom) => custom.writes_values }
            }

            /// See [`ViewInfo::SHOWS_ONE_TRACE`].
            pub fn shows_one_trace(self) -> bool {
                match self { $(Self::$kind => <$view>::SHOWS_ONE_TRACE,)* Self::Custom(custom) => custom.shows_one_trace }
            }

            /// See [`ViewInfo::SHOWS_ACTIONS`].
            pub fn shows_actions(self) -> bool {
                match self { $(Self::$kind => <$view>::SHOWS_ACTIONS,)* Self::Custom(custom) => custom.shows_actions }
            }

            /// See [`ViewInfo::SHOWS_LOGS`].
            pub fn shows_logs(self) -> bool {
                match self { $(Self::$kind => <$view>::SHOWS_LOGS,)* Self::Custom(custom) => custom.shows_logs }
            }

            /// What a view of this type can show of `trace`.
            pub fn selectable(self, trace: Trace<'_>) -> Selectable {
                match self {
                    $(Self::$kind => <$view>::selectable(trace),)*
                    Self::Custom(custom) => (custom.selectable)(trace),
                }
            }

            /// See [`ViewInfo::shows_apart`].
            pub fn shows_apart(self, trace: Trace<'_>) -> bool {
                match self {
                    $(Self::$kind => <$view>::shows_apart(trace),)*
                    Self::Custom(custom) => (custom.shows_apart)(trace),
                }
            }

            fn create(self) -> ViewKind {
                match self {
                    $(Self::$kind => ViewKind::$kind(<$view>::default()),)*
                    Self::Custom(custom) => ViewKind::Custom(custom::CustomView::new(custom)),
                }
            }
        }

        impl ViewKind {
            pub fn view_type(&self) -> ViewType {
                match self {
                    $(Self::$kind(_) => ViewType::$kind,)*
                    Self::Custom(custom) => ViewType::Custom(custom.custom_type()),
                }
            }

            fn viewable(&mut self) -> &mut dyn Viewable {
                match self { $(Self::$kind(view) => view,)* Self::Custom(custom) => custom.viewable() }
            }

            fn viewable_ref(&self) -> &dyn Viewable {
                match self { $(Self::$kind(view) => view,)* Self::Custom(custom) => custom.viewable_ref() }
            }
        }
    };
}

views! {
    Plot: plot::Plot,
    States: states::States,
    Table: table::Table,
    Xy: xy::Xy,
    // called Histogram before it showed boxes too
    #[serde(alias = "Histogram")]
    Distribution: distribution::Distribution,
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

/// A text colour readable on `background`: the darkest or the lightest surface of the
/// themes.
pub fn text_color_on(background: egui::Color32) -> egui::Color32 {
    let luminance = 0.299 * f32::from(background.r())
        + 0.587 * f32::from(background.g())
        + 0.114 * f32::from(background.b());
    if luminance > 140.0 {
        theme::DARK.canvas
    } else {
        theme::LIGHT.card
    }
}

/// Each path with its short name and its index, which picks its color. Colors follow the
/// order of the selection, so they do not change as traces arrive.
pub fn indexed_names(paths: &BTreeSet<String>) -> impl Iterator<Item = (&str, &str, usize)> + '_ {
    let names = short_names(paths.iter().map(String::as_str));
    paths
        .iter()
        .zip(names)
        .enumerate()
        .map(|(index, (path, name))| (path.as_str(), name, index))
}

/// The hint of a view with traces to show but none checked.
pub const CHECK_TRACES: &str = "Check the traces to show in the tree on the left";

/// The diameter of the dots in the legends, in points.
pub const LEGEND_DOT_SIZE: f32 = 8.0;

/// Shows a dot in `color` as in the legends.
pub fn legend_dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (dot, _) = ui.allocate_exact_size(egui::Vec2::splat(LEGEND_DOT_SIZE), egui::Sense::hover());
    ui.painter()
        .circle_filled(dot.center(), LEGEND_DOT_SIZE / 2.0, color);
}

/// The size of `text` in `style` on one line.
pub fn text_size(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> egui::Vec2 {
    let font = style.resolve(ui.style());
    (ui.painter())
        .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
}

/// The widest `needed` since `slot` last changed, kept under `id`. Keeps a value in place
/// while it changes, until a new slot (e.g. another range or format) starts it anew.
pub fn sticky_width(ctx: &egui::Context, id: egui::Id, slot: String, needed: f32) -> f32 {
    ctx.data_mut(|data| {
        let (last_slot, width) = data.get_temp_mut_or_default::<(String, f32)>(id);
        if *last_slot != slot {
            *last_slot = slot;
            *width = 0.0;
        }
        *width = width.max(needed);
        *width
    })
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
pub const PROPERTY_LABEL_WIDTH: f32 = 84.0;

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

/// Puts `item` in `set` if `member`, else takes it out.
pub fn set_member<T, Q>(set: &mut BTreeSet<T>, item: &Q, member: bool)
where
    T: Ord + std::borrow::Borrow<Q>,
    Q: Ord + ToOwned<Owned = T> + ?Sized,
{
    if member {
        if !set.contains(item) {
            set.insert(item.to_owned());
        }
    } else {
        set.remove(item);
    }
}

/// Adds a row labelled `label` to the grid of a properties section with a checkbox for
/// `value` that shows `hint` on hover.
pub fn checkbox_row(ui: &mut egui::Ui, label: &str, value: &mut bool, hint: &str) {
    property_label(ui, label);
    ui.checkbox(value, "").on_hover_text(hint);
    ui.end_row();
}

/// Adds a row per path of `paths` to the grid of a properties section: the short name
/// of the path (the path on hover), then what `add` shows for it.
pub fn path_rows<P: AsRef<str>>(
    ui: &mut egui::Ui,
    paths: &[P],
    mut add: impl FnMut(&mut egui::Ui, &P),
) {
    let names = short_names(paths.iter().map(|path| path.as_ref()));
    for (path, name) in paths.iter().zip(names) {
        property_label(ui, name).on_hover_text(path.as_ref());
        add(ui, path);
        ui.end_row();
    }
}

/// Shows a combo box to choose `value` among `options` by their names, with `hint` of
/// each option on hover.
pub fn choice_combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    width: Option<f32>,
    value: &mut T,
    options: impl IntoIterator<Item = (T, &'static str)>,
    hint: Option<fn(T) -> &'static str>,
) -> egui::Response {
    let options: Vec<(T, &'static str)> = options.into_iter().collect();
    let current = options
        .iter()
        .find(|(option, _)| option == value)
        .map_or("", |&(_, name)| name);
    let mut combo = egui::ComboBox::from_id_salt(id_salt).selected_text(current);
    if let Some(width) = width {
        combo = combo.width(width);
    }
    combo
        .show_ui(ui, |ui| {
            for (option, name) in options {
                let response = ui.selectable_value(value, option, name);
                if let Some(hint) = hint {
                    response.on_hover_text(hint(option));
                }
            }
        })
        .response
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

/// The hint of a Min or Max field: the end in use, or "auto".
fn end_hint(end: Option<f64>) -> String {
    end.map_or_else(
        || "auto".to_owned(),
        |end| format_significant(end, SHOWN_DIGITS),
    )
}

/// Adds a row labelled `label` to the grid of a properties section with a field for
/// `value`, which is `None` while the field is empty. The empty field shows `hint`, the
/// end in use. Text that is no finite number is kept in the field, marked as an error,
/// and leaves `value` as it was.
pub fn optional_number_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    hint: Option<f64>,
) {
    end_row(ui, label, value, hint, None);
}

/// Why the text of a Min or Max field is no end of an axis.
const NOT_AN_END: &str = "Enter a finite number, or nothing to fit";

/// The end of an axis that the text of a Min or Max field sets, `None` while it is
/// empty, or why the text sets none.
fn parse_end(text: &str) -> Result<Option<f64>, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<f64>() {
        Ok(end) if end.is_finite() => Ok(Some(end)),
        _ => Err(NOT_AN_END),
    }
}

/// Adds the rows "Min" and "Max" to the grid of a properties section with fields for
/// the ends of an axis, as [`optional_number_row`] does. An end not set falls back to
/// that of `fitted`; the empty fields show the ends of `hints`, those in use. A set end
/// that is not below, or above, the other one is marked as an error.
pub(crate) fn optional_range_rows(
    ui: &mut egui::Ui,
    (min, max): (&mut Option<f64>, &mut Option<f64>),
    fitted: Option<(f64, f64)>,
    hints: Option<(f64, f64)>,
) {
    let [min_problem, max_problem] = Ends {
        min: *min,
        max: *max,
    }
    .problems(fitted);
    end_row(ui, "Min", min, hints.map(|(low, _)| low), min_problem);
    end_row(ui, "Max", max, hints.map(|(_, high)| high), max_problem);
}

/// Adds a row of [`optional_number_row`], its field marked as an error with `problem`
/// on hover if there is one and the text is a number.
fn end_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<f64>,
    hint: Option<f64>,
    problem: Option<String>,
) {
    property_label(ui, label);
    // text that is no number stays in the field of this value, and not in that of
    // another view whose properties show in the same place
    let place = std::ptr::from_ref(value) as usize;
    let id = ui.id().with(("optional number", label, place));
    let kept: Option<String> = ui.data(|data| data.get_temp(id));
    let mut text = kept.unwrap_or_else(|| value.map_or_else(String::new, |v| v.to_string()));
    let response = ui.add(theme::wide_field(&mut text).hint_text(end_hint(hint)));
    let parsed = parse_end(&text);
    if response.changed()
        && let Ok(end) = parsed
    {
        *value = end;
    }
    if response.has_focus() || parsed.is_err() {
        ui.data_mut(|data| data.insert_temp(id, text));
    } else {
        ui.data_mut(|data| data.remove::<String>(id));
    }
    if let Some(problem) = parsed.err().map(str::to_owned).or(problem) {
        mark(ui, response, theme::tokens(ui.visuals()).error, &problem);
    }
    ui.end_row();
}

/// Marks the control of `response` framed in `color`, e.g. `error` for a value that is
/// wrong or `warn` for one changed in use, with `reason` on hover.
pub(crate) fn mark(
    ui: &egui::Ui,
    response: egui::Response,
    color: egui::Color32,
    reason: &str,
) -> egui::Response {
    let stroke = egui::Stroke::new(1.0, color);
    ui.painter().rect_stroke(
        response.rect,
        theme::CONTROL_RADIUS,
        stroke,
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(reason)
}

named_enum! {
    /// What an axis fits while its ends are not set.
    #[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub enum Fit {
        /// The ranges the target declares, grown by values beyond them.
        #[default]
        Expected => "Expected",
        /// The values of the shown traces.
        Values => "Values",
    }
}

/// The union of two optional ranges.
pub fn union(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some((low_a, high_a)), Some((low_b, high_b))) => {
            Some((low_a.min(low_b), high_a.max(high_b)))
        }
        (a, b) => a.or(b),
    }
}

/// Adds the row "Fit to" to the grid of a properties section, only while `declared`,
/// that is while a shown trace declares a range.
pub fn fit_row(ui: &mut egui::Ui, fit: &mut Fit, declared: bool) {
    if declared {
        choice_row(ui, "Fit to", fit, &Fit::CHOICES);
    }
}

/// The ends of an axis as set in the properties, each `None` to fit the shown values,
/// see [`range_rows`]. Also reads the fixed range of earlier versions: both ends, or
/// `null` for none.
#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Default, PartialEq, Debug)]
#[serde(from = "SavedEnds")]
pub struct Ends {
    /// The low end, or `None` to fit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The high end, or `None` to fit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// The saved forms of [`Ends`].
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum SavedEnds {
    /// The fixed range of earlier versions, `None` to fit.
    Range(Option<(f64, f64)>),
    Ends {
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
}

impl From<SavedEnds> for Ends {
    fn from(saved: SavedEnds) -> Self {
        match saved {
            SavedEnds::Range(range) => Self {
                min: range.map(|(min, _)| min),
                max: range.map(|(_, max)| max),
            },
            SavedEnds::Ends { min, max } => Self { min, max },
        }
    }
}

impl Ends {
    /// Both ends of `range`.
    pub fn fixed((min, max): (f64, f64)) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Whether neither end is set.
    pub fn is_auto(self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// The range from the ends that are set and the ends of `fitted`, the range that
    /// fits the shown values, for those that are not. `fitted` if the ends are not in
    /// order, see `Ends::problems`, `None` if an end is unknown.
    pub fn over(self, fitted: Option<(f64, f64)>) -> Option<(f64, f64)> {
        let min = self.min.or(fitted.map(|(min, _)| min))?;
        let max = self.max.or(fitted.map(|(_, max)| max))?;
        if min < max { Some((min, max)) } else { fitted }
    }

    /// What is wrong with the Min and the Max over `fitted`: the ends that are set
    /// while the low end is not below the high one, with the reason.
    pub(crate) fn problems(self, fitted: Option<(f64, f64)>) -> [Option<String>; 2] {
        let low = self.min.or(fitted.map(|(min, _)| min));
        let high = self.max.or(fitted.map(|(_, max)| max));
        match (low, high) {
            (Some(low), Some(high)) if low >= high => {
                let written = |end: f64| format_significant(end, SHOWN_DIGITS);
                [
                    (self.min).map(|_| format!("Min must be below Max, {}", written(high))),
                    (self.max).map(|_| format!("Max must be above Min, {}", written(low))),
                ]
            }
            _ => [None, None],
        }
    }
}

/// Adds the rows Min and Max to the grid of a properties section for `ends`: fields
/// that are empty to fit, showing then the end of `fitted`, the range that fits the
/// shown values, as their hint. Text that is no finite number stays in its field,
/// marked, and leaves its end as it was; ends that are not in order are marked too, and
/// views fit the values instead. Gives whether an end changed.
pub fn range_rows(ui: &mut egui::Ui, ends: &mut Ends, fitted: Option<(f64, f64)>) -> bool {
    let before = *ends;
    let [min_problem, max_problem] = ends.problems(fitted);
    let hints = (fitted.map(|(min, _)| min), fitted.map(|(_, max)| max));
    end_row(ui, "Min", &mut ends.min, hints.0, min_problem);
    end_row(ui, "Max", &mut ends.max, hints.1, max_problem);
    *ends != before
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
        // a new log view shows all logs, also those of paths that arrive later
        let wildcards = if view_type.shows_logs() {
            BTreeSet::from([ROOT_KEY.to_owned()])
        } else {
            BTreeSet::new()
        };
        Self {
            name,
            kind: view_type.create(),
            formats: Formats::default(),
            wildcards,
            expanded: None,
        }
    }

    pub fn view_type(&self) -> ViewType {
        self.kind.view_type()
    }

    /// Changes the views this view refers to by `map`, dropping those it maps to
    /// `None`, e.g. when a view is deleted or a layout saved or loaded.
    pub fn relink(&mut self, map: impl Fn(ViewId) -> Option<ViewId>) {
        if let ViewKind::Distribution(distribution) = &mut self.kind {
            distribution.link = distribution.link.and_then(map);
        }
    }

    /// Gives this view the links to other views of `before`, an earlier state of it,
    /// that it lost, e.g. to the views a delete removed that come back by an undo.
    pub(crate) fn relink_as(&mut self, before: &View) {
        if let (ViewKind::Distribution(now), ViewKind::Distribution(before)) =
            (&mut self.kind, &before.kind)
            && now.link.is_none()
        {
            now.link = before.link;
        }
    }

    /// Whether the view shows traces, actions or logs chosen in the tree of the left
    /// panel.
    pub fn has_selection(&self) -> bool {
        self.view_type().has_selection()
    }

    /// What the view shows of the trace tree: traces, records, or the chosen actions or
    /// logs for an actions or log view, or `None` if it shows nothing of it.
    pub fn selection_mut(&mut self) -> Option<ViewSelection<'_>> {
        let view_type = self.view_type();
        let (paths, records) = self.kind.viewable().selection()?;
        Some(ViewSelection {
            paths,
            wildcards: &mut self.wildcards,
            picks: Picks { view_type, records },
        })
    }

    /// Adds what was added to the store below the wildcards of the view.
    fn expand_wildcards(&mut self, store: &dyn Store) {
        let Some(picks) = self.selection_mut().map(|selection| selection.picks) else {
            return;
        };
        let expanded = Some((store.paths_version(), picks));
        if self.expanded != expanded
            && let Some(selection) = self.selection_mut()
        {
            trace_selection::expand_wildcards(store, selection);
            self.expanded = expanded;
        }
    }
}

impl Viewable for View {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.expand_wildcards(context.store);
        let context = &mut context.with_formats(&self.formats);
        self.kind.viewable().view(ui, context);
    }

    fn colored_paths(&self, store: &dyn Store) -> Vec<String> {
        self.kind.viewable_ref().colored_paths(store)
    }

    fn title_ui(&self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        let context = &mut context.with_formats(&self.formats);
        self.kind.viewable_ref().title_ui(ui, context);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        self.kind.viewable().properties_ui(ui, context);
        match &mut self.kind {
            ViewKind::Table(table) if table.layout == table::TableLayout::Records => {
                table.columns_ui(ui, context.store, &mut self.formats);
            }
            _ => self.traces_ui(ui, context),
        }
    }
}

impl View {
    /// Shows each shown trace with its number format, its unit and what the target says
    /// of it, if the view writes the values of its traces; the formats are greyed if it
    /// takes them from a linked plot.
    fn traces_ui(&mut self, ui: &mut egui::Ui, context: &PropertiesContext<'_>) {
        if !self.view_type().writes_values() {
            return;
        }
        let store = context.store;
        let linked = matches!(&self.kind, ViewKind::Distribution(distribution) if distribution.following(context.plots));
        let paths: Vec<String> = match self.selection_mut() {
            Some(selection) if !selection.paths.is_empty() => {
                selection.paths.iter().cloned().collect()
            }
            _ => return,
        };
        properties_section(ui, "Traces", |ui| {
            let names = short_names(paths.iter().map(String::as_str));
            for (path, name) in paths.iter().zip(names) {
                property_label(ui, name).on_hover_text(path);
                let kind = number_kind(store, path);
                ui.add_enabled_ui(!linked, |ui| {
                    number_format_ui(
                        ui,
                        &mut self.formats,
                        path,
                        kind,
                        store.labels(path).is_some(),
                        store.unit(path),
                    );
                })
                .response
                .on_disabled_hover_text("Set by the linked plot");
                ui.end_row();
                trace_facts_rows(ui, store, path);
            }
        });
    }
}

/// Adds the rows of what the target says of the trace at `path` to a properties grid,
/// each only if it says it: the description, the expected range and the labels.
pub fn trace_facts_rows(ui: &mut egui::Ui, store: &dyn Store, path: &str) {
    let weak = ui.visuals().weak_text_color();
    let text = |text: String| egui::RichText::new(text).color(weak);
    if let Some(description) = store.description(path) {
        ui.label("");
        ui.add(egui::Label::new(text(description.to_owned())).wrap());
        ui.end_row();
    }
    if let Some(range) = store.expected(path) {
        ui.label("");
        ui.label(text(expected_text(range)));
        ui.end_row();
    }
    if let Some(labels) = store.labels(path) {
        let (line, hover) = labels_text(labels);
        ui.label("");
        ui.add(egui::Label::new(text(line)).truncate())
            .on_hover_text(hover);
        ui.end_row();
    }
}

/// `Expected <min> to <max>`.
fn expected_text((min, max): (f64, f64)) -> String {
    format!(
        "Expected {} to {}",
        format_significant(min, SHOWN_DIGITS),
        format_significant(max, SHOWN_DIGITS)
    )
}

/// The labels on one line and, for the hover, one per line.
fn labels_text(labels: &Labels) -> (String, String) {
    let each = |(number, label): (&i64, &String)| format!("{number} {label}");
    let line: Vec<String> = labels.iter().map(each).collect();
    (line.join(", "), line.join("\n"))
}

/// The hover text of a trace: its path, its type and its description if any.
pub fn trace_hover(store: &dyn Store, trace: Trace<'_>) -> String {
    let mut hover = format!("{}\n{}", trace.path, trace.samples.type_label());
    if let Some(description) = store.description(trace.path) {
        hover.push('\n');
        hover.push_str(description);
    }
    hover
}

/// Shows a choice of the format in `formats` under `key` for numbers of `kind`,
/// disabled if there are none, followed by `unit`. Labels are offered if `labelled`.
fn number_format_ui(
    ui: &mut egui::Ui,
    formats: &mut Formats,
    key: &str,
    kind: Option<NumberKind>,
    labelled: bool,
    unit: Option<&str>,
) {
    ui.horizontal(|ui| {
        let current = formats.format_of(key, labelled);
        let choices = kind.map_or(&[][..], |kind| DisplayFormat::of_kind(kind, labelled));
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
                            formats.choose(key, format, labelled);
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
fn number_kind(store: &dyn Store, path: &str) -> Option<NumberKind> {
    match store.series(path, Window::NONE).data {
        Some(series) => Some(series.kind),
        None => match store
            .trace(path, Window::NONE)
            .data
            .map(|trace| trace.samples)
        {
            Some(Samples::Arrays { elements, .. }) => elements.kind(),
            _ => None,
        },
    }
}

/// The unit of the traces at `paths` if they all have the same.
pub fn common_unit<'a, 'b>(
    store: &'a dyn Store,
    paths: impl IntoIterator<Item = &'b str>,
) -> Option<&'a str> {
    let mut units = paths.into_iter().map(|path| store.unit(path));
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
mod tests;
