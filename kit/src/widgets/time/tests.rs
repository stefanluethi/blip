use super::*;
use crate::store::MemoryStore;
use egui_kittest::kittest::NodeT;

fn time_range(first: f64, latest: f64, window: f64) -> TimeRange {
    TimeRange {
        window,
        span: Some((first, latest)),
        ..TimeRange::default()
    }
}

#[test]
fn time_base_is_guessed_from_the_value() {
    assert_eq!(TimeBase::guess(1_700_000_000_000_000_000), TimeBase::Unix);
    assert_eq!(TimeBase::guess(3_600_000_000_000), TimeBase::SinceBoot);
}

#[test]
fn time_since_boot_is_formatted_as_hours_minutes_seconds() {
    let format =
        |seconds: f64, decimals| TimeBase::SinceBoot.format(timestamp_at(0, seconds), decimals);
    assert_eq!(format(3723.4567, 3), "1:02:03.457");
    assert_eq!(format(12.3, 1), "0:00:12.3");
    assert_eq!(format(59.96, 1), "0:01:00.0");
    assert_eq!(format(90000.0, 1), "25:00:00.0");
    assert_eq!(TimeBase::SinceBoot.format(1, 9), "0:00:00.000000001");
}

#[test]
fn unix_time_is_formatted_as_time_of_day() {
    let text = TimeBase::Unix.format(1_700_000_000_250_000_000, 2);
    assert_eq!(text.len(), "hh:mm:ss.25".len());
    assert!(text.ends_with(":20.25"));
    // nanoseconds are kept also at Unix times
    let text = TimeBase::Unix.format(1_700_000_000_000_000_001, 9);
    assert!(text.ends_with(":20.000000001"), "{text}");
}

#[test]
fn unix_time_is_formatted_with_the_date_if_asked() {
    // 2023-11-14 22:13:20 UTC, the 14th or the 15th in local time
    let text = TimeBase::Unix.format_dated(1_700_000_000_250_000_000, 2);
    assert_eq!(text.len(), "yyyy-mm-dd hh:mm:ss.25".len());
    assert!(text.starts_with("2023-11-1"), "{text}");
    assert!(text.ends_with(":20.25"));
    // time since boot has no date
    let text = TimeBase::SinceBoot.format_dated(12_300_000_000, 1);
    assert_eq!(text, "0:00:12.3");
}

#[test]
fn times_count_from_the_origin_in_nanoseconds() {
    let origin = 1_700_000_000_000_000_000;
    assert_eq!(seconds_since(origin, origin + 1_500), 1.5e-6);
    assert_eq!(seconds_since(origin, origin - 2_000_000_000), -2.0);
    assert_eq!(timestamp_at(origin, 1.5e-6), origin + 1_500);
    assert_eq!(timestamp_at(5, -1.0), 0);
}

#[test]
fn live_window_ends_at_newest_data() {
    let time = time_range(100.0, 150.0, 20.0);
    assert_eq!(time.range(), Some((130.0, 150.0)));
}

#[test]
fn panning_back_leaves_live_mode_and_panning_to_the_end_returns() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.pan_zoom(120.0, 140.0);
    assert!(!time.is_live());
    assert_eq!(time.range(), Some((120.0, 140.0)));
    time.pan_zoom(135.0, 155.0);
    assert!(time.is_live());
    assert_eq!(time.range(), Some((130.0, 150.0)));
}

#[test]
fn zooming_keeps_live_mode() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.pan_zoom(135.0, 145.0);
    assert!(time.is_live());
    assert_eq!(time.range(), Some((140.0, 150.0)));
}

