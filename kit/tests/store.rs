//! The widgets read through the [`Store`] trait only: a store that is not the
//! in-memory one feeds a plot, and sees what the plot asks for.

use blip_kit::store::actions::{Actions, Invoker};
use blip_kit::store::logs::Logs;
use blip_kit::store::numbers::Labels;
use blip_kit::store::time_base::TimeBase;
use blip_kit::store::traces::{Series, Trace};
use blip_kit::store::{Activity, Loaded, MemoryStore, Store, Version, Window};
use blip_kit::widgets::time::TimeRange;
use blip_kit::widgets::view::plot::Plot;
use blip_kit::widgets::view::{ViewContext, Viewable as _};
use std::cell::RefCell;

/// A store that loads: it keeps what it loaded so far in an in-memory store, notes
/// the windows it is asked for and says that more is on its way.
#[derive(Default)]
struct Loading {
    loaded: MemoryStore,
    asked: RefCell<Vec<(String, Window)>>,
}

impl Store for Loading {
    fn paths_version(&self) -> Version {
        self.loaded.paths_version()
    }
    fn samples_version(&self) -> Version {
        self.loaded.samples_version()
    }
    fn pending(&self) -> bool {
        true
    }
    fn origin(&self) -> Option<u64> {
        self.loaded.origin()
    }
    fn time_base(&self) -> Option<TimeBase> {
        self.loaded.time_base()
    }
    fn time_span(&self) -> Option<(u64, u64)> {
        self.loaded.time_span()
    }
    fn activity(&self, start: u64, step: u64, bins: usize) -> Vec<Activity> {
        self.loaded.activity(start, step, bins)
    }
    fn traces(&self) -> Box<dyn Iterator<Item = Trace<'_>> + '_> {
        Store::traces(&self.loaded)
    }
    fn kinds(&self) -> u64 {
        self.loaded.kinds()
    }
    fn trace(&self, path: &str, window: Window) -> Loaded<Option<Trace<'_>>> {
        self.asked.borrow_mut().push((path.to_owned(), window));
        Loaded::partial(self.loaded.trace(path, window).data)
    }
    fn series(&self, path: &str, window: Window) -> Loaded<Option<Series<'_>>> {
        self.asked.borrow_mut().push((path.to_owned(), window));
        Loaded::partial(self.loaded.series(path, window).data)
    }
    fn field_paths<'a>(&'a self, path: &str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        self.loaded.field_paths(path)
    }
    fn metadata(&self, path: &str) -> Option<&proto::TraceMetadata> {
        self.loaded.metadata(path)
    }
    fn unit(&self, path: &str) -> Option<&str> {
        self.loaded.unit(path)
    }
    fn description(&self, path: &str) -> Option<&str> {
        self.loaded.description(path)
    }
    fn labels(&self, path: &str) -> Option<&Labels> {
        self.loaded.labels(path)
    }
    fn expected(&self, path: &str) -> Option<(f64, f64)> {
        self.loaded.expected(path)
    }
    fn logs(&self, window: Window) -> Loaded<&Logs> {
        Loaded::partial(self.loaded.logs(window).data)
    }
    fn actions(&self) -> &Actions {
        self.loaded.actions()
    }
    fn readback(&self, readback: &str) -> Option<Trace<'_>> {
        self.loaded.readback(readback)
    }
    fn readback_samples(&self, readback: Option<&str>) -> usize {
        self.loaded.readback_samples(readback)
    }
}

struct State {
    plot: Plot,
    store: Loading,
    time: TimeRange,
    invoker: Invoker,
}

#[test]
fn a_plot_reads_another_store_and_asks_for_what_it_shows() {
    let mut store = Loading::default();
    for second in 0..=40u64 {
        let sample = proto::Message::Sample(proto::Sample {
            timestamp: second * 1_000_000_000,
            path: "a".to_owned(),
            value: proto::Data::Float(second as f64),
        });
        let frame = serde_cbor_2::to_vec(&sample).expect("a frame");
        store.loaded.receive("target", &frame);
    }
    assert!(store.pending());
    let mut plot = Plot::default();
    plot.selection()
        .expect("a plot shows traces")
        .0
        .insert("a".to_owned());
    let state = State {
        plot,
        store,
        time: TimeRange::default(),
        invoker: Invoker::default(),
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state
                    .time
                    .update(&state.store, ui.input(|input| input.time));
                let mut context =
                    ViewContext::new(&state.store, &mut state.time, &mut state.invoker);
                state.plot.view(ui, &mut context);
            },
            state,
        );
    harness.run();

    let state = harness.state();
    let shown = state.time.timestamps().expect("data to show");
    let asked = state.store.asked.borrow();
    // the plot asked for the samples of the shown time range, as fine as it is wide
    let drawn = asked
        .iter()
        .find(|(path, window)| path == "a" && window.range == Some(shown))
        .expect("the plot asks for the shown range");
    assert!(drawn.1.max_points >= 2 && drawn.1.max_points <= 2 * 400);
    // and never for all samples of the trace
    assert!(asked.iter().all(|(_, window)| *window != Window::ALL));
}
