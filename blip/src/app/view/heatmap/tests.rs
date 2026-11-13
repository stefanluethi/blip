use super::super::ViewContext;
use super::*;
use crate::app::model::traces::Traces;
use crate::app::time::TimeRange;
use egui_kittest::kittest::Queryable;
use proto::Data;

#[test]
fn colors_follow_the_scale() {
    assert_eq!(color_of(0.0, (0.0, 1.0)), COLOR_MAP[0]);
    assert_eq!(color_of(1.0, (0.0, 1.0)), COLOR_MAP[8]);
    assert_eq!(color_of(5.0, (0.0, 1.0)), COLOR_MAP[8]);
    assert_eq!(color_of(0.5, (0.0, 1.0)), COLOR_MAP[4]);
    assert_eq!(color_of(f64::NAN, (0.0, 1.0)), Color32::TRANSPARENT);
    assert_eq!(color_of(3.0, (3.0, 3.0)), COLOR_MAP[4]);
}

#[test]
fn value_range_skips_nulls() {
    assert_eq!(value_range(&[f64::NAN, 2.0, -1.0]), Some((-1.0, 2.0)));
    assert_eq!(value_range(&[f64::NAN]), None);
}

#[test]
fn over_time_holds_each_sample_until_the_next() {
    let mut traces = Traces::default();
    for (second, elements) in [(1, [1.0, 2.0]), (3, [3.0, 4.0])] {
        traces.append(&proto::Sample {
            timestamp: second * 1_000_000_000,
            path: "a".to_owned(),
            value: Data::Array(elements.iter().copied().map(Data::Float).collect()),
        });
    }
    let trace = traces.get("a").unwrap();
    // columns centered at 0.5, 1.5, ... 5.5 seconds
    let grid = over_time(trace, 2, (0.0, 6.0), 4.0, 6, 0);
    let bits = |values: &[f64]| {
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    };
    let nan = f64::NAN;
    // element 1 is the top row
    assert_eq!(bits(&grid[..6]), bits(&[nan, 2.0, 2.0, 4.0, nan, nan]));
    assert_eq!(bits(&grid[6..]), bits(&[nan, 1.0, 1.0, 3.0, nan, nan]));
}

fn frame_traces() -> Traces {
    let mut traces = Traces::default();
    for (second, elements) in [(1, [1.0, 2.0]), (3, [3.0, 4.0])] {
        traces.append(&proto::Sample {
            timestamp: second * 1_000_000_000,
            path: "a".to_owned(),
            value: Data::Array(elements.iter().copied().map(Data::Float).collect()),
        });
    }
    traces
}

#[test]
fn the_tooltips_of_a_heatmap_describe_its_trace() {
    use super::super::tooltip::Footer;
    use crate::app::time::TimeBase;
    let traces = frame_traces();
    let a = traces.get("a").unwrap();
    let mut text = ValueText {
        format: DisplayFormat::Default,
        unit: None,
        labels: None,
        description: Some("Thermal frame"),
    };
    let footer = Some(Footer {
        name: "frame".into(),
        description: "Thermal frame".into(),
    });
    assert_eq!(
        cell_tooltip("frame", text, (0, 0), Number::Float(1.0)).footer,
        footer
    );
    assert_eq!(
        element_tooltip("frame", "thermal/frame", text, 0, Number::Float(1.0)).footer,
        footer
    );
    let band = band_tooltip(a, text, TimeBase::SinceBoot, 2_000_000_000, 1);
    assert_eq!(
        band.footer.map(|footer| footer.description),
        Some("Thermal frame".into())
    );
    text.description = None;
    assert_eq!(
        cell_tooltip("frame", text, (0, 0), Number::Null).footer,
        None
    );
}

#[test]
fn the_tooltips_of_bands_cells_and_elements() {
    use crate::app::time::TimeBase;
    let traces = frame_traces();
    let a = traces.get("a").unwrap();
    let text = ValueText {
        format: DisplayFormat::Default,
        unit: Some("°C"),
        labels: None,
        description: Some("Thermal frame"),
    };
    let tooltip = band_tooltip(a, text, TimeBase::SinceBoot, 2_000_000_000, 1);
    assert_eq!(tooltip.header, Some(Header::Text("0:00:02.000".into())));
    assert_eq!(
        tooltip.groups,
        [vec![Row::new(
            Label::Weak("[1]".into()),
            [Value::text("2 °C")]
        )]]
    );
    let tooltip = band_tooltip(a, text, TimeBase::SinceBoot, 500_000_000, 0);
    assert_eq!(tooltip.groups[0][0].values, [Value::missing()]);

    let tooltip = cell_tooltip("frame", text, (1, 2), Number::Float(21.5));
    assert_eq!(tooltip.header, Some(Header::Text("[1][2]".into())));
    assert_eq!(
        tooltip.groups,
        [vec![Row::new(
            Label::Weak("frame".into()),
            [Value::text("21.5 °C")]
        )]]
    );
    let tooltip = cell_tooltip("frame", text, (1, 2), Number::Null);
    assert_eq!(tooltip.groups[0][0].values, [Value::faint("null")]);

    let tooltip = element_tooltip("frame", "thermal/frame", text, 3, Number::Float(0.5));
    assert_eq!(tooltip.header, Some(Header::Text("[3]".into())));
    let trace = TraceName {
        name: "frame".into(),
        path: "thermal/frame".into(),
        index: 0,
    };
    assert_eq!(
        tooltip.groups,
        [vec![Row::new(Label::Trace(trace), [Value::text("0.5 °C")])]]
    );
}

