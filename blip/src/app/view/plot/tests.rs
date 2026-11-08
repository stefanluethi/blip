use super::*;
use crate::app::model::traces::Traces;

/// A float trace `a` of `count` values 1 ms apart, with a null every 7th value if
/// `with_nulls`.
fn traces(count: u64, with_nulls: bool) -> Traces {
    let mut traces = Traces::default();
    for i in 0..count {
        let value = if with_nulls && i % 7 == 3 {
            proto::Data::Null
        } else {
            proto::Data::Float((i % 7) as f64)
        };
        traces.append(&proto::Value {
            timestamp: i * 1_000_000,
            path: "a".to_owned(),
            value,
        });
    }
    traces
}

#[test]
fn downsample_keeps_small_series() {
    let traces = traces(10, false);
    assert_eq!(downsample(traces.series("a").unwrap(), 0, 100).len(), 10);
}

#[test]
fn downsample_limits_points_and_keeps_extremes() {
    let traces = traces(1_000_003, false);
    let points = downsample(traces.series("a").unwrap(), 0, 300);
    assert!(points.len() <= 1_000);
    assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
    assert!(points.iter().any(|p| p[1] == 0.0));
    assert!(points.iter().any(|p| p[1] == 6.0));
}

#[test]
fn downsample_keeps_gaps() {
    let traces = traces(1_000_003, true);
    let points = downsample(traces.series("a").unwrap(), 0, 300);
    assert!(points.len() <= 1_000);
    assert!(points.windows(2).all(|p| p[0][0] <= p[1][0]));
    assert!(points.iter().any(|p| p[1].is_nan()));
    assert!(points.iter().any(|p| p[1] == 6.0));
}

#[test]
fn y_bounds_fit_the_ends_that_are_not_set() {
    let fitted = Some((-1.0, 5.0));
    assert_eq!(y_bounds(None, None, fitted), fitted);
    assert_eq!(y_bounds(Some(0.0), None, fitted), Some((0.0, 5.0)));
    assert_eq!(y_bounds(None, Some(2.0), fitted), Some((-1.0, 2.0)));
    assert_eq!(y_bounds(Some(0.0), Some(1.0), None), Some((0.0, 1.0)));
    assert_eq!(y_bounds(Some(0.0), None, None), None);
    assert_eq!(y_bounds(Some(6.0), None, fitted), None);
}

struct State {
    plot: Plot,
    model: Model,
    time: crate::app::time::TimeRange,
    invoker: crate::app::model::actions::Invoker,
}

/// A harness showing a plot of a trace `a` whose value is the time in seconds from 0
/// to 40 seconds, of which the last 20 seconds are shown.
fn plot_harness() -> egui_kittest::Harness<'static, State> {
    let mut model = Model::default();
    for second in 0..=40 {
        model.traces.append(&proto::Value {
            timestamp: second * 1_000_000_000,
            path: "a".to_owned(),
            value: proto::Data::Float(second as f64),
        });
    }
    let mut plot = Plot::default();
    plot.traces.insert("a".to_owned());
    plot_harness_of(plot, model)
}

/// A harness showing `plot` of `model`.
fn plot_harness_of(plot: Plot, model: Model) -> egui_kittest::Harness<'static, State> {
    let state = State {
        plot,
        model,
        time: Default::default(),
        invoker: Default::default(),
    };
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        // fast enough for double clicks
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(60)
        .build_ui_state(
            |ui, state: &mut State| {
                state
                    .time
                    .update(&state.model, ui.input(|input| input.time));
                let mut context = ViewContext {
                    model: &state.model,
                    time: &mut state.time,
                    invoker: &mut state.invoker,
                    formats: &super::super::NO_FORMATS,
                    titled: false,
                    plots: &super::super::NO_PLOTS,
                };
                state.plot.view(ui, &mut context);
            },
            state,
        )
}

