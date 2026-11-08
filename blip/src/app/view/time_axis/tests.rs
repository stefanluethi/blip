use super::*;
use crate::app::model::Model;
use crate::app::model::traces::Traces;
use egui_kittest::Harness;

/// A harness showing a time plot of data from 0 to 40 seconds, its state is the
/// shared time range.
fn time_plot_harness() -> Harness<'static, TimeRange> {
    let mut traces = Traces::default();
    for second in 0..=40 {
        traces.append(&proto::Value {
            timestamp: second * 1_000_000_000,
            path: "a".to_owned(),
            value: proto::Data::Float(1.0),
        });
    }
    let mut model = Model::default();
    model.traces = traces;
    Harness::builder()
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui_state(
            move |ui, time: &mut TimeRange| {
                time.update(&model, ui.input(|input| input.time));
                let id = ui.id().with("plot");
                show(id, plot(id, time, ui.visuals()), ui, time, false, |_, _| {});
            },
            TimeRange::default(),
        )
}

/// Drags the pointer from `from` to `to` in steps.
fn drag(harness: &mut Harness<'_, TimeRange>, from: egui::Pos2, to: egui::Pos2) {
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
fn clicking_a_time_plot_sets_the_cursor_which_can_be_dragged() {
    let mut harness = time_plot_harness();
    harness.run();
    assert!(harness.state().is_live());
    assert_eq!(harness.state().cursor(), Some(40.0));

    // the window from 20 to 40 seconds spans the plot
    let click = egui::pos2(300.0, 100.0);
    harness.drag_at(click);
    harness.run();
    harness.drop_at(click);
    harness.run();
    let time = harness.state();
    assert!(!time.is_live());
    let clicked = time.cursor().unwrap();
    assert!((30.0..40.0).contains(&clicked), "{clicked}");
    let range = time.range();

    // dragging the cursor moves it and not the window
    drag(&mut harness, click, egui::pos2(350.0, 100.0));
    let time = harness.state();
    assert!(time.cursor().unwrap() > clicked + 1.0);
    assert_eq!(time.range(), range);

    // dragging elsewhere pans the window and leaves the cursor
    let cursor = time.cursor();
    drag(
        &mut harness,
        egui::pos2(100.0, 100.0),
        egui::pos2(150.0, 100.0),
    );
    let time = harness.state();
    assert_eq!(time.cursor(), cursor);
    assert_ne!(time.range(), range);
}

#[test]
fn scrolling_a_time_plot_zooms_the_window() {
    let mut harness = time_plot_harness();
    harness.run();
    let window = |harness: &Harness<'_, TimeRange>| {
        let (start, end) = harness.state().range().unwrap();
        end - start
    };
    let before = window(&harness);

    // scrolling up shows a shorter window
    super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 100.0), 100.0);
    let zoomed_in = window(&harness);
    assert!(zoomed_in < before * 0.9, "{zoomed_in} {before}");

    // scrolling down shows a longer window
    super::super::scroll_wheel(&mut harness, egui::pos2(200.0, 100.0), -200.0);
    assert!(window(&harness) > zoomed_in * 1.1);
}

#[test]
fn closest_index_picks_the_closest_timestamp() {
    let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
    assert_eq!(closest_index(&timestamps, 2_400_000), Some(2));
    assert_eq!(closest_index(&timestamps, 2_600_000), Some(3));
    assert_eq!(closest_index(&timestamps, 1_000_000_000), Some(9));
    assert!(closest_index(&[], 1).is_none());
}

#[test]
fn index_at_picks_the_current_sample() {
    let timestamps: Vec<u64> = (0..10).map(|i| i * 1_000_000).collect(); // 1 ms apart
    assert_eq!(index_at(&timestamps, 2_600_000), Some(2));
    assert_eq!(index_at(&timestamps, 3_000_000), Some(3));
    assert_eq!(index_at(&timestamps, 1_000_000_000), Some(9));
    assert!(index_at(&timestamps[1..], 0).is_none());
}

#[test]
fn visible_range_includes_neighbours() {
    let timestamps: Vec<u64> = (0..100).map(|i| i * 1_000_000).collect(); // 1 ms apart
    let ms = |ms: f64| (ms * 1.0e6) as u64;
    assert_eq!(
        visible_range(&timestamps, Some((ms(10.5), ms(20.5)))),
        10..22
    );
    assert_eq!(visible_range(&timestamps, None), 0..100);
    assert!(visible_range(&timestamps, Some((ms(1000.0), ms(2000.0)))).len() <= 1);
    assert!(within(&timestamps, (ms(1000.0), ms(2000.0))).is_empty());
}
