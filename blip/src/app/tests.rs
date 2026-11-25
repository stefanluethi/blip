use super::*;

#[test]
fn the_message_rate_is_measured_per_second() {
    let mut rate = MessageRate::default();
    rate.update(10.0, 0);
    rate.update(10.5, 300);
    assert_eq!(rate.per_second, None);
    rate.update(11.0, 200);
    assert_eq!(rate.per_second, Some(500.0));
    rate.update(11.2, 50);
    assert_eq!(rate.per_second, Some(500.0));
}

#[test]
fn thousands_are_grouped() {
    assert_eq!(group_thousands(7), "7");
    assert_eq!(group_thousands(1284), "1\u{202f}284");
    assert_eq!(group_thousands(1_234_567), "1\u{202f}234\u{202f}567");
}

fn value(timestamp: u64) -> proto::Message {
    value_of("rpm", timestamp)
}

fn value_of(path: &str, timestamp: u64) -> proto::Message {
    proto::Message::Sample(proto::Sample {
        timestamp,
        path: path.to_owned(),
        value: proto::Data::Uint(1),
    })
}

fn unit() -> proto::Message {
    unit_of("rpm")
}

fn unit_of(path: &str) -> proto::Message {
    proto::Message::TraceMetadata(proto::TraceMetadata {
        path: path.to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    })
}

#[test]
fn a_recording_starts_only_while_connected_and_clears_the_data() {
    let mut state = State::default();
    assert!(!state.session.recording);
    state.session.inbox("a", 0.0).message(&unit());
    state.session.inbox("a", 0.0).message(&value(1_000_000_000));
    assert!(!state.session.store.has_samples());
    state.set_recording(true, false);
    assert!(!state.session.recording);

    state.set_recording(true, true);
    state.session.inbox("a", 0.0).message(&value(2_000_000_000));
    state.session.inbox("a", 0.0).message(&value(3_000_000_000));
    state.time.update(&state.session.store, 0.0);
    state.time.set_cursor(2.5);
    // stopping keeps the data, also when no longer connected
    state.set_recording(false, false);
    assert!(!state.session.recording);
    assert!(state.session.store.has_samples());

    state.set_recording(true, true);
    assert!(state.session.recording);
    assert!(!state.session.store.has_samples());
    assert_eq!(state.session.store.traces.unit("rpm"), Some("rpm"));
    assert!(state.time.is_live() && state.time.is_playing());
}

#[test]
fn the_record_button_is_disabled_until_connected() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 60.0))
        .build_ui_state(
            |ui, (state, connected): &mut (State, bool)| state.bottom_ui(ui, *connected),
            (State::default(), false),
        );
    harness.run();
    harness.get_by_label_contains("Record").click();
    harness.run();
    assert!(!harness.state().0.session.recording);

    harness.state_mut().1 = true;
    harness.run();
    harness.get_by_label_contains("Record").click();
    harness.run();
    assert!(harness.state().0.session.recording);

    harness.state_mut().1 = false;
    harness.run();
    harness.get_by_label_contains("Record").click();
    harness.run();
    assert!(!harness.state().0.session.recording);
}

#[test]
fn opening_a_recording_stops_recording_and_starts_afresh() {
    let mut recorded = Session::recording();
    recorded.inbox("a", 0.0).message(&value(1));
    let file = files::File {
        name: "a.blip".to_owned(),
        bytes: recorded.recorder.to_file(),
    };
    let mut app = App::default();
    app.state
        .session
        .inbox("old", 0.0)
        .message(&unit_of("temp"));
    app.state.session.recording = true;
    app.trace_search = "temp".to_owned();
    let generation = app.traces_generation;
    app.open_all(file).unwrap();
    assert!(!app.state.session.recording);
    assert!(app.state.session.store.has_samples());
    assert!(app.trace_search.is_empty());
    assert_ne!(app.traces_generation, generation);
    // the unit before is gone: clearing the data, which keeps the units and
    // actions to record them again, has none of the recording file to keep
    app.state.clear_data();
    assert_eq!(app.state.session.recorder.messages(), 0);
}

