use super::*;
use crate::app::model::traces::Traces;
use proto::Data;

/// Traces of the given samples at the given timestamps.
fn traces(samples: &[(u64, &str, Data)]) -> Traces {
    let mut traces = Traces::default();
    for (timestamp, path, value) in samples {
        traces.append(&proto::Value {
            timestamp: *timestamp,
            path: path.to_string(),
            value: value.clone(),
        });
    }
    traces
}

#[test]
fn samples_are_paired_with_the_current_x() {
    let traces = traces(&[
        (1, "y", Data::Float(10.0)),
        (2, "x", Data::Float(1.0)),
        (2, "y", Data::Float(20.0)),
        (3, "y", Data::Float(30.0)),
        (4, "x", Data::Float(2.0)),
        (5, "y", Data::Null),
    ]);
    let (x, y) = (traces.series("x").unwrap(), traces.series("y").unwrap());
    let points = pair(x, y, 0..y.len());
    assert!(points[0][0].is_nan());
    assert_eq!(points[1], [1.0, 20.0]);
    assert_eq!(points[2], [1.0, 30.0]);
    assert_eq!(points[3][0], 2.0);
    assert!(!is_point(&points[3]));
    assert_eq!(pair(x, y, 2..3), [[1.0, 30.0]]);
}

/// An XY view of the traces at `paths` with their axes assigned.
fn xy_of(paths: &[&str]) -> Xy {
    let mut xy = Xy {
        traces: paths.iter().map(|&path| path.to_owned()).collect(),
        ..Xy::default()
    };
    xy.assign_axes(&Model::default());
    xy
}

/// The lines of `xy` as owned pairs of x and y paths.
fn lines_of(xy: &Xy) -> Vec<(String, String)> {
    (xy.lines().into_iter())
        .map(|(x, y)| (x.to_owned(), y.to_owned()))
        .collect()
}

fn line(x: &str, y: &str) -> (String, String) {
    (x.to_owned(), y.to_owned())
}

#[test]
fn the_first_trace_is_the_x_trace_unless_siblings_pair_up() {
    let xy = xy_of(&["a", "b", "c"]);
    assert_eq!(lines_of(&xy), [line("a", "b"), line("a", "c")]);

    let xy = xy_of(&[
        "heading",
        "one/pose.x",
        "one/pose.y",
        "two/pos/X",
        "two/pos/Y",
    ]);
    assert_eq!(
        lines_of(&xy),
        [
            line("one/pose.x", "heading"),
            line("one/pose.x", "one/pose.y"),
            line("two/pos/X", "two/pos/Y"),
        ]
    );
}

#[test]
fn newly_shown_traces_keep_the_axes_of_the_others() {
    let mut xy = xy_of(&["a", "b"]);
    // a y trace whose sibling x is already an x trace
    for path in ["c", "p.x", "p.y"] {
        xy.traces.insert(path.to_owned());
        xy.assign_axes(&Model::default());
    }
    assert_eq!(
        lines_of(&xy),
        [
            line("a", "b"),
            line("a", "c"),
            line("a", "p.x"),
            line("a", "p.y")
        ]
    );
    xy.set_axis("p.x", true);
    xy.against.remove("p.y");
    xy.assign_axes(&Model::default());
    assert_eq!(lines_of(&xy)[2], line("p.x", "p.y"));
}

#[test]
fn a_y_trace_checked_before_its_x_sibling_pairs_up_with_it() {
    let mut xy = xy_of(&["pose.y"]);
    assert_eq!(xy.xs, ["pose.y".to_owned()].into());
    xy.traces.insert("pose.x".to_owned());
    xy.assign_axes(&Model::default());
    assert_eq!(lines_of(&xy), [line("pose.x", "pose.y")]);

    // also when it was plotted against another x trace
    let mut xy = xy_of(&["a", "pose.y"]);
    xy.traces.insert("pose.x".to_owned());
    xy.assign_axes(&Model::default());
    assert_eq!(lines_of(&xy), [line("pose.x", "pose.y")]);

    // but not when other traces are plotted against it
    let mut xy = xy_of(&["a", "pose.y"]);
    xy.set_axis("pose.y", true);
    xy.set_axis("a", false);
    assert_eq!(lines_of(&xy), [line("pose.y", "a")]);
    xy.traces.insert("pose.x".to_owned());
    xy.assign_axes(&Model::default());
    assert_eq!(xy.against["a"], "pose.y");
    assert!(xy.xs.contains("pose.y"));
}

