use super::super::ViewContext;
use super::*;
use crate::store::traces::Traces;
use crate::widgets::time::TimeRange;
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
    use crate::widgets::time::TimeBase;
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
        text.tooltip(
            "[0][0]".into(),
            Label::Weak("frame".into()),
            Some(Number::Float(1.0)),
            "frame"
        )
        .footer,
        footer
    );
    assert_eq!(
        text.tooltip(
            "[0]".into(),
            Label::Trace(TraceName::new("frame", "thermal/frame", 0)),
            Some(Number::Float(1.0)),
            "frame"
        )
        .footer,
        footer
    );
    let band = band_tooltip(a, text, TimeBase::SinceBoot, 2_000_000_000, 1);
    assert_eq!(
        band.footer.map(|footer| footer.description),
        Some("Thermal frame".into())
    );
    text.description = None;
    assert_eq!(
        text.tooltip(
            "[0][0]".into(),
            Label::Weak("frame".into()),
            Some(Number::Null),
            "frame"
        )
        .footer,
        None
    );
}

#[test]
fn the_tooltips_of_bands_cells_and_elements() {
    use crate::widgets::time::TimeBase;
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

    let tooltip = text.tooltip(
        "[1][2]".into(),
        Label::Weak("frame".into()),
        Some(Number::Float(21.5)),
        "frame",
    );
    assert_eq!(tooltip.header, Some(Header::Text("[1][2]".into())));
    assert_eq!(
        tooltip.groups,
        [vec![Row::new(
            Label::Weak("frame".into()),
            [Value::text("21.5 °C")]
        )]]
    );
    let tooltip = text.tooltip(
        "[1][2]".into(),
        Label::Weak("frame".into()),
        Some(Number::Null),
        "frame",
    );
    assert_eq!(tooltip.groups[0][0].values, [Value::faint("null")]);

    let label = Label::Trace(TraceName::new("frame", "thermal/frame", 0));
    let tooltip = text.tooltip("[3]".into(), label, Some(Number::Float(0.5)), "frame");
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
    model: crate::store::MemoryStore,
    time: TimeRange,
    invoker: crate::store::actions::Invoker,
}

/// A harness showing a heatmap of the trace `frame` (1D, `[0, 1, 2, 3]` V every second
/// to 40 s) or `grid` (2D, 3 × 3 of 10 * row + column °C at 0 and 1 s).
fn heatmap_harness(path: &str, mode: ArrayMode) -> egui_kittest::Harness<'static, State> {
    let mut model = crate::store::MemoryStore::default();
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
                    store: &state.model,
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
fn the_colour_scale_fits_the_values_or_the_expected_range_grown_by_them() {
    let mut heatmap = Heatmap {
        expected: Some((0.0, 100.0)),
        fitted_range: Some((-5.0, 7.0)),
        ..Heatmap::default()
    };
    assert_eq!(heatmap.shown_range(), (-5.0, 100.0));
    heatmap.fit = Fit::Values;
    assert_eq!(heatmap.shown_range(), (-5.0, 7.0));
}

#[test]
fn each_end_of_the_colour_scale_is_set_alone_and_wrong_ends_are_left_out() {
    let mut heatmap = Heatmap {
        expected: Some((0.0, 100.0)),
        fitted_range: Some((3.0, 7.0)),
        ..Heatmap::default()
    };
    heatmap.value_range.min = Some(10.0);
    assert_eq!(heatmap.shown_range(), (10.0, 100.0));
    heatmap.value_range.max = Some(50.0);
    assert_eq!(heatmap.shown_range(), (10.0, 50.0));
    // a Min past the Max fits the values again, as the properties mark it
    heatmap.value_range.min = Some(60.0);
    assert_eq!(heatmap.shown_range(), (0.0, 100.0));
    let [min, max] = heatmap.value_range.problems(heatmap.fitted());
    assert_eq!(min.as_deref(), Some("Min must be below Max, 50"));
    assert_eq!(max.as_deref(), Some("Max must be above Min, 60"));
}

