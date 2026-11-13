use super::*;

fn box_of(values: &[f64], whiskers: Whiskers) -> BoxStats {
    BoxStats::of(values.iter().copied(), whiskers).expect("a box")
}

#[test]
fn quartiles_interpolate_between_the_samples_as_numpy() {
    // np.percentile([1, 2, 3, 4], [25, 50, 75]) is 1.75, 2.5, 3.25
    let stats = box_of(&[4.0, 1.0, 3.0, 2.0], Whiskers::Iqr);
    assert_eq!((stats.q1, stats.median, stats.q3), (1.75, 2.5, 3.25));
    assert_eq!((stats.min, stats.max, stats.mean), (1.0, 4.0, 2.5));
    // np.percentile([7, 15, 36, 39, 40, 41], [25, 50, 75]) is 20.25, 37.5, 39.75
    let stats = box_of(&[41.0, 7.0, 39.0, 15.0, 40.0, 36.0], Whiskers::Iqr);
    assert_eq!((stats.q1, stats.median, stats.q3), (20.25, 37.5, 39.75));
    // on an order statistic, the sample itself
    let sorted: Vec<f64> = (0..=100).map(f64::from).collect();
    for p in [0.0, 0.01, 0.05, 0.5, 0.95, 0.99, 1.0] {
        assert_eq!(quantile(&sorted, p), 100.0 * p);
    }
    assert_eq!(quantile(&[3.0], 0.25), 3.0);
}

#[test]
fn whiskers_end_at_the_fences_the_percentiles_or_the_extremes() {
    let mut values: Vec<f64> = (1..=20).map(f64::from).collect();
    values.extend([-50.0, 100.0]);
    // 1.5×IQR: the furthest samples within 1.5 box lengths of the box
    let stats = box_of(&values, Whiskers::Iqr);
    let reach = 1.5 * (stats.q3 - stats.q1);
    assert!(stats.q1 - reach <= 1.0 && stats.q3 + reach >= 20.0);
    assert!(stats.q1 - reach > -50.0 && stats.q3 + reach < 100.0);
    assert_eq!((stats.low, stats.high), (1.0, 20.0));
    assert_eq!(stats.outliers, [-50.0, 100.0]);
    assert_eq!((stats.min, stats.max, stats.samples), (-50.0, 100.0, 22));
    // Min–max: the extremes, without outliers
    let stats = box_of(&values, Whiskers::MinMax);
    assert_eq!((stats.low, stats.high), (-50.0, 100.0));
    assert!(stats.outliers.is_empty());
    // the percentiles, with the samples beyond them as outliers
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    for (whiskers, (low, high)) in [(Whiskers::P5, (0.05, 0.95)), (Whiskers::P1, (0.01, 0.99))] {
        let stats = box_of(&values, whiskers);
        assert_eq!(
            (stats.low, stats.high),
            (quantile(&sorted, low), quantile(&sorted, high))
        );
        let beyond: Vec<f64> = (sorted.iter().copied())
            .filter(|&value| value < stats.low || value > stats.high)
            .collect();
        assert_eq!(stats.outliers, beyond);
        assert!(stats.outliers.contains(&-50.0) && stats.outliers.contains(&100.0));
    }
}

#[test]
fn nulls_infinite_samples_and_equal_values() {
    // nulls, NaN in the samples, and infinite samples are not in the box
    let stats = box_of(
        &[f64::NAN, 1.0, f64::INFINITY, 3.0, f64::NEG_INFINITY, 2.0],
        Whiskers::Iqr,
    );
    assert_eq!((stats.samples, stats.min, stats.max), (3, 1.0, 3.0));
    assert!(BoxStats::of([f64::NAN].into_iter(), Whiskers::Iqr).is_none());
    assert!(BoxStats::of(std::iter::empty(), Whiskers::Iqr).is_none());
    // all of one value: the box is the median alone, with the mean on it
    for whiskers in Whiskers::ALL {
        let stats = box_of(&[2.5; 7], whiskers);
        let ends = [
            stats.min, stats.low, stats.q1, stats.q3, stats.high, stats.max,
        ];
        assert!(ends.iter().all(|&end| end == 2.5), "{whiskers:?}");
        assert_eq!((stats.median, stats.mean), (2.5, 2.5));
        assert!(stats.outliers.is_empty());
    }
    // far from zero, the mean stays precise
    let stats = box_of(&[1.0e12 + 1.0, 1.0e12 + 2.0], Whiskers::Iqr);
    assert_eq!(stats.mean, 1.0e12 + 1.5);
}

