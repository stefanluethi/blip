use super::*;
use crate::store::MemoryStore;
use crate::store::testing::append;
use crate::store::traces::Traces;
use crate::widgets::view::tests::{double_click, drag};
use egui_kittest::Harness;

/// A harness showing a time plot of data from 0 to 40 seconds, its state is the
/// shared time range.
fn time_plot_harness() -> Harness<'static, TimeRange> {
    time_plot_harness_stepping(1.0 / 4.0)
}

/// [`time_plot_harness`] with `step` seconds from frame to frame.
fn time_plot_harness_stepping(step: f32) -> Harness<'static, TimeRange> {
    let mut traces = Traces::default();
    for second in 0..=40 {
        append(
            &mut traces,
            second * 1_000_000_000,
            "a",
            proto::Data::Float(1.0),
        );
    }
    let mut model = MemoryStore::default();
    model.traces = traces;
    Harness::builder()
        .with_size(egui::vec2(400.0, 200.0))
        .with_step_dt(step)
        .build_ui_state(
            move |ui, time: &mut TimeRange| {
                time.update(&model, ui.input(|input| input.time));
                let id = ui.id().with("plot");
                show(id, plot(id, time, ui.visuals()), ui, time, false, |_, _| {});
            },
            TimeRange::default(),
        )
}

/// A click at `at`.
fn click(harness: &mut Harness<'_, TimeRange>, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run();
    harness.drag_at(at);
    harness.run();
    harness.drop_at(at);
    harness.run();
}

#[test]
fn clicking_a_time_plot_sets_the_cursor_which_can_be_dragged() {
    let mut harness = time_plot_harness();
    harness.run();
    assert!(harness.state().is_live());
    assert_eq!(harness.state().cursor(), Some(40.0));

    // the window from 20 to 40 seconds spans the plot
    let at = egui::pos2(300.0, 100.0);
    click(&mut harness, at);
    let time = harness.state();
    assert!(!time.is_live());
    let clicked = time.cursor().unwrap();
    assert!((30.0..40.0).contains(&clicked), "{clicked}");
    let range = time.range();

    // dragging the cursor moves it and not the window
    drag(&mut harness, at, egui::pos2(350.0, 100.0));
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
fn a_double_click_leaves_the_time_as_it_was() {
    // fast enough for double clicks
    let mut harness = time_plot_harness_stepping(1.0 / 60.0);
    harness.run();
    assert!(harness.state().is_live());

    // live stays live, the cursor follows the end
    double_click(&mut harness, egui::pos2(300.0, 100.0));
    let time = harness.state();
    assert!(time.is_live());
    assert!(!time.cursor_is_set());

    // a cursor put somewhere stays there
    click(&mut harness, egui::pos2(250.0, 100.0));
    let cursor = harness.state().cursor();
    let range = harness.state().range();
    assert!(harness.state().cursor_is_set());
    // a while later, so that this is no double click with the click before
    harness.run_steps(30);
    double_click(&mut harness, egui::pos2(100.0, 100.0));
    assert_eq!(harness.state().cursor(), cursor);
    assert_eq!(harness.state().range(), range);
}

#[test]
fn a_vertical_drag_pans_no_time() {
    let mut harness = time_plot_harness();
    harness.run();
    let range = harness.state().range();
    drag(
        &mut harness,
        egui::pos2(200.0, 40.0),
        egui::pos2(203.0, 160.0),
    );
    let time = harness.state();
    assert!(time.is_live());
    assert_eq!(time.range(), range);
}

#[test]
fn time_decimals_follow_the_zoom() {
    assert_eq!(time_decimals(2.0), 0);
    assert_eq!(time_decimals(0.05), 2);
    assert_eq!(time_decimals(0.002), 3);
    assert_eq!(time_decimals(1.0e-12), 9);
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
