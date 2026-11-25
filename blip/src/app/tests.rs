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

fn value(timestamp: u64) -> Vec<u8> {
    value_of("rpm", timestamp)
}

fn value_of(path: &str, timestamp: u64) -> Vec<u8> {
    serde_cbor_2::to_vec(&proto::Message::Sample(proto::Sample {
        timestamp,
        path: path.to_owned(),
        value: proto::Data::Uint(1),
    }))
    .unwrap()
}

fn unit() -> Vec<u8> {
    unit_of("rpm")
}

fn unit_of(path: &str) -> Vec<u8> {
    serde_cbor_2::to_vec(&proto::Message::TraceMetadata(proto::TraceMetadata {
        path: path.to_owned(),
        unit: "rpm".to_owned(),
        ..Default::default()
    }))
    .unwrap()
}

#[test]
fn a_recording_starts_only_while_connected_and_clears_the_data() {
    let mut state = State::default();
    assert!(!state.session.recording);
    state.session.receive("a", &unit(), 0.0);
    state.session.receive("a", &value(1_000_000_000), 0.0);
    assert!(!state.session.store.has_samples());
    state.set_recording(true, false);
    assert!(!state.session.recording);

    state.set_recording(true, true);
    state.session.receive("a", &value(2_000_000_000), 0.0);
    state.session.receive("a", &value(3_000_000_000), 0.0);
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
            |ui, (state, connected): &mut (State, bool)| {
                state.bottom_ui(ui, *connected);
            },
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
    let mut recorded = model::MemoryStore::default();
    recorded.receive("a", &value(1));
    let file = files::File {
        name: "a.blip".to_owned(),
        bytes: recorded.recording.to_file(),
    };
    let mut app = App::default();
    app.state.session.receive("old", &unit_of("temp"), 0.0);
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
    assert_eq!(app.state.session.store.recording.messages(), 0);
}

#[test]
fn the_first_connection_to_open_starts_afresh_unless_recording() {
    let mut state = State::default();
    state.set_recording(true, true);
    state.session.receive("old", &unit(), 0.0);
    state.session.receive("old", &value(1), 0.0);
    // a connection opening while recording keeps the recording going
    assert!(!state.apply(Received::FirstOpened, 0.0));
    assert!(state.session.recording && state.session.store.has_samples());

    // nor does it drop the recording while it is unsaved
    state.set_recording(false, false);
    assert!(!state.apply(Received::FirstOpened, 0.0));
    assert!(state.session.store.has_samples());

    state.session.mark_saved();
    assert!(state.apply(Received::FirstOpened, 0.0));
    assert!(state.session.store.traces.iter().next().is_none());
    assert_eq!(state.session.store.recording.messages(), 0);
    let message = unit();
    let received = Received::Message {
        source: "new",
        message: &message,
    };
    assert!(!state.apply(received, 0.0));
    // a recording starts with the unit of the new connection only, not the old one
    state.set_recording(true, true);
    assert_eq!(state.session.store.recording.messages(), 1);
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
        .receive("a", &value_of("motor/rpm", 1), 0.0);
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
        .receive("a", &value_of("motor/rpm", 1), 0.0);
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

    let mut recorded = model::MemoryStore::default();
    for timestamp in 0..200_000 {
        recorded.receive("a", &value(timestamp));
    }
    let file = || files::File {
        name: "a.blip".to_owned(),
        bytes: recorded.recording.to_file(),
    };
    let ctx = Context::default();
    let mut app = App::default();
    app.state.session.receive("old", &unit_of("temp"), 0.0);
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
    assert_eq!(app.state.session.store.recording.messages(), 200_000);

    // connecting while reading cancels it
    let mut app = App::default();
    app.open(file()).unwrap();
    app.connections.connect_first(&ctx);
    app.continue_opening(&ctx);
    assert!(app.opening.is_none());
    assert!(!app.state.session.store.has_samples());
    assert!(app.toasts.shown()[0].1.contains("cancelled by connecting"));

    // an unsaved recording goes on until the user chose to discard it and the file is
    // read; cancelled, the data shown stays
    let mut app = App::default();
    app.state.session.recording = true;
    app.state.session.receive("old", &value(1), 0.0);
    app.open(file()).unwrap();
    assert!(app.opening.is_none());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| eframe::App::ui(app, ui, &mut Frame::_new_kittest()),
            app,
        );
    harness.run_steps(2);
    harness.get_by_label("Discard").click();
    harness.run_steps(2);
    assert!(harness.state().opening.is_some());
    assert!(harness.state().state.session.recording);
    harness.state_mut().continue_opening(&ctx);
    harness.run_steps(2);
    assert!(harness.query_by_label_contains("Opening a.blip").is_some());
    harness.get_by_label(egui_phosphor::regular::X).click();
    harness.run_steps(2);
    assert!(harness.state().opening.is_none());
    assert!(harness.state().state.session.recording);
    assert!(harness.state().state.session.store.has_samples());
}