#[test]
fn heatmaps_saved_with_a_fixed_or_no_range_load_to_the_same_scale() {
    let fixed: Heatmap =
        serde_json::from_str(r#"{"traces": ["a"], "value_range": [1.0, 5.0]}"#).unwrap();
    assert_eq!(fixed.value_range, Ends::fixed((1.0, 5.0)));
    let fitted = Heatmap {
        expected: Some((0.0, 100.0)),
        fitted_range: Some((3.0, 7.0)),
        ..fixed
    };
    assert_eq!(fitted.shown_range(), (1.0, 5.0));
    // an automatic range took the expected one, else the values, as now
    for json in [r#"{"value_range": null}"#, r#"{"traces": ["a"]}"#] {
        let auto: Heatmap = serde_json::from_str(json).unwrap();
        assert_eq!(
            (auto.value_range, auto.fit),
            (Ends::default(), Fit::Expected)
        );
        let mut auto = Heatmap {
            expected: Some((0.0, 100.0)),
            fitted_range: Some((3.0, 7.0)),
            ..auto
        };
        assert_eq!(auto.shown_range(), (0.0, 100.0));
        auto.expected = None;
        assert_eq!(auto.shown_range(), (3.0, 7.0));
        // the line at the cursor time as before: solid, every element marked
        assert_eq!(auto.points, ShowPoints::Always);
        assert_eq!(auto.line_width, 1.5);
        assert!(auto.line_styles.is_empty());
    }
    let set = Heatmap {
        value_range: Ends {
            min: None,
            max: Some(9.0),
        },
        fit: Fit::Values,
        points: ShowPoints::Never,
        line_width: 3.0,
        ..Heatmap::default()
    };
    let json = serde_json::to_string(&set).unwrap();
    let loaded: Heatmap = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
}

#[test]
fn a_grid_fills_its_frame_with_square_cells_only_while_they_fill_half_of_it() {
    // 4 × 4 cells in a frame of 200 × 100: square cells of 25 fill half, so they stretch
    let [x, y] = grid_bounds(egui::vec2(200.0, 100.0), (4, 4));
    assert_eq!((x, y), ((-0.5, 3.5), (-3.5, 0.5)));
    // in a frame of 150 × 100 they fill two thirds: square, the columns centred
    let [x, y] = grid_bounds(egui::vec2(150.0, 100.0), (4, 4));
    assert_eq!(y, (-3.5, 0.5));
    assert!((x.1 - x.0 - 6.0).abs() < 1.0e-9, "{x:?}");
    assert!((x.0 + x.1 - 3.0).abs() < 1.0e-9, "{x:?}");
    // a small array in a tall card is stretched to fill it
    let [x, y] = grid_bounds(egui::vec2(80.0, 200.0), (8, 8));
    assert_eq!((x, y), ((-0.5, 7.5), (-7.5, 0.5)));
    // but its cells get no more than three times as high as wide: 10 × 30 in a card
    // of 80 × 640, which shows 21⅓ rows, centred
    let [x, y] = grid_bounds(egui::vec2(80.0, 640.0), (8, 8));
    assert_eq!(x, (-0.5, 7.5));
    assert!((y.1 - y.0 - 640.0 / 30.0).abs() < 1.0e-9, "{y:?}");
    assert!((y.0 + y.1 + 7.0).abs() < 1.0e-9, "{y:?}");
}

/// The y range the line at the cursor shows.
fn line_y(harness: &egui_kittest::Harness<'static, State>) -> (f64, f64) {
    let heatmap = &harness.state().heatmap;
    (heatmap.y_view)
        .or_else(|| heatmap.value_range.over(heatmap.fitted_y()))
        .expect("a y range")
}

#[test]
fn the_y_axis_at_the_cursor_stays_where_zoomed_until_a_double_click() {
    let mut harness = heatmap_harness("frame", ArrayMode::AtCursor);
    harness.run();
    // the values 0 to 3 with a margin
    let (low, high) = line_y(&harness);
    assert!((low + 0.15).abs() < 1.0e-9 && (high - 3.15).abs() < 1.0e-9);
    assert!(harness.state().heatmap.y_view.is_none());

    super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 150.0), 100.0);
    let (low, high) = harness.state().heatmap.y_view.expect("zoomed");
    assert!(high - low < 3.3 * 0.9, "{low} {high}");
    // it stays while the frame is shown again
    harness.run_steps(5);
    assert_eq!(harness.state().heatmap.y_view, Some((low, high)));

    let at = egui::pos2(200.0, 150.0);
    for pressed in [true, false, true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert!(harness.state().heatmap.y_view.is_none());
}