#[test]
fn zooming_beyond_the_window_limits_neither_pans_nor_pauses() {
    let mut time = time_range(100.0, 150.0, MIN_WINDOW);
    time.pan_zoom(149.0, 149.0 + MIN_WINDOW / 2.0);
    assert!(time.is_live());
    assert_eq!(time.range(), Some((150.0 - MIN_WINDOW, 150.0)));

    let mut time = time_range(100.0, 150.0, MAX_WINDOW);
    time.pause();
    let range = time.range();
    time.pan_zoom(120.0 - MAX_WINDOW, 130.0 + MAX_WINDOW);
    assert_eq!(time.range(), range);
}

#[test]
fn cursor_is_at_the_window_end_until_set() {
    let mut time = time_range(100.0, 150.0, 20.0);
    assert_eq!(time.cursor(), Some(150.0));
    time.pan_zoom(120.0, 140.0);
    assert_eq!(time.cursor(), Some(140.0));
    time.set_cursor(125.0);
    assert_eq!(time.cursor(), Some(125.0));
    time.set_cursor(10.0);
    assert_eq!(time.cursor(), Some(100.0));
}

#[test]
fn setting_the_cursor_stops_live_mode_until_going_live() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.set_cursor(140.0);
    assert!(!time.is_live());
    assert_eq!(time.range(), Some((130.0, 150.0)));
    // newer data does not move the window away from the cursor
    time.span = Some((100.0, 160.0));
    assert_eq!(time.range(), Some((130.0, 150.0)));
    assert_eq!(time.cursor(), Some(140.0));

    // until going live, also by panning to the end
    time.pan_zoom(145.0, 165.0);
    assert!(!time.is_live());
    assert_eq!(time.range(), Some((140.0, 160.0)));
    time.go_live();
    assert!(time.is_live());
    assert_eq!(time.cursor(), Some(160.0));
}

#[test]
fn going_to_a_time_moves_the_window_only_if_it_is_not_shown() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.go_to(140.0);
    assert_eq!(time.range(), Some((130.0, 150.0)));
    assert_eq!(time.cursor(), Some(140.0));
    time.go_to(110.0);
    assert_eq!(time.range(), Some((100.0, 120.0)));
    assert_eq!(time.cursor(), Some(110.0));
}

#[test]
fn durations_are_formatted_short_and_read_back() {
    assert_eq!(format_duration(0.0123), "12.3 ms");
    assert_eq!(format_duration(20.0), "20.0 s");
    assert_eq!(format_duration(1.5), "1.50 s");
    assert_eq!(format_duration(150.0), "2.50 min");
    assert_eq!(format_duration(7200.0), "2.00 h");
    assert_eq!(parse_duration("500 ms"), Some(0.5));
    assert_eq!(parse_duration("20"), Some(20.0));
    assert_eq!(parse_duration("2.5min"), Some(150.0));
    assert_eq!(parse_duration(" 1 h "), Some(3600.0));
    assert_eq!(parse_duration("2 weeks"), None);
    assert_eq!(parse_duration("250 us"), Some(250.0e-6));
    assert_eq!(parse_duration("250µs"), Some(250.0e-6));
    assert_eq!(parse_duration("3 sec"), Some(3.0));
    assert_eq!(parse_duration("2 hr"), Some(7200.0));
    assert_eq!(parse_duration("1 d"), Some(86400.0));
    assert_eq!(parse_duration("500 MS"), Some(0.5));
    assert_eq!(parse_duration("5 Min"), Some(300.0));
    assert_eq!(parse_duration("-5 s"), None);
    assert_eq!(parse_duration("fast"), None);
    assert_eq!(parse_duration("0"), Some(0.0));
    for seconds in [0.0123, 1.5, 20.0, 150.0, 7200.0] {
        let read = parse_duration(&format_duration(seconds)).unwrap();
        assert!((read - seconds).abs() < seconds * 1.0e-9, "{seconds}");
    }
}

