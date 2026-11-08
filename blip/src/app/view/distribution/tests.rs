use super::*;
use crate::app::model::actions::Invoker;
use crate::app::view::stats::Stats;
use crate::app::view::{NO_PLOTS, PlotInfo, ViewType};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use proto::Data;

/// Appends the `values` of the trace at `path` to `model`, one every `step`
/// seconds from `start` seconds.
fn append(model: &mut Model, path: &str, (start, step): (f64, f64), values: &[Data]) {
    for (index, value) in values.iter().enumerate() {
        let seconds = start + index as f64 * step;
        model.traces.append(&proto::Value {
            timestamp: (seconds * 1.0e9).round() as u64,
            path: path.to_owned(),
            value: value.clone(),
        });
    }
}

fn floats(values: impl IntoIterator<Item = f64>) -> Vec<Data> {
    values.into_iter().map(Data::Float).collect()
}

/// A distribution of the traces at `paths`.
fn distribution_of(paths: &[&str]) -> Distribution {
    Distribution {
        traces: paths.iter().map(|&path| path.to_owned()).collect(),
        ..Distribution::default()
    }
}

/// The time range of `model` shown live.
fn live(model: &Model) -> TimeRange {
    let mut time = TimeRange::default();
    time.update(model, 0.0);
    time
}

/// What `distribution` counts of `model` in the window of `time`, on a value axis
/// 1000 points long.
fn counted(distribution: &Distribution, model: &Model, time: &TimeRange) -> Counted {
    let counting = distribution.counting(model, time, &NO_PLOTS, 1000.0);
    counting.counted.expect("samples in the window")
}

/// The samples of the trace at `path` in the window of `time` that are not null.
fn samples_in_window(model: &Model, time: &TimeRange, path: &str) -> u64 {
    let (start, end) = time.range().unwrap();
    let (start, end) = (time.timestamp(start), time.timestamp(end));
    let series = model.traces.series(path).unwrap();
    (0..series.len())
        .filter(|&index| (start..=end).contains(&series.timestamps[index]))
        .filter(|&index| !series.value(index).is_nan())
        .count() as u64
}

fn total(counts: &Counts) -> u64 {
    counts.bins.iter().sum()
}

/// Whether `value` is 1, 2 or 5 times a power of ten.
fn is_round(value: f64) -> bool {
    let mantissa = value / 10f64.powf(value.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .iter()
        .any(|round| (mantissa - round).abs() < 1.0e-9)
}

#[test]
fn counts_add_up_to_the_samples_of_the_shown_window() {
    let values = (0..=40).map(|second| match second % 7 {
        3 => Data::Null,
        _ => Data::Float(f64::from(second % 5)),
    });
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 1.0), &values.collect::<Vec<_>>());
    let distribution = distribution_of(&["a"]);
    let mut time = live(&model);
    let check = |model: &Model, time: &TimeRange| {
        let counted = counted(&distribution, model, time);
        assert_eq!(
            total(&counted.traces[0]),
            samples_in_window(model, time, "a")
        );
        total(&counted.traces[0])
    };
    // the last 20 s, live, without the nulls
    let shown = check(&model, &time);
    assert_eq!(shown, 21 - 3);
    // panned and zoomed
    time.pan_zoom(5.0, 15.0);
    assert!(check(&model, &time) < shown);
    time.pan_zoom(0.0, 40.0);
    assert_eq!(check(&model, &time), 41 - 6);
    // live, as more samples arrive
    time.go_live();
    time.pan_zoom(20.0, 40.0);
    append(&mut model, "a", (41.0, 1.0), &floats([1.0; 10]));
    time.update(&model, 0.0);
    assert_eq!(time.range(), Some((30.0, 50.0)));
    check(&model, &time);
}

#[test]
fn traces_share_one_set_of_bins_and_nulls_are_not_counted() {
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 0.5), &floats((0..40).map(f64::from)));
    let mut b = floats((0..40).map(|i| f64::from(i) + 7.5));
    b[3] = Data::Null;
    append(&mut model, "b", (0.0, 0.5), &b);
    let distribution = distribution_of(&["a", "b"]);
    let time = live(&model);
    let counted = counted(&distribution, &model, &time);
    assert_eq!(counted.traces.len(), 2);
    for (path, counts) in ["a", "b"].into_iter().zip(&counted.traces) {
        assert_eq!(counts.bins.len(), counted.bins.count);
        assert_eq!(total(counts), samples_in_window(&model, &time, path));
    }
    assert_eq!(total(&counted.traces[1]), 39);
}

#[test]
fn numbers_and_elements_of_small_arrays_are_checked_but_not_bools_or_strings() {
    let mut model = Model::default();
    let array = |len: usize, value: Data| Data::Array(vec![value; len]);
    for (path, value) in [
        ("int", Data::Int(-3)),
        ("float", Data::Float(1.5)),
        ("bool", Data::Bool(true)),
        ("text", Data::from("on")),
        ("small", array(3, Data::Float(1.0))),
        ("large", array(MAX_BINS, Data::Float(1.0))),
        ("bools", array(3, Data::Bool(false))),
    ] {
        append(&mut model, path, (0.0, 1.0), &[value]);
    }
    let selectable = |path| ViewType::Distribution.selectable(model.traces.get(path).unwrap());
    assert!(matches!(selectable("int"), Selectable::Trace));
    assert!(matches!(selectable("float"), Selectable::Trace));
    assert!(matches!(selectable("small"), Selectable::Elements));
    for path in ["bool", "text", "large", "bools"] {
        assert!(matches!(selectable(path), Selectable::No(_)), "{path}");
    }
}

struct State {
    distribution: Distribution,
    model: Model,
    time: TimeRange,
    invoker: Invoker,
    plots: Plots,
    titled: bool,
    formats: Formats,
}

impl State {
    fn new(distribution: Distribution, model: Model) -> Self {
        Self {
            distribution,
            model,
            time: TimeRange::default(),
            invoker: Invoker::default(),
            plots: Plots::default(),
            titled: false,
            formats: Formats::default(),
        }
    }
}

/// A harness showing the distribution of `state`, or its title row with `title`.
fn harness(state: State, title: bool) -> Harness<'static, State> {
    sized_harness(state, title, egui::vec2(400.0, 300.0))
}

/// A harness of `size` showing the distribution of `state`, or its title row with
/// `title`.
fn sized_harness(state: State, title: bool, size: egui::Vec2) -> Harness<'static, State> {
    Harness::builder()
        .with_size(size)
        // fast enough for double clicks
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(60)
        .build_ui_state(
            move |ui, state: &mut State| {
                state
                    .time
                    .update(&state.model, ui.input(|input| input.time));
                let mut context = ViewContext {
                    model: &state.model,
                    time: &mut state.time,
                    invoker: &mut state.invoker,
                    formats: &state.formats,
                    titled: state.titled,
                    plots: &state.plots,
                };
                if title {
                    ui.horizontal(|ui| state.distribution.title_ui(ui, &mut context));
                } else {
                    state.distribution.view(ui, &mut context);
                }
            },
            state,
        )
}

/// A model of the traces `x` and `y` in `x_unit` and `y_unit`.
fn pose(x_unit: &str, y_unit: &str) -> Model {
    let mut model = Model::default();
    append(
        &mut model,
        "pose.x",
        (0.0, 0.1),
        &floats((0..200).map(|i| (f64::from(i) * 0.37) % 10.0)),
    );
    append(
        &mut model,
        "pose.y",
        (0.0, 0.1),
        &floats((0..200).map(|i| (f64::from(i) * 0.53) % 6.0)),
    );
    model.traces.set_unit("pose.x", x_unit);
    model.traces.set_unit("pose.y", y_unit);
    model
}

#[test]
fn the_legend_names_the_traces_in_the_title_row_or_in_a_corner() {
    let distribution = distribution_of(&["pose.x", "pose.y"]);
    // in the title row, with the unit both share and without values
    let mut harness = harness(State::new(distribution.clone(), pose("m", "m")), true);
    harness.run();
    harness.get_by_label("x");
    harness.get_by_label("y");
    harness.get_by_label("m");
    assert_eq!(
        harness
            .get_all_by_role(egui::accesskit::Role::Label)
            .count(),
        3
    );
    let mut harness = self::harness(State::new(distribution.clone(), pose("m", "rad")), true);
    harness.run();
    assert!(harness.query_by_label("m").is_none());

    // in a corner without a title row, not in the view with one
    let mut harness = self::harness(State::new(distribution.clone(), pose("m", "m")), false);
    harness.run();
    let x = harness.get_by_label("x").rect();
    assert!(x.center().x > 300.0 && x.center().y < 100.0, "{x:?}");
    harness.get_by_label("y");
    let mut state = State::new(distribution, pose("m", "m"));
    state.titled = true;
    let mut harness = self::harness(state, false);
    harness.run();
    assert!(harness.query_by_label("x").is_none());
}