struct State {
    heatmap: Heatmap,
    model: crate::app::model::Model,
    time: TimeRange,
    invoker: crate::app::model::actions::Invoker,
}

/// A harness showing a heatmap of the trace `frame` (1D, `[0, 1, 2, 3]` V every second
/// to 40 s) or `grid` (2D, 3 × 3 of 10 * row + column °C at 0 and 1 s).
fn heatmap_harness(path: &str, mode: ArrayMode) -> egui_kittest::Harness<'static, State> {
    let mut model = crate::app::model::Model::default();
    for second in 0..=40 {
        model.traces.append(&proto::Sample {
            timestamp: second * 1_000_000_000,
            path: "frame".to_owned(),
            value: Data::Array((0..4).map(|i| Data::Float(f64::from(i))).collect()),
        });
    }
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "frame".into(),
        unit: "V".into(),
        ..Default::default()
    });
    for second in 0..2 {
        model.traces.append(&proto::Sample {
            timestamp: second * 1_000_000_000,
            path: "grid".to_owned(),
            value: Data::Array(
                (0..3)
                    .map(|r| {
                        Data::Array((0..3).map(|c| Data::Float(f64::from(10 * r + c))).collect())
                    })
                    .collect(),
            ),
        });
    }
    model.traces.set_metadata(&proto::TraceMetadata {
        path: "grid".into(),
        unit: "°C".into(),
        ..Default::default()
    });
    let heatmap = Heatmap {
        traces: [path.to_owned()].into(),
        mode,
        ..Heatmap::default()
    };
    let state = State {
        heatmap,
        model,
        time: Default::default(),
        invoker: Default::default(),
    };
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
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
                state.heatmap.view(ui, &mut context);
            },
            state,
        )
}

/// The texts of the tooltip after hovering the middle of the heatmap.
fn hovered_texts(mut harness: egui_kittest::Harness<'static, State>) -> Vec<String> {
    harness.run();
    harness.hover_at(egui::pos2(200.0, 150.0));
    harness.run_steps(40);
    super::super::tooltip::shown_texts(&harness)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

#[test]
fn hovering_a_heatmap_shows_the_framed_tooltip() {
    let index_of = |texts: &[String]| {
        texts
            .iter()
            .find_map(|text| {
                let index: usize = text.strip_prefix('[')?.strip_suffix(']')?.parse().ok()?;
                (index < 4).then_some(index)
            })
            .unwrap_or_else(|| panic!("an [i] in {texts:?}"))
    };

    let texts = hovered_texts(heatmap_harness("frame", ArrayMode::OverTime));
    let i = index_of(&texts);
    assert!(texts.contains(&format!("{i} V")), "{texts:?}");
    assert!(texts[0].starts_with("0:00:"), "{texts:?}");

    let mut harness = heatmap_harness("frame", ArrayMode::AtCursor);
    harness.run();
    harness.hover_at(egui::pos2(200.0, 150.0));
    harness.run_steps(40);
    assert!(harness.query_by_label_contains(" = ").is_none());
    let texts: Vec<String> = super::super::tooltip::shown_texts(&harness)
        .into_iter()
        .map(|(text, _)| text)
        .collect();
    let i = index_of(&texts);
    assert_eq!(texts[0], format!("[{i}]"));
    assert_eq!(texts[1], "frame");
    assert_eq!(texts[2], format!("{i} V"));

    let texts = hovered_texts(heatmap_harness("grid", ArrayMode::OverTime));
    let cell = texts
        .iter()
        .find_map(|text| {
            let (row, column) = text
                .strip_prefix('[')?
                .strip_suffix(']')?
                .split_once("][")?;
            Some((row.parse::<usize>().ok()?, column.parse::<usize>().ok()?))
        })
        .unwrap_or_else(|| panic!("a [r][c] in {texts:?}"));
    assert!(cell.0 < 3 && cell.1 < 3);
    let value = format!("{} °C", 10 * cell.0 + cell.1);
    assert!(texts.contains(&value), "{texts:?}");
}

#[test]
fn the_colour_scale_starts_from_the_expected_range() {
    let mut heatmap = Heatmap {
        expected: Some((0.0, 100.0)),
        fitted_range: Some((3.0, 7.0)),
        ..Heatmap::default()
    };
    assert_eq!(heatmap.shown_range(), (0.0, 100.0));
    heatmap.expected = None;
    assert_eq!(heatmap.shown_range(), (3.0, 7.0));
}

#[test]
fn a_fixed_range_starts_with_the_one_shown() {
    let mut heatmap = Heatmap {
        expected: Some((0.0, 100.0)),
        fitted_range: Some((3.0, 7.0)),
        ..Heatmap::default()
    };
    let shown = heatmap.shown_range();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(300.0, 200.0))
        .build_ui_state(
            move |ui, heatmap: &mut Heatmap| {
                egui::Grid::new("g").show(ui, |ui| {
                    super::super::range_rows(ui, &mut heatmap.value_range, shown);
                });
            },
            std::mem::take(&mut heatmap),
        );
    harness.run();
    harness.get_by_label("Fixed").click();
    harness.run();
    assert_eq!(harness.state().value_range, Some((0.0, 100.0)));
}
