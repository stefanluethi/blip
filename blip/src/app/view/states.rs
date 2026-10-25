//! A states view: the values of traces over time as a row of coloured bands per
//! trace, one band for each run of equal values.

use super::time_axis::{self, index_at, visible_range};
use super::{
    Formats, PropertiesContext, ViewContext, Viewable, short_names, text_color_on, with_unit,
};
use crate::app::model::Model;
use crate::app::model::numbers::{DisplayFormat, NumberKind, SHOWN_DIGITS};
use crate::app::model::traces::{Samples, Series, Strings};
use crate::app::theme;
use crate::app::time::{TimeBase, timestamp_to_seconds};
use egui::Color32;
use std::collections::BTreeSet;

/// Runs narrower than this many points next to each other are drawn as one band that
/// shows that the value changes too often to tell.
const MIN_RUN_WIDTH: f64 = 3.0;
/// Approximate width of a character of a band label in points, to only label bands
/// wide enough for it.
const LABEL_CHAR_WIDTH: f64 = 8.0;

/// The values of the selected traces as states over time.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct States {
    /// Paths of the shown traces.
    #[serde(alias = "streams")] // the name before layout version 3
    pub traces: BTreeSet<String>,
    /// Height of a band as a fraction of its row, the rest is the gap to the next row.
    pub band_height: f64,
    /// Whether the bands wide enough for their value are labelled with it.
    pub show_values: bool,
}

impl Default for States {
    fn default() -> Self {
        Self {
            traces: BTreeSet::new(),
            band_height: 0.8,
            show_values: true,
        }
    }
}

/// The samples of a trace shown as states.
#[derive(Clone, Copy)]
enum StateSamples<'a> {
    /// Numbers, written in the display format.
    Numbers(Series<'a>, DisplayFormat),
    Strings {
        timestamps: &'a [u64],
        texts: &'a Strings,
    },
}

impl<'a> StateSamples<'a> {
    fn of(model: &'a Model, path: &str, format: DisplayFormat) -> Option<Self> {
        if let Some(series) = model.traces.series(path) {
            return Some(Self::Numbers(series, format));
        }
        let trace = model.traces.get(path)?;
        match &trace.samples {
            Samples::Strings(texts) => Some(Self::Strings {
                timestamps: trace.timestamps,
                texts,
            }),
            _ => None,
        }
    }

    fn timestamps(&self) -> &'a [u64] {
        match self {
            Self::Numbers(series, _) => series.timestamps,
            Self::Strings { timestamps, .. } => timestamps,
        }
    }

    fn is_null(&self, index: usize) -> bool {
        match self {
            Self::Numbers(series, _) => series.number(index).is_null(),
            Self::Strings { texts, .. } => texts.get(index).is_none(),
        }
    }

    /// Whether the samples at `a` and `b` have the same value.
    fn same(&self, a: usize, b: usize) -> bool {
        match self {
            Self::Numbers(series, _) => series.number(a) == series.number(b),
            Self::Strings { texts, .. } => texts.same(a, b),
        }
    }

    /// The value of the sample at `index`, which its colour follows.
    fn label(&self, index: usize) -> String {
        match self {
            Self::Numbers(series, _) => series.number(index).to_string(),
            Self::Strings { .. } => self.text(index),
        }
    }

    /// The value of the sample at `index` as shown, in the display format.
    fn text(&self, index: usize) -> String {
        match self {
            Self::Numbers(series, format) => {
                series.number(index).format(Some(SHOWN_DIGITS), *format)
            }
            Self::Strings { texts, .. } => texts.get(index).unwrap_or("null").to_owned(),
        }
    }

    fn is_bool(&self) -> bool {
        matches!(self, Self::Numbers(series, _) if series.kind == NumberKind::Bool)
    }
}

/// A band from `start` to `end` seconds.
#[derive(Debug, PartialEq)]
struct Run {
    start: f64,
    end: f64,
    /// A sample with the value of the run, or `None` if the run merges runs too narrow
    /// to tell apart.
    index: Option<usize>,
}

/// The runs of equal values of `samples` within `range` seconds, leaving gaps at nulls.
/// Each sample holds its value until the next one, the last one until `latest` seconds.
/// Neighbouring runs shorter than `min_width` seconds are merged into one.
fn runs(samples: StateSamples<'_>, range: (f64, f64), latest: f64, min_width: f64) -> Vec<Run> {
    let timestamps = samples.timestamps();
    let indices = visible_range(timestamps, Some(range));
    let mut runs: Vec<Run> = vec![];
    for index in indices {
        if samples.is_null(index) {
            continue;
        }
        let start = timestamp_to_seconds(timestamps[index]);
        let end = timestamps
            .get(index + 1)
            .map_or(latest.max(start), |&next| timestamp_to_seconds(next));
        match runs.last_mut() {
            Some(run)
                if run.end == start
                    && run
                        .index
                        .is_some_and(|previous| samples.same(previous, index)) =>
            {
                run.end = end;
            }
            _ => runs.push(Run {
                start,
                end,
                index: Some(index),
            }),
        }
    }
    merge_narrow(runs, min_width)
}