#[test]
fn values_are_written_in_the_format_the_traces_share() {
    let mut formats = Formats::default();
    formats.set("a", DisplayFormat::Hex);
    formats.set("b", DisplayFormat::Hex);
    assert_eq!(common_format(&formats, ["a", "b"]), DisplayFormat::Hex);
    assert_eq!(common_format(&formats, ["a", "c"]), DisplayFormat::Default);
    assert_eq!(write_value(16.0, 1.0, DisplayFormat::Hex), "0x10");
    assert_eq!(write_value(16.0, 1.0, DisplayFormat::Default), "16");
    // as many decimals as the step takes
    assert_eq!(write_value(0.5, 0.25, DisplayFormat::Default), "0.50");
    assert_eq!(write_value(-0.0001, 0.1, DisplayFormat::Default), "0.0");
    assert_eq!(write_value(2400.0, 50.0, DisplayFormat::Default), "2400");
}

#[test]
fn bins_show_the_unit_only_if_all_traces_share_one() {
    let counted = Counted {
        bins: Bins {
            start: 0.0,
            width: 0.25,
            count: 4,
        },
        traces: vec![Counts {
            bins: vec![0, 1, 2, 3],
            ..Counts::default()
        }],
        single: false,
    };
    let bin = |unit| Bin {
        index: 2,
        counted: &counted,
        unit,
        format: DisplayFormat::Default,
    };
    assert_eq!(bin(Some("m")).range_text(), "0.50 to 0.75 m");
    assert_eq!(bin(None).range_text(), "0.50 to 0.75");
    let model = pose("m", "rad");
    assert_eq!(super::super::common_unit(&model, ["pose.x"]), Some("m"));
    assert_eq!(
        super::super::common_unit(&model, ["pose.x", "pose.y"]),
        None
    );
}

#[test]
fn without_traces_or_samples_the_view_says_so() {
    let mut harness = harness(State::new(Distribution::default(), Model::default()), false);
    harness.run();
    harness.get_by_label(CHECK_TRACES);

    // only nulls in the shown window
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 1.0), &floats([1.0, 2.0]));
    append(&mut model, "a", (2.0, 1.0), &vec![Data::Null; 30]);
    let mut harness = self::harness(State::new(distribution_of(&["a"]), model), false);
    harness.run();
    harness.get_by_label(NO_SAMPLES);
    assert!(harness.state().distribution.last.is_none());
}

#[test]
fn samples_of_one_value_are_one_bar_centred_on_it() {
    let mut model = Model::default();
    append(&mut model, "t", (0.0, 1.0), &floats([23.4; 30]));
    append(&mut model, "n", (0.0, 1.0), &vec![Data::Int(5); 30]);
    let time = live(&model);
    let counted = counted(&distribution_of(&["t"]), &model, &time);
    assert!(counted.single);
    assert_eq!(counted.bins.count, 1);
    assert!((counted.bins.center(0) - 23.4).abs() < 1.0e-9);
    // a round step near 1 % of the value
    assert_eq!(counted.bins.width, 0.2);
    let bin = Bin {
        index: 0,
        counted: &counted,
        unit: Some("°C"),
        format: DisplayFormat::Default,
    };
    assert_eq!(bin.range_text(), "All 21 samples are 23.4 °C");
    let counted = self::counted(&distribution_of(&["n"]), &model, &time);
    assert_eq!((counted.bins.start, counted.bins.width), (4.5, 1.0));
}

#[test]
fn automatic_bins_number_about_the_square_root_of_the_samples() {
    let distribution = Distribution::default();
    for samples in [4, 100, 400, 10_000, 1_000_000] {
        let bins = distribution.make_bins((0.0, 1.0), false, samples, 0.0, false);
        let target = (samples as f64).sqrt().clamp(MIN_AUTO_BINS, MAX_AUTO_BINS);
        let count = bins.count as f64;
        assert!(
            count >= target / 1.75 && count <= target * 1.75 + 1.0,
            "{samples}: {bins:?}"
        );
        assert!(is_round(bins.width), "{bins:?}");
    }
    // few and many samples
    let few = distribution.make_bins((0.0, 1.0), false, 1, 0.0, false);
    assert_eq!(few.width, 0.1);
    let many = distribution.make_bins((0.0, 1.0), false, 100_000_000, 0.0, false);
    assert_eq!(many.width, 0.02);
    // at least a few points wide, and integers at least 1 wide
    let wide = distribution.make_bins((0.0, 1.0), false, 1_000_000, 0.04, false);
    assert_eq!(wide.width, 0.05);
    let integers = distribution.make_bins((0.0, 5.0), false, 1000, 0.0, true);
    assert_eq!((integers.width, integers.count), (1.0, 6));
}

#[test]
fn the_automatic_width_changes_only_clearly_past_a_step() {
    // 400 samples, 20 bins: the step between 0.1 and 0.2 lies at a span of 3
    let mut distribution = Distribution::default();
    let bins = |distribution: &Distribution, span: f64| {
        distribution.make_bins((0.0, span), false, 400, 0.0, false)
    };
    let shown = |distribution: &mut Distribution, span: f64| {
        let bins = bins(distribution, span);
        distribution.last = Some(Box::new(Last {
            data: (0.0, span),
            values: (0.0, span),
            bins,
            frame: egui::Rect::NOTHING,
            rows: None,
            names: vec![],
        }));
        bins.width
    };
    // without a width before, the nearest round one
    assert_eq!(bins(&distribution, 2.9).width, 0.1);
    assert_eq!(bins(&distribution, 3.1).width, 0.2);
    // around the step, the width stays
    assert_eq!(shown(&mut distribution, 2.9), 0.1);
    for span in [3.1, 2.95, 3.3, 3.5, 2.9, 3.5] {
        assert_eq!(shown(&mut distribution, span), 0.1, "{span}");
    }
    // clearly past it, it changes, and stays when it goes back a little
    assert_eq!(shown(&mut distribution, 3.7), 0.2);
    for span in [2.9, 2.6, 3.2] {
        assert_eq!(shown(&mut distribution, span), 0.2, "{span}");
    }
    assert_eq!(shown(&mut distribution, 2.4), 0.1);
    // a width that is not round, e.g. of a fixed number of bins, is not kept
    distribution.last.as_mut().unwrap().bins.width = 0.13;
    assert_eq!(bins(&distribution, 2.6).width, 0.1);
    // nor one too narrow
    distribution.last.as_mut().unwrap().bins.width = 0.1;
    let wide = distribution.make_bins((0.0, 3.2), false, 400, 0.15, false);
    assert_eq!(wide.width, 0.2);
}

#[test]
fn the_value_axis_fits_the_values_with_a_margin() {
    assert_eq!(fitted_values(None, Some((0.0, 10.0)), None), (-0.5, 10.5));
    assert_eq!(
        fitted_values(None, Some((0.0, 10.0)), Some((2.0, 3.0))),
        (2.0, 3.0)
    );
    assert_eq!(fitted_values(None, None, None), (0.0, 1.0));
}

#[test]
fn bin_edges_are_multiples_of_the_width_and_stay_as_the_window_moves() {
    let distribution = Distribution::default();
    let first = distribution.make_bins((0.13, 2.73), false, 400, 0.0, false);
    for step in 0..20 {
        let low = 0.13 + f64::from(step) * 0.07;
        let bins = distribution.make_bins((low, low + 2.6), false, 400, 0.0, false);
        assert_eq!(bins.width, first.width);
        let multiple = bins.start / bins.width;
        assert!((multiple - multiple.round()).abs() < 1.0e-9, "{bins:?}");
        assert!(bins.start <= low && bins.edge(bins.count) > low + 2.6);
    }
}

#[test]
fn a_fixed_number_or_width_of_bins_is_kept() {
    let number = Distribution {
        bins: BinMode::Number,
        bin_count: 7,
        ..Distribution::default()
    };
    let bins = number.make_bins((1.0, 8.0), false, 1000, 0.5, false);
    assert_eq!(bins.count, 7);
    assert_eq!((bins.start, bins.edge(7)), (1.0, 8.0));
    let width = Distribution {
        bins: BinMode::Width,
        bin_width: 0.25,
        ..Distribution::default()
    };
    let bins = width.make_bins((1.1, 3.0), false, 1000, 0.5, false);
    assert_eq!((bins.start, bins.width, bins.count), (1.0, 0.25, 9));
    // a value at the end of a fixed range is in the last bin
    let bins = width.make_bins((1.0, 3.0), true, 1000, 0.5, false);
    assert_eq!(bins.count, 8);
    assert_eq!(bins.index(3.0), 7);
}

