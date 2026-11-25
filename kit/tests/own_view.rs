//! A view type of an application, written against the public API only: it reads the
//! store, the time range and the theme as the views of the library do, is shown on its
//! own, and is added to the layout of the workbench, which saves it.

use blip_kit::store::actions::Invoker;
use blip_kit::store::session::Session;
use blip_kit::store::traces::Trace;
use blip_kit::store::{MemoryStore, Store as _, Window};
use blip_kit::widgets::theme;
use blip_kit::widgets::time::TimeRange;
use blip_kit::widgets::view::custom;
use blip_kit::widgets::view::plot::Plot;
use blip_kit::widgets::view::{self, Selectable, ViewContext, ViewInfo, ViewType, Viewable};
use blip_kit::workbench::layout::Layout;
use egui_kittest::kittest::Queryable as _;
use std::collections::BTreeSet;

/// Shows the value of each of its traces current at the cursor as a line of text.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
struct Dial {
    traces: BTreeSet<String>,
}

impl Viewable for Dial {
    fn view(&mut self, ui: &mut egui::Ui, context: &mut ViewContext<'_>) {
        if self.traces.is_empty() {
            theme::hint(ui, "No traces to show");
        }
        let Some(t) = context.time.cursor_timestamp() else {
            return;
        };
        for path in &self.traces {
            let Some(series) = context.store.series(path, Window::at(t)).data else {
                continue;
            };
            let Some(index) = series.index_at(t) else {
                continue;
            };
            let unit = context.store.unit(path).unwrap_or_default();
            ui.label(format!("{path}: {} {unit}", series.value(index)));
        }
    }

    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        Some((&mut self.traces, false))
    }
}

impl ViewInfo for Dial {
    const NAME: &'static str = "Dial";
    const ICON: &'static str = "";

    fn selectable(_trace: Trace<'_>) -> Selectable {
        Selectable::Trace
    }
}

fn store() -> MemoryStore {
    let mut session = Session::recording();
    let messages = [
        proto::Message::TraceMetadata(proto::TraceMetadata {
            path: "tank/level".to_owned(),
            unit: "l".to_owned(),
            ..Default::default()
        }),
        proto::Message::Sample(proto::Sample {
            timestamp: 1_000_000_000,
            path: "tank/level".to_owned(),
            value: proto::Data::Float(42.0),
        }),
        proto::Message::Sample(proto::Sample {
            timestamp: 3_000_000_000,
            path: "tank/level".to_owned(),
            value: proto::Data::Float(50.0),
        }),
    ];
    for message in messages {
        session.inbox("target", 0.0).message(&message);
    }
    session.store
}

type Shown = (Dial, Plot, MemoryStore, TimeRange, Invoker);

#[test]
fn a_view_of_the_application_follows_the_time_of_the_plot_beside_it() {
    let dial: Dial = view::showing(["tank/level"]);
    let plot: Plot = view::showing(["tank/level"]);
    let state = (
        dial,
        plot,
        store(),
        TimeRange::default(),
        Invoker::default(),
    );
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, (dial, plot, store, time, invoker): &mut Shown| {
                theme::install(ui.ctx());
                time.update(store, ui.input(|input| input.time));
                dial.view(ui, &mut ViewContext::new(store, time, invoker));
                plot.view(ui, &mut ViewContext::new(store, time, invoker));
            },
            state,
        );
    harness.run();
    // live, it shows the newest value
    harness.get_by_label("tank/level: 50 l");

    // the cursor, which a click in the plot sets too, is the time of both
    harness.state_mut().3.set_cursor(0.5);
    harness.run();
    harness.get_by_label("tank/level: 42 l");
}

#[test]
fn a_registered_view_type_is_kept_by_the_layout() {
    custom::register::<Dial>();
    let dial = ViewType::all()
        .into_iter()
        .find(|view_type| view_type.name() == "Dial")
        .expect("the registered type");

    let mut layout = Layout::default();
    layout.add_view(dial);
    let saved = serde_json::to_value(layout.to_saved()).expect("a saved layout");
    let text = saved.to_string();
    assert!(
        text.contains(r#""Custom":{"state":{"traces":[]},"type":"Dial"}"#),
        "{text}"
    );

    // loaded again, it is saved the same
    let loaded = Layout::from_saved(serde_json::from_value(saved.clone()).expect("a layout"));
    let again = serde_json::to_value(loaded.to_saved()).expect("a saved layout");
    assert_eq!(again, saved);
}

#[test]
fn the_lookups_of_a_series_and_a_trace_find_samples_by_time() {
    let store = store();
    let series = store
        .series("tank/level", Window::at(2_000_000_000))
        .data
        .expect("a series");
    assert_eq!(series.index_at(2_000_000_000), Some(0));
    assert_eq!(series.index_at(500_000_000), None);
    assert_eq!(series.index_at(3_000_000_000), Some(1));
    assert_eq!(series.closest_index(2_600_000_000), Some(1));
    assert_eq!(series.closest_index(1_900_000_000), Some(0));
    assert_eq!(series.within((0, 2_000_000_000)), 0..1);
    assert_eq!(series.within((1_000_000_000, 3_000_000_000)), 0..2);

    let trace = store
        .trace("tank/level", Window::at(2_000_000_000))
        .data
        .expect("a trace");
    assert_eq!(trace.index_at(2_000_000_000), Some(0));
    assert_eq!(trace.index_at(500_000_000), None);
    assert_eq!(trace.index_at(3_000_000_000), Some(1));
    assert_eq!(trace.closest_index(2_600_000_000), Some(1));
    assert_eq!(trace.closest_index(1_900_000_000), Some(0));
    assert_eq!(trace.within((0, 2_000_000_000)), 0..1);
    assert_eq!(trace.within((1_000_000_000, 3_000_000_000)), 0..2);
}
