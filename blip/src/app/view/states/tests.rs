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
    let runs = runs(samples, (0.0, 10.0), 6.0, 0.0, 0);
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
    let runs = runs(samples, (1.5, 2.5), 3.0, 0.0, 0);
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
        .map(|state| {
            let color = state_color(&state.to_string(), false, &egui::Visuals::light());
            color.unwrap().to_array()
        })
        .collect();
    assert_eq!(colors.len(), 4);
    assert_eq!(
        state_color("idle", false, &egui::Visuals::dark()),
        state_color("idle", false, &egui::Visuals::dark())
    );
}

#[test]
fn false_bools_are_empty_and_log_levels_take_their_colors() {
    let visuals = egui::Visuals::dark();
    assert_eq!(state_color("false", true, &visuals), None);
    assert!(state_color("true", true, &visuals).is_some());
    let debug = theme::level_color(&visuals, LogLevel::Debug);
    assert_eq!(state_color("DEBUG", false, &visuals), Some(debug));
    assert_eq!(state_color("debug", false, &visuals), Some(debug));
}