#[test]
fn samples_outside_a_fixed_range_are_counted_below_and_above_it() {
    let bins = Bins {
        start: 2.0,
        width: 1.0,
        count: 3,
    };
    let values = [0.0, 1.9, 2.0, 2.5, f64::NAN, 4.0, 5.0, 5.5, 9.0];
    let counts = count(values.into_iter(), &bins, Some((2.0, 5.0)));
    assert_eq!(counts.bins, [2, 0, 2]);
    assert_eq!((counts.below, counts.above), (2, 2));
    // the mean is of the binned samples
    assert_eq!(counts.mean, Some(3.375));
}

#[test]
fn the_counts_axis_fits_the_tallest_bin_in_view() {
    let counted = Counted {
        bins: Bins {
            start: 0.0,
            width: 1.0,
            count: 4,
        },
        traces: vec![
            Counts {
                bins: vec![1, 9, 3, 2],
                ..Counts::default()
            },
            Counts {
                bins: vec![0, 2, 1, 4],
                ..Counts::default()
            },
        ],
        single: false,
    };
    // the shares of the samples of each trace, 9 of 15 and 4 of 7
    assert_eq!(counted.tallest_in((-0.5, 4.5)), 60.0);
    // zoomed in on the values
    assert_eq!(counted.tallest_in((2.2, 4.5)), 400.0 / 7.0);
    // without samples in view, the share of one sample of the most samples
    assert_eq!(counted.one_sample(), 100.0 / 15.0);
    assert_eq!(counted.tallest_in((10.0, 20.0)), 100.0 / 15.0);
    // it grows at once and shrinks only well below its top
    assert_eq!(fit_top(None, 60.0), 60.0);
    assert_eq!(fit_top(Some(60.0), 80.0), 80.0);
    assert_eq!(fit_top(Some(60.0), 45.0), 60.0);
    assert_eq!(fit_top(Some(60.0), 20.0), 20.0);
    // the bars reach the top of the axis but for some room and the marker strip
    let axis = CountAxis::new(CountScale::Linear, 60.0, 100.0 / 15.0, 200.0, 0.0);
    assert_eq!(axis.floor, 0.0);
    assert!(axis.bars_top > 60.0 && axis.bars_top < 66.0);
    assert_eq!(axis.axis_top, axis.bars_top);
    let strip = CountAxis::new(CountScale::Linear, 60.0, 100.0 / 15.0, 200.0, 50.0);
    assert!((strip.axis_top - strip.bars_top * 200.0 / 150.0).abs() < 1.0e-9);
}

#[test]
fn the_log_counts_axis_marks_powers_of_ten_from_below_one_sample() {
    // one sample of 1000 is 0.1 %, half a decade above the floor
    let axis = CountAxis::new(CountScale::Log, 100.0, 0.1, 200.0, 0.0);
    assert_eq!(axis.floor, -1.5);
    let marks = axis.marks()(egui_plot::GridInput {
        bounds: (axis.floor, axis.axis_top),
        base_step_size: 0.01,
    });
    let labels: Vec<String> = (marks.iter())
        .map(|mark| axis.label(mark.value, mark.step_size))
        .collect();
    assert_eq!(labels, ["0.1 %", "1 %", "10 %", "100 %"]);
    // a bin of one sample is a short bar, an empty one shows nothing
    assert_eq!(axis.position(0.1), Some(-1.0));
    assert!(axis.position(0.1).unwrap() > axis.floor);
    assert_eq!(axis.position(0.0), None);
    assert_eq!(axis.position(10.0), Some(1.0));
    assert_eq!(decade_label(-3), "0.001 %");
    // the floor is rounded down to half a decade, and not marked: one sample of
    // 2000 is 0.05 %
    let axis = CountAxis::new(CountScale::Log, 100.0, 0.05, 200.0, 0.0);
    assert_eq!(axis.floor, -2.0);
    let marks = axis.marks()(egui_plot::GridInput {
        bounds: (axis.floor, axis.axis_top),
        base_step_size: 0.01,
    });
    assert_eq!(marks.first().map(|mark| mark.value), Some(-1.0));
    // the end of the value axis is labelled at the floor, not the counts axis
    let linear = CountAxis::new(CountScale::Linear, 40.0, 1.0, 200.0, 0.0);
    assert_eq!(linear.label(0.0, 10.0), "");
    assert_eq!(linear.label(10.0, 10.0), "10 %");
    assert_eq!(linear.label(2.5, 0.5), "2.5 %");
    // the linear axis is marked up to the bars, also in fractions of a percent
    for (axis, base_step_size) in [
        (linear, 0.5),
        (
            CountAxis::new(CountScale::Linear, 2.0, 0.01, 200.0, 0.0),
            0.05,
        ),
    ] {
        let marks = axis.marks()(egui_plot::GridInput {
            bounds: (0.0, axis.axis_top),
            base_step_size,
        });
        assert!(marks.len() > 2);
        assert!(marks.iter().all(|mark| mark.value <= axis.bars_top));
        assert!(
            marks
                .iter()
                .any(|mark| mark.value > 0.0 && mark.value < 1.0)
                == (base_step_size < 0.1)
        );
    }
}

#[test]
fn the_tooltip_shows_the_share_and_the_count_of_each_trace() {
    let counted = Counted {
        bins: Bins {
            start: 0.0,
            width: 1.0,
            count: 3,
        },
        traces: vec![
            Counts {
                bins: vec![1, 2, 3],
                ..Counts::default()
            },
            // samples outside the bins count towards all samples
            Counts {
                bins: vec![0, 2, 0],
                below: 3,
                above: 3,
                ..Counts::default()
            },
        ],
        single: false,
    };
    let bin = |index| Bin {
        index,
        counted: &counted,
        unit: None,
        format: DisplayFormat::Default,
    };
    let [first, second] = [&counted.traces[0], &counted.traces[1]];
    assert_eq!(bin(1).share_texts(first), ["33.3 %", "2 of 6"]);
    assert_eq!(bin(1).share_texts(second), ["25 %", "2 of 8"]);
    assert_eq!(bin(0).share_texts(second), ["0 %", "0 of 8"]);
    assert_eq!(first.shares(), [100.0 / 6.0, 100.0 / 3.0, 50.0]);
}

#[test]
fn mean_and_sigma_count_each_binned_sample_once() {
    assert_eq!(Distribution::default().markers, Markers::Off);
    let bins = Bins {
        start: 0.0,
        width: 1.0,
        count: 5,
    };
    let counts = count([1.0, 2.0, 3.0, 4.0].into_iter(), &bins, None);
    assert_eq!(counts.mean, Some(2.5));
    // the population standard deviation, divided by the number of samples
    assert!((counts.std_dev.unwrap() - 1.25f64.sqrt()).abs() < 1.0e-12);

    // a sample that holds long counts once, unlike in the stats of a plot
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 1.0), &floats([0.0, 10.0]));
    append(&mut model, "a", (2.0, 1.0), &floats([10.0]));
    append(&mut model, "a", (19.0, 1.0), &floats([0.0]));
    let time = live(&model);
    let counted = counted(&distribution_of(&["a"]), &model, &time);
    let mean = counted.traces[0].mean.unwrap();
    assert_eq!(mean, 5.0);
    let (start, end) = time.range().unwrap();
    let window = (time.timestamp(start), time.timestamp(end));
    let series = model.traces.series("a").unwrap();
    let timed = Stats::of_series(&series, window, None).unwrap();
    assert!((timed.mean - mean).abs() > 1.0, "{} {mean}", timed.mean);
}

#[test]
fn markers_are_a_dot_or_a_dot_and_a_whisker_in_a_row_per_trace() {
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 1.0), &floats([1.0, 2.0, 3.0, 4.0]));
    append(&mut model, "b", (0.0, 1.0), &floats([5.0; 4]));
    let time = live(&model);
    let mut distribution = distribution_of(&["a", "b"]);
    let counting = distribution.counting(&model, &time, &NO_PLOTS, 1000.0);
    let counted = counting.counted.as_ref().unwrap();
    assert!(distribution.markers(&counting.shown, counted).is_empty());
    distribution.markers = Markers::Mean;
    let markers = distribution.markers(&counting.shown, counted);
    assert_eq!(
        markers.iter().map(|marker| marker.row).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(markers.iter().all(|marker| marker.whisker.is_none()));
    distribution.markers = Markers::MeanSigma;
    let markers = distribution.markers(&counting.shown, counted);
    let sigma = 1.25f64.sqrt();
    assert_eq!(markers[0].mean, 2.5);
    assert_eq!(markers[0].whisker, Some((2.5 - sigma, 2.5 + sigma)));
    // a trace without spread has no whisker
    assert_eq!((markers[1].mean, markers[1].whisker), (5.0, None));
}