/// Merges neighbouring runs shorter than `min_width` seconds into one run without a
/// value. A single narrow run keeps its value, so short pulses stay visible.
fn merge_narrow(runs: Vec<Run>, min_width: f64) -> Vec<Run> {
    let is_narrow = |run: &Run| run.end - run.start < min_width;
    let mut merged: Vec<Run> = Vec::with_capacity(runs.len());
    // whether the last merged run is narrow or made of narrow runs
    let mut last_narrow = false;
    for run in runs {
        let narrow = is_narrow(&run);
        match merged.last_mut() {
            Some(last) if narrow && last_narrow && last.end == run.start => {
                last.end = run.end;
                last.index = None;
            }
            _ => merged.push(run),
        }
        last_narrow = narrow;
    }
    merged
}

/// The fill color of a band with the value `label`: a tint of a color of the trace
/// palette.
fn state_color(label: &str, is_bool: bool, visuals: &egui::Visuals) -> Color32 {
    let color = match label.parse::<i64>() {
        _ if is_bool && label == "false" => theme::palette_grey(visuals),
        _ if is_bool => theme::palette_color(visuals, 0),
        // consecutive integers, like the states of a state machine, get distinct colors
        Ok(number) => theme::palette_color(visuals, number.rem_euclid(1 << 16) as usize),
        Err(_) => theme::palette_color(visuals, (fnv1a(label) % (1 << 16)) as usize),
    };
    theme::tint(visuals, color)
}

/// A hash of `text` that stays the same across runs, so values keep their colors.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl States {
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws the bands of `samples` in the row centered at `y`.
    fn plot_row(
        &self,
        samples: StateSamples<'_>,
        y: f64,
        range: (f64, f64),
        latest: f64,
        origin: f64,
        plot_ui: &mut egui_plot::PlotUi<'_>,
    ) {
        let visuals = plot_ui.ctx().global_style().visuals.clone();
        let seconds_per_point = plot_ui.transform().dvalue_dpos()[0].abs();
        let (top, bottom) = (y + self.band_height / 2.0, y - self.band_height / 2.0);
        for run in runs(samples, range, latest, MIN_RUN_WIDTH * seconds_per_point) {
            let start = run.start.max(range.0) - origin;
            let end = run.end.min(range.1) - origin;
            let label = run.index.map(|index| samples.label(index));
            let fill = match &label {
                Some(label) => state_color(label, samples.is_bool(), &visuals),
                None => visuals.weak_text_color(),
            };
            let corners = vec![[start, bottom], [end, bottom], [end, top], [start, top]];
            plot_ui.polygon(
                egui_plot::Polygon::new("", corners)
                    .fill_color(fill)
                    .stroke(egui::Stroke::NONE)
                    .allow_hover(false),
            );
            if self.show_values
                && let Some(label) = run.index.map(|index| samples.text(index))
                && (end - start) / seconds_per_point
                    > (label.chars().count() + 1) as f64 * LABEL_CHAR_WIDTH
            {
                let center = egui_plot::PlotPoint::new((start + end) / 2.0, y);
                plot_ui.text(
                    egui_plot::Text::new("", center, label)
                        .color(text_color_on(fill))
                        .allow_hover(false),
                );
            }
        }
    }

    /// Shows the values of the shown traces current at `t` seconds.
    fn tooltip_ui(
        &self,
        ui: &mut egui::Ui,
        model: &Model,
        formats: &Formats,
        t: f64,
        base: TimeBase,
    ) {
        ui.label(base.format(t, 3));
        let names = short_names(self.traces.iter().map(String::as_str));
        egui::Grid::new("states_tooltip")
            .num_columns(2)
            .show(ui, |ui| {
                for (path, name) in self.traces.iter().zip(names) {
                    let Some(samples) = StateSamples::of(model, path, formats.of(path)) else {
                        continue;
                    };
                    if let Some(index) = index_at(samples.timestamps(), t) {
                        let unit = model.traces.unit(path).filter(|_| !samples.is_null(index));
                        ui.label(name);
                        ui.label(with_unit(samples.text(index), unit));
                        ui.end_row();
                    }
                }
            });
    }

    fn show_states(&self, context: &mut ViewContext<'_>, ui: &mut egui::Ui) {
        let (model, formats, time) = (context.model, context.formats, &mut *context.time);
        let names: Vec<String> = short_names(self.traces.iter().map(String::as_str))
            .into_iter()
            .map(str::to_owned)
            .collect();
        let rows = names.len();
        // the first trace is the top row
        let row_y = move |row: usize| (rows - 1 - row) as f64;
        let id = ui.id().with("states");
        let plot = time_axis::plot(id, time, ui.visuals())
            .show_grid([true, false])
            .y_grid_spacer(move |_input| {
                (0..rows)
                    .map(|row| egui_plot::GridMark {
                        value: row_y(row),
                        step_size: 1.0,
                    })
                    .collect()
            })
            .custom_y_axes(vec![
                egui_plot::AxisHints::new_y()
                    .placement(egui_plot::HPlacement::Left)
                    .min_thickness(50.0)
                    .formatter(move |mark, _range| {
                        let row = rows as f64 - 1.0 - mark.value;
                        names
                            .get(row.round() as usize)
                            .filter(|_| row >= 0.0 && row.fract() == 0.0)
                            .cloned()
                            .unwrap_or_default()
                    }),
            ]);

        let latest = time.latest();
        let response = time_axis::show(id, plot, ui, time, false, |plot_ui, axis| {
            plot_ui.set_plot_bounds_y(-0.5..=rows.max(1) as f64 - 0.5);
            let (Some(range), Some(latest)) = (axis.range, latest) else {
                return;
            };
            for (row, path) in self.traces.iter().enumerate() {
                if let Some(samples) = StateSamples::of(model, path, formats.of(path)) {
                    let y = row_y(row);
                    self.plot_row(samples, y, range, latest, axis.origin, plot_ui);
                }
            }
        });

        if let Some(t) = response.hovered
            && !self.traces.is_empty()
        {
            let base = response.axis.base;
            response
                .response
                .on_hover_ui_at_pointer(|ui| self.tooltip_ui(ui, model, formats, t, base));
        }
    }
}