#[test]
fn legend_slots_fit_the_kind_of_the_values() {
    let mut model = Model::default();
    for (second, (int, bool)) in [(-3, true), (120, false)].into_iter().enumerate() {
        let timestamp = second as u64 * 1_000_000_000;
        for (path, value) in [("i", proto::Data::Int(int)), ("b", proto::Data::Bool(bool))] {
            model.traces.append(&proto::Value {
                timestamp,
                path: path.to_owned(),
                value,
            });
        }
    }
    let slot = |path| legend_slot(&model.traces.series(path).unwrap(), DisplayFormat::Default);
    // integers are written whole, as wide as the largest with its sign
    assert_eq!(slot("i"), (None, Some("-888".to_owned())));
    assert_eq!(slot("b"), (None, Some("false".to_owned())));
    let hex = legend_slot(&model.traces.series("i").unwrap(), DisplayFormat::Hex);
    assert_eq!(hex, (None, None));
}

#[test]
fn values_in_the_title_legend_keep_their_place() {
    use egui_kittest::kittest::Queryable;

    // the sign and the number of digits change, the decimals are fixed
    let values = [-0.14, 0.08026, 9.836, 1.5, -3.2];
    let texts = ["-0.140", "0.080", "9.836", "1.500", "-3.200"];
    let mut model = Model::default();
    for (second, value) in values.into_iter().enumerate() {
        model.traces.append(&proto::Value {
            timestamp: second as u64 * 1_000_000_000,
            path: "v".to_owned(),
            value: proto::Data::Float(value),
        });
    }
    let mut plot = Plot::default();
    plot.traces.insert("v".to_owned());
    let state = State {
        plot,
        model,
        time: Default::default(),
        invoker: Default::default(),
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 40.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.time.update(&state.model, 0.0);
                let mut context = ViewContext {
                    model: &state.model,
                    time: &mut state.time,
                    invoker: &mut state.invoker,
                    formats: &super::super::NO_FORMATS,
                    titled: true,
                    plots: &super::super::NO_PLOTS,
                };
                ui.horizontal(|ui| state.plot.title_ui(ui, &mut context));
            },
            state,
        );
    let mut right_edges = vec![];
    for (second, text) in texts.into_iter().enumerate() {
        let state = harness.state_mut();
        state.time.update(&state.model, 0.0);
        let latest = state.time.latest().unwrap();
        state
            .time
            .set_cursor(latest - (values.len() - 1 - second) as f64);
        harness.run();
        right_edges.push(harness.get_by_label(text).rect().max.x);
    }
    assert!(
        right_edges
            .windows(2)
            .all(|pair| (pair[0] - pair[1]).abs() < 0.01),
        "{right_edges:?}"
    );
}

/// The y range of `plot`, the panned or zoomed one or else the fitted one.
fn shown_y(state: &State) -> (f64, f64) {
    state.plot.shown_y(&state.model).unwrap()
}

fn drag(harness: &mut egui_kittest::Harness<'_, State>, from: egui::Pos2, to: egui::Pos2) {
    harness.hover_at(from);
    harness.run();
    harness.drag_at(from);
    harness.run();
    for step in 1..=5 {
        harness.hover_at(from + (to - from) * step as f32 / 5.0);
        harness.run();
    }
    harness.drop_at(to);
    harness.run();
}

#[test]
fn the_y_axis_fits_all_values_and_not_only_the_shown_ones() {
    let mut harness = plot_harness();
    harness.run();
    let (start, _) = harness.state().time.range().unwrap();
    assert!(start > 10.0, "{start}");
    let (low, high) = shown_y(harness.state());
    assert!(low < 0.0 && high > 40.0, "{low} {high}");
    assert!(harness.state().plot.y_view.is_none());
}