#[test]
fn upright_values_run_along_x_and_sideways_along_y() {
    let mut distribution = Distribution::default();
    assert_eq!(distribution.orientation, Orientation::Upright);
    assert_eq!(distribution.axis_side, AxisSide::Left);
    assert_eq!(distribution.point(1.0, 5.0), [1.0, 5.0]);
    assert_eq!(distribution.movable(false), [true, false]);
    distribution.orientation = Orientation::Sideways;
    assert_eq!(distribution.point(1.0, 5.0), [5.0, 1.0]);
    assert_eq!(distribution.movable(false), [false, true]);
    assert_eq!(distribution.movable(true), [false, false]);
}

/// The plots of a layout with the plot 7, "Motor speed", showing `y_range`.
fn plots(y_range: (f64, f64)) -> Plots {
    Plots(vec![PlotInfo {
        id: ViewId(7),
        name: "Motor speed".to_owned(),
        y_range: Some(y_range),
    }])
}

#[test]
fn a_linked_histogram_bins_the_y_range_of_the_plot_in_either_orientation() {
    let mut model = Model::default();
    append(
        &mut model,
        "a",
        (0.0, 0.1),
        &floats((0..200).map(f64::from)),
    );
    let time = live(&model);
    let mut distribution = Distribution {
        orientation: Orientation::Sideways,
        link: Some(ViewId(7)),
        // its own range is ignored while following
        range: Some((0.0, 1.0)),
        ..distribution_of(&["a"])
    };
    let plots = plots((100.0, 150.0));
    for (mode, count) in [
        (BinMode::Auto, None),
        (BinMode::Number, Some(9)),
        (BinMode::Width, None),
    ] {
        distribution.bins = mode;
        distribution.bin_count = 9;
        distribution.bin_width = 5.0;
        let counting = distribution.counting(&model, &time, &plots, 500.0);
        assert_eq!(counting.fixed, Some((100.0, 150.0)));
        let counted = counting.counted.unwrap();
        let bins = counted.bins;
        assert!(
            bins.start <= 100.0 && bins.edge(bins.count) >= 150.0,
            "{mode:?} {bins:?}"
        );
        if let Some(count) = count {
            assert_eq!(bins.count, count);
        }
        let counts = &counted.traces[0];
        // all 200 samples are in the last 20 s
        assert_eq!((counts.below, total(counts), counts.above), (100, 51, 49));
    }
    // upright, it bins the range of the plot too
    distribution.orientation = Orientation::Upright;
    assert_eq!(distribution.followed(&plots), Some((100.0, 150.0)));
    let counting = distribution.counting(&model, &time, &plots, 500.0);
    assert_eq!(counting.fixed, Some((100.0, 150.0)));
    // a plot not of the layout is not followed, the own range is binned again
    distribution.link = Some(ViewId(8));
    assert_eq!(distribution.followed(&plots), None);
    let counting = distribution.counting(&model, &time, &plots, 500.0);
    assert_eq!(counting.fixed, Some((0.0, 1.0)));
}

/// Drags in `harness` from `from` to `to` in steps.
fn drag(harness: &mut Harness<'_, State>, from: egui::Pos2, to: egui::Pos2) {
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
fn a_linked_plot_sets_the_values_which_do_not_pan_or_zoom() {
    for orientation in Orientation::ALL {
        let mut model = Model::default();
        append(
            &mut model,
            "a",
            (0.0, 0.1),
            &floats((0..200).map(f64::from)),
        );
        let distribution = Distribution {
            orientation,
            link: Some(ViewId(7)),
            value_view: Some((0.0, 10.0)),
            ..distribution_of(&["a"])
        };
        let mut state = State::new(distribution, model);
        state.plots = plots((100.0, 150.0));
        let mut harness = harness(state, false);
        harness.run();
        let values = |harness: &Harness<'_, State>| {
            harness
                .state()
                .distribution
                .last
                .as_deref()
                .cloned()
                .unwrap()
                .values
        };
        assert_eq!(values(&harness), (100.0, 150.0), "{orientation:?}");
        drag(
            &mut harness,
            egui::pos2(200.0, 100.0),
            egui::pos2(100.0, 200.0),
        );
        super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
        assert_eq!(values(&harness), (100.0, 150.0), "{orientation:?}");
        // as the plot is zoomed or panned
        harness.state_mut().plots = plots((120.0, 130.0));
        harness.run();
        assert_eq!(values(&harness), (120.0, 130.0), "{orientation:?}");
    }
}

/// A harness showing the properties of `distribution` in a layout with the plots of
/// [`plots`].
fn properties_harness(distribution: Distribution) -> Harness<'static, Distribution> {
    let model = Model::default();
    let plots = plots((0.0, 1.0));
    Harness::builder()
        .with_size(egui::vec2(300.0, 600.0))
        .build_ui_state(
            move |ui, distribution: &mut Distribution| {
                let connected = Default::default();
                let context = PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &plots,
                };
                distribution.properties_ui(ui, &context);
            },
            distribution,
        )
}

#[test]
fn the_link_is_chosen_in_either_orientation_and_greys_the_range() {
    let link = |name: &'static str| {
        move |node: &egui_kittest::kittest::AccessKitNode<'_>| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some(name)
                && !node.is_disabled()
        }
    };
    let mut harness = properties_harness(Distribution::default());
    harness.run();
    harness.get_by(link("None")).click();
    harness.run();
    harness.get_by_label("Motor speed").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().link, Some(ViewId(7)));
    // the plot sets the range
    assert!(harness.get_by_label("Fixed").accesskit_node().is_disabled());

    // turning sideways keeps the link working
    harness.get_by_label("Sideways").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().link, Some(ViewId(7)));
    harness.get_by(link("Motor speed"));
    harness.get_by_label("Axis side");
    assert!(harness.get_by_label("Fixed").accesskit_node().is_disabled());

    // unlinked, the own range is back
    harness.get_by(link("Motor speed")).click();
    harness.run();
    harness.get_by_label("None").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().link, None);
    assert!(!harness.get_by_label("Fixed").accesskit_node().is_disabled());
}

/// A harness showing a distribution of `pose.x` in m and `pose.y` in rad.
fn pose_harness() -> Harness<'static, State> {
    let state = State::new(distribution_of(&["pose.x", "pose.y"]), pose("m", "m"));
    let mut harness = harness(state, false);
    harness.run();
    harness
}

/// The rect of the tooltip shown in `harness`.
fn tooltip_rect(harness: &Harness<'_, State>) -> egui::Rect {
    let layers = harness
        .ctx
        .memory(|memory| memory.areas().visible_layer_ids());
    let tooltip = (layers.into_iter())
        .find(|layer| layer.order == egui::Order::Tooltip)
        .expect("a tooltip");
    egui::AreaState::load(&harness.ctx, tooltip.id)
        .expect("the area of the tooltip")
        .rect()
}

#[test]
fn the_tooltip_is_as_wide_as_its_texts_wherever_it_opens() {
    let state = State::new(distribution_of(&["pose.x", "pose.y"]), pose("m", "m"));
    let mut harness = sized_harness(state, false, egui::vec2(1000.0, 600.0));
    harness.run();
    for x in [150.0, 400.0, 650.0] {
        // away, so that the tooltip opens anew
        harness.hover_at(egui::pos2(999.0, 599.0));
        harness.run();
        harness.hover_at(egui::pos2(x, 200.0));
        harness.run_steps(40);
        let tooltip = tooltip_rect(&harness);
        let texts = (harness.get_all_by_label_contains(" of "))
            .map(|node| node.rect())
            .reduce(|a, b| a.union(b))
            .expect("the counts");
        // the counts end at the tooltip's margin
        assert!(
            tooltip.right() - texts.right() < 16.0,
            "the counts end at {}, the tooltip at {}",
            texts.right(),
            tooltip.right()
        );
        assert!(tooltip.width() < 250.0, "{} wide", tooltip.width());
    }
}