/// An app with an unsaved recording of one sample, stopped.
fn unsaved() -> App {
    let mut app = App::default();
    app.state.set_recording(true, true);
    app.state.session.receive("a", &value(1), 0.0);
    app.state.set_recording(false, false);
    assert!(app.is_unsaved());
    app
}

/// A harness showing all of `app`.
fn whole(app: App) -> egui_kittest::Harness<'static, App> {
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(1000.0, 600.0))
        .build_ui_state(
            |ui, app: &mut App| eframe::App::ui(app, ui, &mut Frame::_new_kittest()),
            app,
        )
}

#[test]
fn clearing_asks_before_it_drops_an_unsaved_recording() {
    use egui_kittest::kittest::Queryable;

    let mut harness = whole(unsaved());
    harness.state_mut().ask(Asked::Clear);
    harness.run_steps(2);
    assert!(harness.query_by_label(DISCARD_QUESTION).is_some());
    // Escape cancels
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.query_by_label(DISCARD_QUESTION).is_none());
    assert!(harness.state().state.session.store.has_samples());

    harness.state_mut().ask(Asked::Clear);
    harness.run_steps(2);
    harness.get_by_label("Cancel").click();
    harness.run_steps(2);
    assert!(harness.state().asked.is_none());
    assert!(harness.state().state.session.store.has_samples());

    harness.state_mut().ask(Asked::Clear);
    harness.run_steps(2);
    harness.get_by_label("Discard").click();
    harness.run_steps(2);
    assert!(!harness.state().state.session.store.has_samples());

    // with nothing unsaved, nothing is asked
    let mut app = unsaved();
    app.state.session.mark_saved();
    app.ask(Asked::Clear);
    assert!(app.asked.is_none());
    assert!(!app.state.session.store.has_samples());
}

#[test]
fn record_asks_before_it_drops_an_unsaved_recording() {
    let mut app = unsaved();
    // connected, starting would clear the data: it waits for the answer
    assert!(app.state.toggle_recording(true));
    assert!(!app.state.session.recording);
    assert!(app.state.session.store.has_samples());
    // stopping never asks
    app.state.session.mark_saved();
    assert!(!app.state.toggle_recording(true));
    assert!(app.state.session.recording);
    assert!(!app.state.toggle_recording(true));
    assert!(!app.state.session.recording);
}

#[test]
fn the_record_key_asks_like_the_button() {
    let mut harness = whole(unsaved());
    harness.run();
    // not connected, R starts nothing and asks nothing
    harness.key_press(Key::R);
    harness.run();
    assert!(harness.state().asked.is_none());
    assert!(!harness.state().state.session.recording);
}

#[test]
fn shortcuts_wait_while_a_question_is_open() {
    use egui_kittest::kittest::Queryable;

    let mut harness = whole(unsaved());
    harness.state_mut().ask(Asked::Clear);
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(2);
    assert!(harness.state().toasts.shown().is_empty());
    assert!(harness.query_by_label(DISCARD_QUESTION).is_some());
}