impl Viewable for States {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        self.show_states(context, ui);
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, _context: &PropertiesContext<'_>) {
        super::properties_section(ui, "Bands", |ui| {
            super::property_label(ui, "Height");
            ui.add(egui::Slider::new(&mut self.band_height, 0.2..=1.0))
                .on_hover_text("Height of a band as a fraction of its row");
            ui.end_row();
            super::property_label(ui, "Values");
            ui.checkbox(&mut self.show_values, "")
                .on_hover_text("Label the bands wide enough with their value");
            ui.end_row();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::model::traces::Traces;
    use proto::Data;

    /// Traces of the given samples, one every second.
    fn traces(samples: &[(&str, Data)]) -> Traces {
        let mut traces = Traces::default();
        for (second, (path, value)) in samples.iter().enumerate() {
            traces.append(&proto::Value {
                timestamp: second as u64 * 1_000_000_000,
                path: path.to_string(),
                value: value.clone(),
            });
        }
        traces
    }

    fn model(samples: &[(&str, Data)]) -> Model {
        let mut model = Model::default();
        model.traces = traces(samples);
        model
    }

    fn spans(runs: &[Run]) -> Vec<(f64, f64, Option<usize>)> {
        runs.iter()
            .map(|run| (run.start, run.end, run.index))
            .collect()
    }

    #[test]
    fn equal_values_form_one_run_and_nulls_leave_gaps() {
        let model = model(&[
            ("s", Data::from("idle")),
            ("s", Data::from("idle")),
            ("s", Data::from("run")),
            ("s", Data::Null),
            ("s", Data::from("run")),
        ]);
        let samples = StateSamples::of(&model, "s", DisplayFormat::Default).unwrap();
        let runs = runs(samples, (0.0, 10.0), 6.0, 0.0);
        assert_eq!(
            spans(&runs),
            [
                (0.0, 2.0, Some(0)),
                (2.0, 3.0, Some(2)),
                (4.0, 6.0, Some(4))
            ]
        );
        assert_eq!(samples.label(2), "run");
    }

    #[test]
    fn runs_start_before_the_range() {
        let model = model(&[
            ("b", Data::Bool(true)),
            ("b", Data::Bool(true)),
            ("b", Data::Bool(false)),
            ("b", Data::Bool(false)),
        ]);
        let samples = StateSamples::of(&model, "b", DisplayFormat::Default).unwrap();
        assert!(samples.is_bool());
        let runs = runs(samples, (1.5, 2.5), 3.0, 0.0);
        // the run starts at the sample before the range, which is enough to draw it
        assert_eq!(spans(&runs), [(1.0, 2.0, Some(1)), (2.0, 3.0, Some(2))]);
    }

    #[test]
    fn neighbouring_narrow_runs_are_merged() {
        let run = |start, end, index| Run { start, end, index };
        let merged = merge_narrow(
            vec![
                run(0.0, 5.0, Some(0)),
                run(5.0, 5.1, Some(1)),
                run(5.1, 5.2, Some(2)),
                run(5.2, 9.0, Some(3)),
                run(9.0, 9.1, Some(4)),
            ],
            1.0,
        );
        assert_eq!(
            spans(&merged),
            [
                (0.0, 5.0, Some(0)),
                (5.0, 5.2, None),
                (5.2, 9.0, Some(3)),
                (9.0, 9.1, Some(4))
            ]
        );
    }

    #[test]
    fn integer_states_get_distinct_colors() {
        let colors: BTreeSet<[u8; 4]> = (0..4)
            .map(|state| state_color(&state.to_string(), false, &egui::Visuals::light()).to_array())
            .collect();
        assert_eq!(colors.len(), 4);
        assert_eq!(
            state_color("idle", false, &egui::Visuals::dark()),
            state_color("idle", false, &egui::Visuals::dark())
        );
    }
}