#[test]
fn the_names_shares_and_counts_of_a_row_of_the_tooltip_line_up() {
    let mut harness = pose_harness();
    // with the rows as high as in the app
    theme::install(&harness.ctx);
    harness.hover_at(egui::pos2(150.0, 150.0));
    harness.run_steps(40);
    let tooltip = tooltip_rect(&harness);
    let rects = |text: &str, exact: bool| -> Vec<egui::Rect> {
        let nodes: Vec<_> = if exact {
            harness.get_all_by_label(text).collect()
        } else {
            harness.get_all_by_label_contains(text).collect()
        };
        let mut rects: Vec<egui::Rect> = (nodes.into_iter())
            .map(|node| node.rect())
            .filter(|rect| tooltip.contains_rect(*rect))
            .collect();
        rects.sort_by(|a, b| a.top().total_cmp(&b.top()));
        rects
    };
    let names = [rects("x", true), rects("y", true)].concat();
    let (shares, counts) = (rects(" %", false), rects(" of ", false));
    assert_eq!((names.len(), shares.len(), counts.len()), (2, 2, 2));
    for ((name, share), count) in names.iter().zip(&shares).zip(&counts) {
        for value in [share, count] {
            let off = (value.center().y - name.center().y).abs();
            assert!(
                off < 0.5,
                "a value is {off} off its name in {name:?} {value:?}"
            );
        }
        // the share ends where the widest share ends, the count where the widest
        assert_eq!(share.right(), shares[0].right().max(shares[1].right()));
        assert_eq!(count.right(), counts[0].right().max(counts[1].right()));
    }
}

#[test]
fn hovering_a_bin_shows_its_range_and_the_shares_and_counts_of_the_traces() {
    let mut harness = pose_harness();
    let at = egui::pos2(150.0, 150.0);
    harness.hover_at(at);
    harness.run_steps(40);
    let state = harness.state();
    let Some(Hovered::Bin(index)) = state.distribution.hovered else {
        panic!("no bin under the pointer");
    };
    let bins = state.distribution.last.as_deref().cloned().unwrap().bins;
    let range = format!(
        "{} to {} m",
        write_value(bins.edge(index), bins.width, DisplayFormat::Default),
        write_value(bins.edge(index + 1), bins.width, DisplayFormat::Default)
    );
    harness.get_by_label(&range);
    // the names of the traces in the legend and the tooltip
    assert_eq!(harness.get_all_by_label("x").count(), 2);
    assert_eq!(harness.get_all_by_label("y").count(), 2);
    // the share and the count of each trace
    assert_eq!(harness.get_all_by_label_contains(" %").count(), 2);
    assert_eq!(harness.get_all_by_label_contains(" of ").count(), 2);

    harness.hover_at(egui::pos2(399.0, 299.0));
    harness.run();
    assert_eq!(harness.state().distribution.hovered, None);
}

#[test]
fn the_wheel_zooms_and_a_drag_pans_the_values_without_binning_anew() {
    let mut harness = pose_harness();
    let state = harness.state();
    let (range, cursor) = (state.time.range(), state.time.cursor());
    let last = state.distribution.last.as_deref().cloned().unwrap();
    let (low, high) = last.values;
    assert!(state.distribution.value_view.is_none());

    super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
    let state = harness.state();
    let (zoomed_low, zoomed_high) = state.distribution.value_view.unwrap();
    assert!(zoomed_high - zoomed_low < (high - low) * 0.9);
    assert_eq!(
        state.distribution.last.as_deref().cloned().unwrap().bins,
        last.bins
    );
    assert_eq!((state.time.range(), state.time.cursor()), (range, cursor));
    // the counts axis fits the tallest bin in view
    let top = state.distribution.share_top.unwrap();
    assert!(top > 0.0 && top <= 100.0);

    drag(
        &mut harness,
        egui::pos2(150.0, 150.0),
        egui::pos2(250.0, 140.0),
    );
    let state = harness.state();
    let (panned_low, panned_high) = state.distribution.value_view.unwrap();
    assert!(panned_low < zoomed_low);
    assert!(((panned_high - panned_low) - (zoomed_high - zoomed_low)).abs() < 1.0e-6);
    assert_eq!(
        state.distribution.last.as_deref().cloned().unwrap().bins,
        last.bins
    );
    assert_eq!((state.time.range(), state.time.cursor()), (range, cursor));

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
    let state = harness.state();
    assert!(state.distribution.value_view.is_none());
    assert_eq!(
        state.distribution.last.as_deref().cloned().unwrap().values,
        (low, high)
    );
    assert_eq!((state.time.range(), state.time.cursor()), (range, cursor));
}

#[test]
fn settings_are_saved_with_defaults_for_those_missing() {
    let distribution: Distribution = serde_json::from_str(r#"{"traces": ["a"]}"#).unwrap();
    assert_eq!(distribution.bins, BinMode::Auto);
    assert_eq!(distribution.bin_count, 20);
    assert_eq!(distribution.link, None);
    let json = serde_json::to_string(&Distribution::default()).unwrap();
    assert!(!json.contains("link"), "{json}");
    let distribution = Distribution {
        bins: BinMode::Width,
        bin_width: 0.5,
        bin_count: 12,
        range: Some((1.0, 2.0)),
        counts: CountScale::Log,
        orientation: Orientation::Sideways,
        axis_side: AxisSide::Right,
        link: Some(ViewId(3)),
        markers: Markers::MeanSigma,
        ..distribution_of(&["a"])
    };
    let loaded: Distribution =
        serde_json::from_str(&serde_json::to_string(&distribution).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        serde_json::to_value(&distribution).unwrap()
    );
}

#[test]
fn bins_of_whole_widths_over_integers_are_centred_on_them() {
    let auto = Distribution::default();
    let bins = auto.make_bins((5.0, 6.0), false, 100, 0.0, true);
    assert_eq!((bins.start, bins.width, bins.count), (4.5, 1.0, 2));
    // the bar of 5 is where it was as the only value
    let single = auto.make_bins((5.0, 5.0), false, 100, 0.0, true);
    assert_eq!((single.start, single.width), (4.5, 1.0));
    let width = Distribution {
        bins: BinMode::Width,
        bin_width: 2.0,
        ..Distribution::default()
    };
    // no integer on an edge, two in each bin
    let bins = width.make_bins((3.0, 9.0), false, 100, 0.0, true);
    assert_eq!((bins.start, bins.edge(bins.count)), (1.5, 9.5));
    assert_eq!(
        (bins.index(3.0), bins.index(4.0), bins.index(5.0)),
        (0, 1, 1)
    );
    assert_eq!(bins.index(9.0), 3);
    // floats and widths below 1 are not
    assert_eq!(
        width.make_bins((3.0, 9.0), false, 100, 0.0, false).start,
        2.0
    );
    let narrow = Distribution {
        bin_width: 0.5,
        ..width
    };
    assert_eq!(
        narrow.make_bins((3.2, 9.0), false, 100, 0.0, true).start,
        3.0
    );
    // a fixed range of integers keeps its last value in the last bin
    let bins = Distribution::default().make_bins((0.0, 10.0), true, 10_000, 0.0, true);
    assert_eq!(
        (bins.start, bins.edge(bins.count), bins.index(10.0)),
        (-0.5, 10.5, 10)
    );
}

#[test]
fn infinite_samples_are_counted_beyond_the_bins_also_without_a_fixed_range() {
    let mut model = Model::default();
    let mut values = floats([1.0, 2.0, 3.0, f64::INFINITY, f64::NEG_INFINITY]);
    values.push(Data::Float(f64::INFINITY));
    append(&mut model, "a", (0.0, 1.0), &values);
    let time = live(&model);
    let counted = counted(&distribution_of(&["a"]), &model, &time);
    let counts = &counted.traces[0];
    assert_eq!((counts.below, total(counts), counts.above), (1, 3, 2));
    assert_eq!(counts.mean, Some(2.0));
    assert_eq!(counted.outside(), (1, 2));
}

#[test]
fn a_link_to_a_plot_without_a_range_yet_turns_off_the_own_range_and_zoom() {
    let mut model = Model::default();
    append(
        &mut model,
        "a",
        (0.0, 0.1),
        &floats((0..200).map(f64::from)),
    );
    let distribution = Distribution {
        orientation: Orientation::Sideways,
        link: Some(ViewId(7)),
        range: Some((0.0, 1.0)),
        ..distribution_of(&["a"])
    };
    let plots = Plots(vec![PlotInfo {
        id: ViewId(7),
        name: "Motor speed".to_owned(),
        y_range: None,
    }]);
    let time = live(&model);
    let counting = distribution.counting(&model, &time, &plots, 500.0);
    assert!(counting.following);
    assert_eq!(counting.fixed, None);
    let mut state = State::new(distribution.clone(), model);
    state.plots = plots.clone();
    let mut harness = harness(state, false);
    harness.run();
    super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
    assert_eq!(harness.state().distribution.value_view, None);
    // greyed in the properties
    let model = Model::default();
    let mut properties = Harness::builder()
        .with_size(egui::vec2(300.0, 600.0))
        .build_ui_state(
            move |ui, distribution: &mut Distribution| {
                let connected = Default::default();
                let context = PropertiesContext {
                    model: &model,
                    connected: &connected,
                    plots: &plots,
                };
                distribution.properties_ui(ui, &context);
            },
            distribution,
        );
    properties.run();
    assert!(
        properties
            .get_by_label("Fixed")
            .accesskit_node()
            .is_disabled()
    );
    properties.get_by_label("Mode");
}

#[test]
fn the_room_beyond_the_bars_holds_the_markers_and_the_outside_counts() {
    let room = Room {
        orientation: Orientation::Upright,
        markers: 23.0,
        outside: Some(egui::vec2(40.0, TEXT_ROW)),
    };
    assert_eq!(room.length(), 23.0 + OVERLAY_INSET + TEXT_ROW + MARKER_GAP);
    let sideways = Room {
        orientation: Orientation::Sideways,
        ..room.clone()
    };
    assert_eq!(sideways.length(), 23.0 + OVERLAY_INSET + 40.0 + MARKER_GAP);
    let empty = Room {
        markers: 0.0,
        outside: None,
        ..room.clone()
    };
    assert_eq!(empty.length(), 0.0);
    // the count of the samples above goes in the corner beyond the marker strip
    let transform = egui_plot::PlotTransform::new(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0)),
        egui_plot::PlotBounds::from_min_max([0.0, 0.0], [1.0, 1.0]),
        egui::Vec2b::FALSE,
    );
    let overlay = Overlay {
        painter: egui::Painter::new(
            egui::Context::default(),
            egui::LayerId::background(),
            egui::Rect::EVERYTHING,
        ),
        transform,
        room,
    };
    let [(below, _), (above, _)] = overlay.outside_positions();
    assert_eq!(
        above,
        egui::pos2(400.0 - OVERLAY_INSET, 23.0 + OVERLAY_INSET)
    );
    assert_eq!(below, egui::pos2(OVERLAY_INSET, 23.0 + OVERLAY_INSET));
}

