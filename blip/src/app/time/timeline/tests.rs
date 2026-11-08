use super::*;
use egui_kittest::Harness;

/// The time range, the data and where the timeline was shown.
type State = (TimeRange, Model, egui::Rect);

/// A harness showing the timeline over values from 0 to 100 seconds and an error at
/// 30 seconds, with a live window of 20 seconds.
fn harness() -> Harness<'static, State> {
    harness_until(100)
}

/// Like [`harness`], with values until `last` seconds.
fn harness_until(last: u64) -> Harness<'static, State> {
    let mut model = Model::default();
    let mut receive = |message: proto::Message| {
        model.receive("", &serde_cbor_2::to_vec(&message).unwrap());
    };
    for second in 0..=last {
        receive(proto::Message::Value(proto::Value {
            timestamp: second * 1_000_000_000,
            path: "a".to_owned(),
            value: proto::Data::Float(1.0),
        }));
    }
    receive(proto::Message::Log(proto::LogRecord {
        timestamp: 30_000_000_000,
        path: String::new(),
        level: 4,
        component: String::new(),
        message: "oops".to_owned(),
    }));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 60.0))
        .build_ui_state(
            |ui, (time, model, rect): &mut State| {
                time.update(model, 0.0);
                let top_left = ui.cursor().min;
                show(time, ui, model);
                *rect = egui::Rect::from_min_max(top_left, ui.min_rect().max);
            },
            (TimeRange::default(), model, egui::Rect::NOTHING),
        );
    harness.run();
    harness
}

/// The x coordinate of `t` seconds while the timeline shows all data, and the y
/// coordinates of the ruler, the middle of the track and the bars of the logs.
fn at(harness: &Harness<'_, State>, t: f64) -> (f32, f32, f32, f32) {
    let rect = harness.state().2;
    let x = rect.left() + (t / 100.0) as f32 * rect.width();
    // the track is as high as the controls, by default 18 points
    let ruler = rect.top() + 4.0;
    let track = rect.bottom() - 9.0;
    let logs = rect.bottom() - 2.0;
    (x, ruler, track, logs)
}

/// Scrolls the mouse wheel by `delta` points at `pos`, passed through
/// [`take_wheel`](super::super::take_wheel) as by the app.
fn wheel(harness: &mut Harness<'_, State>, pos: egui::Pos2, delta: egui::Vec2) {
    harness.hover_at(pos);
    harness.run();
    let ctx = harness.ctx.clone();
    let input = harness.input_mut();
    input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta,
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    super::super::take_wheel(&ctx, input);
    harness.run();
}

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

fn click(harness: &mut Harness<'_, State>, pos: egui::Pos2) {
    harness.hover_at(pos);
    harness.run();
    harness.drag_at(pos);
    harness.run();
    harness.drop_at(pos);
    harness.run();
}

fn assert_near(value: f64, expected: f64) {
    // a point of the timeline is about 0.1 seconds
    assert!((value - expected).abs() < 0.5, "{value} is not {expected}");
}

#[test]
fn dragging_the_start_of_the_thumb_lengthens_the_window_and_stays_live() {
    let mut harness = harness();
    let (from, _, y, _) = at(&harness, 80.0);
    let (to, ..) = at(&harness, 60.0);
    drag(&mut harness, egui::pos2(from, y), egui::pos2(to, y));
    let time = &harness.state().0;
    assert!(time.is_live() && time.is_playing());
    let (start, end) = time.range().unwrap();
    assert_near(start, 60.0);
    assert_eq!(end, 100.0);
}

#[test]
fn dragging_the_end_of_the_thumb_pauses_the_window_there() {
    let mut harness = harness();
    let (from, _, y, _) = at(&harness, 100.0);
    let (to, ..) = at(&harness, 90.0);
    drag(&mut harness, egui::pos2(from - 1.0, y), egui::pos2(to, y));
    let time = &harness.state().0;
    assert!(!time.is_live() && !time.is_playing());
    let (start, end) = time.range().unwrap();
    assert_near(start, 80.0);
    assert_near(end, 90.0);
}

#[test]
fn dragging_the_thumb_moves_the_window() {
    let mut harness = harness();
    let (from, _, y, _) = at(&harness, 90.0);
    let (to, ..) = at(&harness, 50.0);
    drag(&mut harness, egui::pos2(from, y), egui::pos2(to, y));
    let (start, end) = harness.state().0.range().unwrap();
    assert_near(start, 40.0);
    assert_near(end, 60.0);
}