#[test]
fn the_wheel_is_taken_for_the_timeline_only_while_it_is_hovered() {
    let wheel = |modifiers| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        modifiers,
    };
    let ctx = egui::Context::default();
    let line = ctx.options(|options| options.input_options.line_scroll_speed);
    let mut input = egui::RawInput {
        events: vec![wheel(egui::Modifiers::NONE)],
        ..Default::default()
    };
    take_wheel(&ctx, &mut input);
    assert_eq!(input.events.len(), 1);

    // taken in whole, with Shift sideways, but not with Ctrl, which zooms the UI
    ctx.data_mut(|data| data.insert_temp(timeline_hovered_id(), true));
    input.events = vec![
        wheel(egui::Modifiers::NONE),
        wheel(egui::Modifiers::SHIFT),
        wheel(egui::Modifiers::CTRL),
    ];
    take_wheel(&ctx, &mut input);
    assert_eq!(input.events, vec![wheel(egui::Modifiers::CTRL)]);
    let taken = ctx.data_mut(|data| data.remove_temp::<egui::Vec2>(timeline_wheel_id()));
    assert_eq!(taken, Some(egui::vec2(line, line)));
    // until hovered again
    ctx.data_mut(|data| data.insert_temp(timeline_hovered_id(), false));
    input.events = vec![wheel(egui::Modifiers::NONE)];
    take_wheel(&ctx, &mut input);
    assert_eq!(input.events.len(), 1);
}

#[test]
fn window_end_stays_within_data() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.pan_zoom(50.0, 70.0);
    assert_eq!(time.range(), Some((100.0, 120.0)));
}

#[test]
fn a_paused_window_stays_while_data_arrives_and_plays_on_from_there() {
    let mut time = time_range(100.0, 150.0, 20.0);
    assert!(time.playing);
    time.toggle_play_as(true);
    assert!(!time.is_live() && !time.playing);
    time.span = Some((100.0, 160.0));
    time.play_on(1.0);
    time.play_on(3.0);
    assert_eq!(time.range(), Some((130.0, 150.0)));

    time.toggle_play_as(true);
    time.play_on(4.0);
    time.play_on(6.5);
    assert!(time.playing && !time.is_live());
    assert_eq!(time.range(), Some((132.5, 152.5)));
    // the time while paused is skipped
    time.toggle_play_as(true);
    time.play_on(10.0);
    time.toggle_play_as(true);
    time.play_on(20.0);
    time.play_on(21.0);
    assert_eq!(time.range(), Some((133.5, 153.5)));
}

#[test]
fn playing_moves_the_cursor_along_and_goes_live_at_the_newest_data() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.pan_zoom(120.0, 140.0);
    time.set_cursor(130.0);
    time.toggle_play_as(true);
    time.play_on(0.0);
    time.play_on(5.0);
    assert_eq!(time.range(), Some((125.0, 145.0)));
    assert_eq!(time.cursor(), Some(135.0));
    time.play_on(10.0);
    assert!(time.is_live() && time.playing);
    assert_eq!(time.cursor(), Some(150.0));
}

#[test]
fn panning_and_setting_the_cursor_pause_and_zooming_plays_on() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.pan_zoom(120.0, 140.0);
    assert!(!time.playing);
    time.toggle_play_as(true);
    time.pan_zoom(130.0, 140.0);
    assert!(time.playing);
    assert_eq!(time.range(), Some((130.0, 140.0)));
    time.set_cursor(135.0);
    assert!(!time.playing);
    // setting the cursor while live pauses at the newest data
    let mut time = time_range(100.0, 150.0, 20.0);
    time.set_cursor(140.0);
    assert!(!time.playing);
    assert_eq!(time.range(), Some((130.0, 150.0)));
}

#[test]
fn zooming_while_paused_stays_paused_also_at_the_newest_data() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.pan_zoom(120.0, 140.0);
    time.pan_zoom(135.0, 165.0);
    assert!(!time.playing && !time.is_live());
    assert_eq!(time.range(), Some((120.0, 150.0)));
}