#[test]
fn outlines_run_over_the_bins_that_are_not_empty() {
    let bins = Bins {
        start: 0.0,
        width: 1.0,
        count: 6,
    };
    let axis = CountAxis::new(CountScale::Linear, 3.0, 1.0, 100.0, 0.0);
    let runs = outline_runs(&[0.0, 2.0, 3.0, 0.0, 1.0, 0.0], &bins, axis);
    assert_eq!(
        runs,
        [
            vec![
                [1.0, 0.0],
                [1.0, 2.0],
                [2.0, 2.0],
                [2.0, 3.0],
                [3.0, 3.0],
                [3.0, 0.0]
            ],
            vec![[4.0, 0.0], [4.0, 1.0], [5.0, 1.0], [5.0, 0.0]],
        ]
    );
    assert!(outline_runs(&[0.0; 6], &bins, axis).is_empty());
    let last = outline_runs(&[0.0, 0.0, 0.0, 0.0, 0.0, 4.0], &bins, axis);
    assert_eq!(last, [vec![[5.0, 0.0], [5.0, 4.0], [6.0, 4.0], [6.0, 0.0]]]);
}

#[test]
fn the_title_legend_cuts_the_names_and_keeps_the_unit() {
    let mut model = Model::default();
    for path in [
        "robot/shoulder.position_error",
        "robot/elbow.position_error",
    ] {
        append(&mut model, path, (0.0, 1.0), &floats([1.0]));
        model.traces.set_unit(path, "rad");
    }
    let distribution = distribution_of(&[
        "robot/shoulder.position_error",
        "robot/elbow.position_error",
    ]);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(180.0, 30.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let mut context = ViewContext {
                    model: &state.model,
                    time: &mut state.time,
                    invoker: &mut state.invoker,
                    formats: &state.formats,
                    titled: true,
                    plots: &state.plots,
                };
                ui.horizontal(|ui| state.distribution.title_ui(ui, &mut context));
            },
            State::new(distribution, model),
        );
    harness.run();
    let unit = harness.get_by_label("rad").rect();
    assert!(unit.right() <= 180.0, "{unit:?}");
    // the first name is cut off before the unit, the second one is left out
    let labels: Vec<egui::Rect> = (harness.get_all_by_role(egui::accesskit::Role::Label))
        .map(|node| node.rect())
        .collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    let first = labels[0];
    assert!(first.right() <= unit.left(), "{first:?} {unit:?}");
}

#[test]
fn the_value_axis_of_a_sideways_histogram_is_on_the_side_chosen() {
    for (side, on_left) in [(AxisSide::Left, true), (AxisSide::Right, false)] {
        let distribution = Distribution {
            orientation: Orientation::Sideways,
            axis_side: side,
            ..distribution_of(&["pose.x"])
        };
        let mut harness = harness(State::new(distribution, pose("m", "m")), false);
        harness.run();
        harness.run();
        let frame = harness
            .state()
            .distribution
            .last
            .as_deref()
            .cloned()
            .unwrap()
            .frame;
        let (left, right) = (frame.left(), 400.0 - frame.right());
        assert_eq!(left > right, on_left, "{side:?} {frame:?}");
        assert!(
            left.max(right) >= VALUE_AXIS_THICKNESS,
            "{side:?} {frame:?}"
        );
    }
}

#[test]
fn plots_of_the_same_name_are_told_apart_by_a_number() {
    let plot = |id, name: &str| PlotInfo {
        id: ViewId(id),
        name: name.to_owned(),
        y_range: None,
    };
    let plots = Plots(vec![plot(1, "Speed"), plot(2, "Pose"), plot(3, "Speed")]);
    assert_eq!(plot_names(&plots), ["Speed (1)", "Pose", "Speed (2)"]);
}

#[test]
fn infinite_samples_alone_show_their_counts_on_empty_axes() {
    let mut model = Model::default();
    let values = floats([f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY]);
    append(&mut model, "a", (0.0, 1.0), &values);
    let distribution = distribution_of(&["a"]);
    let time = live(&model);
    let counting = distribution.counting(&model, &time, &NO_PLOTS, 500.0);
    assert!(counting.counted.is_none());
    assert_eq!(counting.outside, (1, 2));
    let mut harness = harness(State::new(distribution, model), false);
    harness.run();
    assert!(harness.query_by_label(NO_SAMPLES).is_none());
}

/// The boxes `distribution` shows of `model` in the window of `time`, following a
/// plot of `plots`.
fn boxes_of(
    distribution: &mut Distribution,
    model: &Model,
    time: &TimeRange,
    plots: &Plots,
) -> Vec<Option<boxes::BoxStats>> {
    let counting = distribution.counting(model, time, plots, 1000.0);
    let window = counting.window.expect("a window");
    boxes::boxes(
        &mut distribution.boxes,
        (model.samples_version(), window),
        distribution.whiskers,
        &distribution.traces,
        &counting.shown,
    )
    .to_vec()
}

#[test]
fn boxes_count_each_sample_once_over_the_shown_window() {
    let values = (0..=40).map(|second| match second % 7 {
        3 => Data::Null,
        _ => Data::Float(f64::from(second % 5)),
    });
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 1.0), &values.collect::<Vec<_>>());
    let mut distribution = Distribution {
        show: Show::Boxes,
        ..distribution_of(&["a"])
    };
    let mut time = live(&model);
    let samples = |distribution: &mut Distribution, model: &Model, time: &TimeRange| {
        let boxes = boxes_of(distribution, model, time, &NO_PLOTS);
        let samples = boxes[0].as_ref().unwrap().samples as u64;
        assert_eq!(samples, samples_in_window(model, time, "a"));
        samples
    };
    // the last 20 s, live, without the nulls
    assert_eq!(samples(&mut distribution, &model, &time), 21 - 3);
    // anew as the window is panned and zoomed, and moves on while live
    time.pan_zoom(0.0, 40.0);
    assert_eq!(samples(&mut distribution, &model, &time), 41 - 6);
    time.pan_zoom(5.0, 15.0);
    samples(&mut distribution, &model, &time);
    time.go_live();
    time.pan_zoom(20.0, 40.0);
    append(&mut model, "a", (41.0, 1.0), &floats([1.0; 10]));
    time.update(&model, 0.0);
    samples(&mut distribution, &model, &time);

    // a sample that holds long counts once, as in the bars
    let mut model = Model::default();
    append(&mut model, "a", (0.0, 1.0), &floats([0.0, 10.0]));
    append(&mut model, "a", (2.0, 1.0), &floats([10.0]));
    append(&mut model, "a", (19.0, 1.0), &floats([0.0]));
    let time = live(&model);
    let stats = boxes_of(&mut distribution, &model, &time, &NO_PLOTS)[0]
        .clone()
        .unwrap();
    assert_eq!((stats.samples, stats.mean, stats.median), (4, 5.0, 5.0));

    // the box does not depend on the range of the value axis
    distribution.range = Some((100.0, 200.0));
    let fixed = boxes_of(&mut distribution, &model, &time, &NO_PLOTS);
    distribution.link = Some(ViewId(7));
    let linked = boxes_of(&mut distribution, &model, &time, &plots((3.0, 4.0)));
    assert_eq!(fixed, [Some(stats.clone())]);
    assert_eq!(linked, [Some(stats)]);
}