#[test]
fn a_vertical_drag_pans_the_y_axis_and_a_double_click_fits_it_again() {
    let mut harness = plot_harness();
    harness.run();
    let range = harness.state().time.range();
    let (low, high) = shown_y(harness.state());

    // a horizontal drag pans the time and leaves the y axis
    drag(
        &mut harness,
        egui::pos2(150.0, 150.0),
        egui::pos2(250.0, 152.0),
    );
    assert_ne!(harness.state().time.range(), range);
    assert!(harness.state().plot.y_view.is_none());

    // a vertical drag pans the y axis and leaves the time
    let range = harness.state().time.range();
    drag(
        &mut harness,
        egui::pos2(150.0, 100.0),
        egui::pos2(152.0, 200.0),
    );
    assert_eq!(harness.state().time.range(), range);
    let (panned_low, panned_high) = harness.state().plot.y_view.unwrap();
    assert!(panned_low > low + 1.0, "{panned_low} {low}");
    assert!(((panned_high - panned_low) - (high - low)).abs() < 1.0e-6);
    // and stays so in later frames
    harness.run();
    assert_eq!(harness.state().plot.y_view, Some((panned_low, panned_high)));

    // without `drop_at`, which takes the pointer away
    let at = egui::pos2(200.0, 150.0);
    harness.hover_at(at);
    for pressed in [true, false, true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert!(harness.state().plot.y_view.is_none());
}

#[test]
fn the_wheel_with_alt_zooms_the_y_axis_only() {
    let mut harness = plot_harness();
    harness.run();
    let range = harness.state().time.range();
    let (low, high) = shown_y(harness.state());

    let at = egui::pos2(200.0, 150.0);
    harness.hover_at(at);
    harness.run();
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::ALT));
    harness.run();
    let ctx = harness.ctx.clone();
    let input = harness.input_mut();
    input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 100.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::ALT,
    });
    super::super::zoom_with_wheel(&ctx, input);
    harness.run_steps(60);

    assert_eq!(harness.state().time.range(), range);
    let (zoomed_low, zoomed_high) = harness.state().plot.y_view.unwrap();
    assert!(
        zoomed_high - zoomed_low < (high - low) * 0.9,
        "{zoomed_low} {zoomed_high}"
    );
}

#[test]
fn stats_of_shown_traces_are_shown_below_the_plot() {
    use egui_kittest::kittest::Queryable;
    use stats::Stat;

    let mut harness = plot_harness();
    let measurement = |trace: &str, stat| Measurement {
        trace: trace.to_owned(),
        stat,
    };
    harness.state_mut().plot.stats = vec![
        measurement("a", Stat::Max),
        measurement("a", Stat::Frequency),
        measurement("b", Stat::Min),
    ];
    harness.run();
    // the last 20 of 40 seconds of a ramp are shown
    assert!(harness.query_by_label("Max(a)").is_some());
    assert!(harness.query_by_label("40").is_some());
    // a ramp has no cycles, and b is not shown
    assert!(harness.query_by_label("Freq(a)").is_some());
    assert!(harness.query_by_label("—").is_some());
    assert!(harness.query_by_label("Min(b)").is_none());
    assert_eq!(harness.state().plot.stats.len(), 3, "stats of b are kept");
}

#[test]
fn stats_are_saved_with_the_plot() {
    let mut plot = Plot::default();
    assert!(!serde_json::to_string(&plot).unwrap().contains("stats"));
    plot.stats.push(Measurement {
        trace: "a".to_owned(),
        stat: stats::Stat::PkPk,
    });
    let json = serde_json::to_string(&plot).unwrap();
    let loaded: Plot = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.stats, plot.stats);
}

#[test]
fn line_styles_of_traces_no_longer_shown_are_forgotten() {
    let mut styles = LineStyles::default();
    styles.set("a", LineStyle::Dashed);
    styles.set("b", LineStyle::Dotted);
    styles.set("c", LineStyle::Solid);
    assert_eq!(styles.0.len(), 2);
    styles.retain_shown(&["a".to_owned()].into());
    assert_eq!(styles.of("a"), LineStyle::Dashed);
    assert_eq!(styles.of("b"), LineStyle::Solid);
    assert_eq!(styles.0.len(), 1);
}