#[test]
fn the_buttons_pause_and_go_live() {
    fn time<'a>(harness: &'a egui_kittest::Harness<'_, (TimeRange, bool)>) -> &'a TimeRange {
        &harness.state().0
    }

    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 60.0))
        .build_ui_state(
            |ui, (time, recording): &mut (TimeRange, bool)| {
                time.ui(ui, &MemoryStore::default(), *recording, |_| ());
            },
            (time_range(100.0, 150.0, 20.0), true),
        );
    harness.run();
    harness.get_by_label(regular::PAUSE).click();
    harness.run();
    assert!(!time(&harness).playing);
    assert_eq!(time(&harness).range(), Some((130.0, 150.0)));
    // playing repaints each frame
    harness.get_by_label(regular::PLAY).click();
    harness.run_steps(2);
    assert!(time(&harness).playing);

    harness.get_by_label(regular::PAUSE).click();
    harness.step();
    harness.run();
    harness.get_by_label_contains("Live").click();
    harness.run();
    assert!(time(&harness).is_live() && time(&harness).playing);

    // without recording, nothing moves at the end of the data: Play shows, disabled,
    // and so is Live
    harness.state_mut().1 = false;
    harness.run();
    assert!(
        harness
            .get_by_label(regular::PLAY)
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_label_contains("Live")
            .accesskit_node()
            .is_disabled()
    );
    // away from the end, Live goes back there, as in an opened recording
    harness.state_mut().0.pan_zoom(110.0, 130.0);
    harness.run();
    assert!(!time(&harness).is_live());
    harness.get_by_label_contains("Live").click();
    harness.run();
    assert!(time(&harness).is_live());
}

/// A harness showing the controls of `time` while recording.
fn controls_harness(time: TimeRange) -> egui_kittest::Harness<'static, TimeRange> {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(600.0, 300.0))
        .build_ui_state(|ui, time: &mut TimeRange| time.controls_ui(ui, true), time);
    harness.run();
    harness
}

/// Types `text` into the window field of `harness` in place of what it holds, and
/// presses Enter.
fn enter_window(harness: &mut egui_kittest::Harness<'_, TimeRange>, text: &str) {
    use egui_kittest::kittest::Queryable;
    let field = || egui::accesskit::Role::TextInput;
    harness.get_by_role(field()).focus();
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.get_by_role(field()).type_text(text);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
}

#[test]
fn the_window_field_keeps_and_marks_what_it_cannot_read() {
    use egui_kittest::kittest::Queryable;
    let mut harness = controls_harness(time_range(100.0, 150.0, 20.0));
    enter_window(&mut harness, "5 Sec");
    assert_eq!(harness.state().window, 5.0);
    assert!(harness.query_all_by_value("5.00 s").next().is_some());

    // not a length: the text stays to correct it, the window too
    enter_window(&mut harness, "5 parsecs");
    assert_eq!(harness.state().window, 5.0);
    assert!(harness.query_all_by_value("5 parsecs").next().is_some());
    let hover = |harness: &mut egui_kittest::Harness<'_, TimeRange>, hint: &str| {
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .hover();
        harness.run_steps(3);
        harness.query_by_label_contains(hint).is_some()
    };
    assert!(hover(&mut harness, "Not a length: write a number with µs"));

    // too long: limited, and told so while it stays
    enter_window(&mut harness, "30 d");
    assert_eq!(harness.state().window, MAX_WINDOW);
    assert!(hover(&mut harness, "at least and"));
    harness.state_mut().window = 20.0;
    harness.run();
    assert!(hover(&mut harness, "Window: the length"));
}

#[test]
fn the_window_presets_set_round_lengths() {
    use egui_kittest::kittest::Queryable;
    let mut harness = controls_harness(time_range(100.0, 150.0, 20.0));
    harness.get_by_label(regular::CARET_DOWN).click();
    harness.run();
    harness.get_by_label("500 ms").click();
    harness.run();
    assert_eq!(harness.state().window, 0.5);
    for pair in WINDOW_PRESETS.windows(2) {
        assert!(pair[0].0 < pair[1].0);
        assert_eq!(parse_duration(pair[0].1), Some(pair[0].0));
    }
}