#[test]
fn the_form_is_bars_by_default_and_boxes_give_each_trace_a_row() {
    assert_eq!(Distribution::default().show, Show::Bars);
    for orientation in Orientation::ALL {
        let distribution = Distribution {
            show: Show::Boxes,
            orientation,
            ..distribution_of(&["pose.x", "pose.y"])
        };
        let mut harness = harness(State::new(distribution, pose("m", "m")), false);
        harness.run();
        let state = harness.state();
        let last = state.distribution.last.as_deref().cloned().unwrap();
        let rows = last.rows.unwrap();
        let across = match orientation {
            Orientation::Upright => last.frame.height(),
            Orientation::Sideways => last.frame.width(),
        };
        assert_eq!(rows.count, 2);
        assert!(
            (rows.length() - across / 2.0).abs() < 1.0e-3,
            "{orientation:?}"
        );
        let covered = rows.rect(0).union(rows.rect(1));
        assert!(
            (covered.area() - last.frame.area()).abs() < 1.0,
            "{orientation:?}"
        );
        // from the top down, from the left
        let first = rows.rect(0).center();
        match orientation {
            Orientation::Upright => assert!(first.y < last.frame.center().y),
            Orientation::Sideways => assert!(first.x < last.frame.center().x),
        }
        assert_eq!(last.names, ["x", "y"]);
        // no bins or counts
        assert_eq!(state.distribution.share_top, None);
    }
}

#[test]
fn the_names_of_the_rows_are_cut_at_a_third_of_the_view() {
    let paths = [
        "arm/joint.shoulder_position_error",
        "arm/joint.elbow_position_error",
    ];
    let model = || {
        let mut model = Model::default();
        for path in paths {
            append(
                &mut model,
                path,
                (0.0, 0.1),
                &floats((0..50).map(f64::from)),
            );
        }
        model
    };
    for orientation in Orientation::ALL {
        let distribution = Distribution {
            show: Show::Boxes,
            orientation,
            ..distribution_of(&paths)
        };
        let state = State::new(distribution, model());
        let mut harness = sized_harness(state, false, egui::vec2(300.0, 200.0));
        harness.run();
        let last = harness
            .state()
            .distribution
            .last
            .as_deref()
            .cloned()
            .unwrap();
        let full = ["elbow_position_error", "shoulder_position_error"];
        for (name, full) in last.names.iter().zip(full) {
            assert!(name.ends_with('…'), "{name}");
            assert!(full.starts_with(name.trim_end_matches('…')), "{name}");
        }
        let font = egui::FontId::proportional(13.0);
        let widths: Vec<f32> = (last.names.iter())
            .map(|name| {
                harness.ctx.fonts_mut(|fonts| {
                    let galley = fonts.layout_no_wrap(
                        name.clone(),
                        font.clone(),
                        egui::Color32::PLACEHOLDER,
                    );
                    galley.size().x
                })
            })
            .collect();
        assert!(widths.iter().all(|&width| width <= 100.0), "{widths:?}");
        if orientation == Orientation::Upright {
            // the names take at most a third of the view
            assert!(last.frame.left() <= 300.0 / 3.0 + 8.0, "{:?}", last.frame);
        } else {
            // and fit their columns
            let column = last.frame.width() / 2.0;
            assert!(widths.iter().all(|&width| width < column), "{widths:?}");
        }
    }
}

#[test]
fn both_puts_a_box_per_trace_in_the_strip_of_the_markers() {
    let distribution = Distribution {
        show: Show::Both,
        markers: Markers::MeanSigma,
        ..distribution_of(&["pose.x", "pose.y"])
    };
    let mut harness = harness(State::new(distribution, pose("m", "m")), false);
    harness.run();
    let state = harness.state();
    let last = state.distribution.last.as_deref().cloned().unwrap();
    let rows = last.rows.unwrap();
    assert_eq!((rows.count, rows.length()), (2, 14.0));
    assert_eq!(rows.style.thickness, 10.0);
    assert_eq!(rows.centre(0), last.frame.top() + MARKER_INSET + 7.0);
    // the boxes take the place of the markers, above the bars
    let model = &state.model;
    let counting = state
        .distribution
        .counting(model, &state.time, &NO_PLOTS, 1000.0);
    let counted = counting.counted.as_ref().unwrap();
    assert!(
        state
            .distribution
            .markers(&counting.shown, counted)
            .is_empty()
    );
    assert!(state.distribution.share_top.is_some());
}

#[test]
fn a_trace_without_samples_keeps_its_row() {
    let model = || {
        let mut model = Model::default();
        append(&mut model, "a", (0.0, 1.0), &floats((0..40).map(f64::from)));
        append(&mut model, "b", (0.0, 1.0), &floats([1.0, 2.0, 3.0]));
        model
    };
    for show in [Show::Boxes, Show::Both] {
        let distribution = Distribution {
            show,
            ..distribution_of(&["a", "b"])
        };
        let mut harness = harness(State::new(distribution, model()), false);
        harness.run();
        let last = harness
            .state()
            .distribution
            .last
            .as_deref()
            .cloned()
            .unwrap();
        let rows = last.rows.unwrap();
        assert_eq!(rows.count, 2, "{show:?}");
        let state = harness.state_mut();
        let boxes = boxes_of(
            &mut state.distribution,
            &state.model,
            &state.time,
            &NO_PLOTS,
        );
        assert!(boxes[0].is_some() && boxes[1].is_none(), "{show:?}");
        // its tooltip says so
        harness.hover_at(egui::pos2(last.frame.center().x, rows.centre(1)));
        harness.run_steps(40);
        assert_eq!(harness.state().distribution.hovered, Some(Hovered::Row(1)));
        harness.get_by_label(boxes::NO_SAMPLES);
        assert!(harness.query_by_label("Median").is_none());
    }
}

#[test]
fn the_properties_hide_the_rows_that_do_not_apply_to_the_form() {
    let mut harness = properties_harness(Distribution {
        bins: BinMode::Width,
        bin_width: 0.5,
        ..Distribution::default()
    });
    harness.run();
    let shows = |harness: &Harness<'_, Distribution>, labels: &[&str]| -> Vec<bool> {
        (labels.iter())
            .map(|label| harness.query_all_by_label(label).next().is_some())
            .collect()
    };
    let labels = ["Markers", "Whiskers", "Mode", "Width", "Scale", "Range"];
    assert_eq!(
        shows(&harness, &labels),
        [true, false, true, true, true, true]
    );
    let slot = harness.get_by_label("Markers").rect().top();

    harness.get_by_label("Boxes").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().show, Show::Boxes);
    assert_eq!(
        shows(&harness, &labels),
        [false, true, false, false, false, true]
    );
    // in the place of the markers
    assert!((harness.get_by_label("Whiskers").rect().top() - slot).abs() < 4.0);
    harness
        .get_by(|node| {
            node.role() == egui::accesskit::Role::ComboBox
                && node.value().as_deref() == Some("1.5×IQR")
        })
        .click();
    harness.run();
    harness.get_by_label("5–95 %").click();
    // the hover text of the choice under the pointer fades in
    harness.run_steps(4);
    assert_eq!(harness.state().whiskers, Whiskers::P5);
    harness.event(egui::Event::PointerGone);
    harness.run_steps(4);

    harness.get_by_label("Both").click();
    harness.run();
    harness.run();
    assert_eq!(
        shows(&harness, &labels),
        [false, true, true, true, true, true]
    );
    harness.get_by_label("Bars").click();
    harness.run();
    harness.run();
    assert_eq!(
        shows(&harness, &labels),
        [true, false, true, true, true, true]
    );
    // the hidden settings are kept
    let state = harness.state();
    assert_eq!((state.bins, state.bin_width), (BinMode::Width, 0.5));
    assert_eq!(state.whiskers, Whiskers::P5);
}