#[test]
fn saving_nothing_tells_so() {
    let mut harness = whole(App::default());
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let shown = harness.state().toasts.shown();
    assert_eq!(shown, [(toasts::Level::Info, "Nothing recorded to save")]);
}

#[test]
fn an_opened_recording_is_named_in_the_chip_until_other_data_replaces_it() {
    use egui_kittest::kittest::Queryable;

    let mut recorded = model::MemoryStore::default();
    recorded.receive("a", &value(1));
    let file = files::File {
        name: "/home/me/runs/a.blip".to_owned(),
        bytes: recorded.recording.to_file(),
    };
    let mut app = App::default();
    app.open_all(file).unwrap();
    assert_eq!(app.opened.as_deref(), Some("a.blip"));
    let mut harness = menu_bar(app);
    harness.run();
    assert!(harness.query_by_label("a.blip").is_some());
    assert!(harness.query_by_label("Not connected").is_none());

    harness.state_mut().ask(Asked::Clear);
    harness.run();
    assert!(harness.query_by_label("Not connected").is_some());
}

/// A harness showing the menu bar of `app`.
fn menu_bar(app: App) -> egui_kittest::Harness<'static, App> {
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(1000.0, 60.0))
        .build_ui_state(|ui, app: &mut App| app.menu_ui(ui), app)
}

#[test]
fn file_names_drop_their_folders() {
    assert_eq!(file_name("/home/me/a.blip"), "a.blip");
    assert_eq!(file_name("C:\\runs\\a.blip"), "a.blip");
    assert_eq!(file_name("a.blip"), "a.blip");
    assert_eq!(shortened("a.blip", 32), "a.blip");
    assert_eq!(shortened("abcdef", 4), "abc…");
}

#[test]
fn a_recording_opened_in_part_is_toasted() {
    let mut recorded = model::MemoryStore::default();
    for timestamp in 0..1000 {
        recorded.receive("a", &value(timestamp));
    }
    let mut bytes = recorded.recording.to_file();
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
    app.connections.set_saved(vec![connections::Connection {
        name: "bench".to_owned(),
        address: "ws://bench:3000/ws/outpost".to_owned(),
    }]);
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
    harness.get_by_label("Import and replace").click();
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
    harness.get_by_label("Presets: Preset 1").click();
    harness.run();
    harness.get_by_label("Default").click();
    harness.run();
    assert_eq!(harness.state().presets.active().name, "Preset 1");
}

/// The layout of `app` in its saved form, to compare.
fn saved_layout(app: &App) -> String {
    serde_json::to_string(&app.layout.to_saved()).unwrap()
}

/// An app with a saved recording of samples over ten seconds, shown paused before its
/// end, and a plot and a log view, the plot selected.
fn recorded() -> App {
    let mut app = App {
        layout: layout::Layout::with_plot_and_logs(),
        ..App::default()
    };
    app.layout.select_view_showing_traces();
    app.state.set_recording(true, true);
    for second in 0..=10 {
        app.state
            .session
            .receive("a", &value(second * 1_000_000_000), 0.0);
    }
    app.state.set_recording(false, false);
    app.state.session.mark_saved();
    // a pan pauses the window before the end
    app.state.time.update(&app.state.session.store, 0.0);
    app.state.time.pan_zoom(0.0, 2.0);
    app.state.time.pan_zoom(1.0, 3.0);
    assert!(!app.state.time.is_playing());
    app
}

#[test]
fn space_plays_and_pauses() {
    let mut harness = whole(recorded());
    harness.run();
    // playing repaints all the time
    harness.key_press(Key::Space);
    harness.run_steps(2);
    assert!(harness.state().state.time.is_playing());
    harness.key_press(Key::Space);
    harness.run_steps(2);
    assert!(!harness.state().state.time.is_playing());
}