#[test]
fn live_releases_a_set_cursor_in_a_recording() {
    let mut time = time_range(100.0, 150.0, 20.0);
    assert!(!time.can_go_live(false));
    time.set_cursor(140.0);
    assert!(time.can_go_live(false));
    time.go_live();
    assert!(!time.cursor_is_set() && time.is_live());
    assert!(!time.can_go_live(false));
    assert!(time.can_go_live(true));
}

#[test]
fn releasing_the_cursor_keeps_the_window() {
    let mut time = time_range(100.0, 150.0, 20.0);
    time.go_to(110.0);
    let range = time.range();
    time.release_cursor();
    assert!(!time.cursor_is_set());
    assert_eq!(time.range(), range);
    assert_eq!(time.cursor(), range.map(|(_, end)| end));
}

#[test]
fn play_shows_only_while_something_moves_and_needs_data() {
    // an opened recording: live at its end, not recording, nothing moves
    let mut time = time_range(100.0, 150.0, 20.0);
    assert!(time.playing && !time.moves(false));
    assert!(!time.can_play(false));
    // while recording, live moves on
    assert!(time.moves(true) && time.can_play(true));
    // moved back, it plays from there
    time.pan_zoom(110.0, 130.0);
    assert!(!time.moves(false) && time.can_play(false));
    time.toggle_play_as(false);
    assert!(time.moves(false));
    // without data there is nothing to play
    let time = TimeRange::default();
    assert!(!time.can_play(true) && !time.can_play(false));
}

#[test]
fn the_window_shows_how_far_it_is_behind_the_newest_data() {
    let mut time = time_range(100.0, 150.0, 20.0);
    assert_eq!(time.behind(), None);
    time.pan_zoom(110.0, 130.0);
    assert_eq!(time.behind(), Some(20.0));
}

#[test]
fn window_and_cursor_in_nanoseconds() {
    let time = TimeRange {
        origin: 5,
        ..time_range(0.0, 10.0, 4.0)
    };
    assert_eq!(time.timestamps(), Some((6_000_000_005, 10_000_000_005)));
    assert_eq!(time.cursor_timestamp(), Some(10_000_000_005));
    let empty = TimeRange::default();
    assert_eq!(empty.timestamps(), None);
    assert_eq!(empty.cursor_timestamp(), None);
}

#[test]
fn times_count_nanoseconds_and_the_range_takes_the_base_of_the_model() {
    assert_eq!(seconds_since(0, 1_500_000_000), 1.5);
    let mut model = MemoryStore::default();
    let frame = |message| serde_cbor_2::to_vec(&message).unwrap();
    model.receive(
        "a",
        &frame(proto::Message::Hello(proto::Hello {
            version: 1,
            name: None,
            clock: Some(proto::Clock::Boot),
            sender: None,
        })),
    );
    model.receive(
        "a",
        &frame(proto::Message::Sample(proto::Sample {
            timestamp: 1_700_000_000_000_000_000,
            path: "rpm".to_owned(),
            value: proto::Data::Uint(1),
        })),
    );
    let mut range = TimeRange::default();
    range.update(&model, 0.0);
    assert_eq!(range.base(), TimeBase::SinceBoot);
}

#[test]
fn leaving_the_window_field_unchanged_keeps_a_length_set_meanwhile() {
    use egui_kittest::kittest::Queryable;
    let mut harness = controls_harness(time_range(100.0, 150.0, 20.0));
    harness
        .get_by_role(egui::accesskit::Role::TextInput)
        .focus();
    harness.run_steps(2);
    // e.g. a zoom of a plot with the wheel
    harness.state_mut().window = 3.0;
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(harness.state().window, 3.0);
    // typed text is still taken
    enter_window(&mut harness, "4 s");
    assert_eq!(harness.state().window, 4.0);
}