#[test]
fn a_linked_view_follows_the_plot_in_all_forms() {
    let model = || {
        let mut model = Model::default();
        append(
            &mut model,
            "a",
            (0.0, 0.1),
            &floats((0..200).map(f64::from)),
        );
        model
    };
    for show in Show::ALL {
        for orientation in Orientation::ALL {
            let distribution = Distribution {
                show,
                orientation,
                link: Some(ViewId(7)),
                ..distribution_of(&["a"])
            };
            let mut state = State::new(distribution, model());
            state.plots = plots((100.0, 150.0));
            let mut harness = harness(state, false);
            harness.run();
            let values = |harness: &Harness<'_, State>| {
                let last = harness.state().distribution.last.as_deref().cloned();
                last.unwrap().values
            };
            assert_eq!(values(&harness), (100.0, 150.0), "{show:?} {orientation:?}");
            super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
            assert_eq!(values(&harness), (100.0, 150.0), "{show:?} {orientation:?}");
            harness.state_mut().plots = plots((120.0, 130.0));
            harness.run();
            assert_eq!(values(&harness), (120.0, 130.0), "{show:?} {orientation:?}");
        }
    }
}

/// The rects of the labels `texts` in the tooltip of `harness`, in their order.
fn tooltip_rects(harness: &Harness<'_, State>, texts: &[&str]) -> Vec<egui::Rect> {
    let tooltip = tooltip_rect(harness);
    (texts.iter())
        .map(|text| {
            (harness.get_all_by_label(text))
                .map(|node| node.rect())
                .find(|rect| tooltip.contains_rect(*rect))
                .unwrap_or_else(|| panic!("{text} in the tooltip"))
        })
        .collect()
}

#[test]
fn hovering_a_row_lights_it_and_shows_the_box_of_the_trace() {
    for orientation in Orientation::ALL {
        let distribution = Distribution {
            show: Show::Boxes,
            orientation,
            ..distribution_of(&["pose.x", "pose.y"])
        };
        let mut state = State::new(distribution, pose("m", "m"));
        state.formats.set("pose.x", DisplayFormat::Scientific);
        let mut harness = harness(state, false);
        theme::install(&harness.ctx);
        harness.run();
        let last = harness
            .state()
            .distribution
            .last
            .as_deref()
            .cloned()
            .unwrap();
        let rows = last.rows.unwrap();
        // anywhere in the row, also away from the box
        let at = match orientation {
            Orientation::Upright => egui::pos2(last.frame.left() + 2.0, rows.centre(0) + 10.0),
            Orientation::Sideways => egui::pos2(rows.centre(0) + 10.0, last.frame.top() + 2.0),
        };
        harness.hover_at(at);
        harness.run_steps(40);
        assert_eq!(
            harness.state().distribution.hovered,
            Some(Hovered::Row(0)),
            "{orientation:?}"
        );
        let names = [
            "Min",
            "Low whisker",
            "Q1",
            "Median",
            "Mean",
            "Q3",
            "High whisker",
            "Max",
        ];
        let rects = tooltip_rects(&harness, &names);
        // in the order drawn: low to high upright, high to low sideways
        for pair in rects.windows(2) {
            match orientation {
                Orientation::Upright => assert!(pair[0].top() < pair[1].top()),
                Orientation::Sideways => assert!(pair[0].top() > pair[1].top()),
            }
        }
        // the trace and its values in its number format with its unit, right aligned
        tooltip_rects(&harness, &["x"]);
        let state = harness.state();
        let mut distribution = state.distribution.clone();
        let stats = boxes_of(&mut distribution, &state.model, &state.time, &NO_PLOTS)[0]
            .clone()
            .unwrap();
        let step = boxes::value_step(&stats);
        let texts: Vec<String> = (boxes::tooltip_rows(&stats, Whiskers::Iqr, orientation))
            .into_iter()
            .map(|(_, value, _)| {
                format!("{} m", write_value(value, step, DisplayFormat::Scientific))
            })
            .collect();
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let values = tooltip_rects(&harness, &texts);
        let right = values[0].right();
        assert!(values.iter().all(|rect| (rect.right() - right).abs() < 0.5));
        // the counts under the rule
        let counts = tooltip_rects(&harness, &["Samples", "Outliers"]);
        let lowest = rects.iter().map(|rect| rect.bottom()).fold(0.0, f32::max);
        assert!(counts[0].top() > lowest && counts[1].top() > counts[0].top());
        let samples = stats.samples.to_string();
        let count = tooltip_rects(&harness, &[samples.as_str()]);
        assert!(
            (count[0].right() - right).abs() < 0.5,
            "{count:?} {values:?}"
        );
    }
}

#[test]
fn min_max_whiskers_leave_out_their_rows_and_percentiles_are_named() {
    for (whiskers, low, high) in [
        (Whiskers::MinMax, None, None),
        (Whiskers::P5, Some("5 %"), Some("95 %")),
        (Whiskers::P1, Some("1 %"), Some("99 %")),
    ] {
        let distribution = Distribution {
            show: Show::Boxes,
            whiskers,
            ..distribution_of(&["pose.x", "pose.y"])
        };
        let mut harness = harness(State::new(distribution, pose("m", "m")), false);
        harness.run();
        let last = harness
            .state()
            .distribution
            .last
            .as_deref()
            .cloned()
            .unwrap();
        let rows = last.rows.unwrap();
        harness.hover_at(egui::pos2(last.frame.center().x, rows.centre(1)));
        harness.run_steps(40);
        tooltip_rects(&harness, &["y", "Min", "Q1", "Median", "Mean", "Q3", "Max"]);
        for name in ["Low whisker", "High whisker"] {
            assert!(harness.query_by_label(name).is_none(), "{whiskers:?}");
        }
        match (low, high) {
            (Some(low), Some(high)) => {
                tooltip_rects(&harness, &[low, high, "Samples", "Outliers"]);
            }
            _ => {
                tooltip_rects(&harness, &["Samples"]);
                assert!(harness.query_by_label("Outliers").is_none());
            }
        }
    }
}

#[test]
fn hovering_the_strip_of_both_shows_a_box_and_below_it_a_bin() {
    let distribution = Distribution {
        show: Show::Both,
        ..distribution_of(&["pose.x", "pose.y"])
    };
    let mut harness = harness(State::new(distribution, pose("m", "m")), false);
    harness.run();
    let last = harness
        .state()
        .distribution
        .last
        .as_deref()
        .cloned()
        .unwrap();
    let rows = last.rows.unwrap();
    harness.hover_at(egui::pos2(last.frame.center().x, rows.centre(1)));
    harness.run_steps(40);
    assert_eq!(harness.state().distribution.hovered, Some(Hovered::Row(1)));
    harness.get_by_label("Median");
    harness.hover_at(egui::pos2(
        last.frame.center().x,
        last.frame.bottom() - 10.0,
    ));
    harness.run_steps(4);
    assert!(matches!(
        harness.state().distribution.hovered,
        Some(Hovered::Bin(_))
    ));
}

#[test]
fn show_and_whiskers_are_saved_and_old_histograms_load_as_bars() {
    let distribution = Distribution {
        show: Show::Both,
        whiskers: Whiskers::P1,
        ..distribution_of(&["a"])
    };
    let json = serde_json::to_string(&distribution).unwrap();
    let loaded: Distribution = serde_json::from_str(&json).unwrap();
    assert_eq!((loaded.show, loaded.whiskers), (Show::Both, Whiskers::P1));
    // a view saved by the name it had before it showed boxes
    let old = r#"{"name": "Spread", "kind": {"Histogram": {"traces": ["a"], "markers": "Mean"}}}"#;
    let view: crate::app::view::View = serde_json::from_str(old).unwrap();
    assert_eq!(view.view_type(), ViewType::Distribution);
    assert_eq!(view.view_type().name(), "Distribution");
    let crate::app::view::ViewKind::Distribution(loaded) = &view.kind else {
        panic!("a distribution");
    };
    assert_eq!(loaded.show, Show::Bars);
    assert_eq!(loaded.whiskers, Whiskers::Iqr);
    assert_eq!(loaded.markers, Markers::Mean);
    // saved by its new name
    let json = serde_json::to_string(&view).unwrap();
    assert!(json.contains(r#""Distribution":"#), "{json}");
}