#[test]
fn showing_an_array_keeps_the_axes_of_the_traces() {
    let mut xy = xy_of(&["p.x", "p.y", "rpm"]);
    xy.set_axis("rpm", true);
    xy.against.insert("p.y".to_owned(), "rpm".to_owned());
    let lines = lines_of(&xy);
    let mut model = Model::default();
    model.traces.append(&proto::Value {
        timestamp: 0,
        path: "adc".to_owned(),
        value: Data::Array(vec![Data::Float(1.0); 4]),
    });
    xy.traces.insert("adc".to_owned());
    xy.assign_axes(&model);
    xy.traces.remove("adc");
    xy.assign_axes(&model);
    assert_eq!(lines_of(&xy), lines);
}

#[test]
fn the_colours_follow_the_axes_before_the_view_assigns_them() {
    let mut xy = xy_of(&["a"]);
    xy.traces.extend(["b".to_owned(), "c".to_owned()]);
    assert_eq!(xy.colored_paths(&Model::default()), ["b", "c"]);
    assert!(xy.against.is_empty(), "the view is unchanged");
}

#[test]
fn y_traces_of_a_gone_x_trace_move_to_another_one() {
    let mut xy = xy_of(&["a", "b", "p.x", "p.y"]);
    xy.set_axis("a", true);
    xy.against.insert("b".to_owned(), "p.x".to_owned());
    xy.traces.remove("p.x");
    xy.assign_axes(&Model::default());
    assert_eq!(lines_of(&xy), [line("a", "b"), line("a", "p.y")]);

    // without an x trace, the first trace takes its place
    xy.traces.remove("a");
    xy.assign_axes(&Model::default());
    assert_eq!(lines_of(&xy), [line("b", "p.y")]);
}

#[test]
fn the_only_x_trace_put_on_the_y_axis_gives_way_to_another() {
    let mut xy = xy_of(&["a", "b", "c"]);
    xy.set_axis("a", false);
    xy.assign_axes(&Model::default());
    assert_eq!(xy.xs, ["b".to_owned()].into());
    assert_eq!(lines_of(&xy), [line("b", "a"), line("b", "c")]);

    // with another x trace, the y traces of the one moved stay on that
    xy.set_axis("c", true);
    xy.against.insert("a".to_owned(), "c".to_owned());
    xy.set_axis("c", false);
    xy.assign_axes(&Model::default());
    assert_eq!(lines_of(&xy), [line("b", "a"), line("b", "c")]);
}

#[test]
fn lines_are_named_after_the_parent_they_share() {
    assert_eq!(
        line_names(&[
            ("one/pose.x", "one/pose.y"),
            ("two/pose.x", "two/pose.y"),
            ("one/pose.x", "speed"),
        ]),
        ["one/pose", "two/pose", "speed"]
    );
    // two lines of one parent are named after their y traces
    assert_eq!(
        line_names(&[("pose.x", "pose.y"), ("pose.x", "pose.z"), ("x", "y")]),
        ["pose.y", "z", "y"]
    );
}

#[test]
fn the_trail_fades_towards_the_oldest_step() {
    assert_eq!(trail_opacity(0.0, 0, 8), 1.0);
    assert_eq!(trail_opacity(0.85, 7, 8), 1.0);
    assert!((trail_opacity(1.0, 0, 4) - 0.25).abs() < 1.0e-6);
    assert!((trail_opacity(0.5, 1, 4) - 0.75).abs() < 1.0e-6);
    // out of range in a layout file
    assert!(trail_opacity(2.0, 0, 4) > 0.0);
}

/// A transform showing plot coordinates as screen points, 1 per unit.
fn unit_transform() -> egui_plot::PlotTransform {
    egui_plot::PlotTransform::new(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
        egui_plot::PlotBounds::from_min_max([0.0, 0.0], [100.0, 100.0]),
        egui::Vec2b::FALSE,
    )
}