#[test]
fn a_box_wholly_beyond_the_value_axis_points_to_its_edge() {
    let stats = box_of(&[10.0, 11.0, 12.0, 13.0], Whiskers::Iqr);
    assert_eq!(beyond(&stats, (0.0, 5.0)), Some(true));
    assert_eq!(beyond(&stats, (20.0, 30.0)), Some(false));
    // a box partly shown is cut at the edge instead
    assert_eq!(beyond(&stats, (11.0, 30.0)), None);
    assert_eq!(beyond(&stats, (0.0, 11.0)), None);
}

#[test]
fn a_box_is_40_percent_of_its_row_at_most_24_pt() {
    let frame =
        |height: f32| egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, height));
    for (height, count, thickness) in [(200.0, 4, 20.0), (300.0, 1, 24.0), (60.0, 6, 6.0)] {
        let rows = Rows::boxes(Orientation::Upright, frame(height), count);
        assert_eq!(rows.style.thickness, thickness, "{height} {count}");
        assert_eq!(rows.style.cap, (thickness / 2.0).min(MAX_CAP));
    }
    // each trace an equal row, from the top down upright, from the left sideways
    let rows = Rows::boxes(Orientation::Upright, frame(200.0), 4);
    assert_eq!(rows.length(), 50.0);
    assert_eq!(rows.centre(0), 45.0);
    assert_eq!(rows.at(egui::pos2(100.0, 130.0)), Some(2));
    assert_eq!(rows.rect(2).x_range(), frame(200.0).x_range());
    let rows = Rows::boxes(Orientation::Sideways, frame(200.0), 3);
    assert_eq!((rows.length(), rows.centre(0)), (100.0, 60.0));
    assert_eq!(rows.at(egui::pos2(250.0, 100.0)), Some(2));
    assert_eq!(rows.at(egui::pos2(250.0, 300.0)), None);
    // in the strip of Both, 10 pt boxes in 14 pt rows from the end of the counts
    let strip = Rows::strip(Orientation::Upright, frame(200.0), 2);
    assert_eq!((strip.length(), strip.style.thickness), (14.0, 10.0));
    assert_eq!(strip.centre(0), 20.0 + MARKER_INSET + 7.0);
    assert_eq!(
        strip.at(egui::pos2(100.0, 20.0 + MARKER_INSET + 20.0)),
        Some(1)
    );
    assert_eq!(strip.at(egui::pos2(100.0, 100.0)), None);
    let strip = Rows::strip(Orientation::Sideways, frame(200.0), 2);
    assert_eq!(strip.centre(0), 310.0 - MARKER_INSET - 7.0);
    assert_eq!(strip.rect(1).width(), 14.0);
}

#[test]
fn the_tooltip_lists_the_values_in_the_order_drawn() {
    let stats = box_of(&[1.0, 2.0, 3.0, 4.0, 5.0, 30.0], Whiskers::Iqr);
    let names = |whiskers, orientation| -> Vec<&str> {
        (tooltip_rows(&stats, whiskers, orientation).into_iter())
            .map(|(name, _, _)| name)
            .collect()
    };
    let upright = [
        "Min",
        "Low whisker",
        "Q1",
        "Median",
        "Mean",
        "Q3",
        "High whisker",
        "Max",
    ];
    assert_eq!(names(Whiskers::Iqr, Orientation::Upright), upright);
    let mut sideways = upright;
    sideways.reverse();
    assert_eq!(names(Whiskers::Iqr, Orientation::Sideways), sideways);
    assert_eq!(
        names(Whiskers::MinMax, Orientation::Upright),
        ["Min", "Q1", "Median", "Mean", "Q3", "Max"]
    );
    assert_eq!(names(Whiskers::P5, Orientation::Upright)[1], "5 %");
    assert_eq!(names(Whiskers::P5, Orientation::Upright)[6], "95 %");
    assert_eq!(names(Whiskers::P1, Orientation::Sideways)[1], "99 %");
    // the box in the text colour, its ends weak
    let rows = tooltip_rows(&stats, Whiskers::Iqr, Orientation::Upright);
    let weights: Vec<Weight> = rows.iter().map(|&(_, _, weight)| weight).collect();
    assert_eq!(weights[2..6], [Weight::Box; 4]);
    assert!([0, 1, 6, 7].iter().all(|&row| weights[row] == Weight::End));
    assert_eq!(rows[7].1, 30.0);
    // written to a thousandth of the range
    assert_eq!(value_step(&stats), 0.02);
    assert_eq!(value_step(&box_of(&[0.0], Whiskers::Iqr)), 1.0);
}