#[test]
fn the_first_connection_to_open_starts_afresh_unless_recording() {
    let mut state = State::default();
    state.set_recording(true, true);
    state.session.inbox("old", 0.0).message(&unit());
    state.session.inbox("old", 0.0).message(&value(1));
    // a connection opening while recording keeps the recording going
    assert!(!state.session.first_opened());
    assert!(state.session.recording && state.session.store.has_samples());

    state.set_recording(false, false);
    assert!(state.session.first_opened());
    assert!(state.session.store.traces.iter().next().is_none());
    assert_eq!(state.session.recorder.messages(), 0);
    state.session.inbox("new", 0.0).message(&unit());
    // a recording starts with the unit of the new connection only, not the old one
    state.set_recording(true, true);
    assert_eq!(state.session.recorder.messages(), 1);
}

#[test]
fn starting_afresh_opens_the_branches_of_the_traces_again() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut app = App {
        layout: layout::Layout::with_plot_and_logs(),
        ..App::default()
    };
    app.layout.select_view_showing_traces();
    app.state.set_recording(true, true);
    app.state
        .session
        .inbox("a", 0.0)
        .message(&value_of("motor/rpm", 1));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(|ui, app: &mut App| app.traces_panel_ui(ui), app);
    harness.run();
    assert!(harness.query_by_label_contains("rpm").is_some());

    // collapse the branch with its button left of its checkbox
    let motor = harness.get_by_label("motor").rect();
    let spacing = egui::Spacing::default();
    let x = motor.left() - spacing.item_spacing.x - spacing.indent / 2.0;
    let toggle = egui::pos2(x, motor.center().y);
    harness.hover_at(toggle);
    harness.drag_at(toggle);
    harness.drop_at(toggle);
    harness.run_steps(20);
    assert!(harness.query_by_label_contains("rpm").is_none());

    harness.state_mut().reset_traces_panel();
    harness.run();
    assert!(harness.query_by_label_contains("rpm").is_some());
}

#[test]
fn a_linked_histogram_shows_the_traces_of_the_plot_greyed() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};

    let saved = serde_json::from_str(
        r#"{"name": "", "kind": "Tabs", "active": 0, "children": [{"Group": {
            "name": "Motor", "kind": "Horizontal", "children": [
                {"View": {"name": "Speed", "kind": {"Plot": {"traces": ["motor/rpm"]}}}},
                {"View": {"name": "Spread", "kind": {"Distribution": {"link": 0}}}}
            ]}}]}"#,
    )
    .unwrap();
    let mut app = App {
        layout: layout::Layout::from_saved(saved),
        ..App::default()
    };
    assert!(app.layout.select_named("Spread"));
    app.state.set_recording(true, true);
    app.state
        .session
        .inbox("a", 0.0)
        .message(&value_of("motor/rpm", 1));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 400.0))
        .build_ui_state(
            |ui, app: &mut App| {
                app.layout.take_linked_traces();
                app.traces_panel_ui(ui);
            },
            app,
        );
    harness.run();
    harness.get_by_label("Shows the traces of the linked plot Speed");
    let rpm = harness.get_by(|node| {
        node.role() == egui::accesskit::Role::CheckBox
            && node.label().is_some_and(|label| label.contains("rpm"))
    });
    assert!(rpm.accesskit_node().is_disabled());
    assert_eq!(
        rpm.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}

#[test]
fn the_connections_open_from_the_chip_and_stay_open_in_their_menus() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .build_ui_state(|ui, app: &mut App| app.menu_ui(ui), App::default());
    harness.run();
    assert!(harness.query_by_label_contains("ws://").is_none());

    harness.get_by_label("Connections").click();
    harness.run();
    assert!(harness.query_by_label_contains("ws://").is_some());

    // the menu of a connection is part of the list, using it keeps the list open
    harness.get_by_label_contains("ws://").click_secondary();
    harness.run();
    harness.get_by_label_contains("Edit address").click();
    harness.run();
    assert!(
        harness
            .query_by_role(egui::accesskit::Role::TextInput)
            .is_some()
    );

    // a click elsewhere closes it
    let elsewhere = egui::pos2(20.0, 400.0);
    harness.hover_at(elsewhere);
    harness.drag_at(elsewhere);
    harness.drop_at(elsewhere);
    harness.run();
    assert!(harness.query_by_label_contains("ws://").is_none());
}