#[test]
fn escape_releases_the_cursor_once_the_connections_are_closed() {
    let mut harness = whole(recorded());
    harness.state_mut().state.time.set_cursor(2.0);
    harness.state_mut().connections_shown = true;
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().connections_shown);
    assert!(harness.state().state.time.cursor_is_set());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().state.time.cursor_is_set());
}

#[test]
fn delete_deletes_the_selected_view_until_undone_from_its_toast() {
    use egui_kittest::kittest::Queryable;

    let mut harness = whole(recorded());
    harness.run();
    let before = saved_layout(harness.state());
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    assert_ne!(saved_layout(harness.state()), before);
    harness.get_by_label("Undo").click();
    harness.run_steps(2);
    assert_eq!(saved_layout(harness.state()), before);
}

#[test]
fn no_shortcut_fires_while_a_text_field_has_the_focus() {
    let mut harness = whole(recorded());
    harness.state_mut().state.time.set_cursor(2.0);
    harness.run();
    let before = saved_layout(harness.state());
    // the search of the traces
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run_steps(2);
    assert!(harness.ctx.text_edit_focused());
    for key in [Key::Space, Key::Delete] {
        harness.key_press(key);
        harness.run_steps(2);
    }
    assert!(!harness.state().state.time.is_playing());
    assert_eq!(saved_layout(harness.state()), before);
    // Escape leaves the field, and only the next one releases the cursor
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.ctx.text_edit_focused());
    assert!(harness.state().state.time.cursor_is_set());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().state.time.cursor_is_set());
}

#[test]
fn delete_waits_while_a_menu_is_open() {
    use egui_kittest::kittest::Queryable;

    let mut harness = whole(recorded());
    harness.run();
    let before = saved_layout(harness.state());
    harness.get_by_label("File").click();
    harness.run_steps(2);
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    assert_eq!(saved_layout(harness.state()), before);
}

#[test]
fn recording_cancels_a_recording_being_opened() {
    let mut recorded = model::MemoryStore::default();
    recorded.receive("a", &value(1));
    let file = files::File {
        name: "a.blip".to_owned(),
        bytes: recorded.recording.to_file(),
    };
    let mut app = App::default();
    app.open(file).unwrap();
    assert!(app.opening.is_some());
    // nothing started
    app.recording_started(false);
    assert!(app.opening.is_some());
    app.state.set_recording(true, true);
    app.recording_started(false);
    assert!(app.opening.is_none());
    assert!(app.toasts.shown()[0].1.contains("cancelled by recording"));
    // a recording that went on stays apart from the opening
    let mut app = App::default();
    app.state.set_recording(true, true);
    app.open(files::File {
        name: "a.blip".to_owned(),
        bytes: recorded.recording.to_file(),
    })
    .unwrap();
    app.recording_started(true);
    assert!(app.opening.is_some());
}

#[test]
fn only_the_last_delete_offers_to_be_undone() {
    use egui_kittest::kittest::Queryable;

    let mut harness = whole(recorded());
    harness.run();
    let before = saved_layout(harness.state());
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    let one_deleted = saved_layout(harness.state());
    harness.state_mut().layout.clear();
    harness.run_steps(2);
    // the toast of the first delete is gone
    assert_eq!(harness.query_all_by_label("Undo").count(), 1);
    harness.get_by_label("Undo").click();
    harness.run_steps(2);
    assert_eq!(saved_layout(harness.state()), one_deleted);
    assert_ne!(saved_layout(harness.state()), before);
    assert!(harness.query_by_label("Undo").is_none());
}

#[test]
fn a_cancelled_quit_asks_the_question_before_it_again() {
    use egui_kittest::kittest::Queryable;

    let mut app = unsaved();
    app.asked = Some(Asked::Quit);
    app.asked_before_quit = Some(Asked::Clear);
    let mut harness = whole(app);
    harness.run_steps(2);
    harness.get_by_label("Cancel").click();
    harness.run_steps(2);
    assert!(matches!(harness.state().asked, Some(Asked::Clear)));
    assert!(harness.query_by_label(DISCARD_QUESTION).is_some());
}
