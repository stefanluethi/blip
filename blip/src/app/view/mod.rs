pub mod heatmap;
pub mod log_table;
pub mod plot;
pub mod states;
pub mod stream_selection;
pub mod table;
pub mod time_axis;
pub mod xy;

use crate::app::model::Model;
use crate::app::model::streams::{NumberKind, Samples, Stream};
use crate::app::time::TimeRange;
use std::collections::BTreeSet;
use stream_selection::MAX_LISTED_ELEMENTS;

/// What views get from the app to show their contents.
pub struct ViewContext<'a> {
    pub model: &'a Model,
    pub time: &'a mut TimeRange,
}

pub trait Viewable {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>);
}

/// A view in the layout, shown as a pane in the center.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct View {
    pub name: String,
    pub kind: ViewKind,
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

/// What a view can show of a stream.
pub enum Selectable {
    /// The stream as a whole.
    Stream,
    /// Each element of an array stream on its own.
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

    /// What a view of this type can show of `stream`.
    pub fn selectable(self, stream: &Stream) -> Selectable {
        match (self, &stream.samples) {
            (ViewType::Logs, _) => Selectable::No("Log views show no streams".to_owned()),
            (ViewType::Plot | ViewType::Xy, Samples::Strings(_)) => {
                Selectable::No("Plots show numbers".to_owned())
            }
            (ViewType::Plot | ViewType::Xy, Samples::Arrays { shape, .. }) => elements(shape),
            (ViewType::Plot | ViewType::Xy, _) => Selectable::Stream,
            (
                ViewType::States,
                Samples::Numbers {
                    kind: NumberKind::Float,
                    ..
                },
            )
            | (
                ViewType::States,
                Samples::Arrays {
                    kind: Some(NumberKind::Float),
                    ..
                },
            ) => Selectable::No("States show strings, bools and integers".to_owned()),
            (ViewType::States, Samples::Arrays { shape, .. }) => elements(shape),
            (ViewType::States, _) => Selectable::Stream,
            (ViewType::Table, _) => Selectable::Stream,
            (ViewType::Heatmap, Samples::Arrays { shape, .. })
                if (1..=2).contains(&shape.len()) =>
            {
                Selectable::Stream
            }
            (ViewType::Heatmap, _) => {
                Selectable::No("Heatmaps show arrays of one or two dimensions".to_owned())
            }
        }
    }

    /// Whether a view of this type shows one stream only.
    pub fn shows_one_stream(self) -> bool {
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

/// Shows the settings of `view` added by `settings` in a row at the bottom of `ui`, and
/// the contents added by `contents` in the space above them.
pub fn settings_below<T>(
    ui: &mut egui::Ui,
    view: &mut T,
    settings: impl FnOnce(&mut T, &mut egui::Ui),
    contents: impl FnOnce(&mut T, &mut egui::Ui),
) {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        settings(view, ui);
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            contents(view, ui);
        });
    });
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
        Self { name, kind }
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

    pub fn shows_streams(&self) -> bool {
        !matches!(self.kind, ViewKind::Logs(_))
    }

    /// The paths of the streams the view shows and its type, or `None` if it does not
    /// show streams.
    pub fn selection_mut(&mut self) -> Option<(&mut BTreeSet<String>, ViewType)> {
        let view_type = self.view_type();
        let paths = match &mut self.kind {
            ViewKind::Plot(plot) => &mut plot.streams,
            ViewKind::States(states) => &mut states.streams,
            ViewKind::Table(table) => &mut table.streams,
            ViewKind::Xy(xy) => &mut xy.streams,
            ViewKind::Heatmap(heatmap) => &mut heatmap.streams,
            ViewKind::Logs(_) => return None,
        };
        Some((paths, view_type))
    }
}

impl Viewable for View {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        match &mut self.kind {
            ViewKind::Plot(plot) => plot.view(ui, context),
            ViewKind::States(states) => states.view(ui, context),
            ViewKind::Table(table) => table.view(ui, context),
            ViewKind::Xy(xy) => xy.view(ui, context),
            ViewKind::Heatmap(heatmap) => heatmap.view(ui, context),
            ViewKind::Logs(logs) => logs.view(ui, context),
        }
    }
}