#[test]
fn patterns_of_long_lines_on_screen_become_solid() {
    let transform = egui_plot::PlotTransform::new(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
        egui_plot::PlotBounds::from_min_max([0.0, 0.0], [1.0, 1.0]),
        egui::Vec2b::FALSE,
    );
    let short = [[0.0, 0.0], [1.0, 1.0]];
    assert_eq!(screen_length(&short, &transform), 2f32.sqrt() * 100.0);
    let dashed = LineStyle::Dashed.plot_style(&short, &transform);
    assert!(matches!(dashed, Some(egui_plot::LineStyle::Dashed { .. })));
    // zigzagging from bottom to top, a gap does not count
    let mut long: Vec<[f64; 2]> = (0..1_000).map(|i| [0.0, f64::from(i % 2)]).collect();
    long.push([f64::NAN, 0.0]);
    assert!(screen_length(&long, &transform) > MAX_PATTERN_LENGTH);
    let dotted = LineStyle::Dotted.plot_style(&long, &transform);
    assert_eq!(dotted, Some(egui_plot::LineStyle::Solid));
    assert_eq!(LineStyle::None.plot_style(&short, &transform), None);
}

#[test]
fn steps_hold_each_value() {
    let stepped = steps(&[[0.0, 1.0], [1.0, 0.0], [3.0, 1.0]]);
    assert_eq!(
        stepped,
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [3.0, 0.0], [3.0, 1.0]]
    );
}

/// `a` is 1.5 V every second, `b` 2 until 20 s and null from 21 s, `c` never received.
fn tooltip_model() -> (Plot, Model) {
    let mut model = Model::default();
    for second in 0..=40u64 {
        let timestamp = second * 1_000_000_000;
        let path = |path: &str| path.to_owned();
        model.traces.append(&proto::Value {
            timestamp,
            path: path("a"),
            value: proto::Data::Float(1.5),
        });
        let value = if second <= 20 {
            proto::Data::Float(2.0)
        } else {
            proto::Data::Null
        };
        model.traces.append(&proto::Value {
            timestamp,
            path: path("b"),
            value,
        });
    }
    model.traces.set_unit("a", "V");
    let mut plot = Plot::default();
    plot.traces.extend(["a", "b", "c"].map(str::to_owned));
    (plot, model)
}

#[test]
fn the_tooltip_has_the_time_and_a_row_per_trace() {
    use super::super::tooltip::{Header, Label, Row, TraceName, Value};
    let (plot, model) = tooltip_model();
    let tooltip = plot.tooltip(
        &model,
        &super::super::NO_FORMATS,
        30_000_000_000,
        TimeBase::SinceBoot,
    );
    let label = |name: &str, index| {
        Label::Trace(TraceName {
            name: name.into(),
            path: name.into(),
            index,
        })
    };
    assert_eq!(tooltip.header, Some(Header::Text("0:00:30.000".into())));
    assert_eq!(
        tooltip.groups,
        [vec![
            Row::new(label("a", 0), [Value::text("1.5 V")]),
            Row::new(label("b", 1), [Value::faint("null")]),
            Row::new(label("c", 2), [Value::missing()]),
        ]]
    );
}

#[test]
fn hovering_the_plot_shows_the_tooltip() {
    let (plot, model) = tooltip_model();
    let mut harness = plot_harness_of(plot, model);
    harness.run();
    harness.hover_at(egui::pos2(200.0, 150.0));
    harness.run_steps(40);
    let texts = super::super::tooltip::shown_texts(&harness);
    let find = |text: &str| {
        texts
            .iter()
            .find(|(shown, _)| shown == text)
            .unwrap_or_else(|| panic!("{text} in {texts:?}"))
            .1
    };
    for name in ["a", "b", "c"] {
        find(name);
    }
    assert!(texts[0].0.starts_with("0:00:"), "{texts:?}");
    let right = find("1.5 V").right();
    assert_eq!(find("null").right(), right);
    assert_eq!(find("—").right(), right);
}