#[test]
fn pressing_on_the_ruler_sets_the_cursor() {
    let mut harness = harness();
    let (x, y, ..) = at(&harness, 50.0);
    click(&mut harness, egui::pos2(x, y));
    let time = &harness.state().0;
    assert!(!time.is_live());
    assert_near(time.cursor().unwrap(), 50.0);
    // the window stays
    assert_eq!(time.range(), Some((80.0, 100.0)));
}

#[test]
fn clicking_the_bar_of_a_log_record_goes_to_it() {
    let mut harness = harness();
    // just after it, in its bin
    let (x, .., y) = at(&harness, 30.1);
    click(&mut harness, egui::pos2(x, y));
    let time = &harness.state().0;
    assert_eq!(time.cursor(), Some(30.0));
    assert_eq!(time.range(), Some((20.0, 40.0)));
}

#[test]
fn scrolling_zooms_the_timeline_and_not_the_window() {
    let mut harness = harness();
    let (x, _, y, _) = at(&harness, 50.0);
    wheel(&mut harness, egui::pos2(x, y), egui::vec2(0.0, 100.0));
    let time = &harness.state().0;
    assert_eq!(time.range(), Some((80.0, 100.0)));
    let (start, end) = view(time).unwrap();
    assert!(end - start < 90.0, "{start} {end}");
    // around the pointer
    assert!(start > 0.0 && end < 100.0 && start < 50.0 && end > 50.0);
    assert!(!time.overview.unwrap().follows);

    // scrolling sideways moves it within the data, at the end following it
    wheel(&mut harness, egui::pos2(x, y), egui::vec2(-2000.0, 0.0));
    let time = &harness.state().0;
    assert_eq!(view(time).unwrap().1, 100.0);
    assert!(time.overview.unwrap().follows);

    // zooming out as far as all data shows all of it again
    wheel(&mut harness, egui::pos2(x, y), egui::vec2(0.0, -400.0));
    assert_eq!(harness.state().0.overview, None);
    assert_eq!(view(&harness.state().0), Some((0.0, 100.0)));
}

#[test]
fn scrolling_a_little_at_a_time_zooms_in_from_all_data() {
    // 101 seconds of data, shown as 120
    let mut harness = harness_until(101);
    assert_eq!(view(&harness.state().0), Some((0.0, 120.0)));
    let (x, _, y, _) = at(&harness, 50.0);
    for _ in 0..10 {
        wheel(&mut harness, egui::pos2(x, y), egui::vec2(0.0, 5.0));
    }
    let (start, end) = view(&harness.state().0).unwrap();
    assert!(end - start < 101.0 * 0.85, "{start} {end}");
}

#[test]
fn zooming_into_the_shortest_span_does_not_panic() {
    // Unix times are too coarse to tell apart the ends of a window of 1 ms exactly
    let first = 1.7e9;
    for extra in [0.0, 1.0e-8, 1.0e-7, 2.0e-7, 5.0e-7, 1.0e-6] {
        let latest = first + MIN_WINDOW + extra;
        let mut time = TimeRange {
            span: Some((first, latest)),
            ..TimeRange::default()
        };
        let scale = Scale {
            x: egui::Rangef::new(0.0, 100.0),
            start: first,
            duration: latest - first,
        };
        for x in [0.0, 50.0, 100.0] {
            zoom(&mut time, &scale, x, 0.5);
            if let Some((start, end)) = view(&time) {
                assert!(start >= first && end <= latest + 1.0e-6, "{start} {end}");
            }
        }
    }
}

#[test]
fn ruler_ticks_are_round_in_the_shown_time() {
    // at whole hours since boot
    assert_eq!(first_tick(100.0, 3600.0, 0.0, 0), 3600.0);
    // at whole local hours half an hour ahead of UTC
    assert_eq!(first_tick(100.0, 3600.0, 1800.0, 0), 1800.0);
    assert_eq!(first_tick(1800.0, 3600.0, 1800.0, 0), 1800.0);
    // counting from an origin 10 minutes and 1 ns past a whole hour
    let origin = 1_700_002_800_000_000_000 + 600_000_000_001;
    let tick = first_tick(100.0, 3600.0, 0.0, origin);
    assert!((tick - (3000.0 - 1.0e-9)).abs() < 1.0e-6, "{tick}");
}