#[test]
fn trail_steps_join_and_end_where_the_pattern_repeats() {
    let transform = unit_transform();
    let points: Vec<[f64; 2]> = (0..8).map(|i| [f64::from(i) * 3.0, 0.0]).collect();

    // without a pattern, the steps split the points evenly and share one point
    let lines = trail_lines(&points, 4, None, &transform);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], [[0.0, 0.0], [3.0, 0.0], [6.0, 0.0]]);
    assert_eq!(lines[3], [[18.0, 0.0], [21.0, 0.0]]);

    // with a pattern of 5, the steps end at whole multiples of it
    let lines = trail_lines(&points, 4, Some(5.0), &transform);
    assert_eq!(lines.len(), 4);
    for pair in lines.windows(2) {
        let end = *pair[0].last().unwrap();
        assert_eq!(pair[1][0], end);
        let periods = end[0] / 5.0;
        assert!((periods - periods.round()).abs() < 1.0e-6, "{end:?}");
    }
    // the first step was to end after 2 points, at 3, so it ends at 5
    assert!((lines[0].last().unwrap()[0] - 5.0).abs() < 1.0e-6);
    assert_eq!(lines[3].last(), Some(&[21.0, 0.0]));
    // all points are drawn
    let drawn: usize = lines.iter().map(Vec::len).sum();
    assert_eq!(drawn, points.len() + 2 * (lines.len() - 1));
}

#[test]
fn a_gap_starts_the_pattern_and_a_pending_step_again() {
    let transform = unit_transform();
    let points = [
        [0.0, 0.0],
        [1.0, 0.0],
        [f64::NAN, 0.0],
        [7.0, 0.0],
        [9.0, 0.0],
    ];
    let lines = trail_lines(&points, 2, Some(5.0), &transform);
    assert_eq!(lines.len(), 2);
    // the step ends at the gap, before the length it was to end at
    assert_eq!(lines[0].len(), 3);
    assert_eq!(lines[0][..2], [[0.0, 0.0], [1.0, 0.0]]);
    assert!(!is_point(&lines[0][2]));
    assert_eq!(lines[1], [[7.0, 0.0], [9.0, 0.0]]);
}

#[test]
fn layouts_without_the_new_properties_fade_as_before() {
    let xy: Xy = serde_json::from_str(r#"{"traces": ["a", "b"]}"#).unwrap();
    assert_eq!(xy.fade, DEFAULT_FADE);
    assert_eq!(xy.points, ShowPoints::Auto);
    let json = serde_json::to_string(&xy).unwrap();
    assert!(!json.contains("line_styles"), "{json}");
}

struct State {
    xy: Xy,
    model: Model,
    time: TimeRange,
    invoker: crate::app::model::actions::Invoker,
}

/// A harness showing `xy` of an array trace `adc` of 32 elements and a float trace
/// `rpm`, both with two samples.
fn xy_harness(xy: Xy) -> egui_kittest::Harness<'static, State> {
    let mut model = Model::default();
    for (second, value) in [(0, 1.0), (1, 2.0)] {
        let timestamp = second * 1_000_000_000;
        let elements = (0..32).map(|i| Data::Float(value * f64::from(i))).collect();
        for (path, data) in [("adc", Data::Array(elements)), ("rpm", Data::Float(value))] {
            model.traces.append(&proto::Value {
                timestamp,
                path: path.to_owned(),
                value: data,
            });
        }
    }
    let state = State {
        xy,
        model,
        time: Default::default(),
        invoker: Default::default(),
    };
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
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
                state.xy.view(ui, &mut context);
            },
            state,
        )
}

#[test]
fn a_single_array_is_plotted_over_its_indices() {
    use egui_kittest::kittest::Queryable;

    let xy = Xy {
        traces: ["adc".to_owned()].into(),
        ..Xy::default()
    };
    let mut harness = xy_harness(xy);
    harness.run();
    let state = harness.state();
    assert_eq!(state.xy.array_paths(&state.model), ["adc"]);
    assert!(harness.query_by_label_contains("Select").is_none());

    // a single trace that is no array is not enough
    let xy = Xy {
        traces: ["rpm".to_owned()].into(),
        ..Xy::default()
    };
    let mut harness = xy_harness(xy);
    harness.run();
    assert!(
        harness
            .state()
            .xy
            .array_paths(&harness.state().model)
            .is_empty()
    );
    assert!(harness.query_by_label_contains("Select").is_some());
}

#[test]
fn thinning_out_keeps_the_last_point() {
    let points: Vec<[f64; 2]> = (0..10).map(|i| [i as f64, 0.0]).collect();
    let thinned = thin_out(points, 4);
    assert!(thinned.len() <= 4);
    assert_eq!(thinned.last(), Some(&[9.0, 0.0]));
}

#[test]
fn axis_label_names_a_single_trace() {
    assert_eq!(axis_label(&["x"], Some("m")), "x [m]");
    assert_eq!(axis_label(&["x", "y"], Some("m")), "[m]");
    assert_eq!(axis_label(&["x"], None), "x");
    assert_eq!(axis_label(&[], None), "");
}