#[test]
fn a_recording_is_opened_across_frames_and_can_be_cancelled() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut recorded = Session::recording();
    for timestamp in 0..200_000 {
        recorded.inbox("a", 0.0).message(&value(timestamp));
    }
    let file = || files::File {
        name: "a.blip".to_owned(),
        bytes: recorded.recorder.to_file(),
    };
    let ctx = Context::default();
    let mut app = App::default();
    app.state
        .session
        .inbox("old", 0.0)
        .message(&unit_of("temp"));
    app.open(file()).unwrap();
    // the data shown stays until all is read
    app.continue_opening(&ctx);
    assert!(app.opening.is_some());
    assert!(!app.state.session.store.has_samples());
    let mut frames = 1;
    while app.opening.is_some() {
        app.continue_opening(&ctx);
        frames += 1;
    }
    assert!(frames > 1);
    assert!(app.state.session.store.has_samples());
    assert_eq!(app.state.session.recorder.messages(), 200_000);

    // connecting while reading cancels it
    let mut app = App::default();
    app.open(file()).unwrap();
    app.connections.connect_first(&ctx, &mut app.state.session);
    app.continue_opening(&ctx);
    assert!(app.opening.is_none());
    assert!(!app.state.session.store.has_samples());
    assert!(app.toasts.shown()[0].1.contains("cancelled by connecting"));

    // cancelled, the data shown stays
    let mut app = App::default();
    app.state.session.recording = true;
    app.state.session.inbox("old", 0.0).message(&value(1));
    app.open(file()).unwrap();
    assert!(!app.state.session.recording);
    app.continue_opening(&ctx);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| eframe::App::ui(app, ui, &mut Frame::_new_kittest()),
            app,
        );
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("Opening a.blip").is_some());
    harness.get_by_label(egui_phosphor::regular::X).click();
    harness.run_steps(2);
    assert!(harness.state().opening.is_none());
    assert!(harness.state().state.session.store.has_samples());
}

#[test]
fn a_recording_opened_in_part_is_toasted() {
    let mut recorded = Session::recording();
    for timestamp in 0..1000 {
        recorded.inbox("a", 0.0).message(&value(timestamp));
    }
    let mut bytes = recorded.recorder.to_file();
    bytes.truncate(bytes.len() / 2);
    let file = files::File {
        name: "a.blip".to_owned(),
        bytes,
    };
    let mut app = App::default();
    app.open_all(file).unwrap();
    let shown = app.toasts.shown();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].0, toasts::Level::Warning);
    assert!(shown[0].1.starts_with("a.blip was opened up to message"));
}

fn addresses(app: &App) -> Vec<String> {
    (app.connections.saved().into_iter())
        .map(|connection| connection.address)
        .collect()
}

fn tabs(app: &App) -> usize {
    app.layout.to_saved().children.len()
}

#[test]
fn switching_presets_keeps_the_layout_and_connections_of_each() {
    let ctx = Context::default();
    let mut app = App::default();
    app.layout.add_view(view::ViewType::Plot);
    app.connections.set_saved(
        vec![connections::Connection {
            name: "bench".to_owned(),
            address: "ws://bench:3000/ws/outpost".to_owned(),
        }],
        &mut app.state.session,
    );
    app.apply_preset(presets::Action::New, &ctx);
    assert_eq!(tabs(&app), 0);
    assert_eq!(addresses(&app), ["ws://127.0.0.1:3000/ws/outpost"]);

    app.apply_preset(presets::Action::Switch(0), &ctx);
    assert_eq!(tabs(&app), 1);
    assert_eq!(addresses(&app), ["ws://bench:3000/ws/outpost"]);

    // a copy has the same, deleting it shows the one after it, else before
    app.apply_preset(presets::Action::Duplicate, &ctx);
    assert_eq!(app.presets.active().name, "Default copy");
    assert_eq!(tabs(&app), 1);
    app.apply_preset(presets::Action::Delete, &ctx);
    assert_eq!(app.presets.active().name, "Preset 1");
    assert_eq!(tabs(&app), 0);
}