#[test]
fn a_zoomed_timeline_follows_the_newest_data_from_its_end() {
    let mut time = TimeRange {
        span: Some((0.0, 100.0)),
        ..TimeRange::default()
    };
    time.overview = Some(Overview {
        start: 70.0,
        duration: 30.0,
        follows: true,
    });
    time.span = Some((0.0, 110.0));
    assert_eq!(view(&time), Some((80.0, 110.0)));
    time.overview = Some(Overview {
        start: 20.0,
        duration: 30.0,
        follows: false,
    });
    assert_eq!(view(&time), Some((20.0, 50.0)));
}

#[test]
fn showing_all_data_rescales_in_steps() {
    assert_eq!(fit_duration(100.0), 100.0);
    assert_eq!(fit_duration(101.0), 120.0);
    assert_eq!(fit_duration(37.0), 40.0);
    assert_eq!(fit_duration(39.9), 40.0);
}

#[test]
fn the_filling_newest_bin_does_not_rescale_the_bars() {
    let value = |seconds: f64| {
        serde_cbor_2::to_vec(&proto::Message::Value(proto::Value {
            timestamp: (seconds * 1.0e9).round() as u64,
            path: "a".to_owned(),
            value: proto::Data::Float(1.0),
        }))
        .unwrap()
    };
    let mut model = Model::default();
    // 10 values a second for 10 seconds, in bins of a second
    for tenth in 0..100 {
        model.receive("", &value(f64::from(tenth) / 10.0));
    }
    let scale = Scale {
        x: egui::Rangef::new(0.0, 11.0 * MIN_BIN_WIDTH),
        start: 0.0,
        duration: 11.0,
    };
    let time = |latest| TimeRange {
        span: Some((0.0, latest)),
        ..TimeRange::default()
    };
    let bins = Bins::of(&model, &time(9.9), &scale);
    assert_eq!(bins.step, 1_000_000_000);
    assert_eq!(bins.newest_bin(), Some(9));
    assert_eq!(bins.scale_max(), (10.0, 0));

    // a burst early in the newest bin neither looks large nor rescales the others
    for thousandth in 0..30 {
        model.receive("", &value(10.0 + f64::from(thousandth) / 1000.0));
    }
    let bins = Bins::of(&model, &time(10.029), &scale);
    assert_eq!(bins.newest_bin(), Some(10));
    assert_eq!(bins.scale_max(), (10.0, 0));
    assert_eq!(bins.rate(10), 30.0);
}

#[test]
fn long_runs_without_samples_are_gaps() {
    // 70 samples in 10 seconds: a gap lasts 10 / 70 * 10 s, three bins of 0.5 s
    let mut counts = vec![5u32; 20];
    counts[3] = 0;
    counts[8..13].fill(0);
    let duration = 20.0 * 0.5;
    assert_eq!(gaps(&counts, duration), vec![(8..13)]);
    // also at the end, now five bins with fewer samples
    counts[15..].fill(0);
    assert_eq!(gaps(&counts, duration), [8..13, 15..20]);
    assert!(gaps(&[0, 0, 1, 0], 4.0).is_empty());
}

#[test]
fn sparse_samples_leave_no_gaps() {
    // a sample a second, binned by 10 ms
    let mut counts = vec![0u32; 1000];
    for second in 0..10 {
        counts[second * 100] = 1;
    }
    assert!(gaps(&counts, 10.0).is_empty());
}

#[test]
fn label_steps_are_round() {
    let steps = [
        (0.003, 0.005),
        (0.7, 1.0),
        (12.0, 20.0),
        (45.0, 60.0),
        (1000.0, 1800.0),
        (5000.0, 7200.0),
        (200_000.0, 259_200.0),
    ];
    for (min, step) in steps {
        let found = label_step(min);
        assert!((found - step).abs() < step * 1.0e-9, "{min}: {found}");
    }
}

#[test]
fn clamp_within_lets_the_lower_bound_win() {
    assert_eq!(clamp_within(5.0, 0.0, 10.0), 5.0);
    assert_eq!(clamp_within(-1.0, 0.0, 10.0), 0.0);
    assert_eq!(clamp_within(11.0, 0.0, 10.0), 10.0);
    assert_eq!(clamp_within(5.0, 10.0, 0.0), 10.0);
    assert_eq!(clamp_within(f64::NAN, 0.0, 10.0), 10.0);
}