#[test]
fn the_tooltip_of_a_box_describes_the_trace() {
    use crate::app::view::tooltip::TraceName;
    let name = TraceName::new("x", "pose.x", 0);
    let tip = |description| Tip {
        trace: name.clone(),
        stats: None,
        whiskers: Whiskers::Iqr,
        orientation: Orientation::Upright,
        format: DisplayFormat::Default,
        unit: None,
        description,
    };
    let footer = tip(Some("Pose x")).tooltip().footer.expect("a footer");
    assert_eq!(
        (footer.name.as_str(), footer.description.as_str()),
        ("x", "Pose x")
    );
    assert_eq!(tip(None).tooltip().footer, None);
}

#[test]
fn the_tooltip_of_a_box_names_the_trace_and_splits_off_the_counts() {
    use crate::app::view::tooltip::{Header, Label, Tone, TraceName, Value};
    let x = TraceName {
        name: "x".into(),
        path: "pose.x".into(),
        index: 0,
    };
    let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
    let tip = |stats: Option<&BoxStats>, whiskers| {
        Tip {
            trace: x.clone(),
            stats,
            whiskers,
            orientation: Orientation::Upright,
            format: DisplayFormat::Default,
            unit: Some("ms"),
            description: Some("Pose x"),
        }
        .tooltip()
    };
    let stats = box_of(&samples, Whiskers::Iqr);
    let tooltip = tip(Some(&stats), Whiskers::Iqr);
    assert_eq!(tooltip.header, Some(Header::Trace(x.clone())));
    assert_eq!(tooltip.groups.len(), 2);
    let names: Vec<&str> = (tooltip.groups[0].iter())
        .map(|row| match &row.label {
            Label::Weak(name) => name.as_str(),
            Label::Trace(_) => panic!("a trace"),
        })
        .collect();
    assert_eq!(
        names,
        [
            "Min",
            "Low whisker",
            "Q1",
            "Median",
            "Mean",
            "Q3",
            "High whisker",
            "Max"
        ]
    );
    let values: Vec<(&str, Tone)> = (tooltip.groups[0].iter())
        .map(|row| (row.values[0].text.as_str(), row.values[0].tone))
        .collect();
    let (weak, text) = (Tone::Weak, Tone::Text);
    assert_eq!(
        values,
        [
            ("1.000 ms", weak),
            ("1.000 ms", weak),
            ("2.000 ms", text),
            ("3.000 ms", text),
            ("3.000 ms", text),
            ("4.000 ms", text),
            ("5.000 ms", weak),
            ("5.000 ms", weak),
        ]
    );
    assert_eq!(
        tooltip.groups[1]
            .iter()
            .map(|row| &row.values[0])
            .collect::<Vec<_>>(),
        [&Value::text("5"), &Value::faint("0")]
    );
    // Min–max leaves out the whiskers and the outliers
    let stats = box_of(&samples, Whiskers::MinMax);
    let tooltip = tip(Some(&stats), Whiskers::MinMax);
    assert_eq!(tooltip.groups[0].len(), 6);
    assert_eq!(tooltip.groups[1].len(), 1);
    // without samples
    let tooltip = tip(None, Whiskers::Iqr);
    assert_eq!(tooltip.note.as_deref(), Some("No samples"));
    assert!(tooltip.groups.is_empty());
}
