pub mod plot;
pub mod log_table;
pub mod stream_selection;

use crate::app::model::Model;
use crate::app::time::TimeRange;

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
    Logs(log_table::LogTable),
}

/// The kinds of views the user can add.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewType {
    Plot,
    Logs,
}

impl ViewType {
    pub const ALL: [ViewType; 2] = [ViewType::Plot, ViewType::Logs];

    pub fn name(self) -> &'static str {
        match self {
            ViewType::Plot => "Plot",
            ViewType::Logs => "Logs",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ViewType::Plot => egui_phosphor::regular::CHART_LINE,
            ViewType::Logs => egui_phosphor::regular::LIST_BULLETS,
        }
    }
}

impl View {
    pub fn new(view_type: ViewType, name: String) -> Self {
        let kind = match view_type {
            ViewType::Plot => ViewKind::Plot(plot::Plot::new()),
            ViewType::Logs => ViewKind::Logs(log_table::LogTable::new()),
        };
        Self { name, kind }
    }

    pub fn view_type(&self) -> ViewType {
        match self.kind {
            ViewKind::Plot(_) => ViewType::Plot,
            ViewKind::Logs(_) => ViewType::Logs,
        }
    }

    pub fn shows_streams(&self) -> bool {
        matches!(self.kind, ViewKind::Plot(_))
    }

    /// The streams the view shows, or `None` if it does not show streams.
    pub fn streams_mut(&mut self) -> Option<&mut std::collections::BTreeSet<String>> {
        match &mut self.kind {
            ViewKind::Plot(plot) => Some(&mut plot.streams),
            ViewKind::Logs(_) => None,
        }
    }
}

impl Viewable for View {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        match &mut self.kind {
            ViewKind::Plot(plot) => plot.view(ui, context),
            ViewKind::Logs(logs) => logs.view(ui, context),
        }
    }
}