#[test]
fn a_presets_file_is_imported_as_chosen_in_its_dialog() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut source = App::default();
    source.layout.add_view(view::ViewType::Logs);
    source.store_preset();
    let mut other = presets::Presets::default();
    other.add_new();
    let mut bytes = source.presets.export(&[0]);
    // a file with the default preset, which replaces the active one, and another
    let mut file: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let new = serde_json::from_slice::<serde_json::Value>(&other.export(&[1])).unwrap();
    file["presets"]
        .as_array_mut()
        .unwrap()
        .push(new["presets"][0].clone());
    bytes = serde_json::to_vec(&file).unwrap();

    let mut app = App::default();
    let file = files::File {
        name: "presets.json".to_owned(),
        bytes,
    };
    app.open(file).unwrap();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| eframe::App::ui(app, ui, &mut Frame::_new_kittest()),
            app,
        );
    harness.run();
    assert!(
        harness
            .query_by_label_contains("replaces the existing one")
            .is_some()
    );
    harness.get_by_label("Import").click();
    harness.run();
    let app = harness.state();
    assert!(!app.presets.has_dialog());
    assert_eq!(tabs(app), 1);
    assert_eq!(app.presets.active().name, "Default");
    assert_eq!(app.presets.names(), ["Default", "Preset 1"]);
}

#[test]
fn presets_are_not_switched_while_recording() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut app = App::default();
    app.apply_preset(presets::Action::New, &Context::default());
    app.state.session.recording = true;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .build_ui_state(|ui, app: &mut App| app.menu_ui(ui), app);
    harness.run();
    harness.get_by_label("Presets").click();
    harness.run();
    harness.get_by_label("Default").click();
    harness.run();
    assert_eq!(harness.state().presets.active().name, "Preset 1");
}

#[test]
fn a_reopened_recording_is_saved_as_it_was_opened() {
    // a file of version 2: the header, then each message after the name of its source
    let mut bytes = vec![0xa2, 0x66];
    bytes.extend(b"format");
    bytes.push(0x6e);
    bytes.extend(b"blip recording");
    bytes.push(0x67);
    bytes.extend(b"version");
    bytes.push(0x02);
    bytes.extend(serde_cbor_2::to_vec(&"a").unwrap());
    bytes.extend(serde_cbor_2::to_vec(&value(1)).unwrap());
    bytes.extend(serde_cbor_2::to_vec(&value(2)).unwrap());

    let mut app = App::default();
    app.open_all(files::File {
        name: "a.blip".to_owned(),
        bytes: bytes.clone(),
    })
    .unwrap();
    assert!(app.state.session.store.has_samples());
    assert_eq!(app.state.session.recorder.to_file(), bytes);
}

#[test]
fn a_connection_that_cannot_connect_is_toasted_with_its_address() {
    let ctx = Context::default();
    let mut app = App::default();
    app.connections.set_saved(
        vec![connections::Connection {
            name: "x".to_owned(),
            address: "ws://127.0.0.1:1/x".to_owned(),
        }],
        &mut app.state.session,
    );
    app.connections.connect_first(&ctx, &mut app.state.session);
    let start = std::time::Instant::now();
    while app.toasts.shown().is_empty() && start.elapsed().as_secs() < 5 {
        eframe::App::logic(&mut app, &ctx, &mut Frame::_new_kittest());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let shown = app.toasts.shown();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].0, toasts::Level::Error);
    assert!(
        shown[0]
            .1
            .starts_with("Cannot connect to ws://127.0.0.1:1/x: "),
        "{shown:?}"
    );
    assert!(app.connections.is_idle());
    assert!(app.state.session.open_sources().is_empty());
}
