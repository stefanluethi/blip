use super::*;
use crate::app::model::logs::DEFAULT_PATH;
use crate::app::time::TimeBase;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A table showing the logs without a path.
fn table() -> LogTable {
    LogTable {
        paths: BTreeSet::from([DEFAULT_PATH.to_owned()]),
        ..LogTable::default()
    }
}

fn create_entry(level: LogLevel, message: &str) -> LogEntry {
    LogEntry {
        time: 0,
        level,
        message: message.to_string(),
    }
}

/// The paths and the indices of the shown entries.
fn shown(table: &LogTable) -> Vec<(&str, u32)> {
    let paths = &table.shown_for.as_ref().unwrap().paths;
    (table.shown.iter())
        .map(|shown| (&*paths[shown.path as usize].path, shown.index))
        .collect()
}

/// The indices of the shown entries of the logs without a path.
fn shown_indices(table: &LogTable) -> Vec<u32> {
    shown(table).into_iter().map(|(_, index)| index).collect()
}

#[test]
fn new_entries_are_filtered_and_all_again_when_the_filter_or_order_changes() {
    let at = |time, level| LogEntry {
        time,
        ..create_entry(level, "message")
    };
    let mut logs = Logs::default();
    logs.push("", at(1, LogLevel::Info));
    logs.push("", at(2, LogLevel::Warn));
    let mut table = table();
    table.filter.hidden_levels.insert(LogLevel::Info);
    table.update_shown(&logs);
    assert_eq!(shown_indices(&table), [1]);

    logs.push("", at(3, LogLevel::Warn));
    logs.push("", at(4, LogLevel::Info));
    table.update_shown(&logs);
    assert_eq!(shown_indices(&table), [1, 2]);

    table.filter.hidden_levels.clear();
    table.update_shown(&logs);
    assert_eq!(shown_indices(&table), [0, 1, 2, 3]);

    // an entry before the others moves them on
    table.filter.hidden_levels.insert(LogLevel::Info);
    table.update_shown(&logs);
    logs.push("", at(0, LogLevel::Warn));
    table.update_shown(&logs);
    assert_eq!(shown_indices(&table), [0, 2, 3]);
}

/// Records at "motor" and at "robot/arm".
fn logs_of_two_paths() -> Logs {
    let mut logs = Logs::default();
    for path in ["motor", "robot/arm"] {
        logs.push(path, create_entry(LogLevel::Info, "message"));
    }
    logs
}

#[test]
fn only_the_logs_of_the_picked_paths_are_shown() {
    let logs = logs_of_two_paths();
    let mut table = LogTable {
        paths: BTreeSet::from(["motor".to_owned()]),
        ..LogTable::default()
    };
    table.update_shown(&logs);
    assert_eq!(shown(&table), [("motor", 0)]);
    table.paths.insert("robot/arm".to_owned());
    table.update_shown(&logs);
    assert_eq!(shown(&table), [("motor", 0), ("robot/arm", 0)]);
    // a path no longer picked is no longer shown
    table.paths.remove("motor");
    table.update_shown(&logs);
    assert_eq!(shown(&table), [("robot/arm", 0)]);
}

#[test]
fn the_entries_of_several_paths_are_shown_in_the_order_of_their_times() {
    let at = |time| LogEntry {
        time,
        ..create_entry(LogLevel::Info, "message")
    };
    let mut logs = Logs::default();
    logs.push("motor", at(1));
    logs.push("robot", at(2));
    logs.push("motor", at(3));
    let mut table = LogTable {
        paths: BTreeSet::from(["motor".to_owned(), "robot".to_owned()]),
        ..LogTable::default()
    };
    table.update_shown(&logs);
    assert_eq!(shown(&table), [("motor", 0), ("robot", 0), ("motor", 1)]);
    // appended to its path, but before an entry of the other one
    logs.push("robot", at(2));
    logs.push("motor", at(4));
    table.update_shown(&logs);
    let expected = [
        ("motor", 0),
        ("robot", 0),
        ("robot", 1),
        ("motor", 1),
        ("motor", 2),
    ];
    assert_eq!(shown(&table), expected);
    // a path whose first entry arrives, after the known ones of the same time
    table.paths.insert("arm".to_owned());
    logs.push("arm", at(3));
    table.update_shown(&logs);
    assert_eq!(shown(&table)[3..5], [("motor", 1), ("arm", 0)]);
    // an entry before the others of its path, which alone is filtered again
    logs.push("robot", at(0));
    table.update_shown(&logs);
    let expected = [
        ("robot", 0),
        ("motor", 0),
        ("robot", 1),
        ("robot", 2),
        ("motor", 1),
        ("arm", 0),
        ("motor", 2),
    ];
    assert_eq!(shown(&table), expected);
}

/// An entry at `time` with the level and the message given.
fn at(time: u64, level: LogLevel, message: &str) -> LogEntry {
    LogEntry {
        time,
        ..create_entry(level, message)
    }
}

fn paths_of(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|path| path.to_string()).collect()
}

#[test]
fn paths_are_written_relative_to_the_folder_they_share() {
    let folder = |paths: &[&str]| common_folder_len(paths.iter().copied());
    assert_eq!(folder(&["log/default", "log/imu", "log/motor/left"]), 4);
    assert_eq!(folder(&["log/imu"]), 4);
    assert_eq!(folder(&["log/a", "log/a/b"]), 4);
    assert_eq!(folder(&["log/a/x", "log/a/y"]), 6);
    assert_eq!(folder(&["log/imu", "motor"]), 0);
    assert_eq!(folder(&["logger/x", "log/y"]), 0);
    assert_eq!(folder(&["motor"]), 0);
    assert_eq!(folder(&[]), 0);
    // multibyte, no panic
    assert_eq!(folder(&["møtor/a", "møtor/b"]), 7);
}

#[test]
fn the_path_is_always_shown_relative_to_the_folder() {
    let harness = |paths: &[&str]| {
        let mut logs = Logs::default();
        for path in ["log/default", "log/imu", "log/motor/left", "motor"] {
            logs.push(path, at(1, LogLevel::Info, "message"));
        }
        let mut table = LogTable {
            paths: paths_of(paths),
            ..LogTable::default()
        };
        let time = TimeRange::default();
        Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| table.ui(ui, &logs, &time))
    };
    let header = format!("Path {}", regular::CARET_DOWN);
    let several = harness(&["log/default", "log/imu", "log/motor/left"]);
    several.get_by_label(&header);
    several.get_by_label("default");
    several.get_by_label("imu");
    several.get_by_label("motor/left");
    let single = harness(&["log/imu"]);
    single.get_by_label(&header);
    single.get_by_label("imu");
    let mixed = harness(&["log/imu", "motor"]);
    mixed.get_by_label("log/imu");
    mixed.get_by_label("motor");
}

#[test]
fn hidden_paths_hide_all_their_entries() {
    let mut logs = Logs::default();
    logs.push("log/default", at(1, LogLevel::Info, "a"));
    logs.push("log/imu", at(2, LogLevel::Warn, "b"));
    logs.push("log/imu", at(3, LogLevel::Info, "c"));
    let mut table = LogTable {
        paths: paths_of(&["log/default", "log/imu"]),
        ..LogTable::default()
    };
    table.filter.hidden_paths.insert("log/imu".to_owned());
    table.update_shown(&logs);
    assert_eq!(shown(&table), [("log/default", 0)]);
    table.filter.hidden_paths.clear();
    table.update_shown(&logs);
    assert_eq!(
        shown(&table),
        [("log/default", 0), ("log/imu", 0), ("log/imu", 1)]
    );
}

#[test]
fn the_path_filter_checks_paths_and_all_or_none() {
    let mut logs = Logs::default();
    logs.push("log/default", at(1, LogLevel::Info, "a"));
    logs.push("log/imu", at(2, LogLevel::Warn, "b"));
    logs.push("log/imu", at(3, LogLevel::Info, "c"));
    logs.push("log/motor/left", at(4, LogLevel::Info, "d"));
    let table = LogTable {
        paths: paths_of(&["log/default", "log/imu", "log/motor/left"]),
        ..LogTable::default()
    };
    let time = TimeRange::default();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 300.0))
        .build_ui_state(
            |ui, (table, logs): &mut (LogTable, Logs)| table.ui(ui, logs, &time),
            (table, logs),
        );
    harness
        .get_by_label(&format!("Path {}", regular::CARET_DOWN))
        .click();
    harness.run_steps(4);
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, "imu")
        .click();
    harness.run_steps(4);
    assert_eq!(
        harness.state().0.filter.hidden_paths,
        paths_of(&["log/imu"])
    );
    harness.get_by_label(&format!("Path {}", regular::FUNNEL));
    harness.get_by_label("None").click();
    harness.run_steps(4);
    assert_eq!(
        harness.state().0.filter.hidden_paths,
        paths_of(&["log/default", "log/imu", "log/motor/left"])
    );
    harness.get_by_label("All").click();
    harness.run_steps(4);
    assert!(harness.state().0.filter.hidden_paths.is_empty());
}

#[test]
fn hidden_paths_are_saved_and_new_paths_show() {
    let mut table = LogTable {
        paths: paths_of(&["log/default", "log/imu"]),
        ..LogTable::default()
    };
    table.filter.hidden_paths.insert("log/imu".to_owned());
    let json = serde_json::to_string(&table).unwrap();
    let mut table: LogTable = serde_json::from_str(&json).unwrap();
    assert_eq!(table.filter.hidden_paths, paths_of(&["log/imu"]));
    let mut logs = Logs::default();
    logs.push("log/default", at(1, LogLevel::Info, "a"));
    logs.push("log/imu", at(2, LogLevel::Info, "b"));
    table.paths.insert("log/gps".to_owned());
    logs.push("log/gps", at(3, LogLevel::Info, "c"));
    table.update_shown(&logs);
    assert_eq!(shown(&table), [("log/default", 0), ("log/gps", 0)]);
}

#[test]
fn autoscroll_button_stays_visible_in_a_small_view() {
    let mut logs = Logs::default();
    for _ in 0..100 {
        logs.push("", create_entry(LogLevel::Info, "message"));
    }
    let mut table = table();
    let time = TimeRange::default();
    let size = egui::vec2(400.0, 120.0);
    let harness = Harness::builder()
        .with_size(size)
        .build_ui(move |ui| table.ui(ui, &logs, &time));
    let button = harness.get_by_label(regular::ARROW_LINE_DOWN);
    assert!(button.rect().bottom() <= size.y);
}

#[test]
fn autoscroll_keeps_up_with_new_entries() {
    let mut logs = Logs::default();
    let mut table = table();
    let time = TimeRange::default();
    let newest = std::rc::Rc::new(std::cell::Cell::new(0));
    let shared = newest.clone();
    let mut frame = 0;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 200.0))
        .build_ui(move |ui| {
            // the harness turns the animations off, which hid the lag
            ui.ctx().all_styles_mut(|style| {
                style.scroll_animation = egui::style::ScrollAnimation::default();
            });
            frame += 1;
            if frame % 7 == 0 {
                let message = format!("message {}", frame / 7);
                logs.push("", create_entry(LogLevel::Info, &message));
                shared.set(frame / 7);
            }
            table.ui(ui, &logs, &time);
        });
    for step in 0..300 {
        harness.input_mut().time = Some(f64::from(step) / 60.0);
        harness.step();
    }
    let footer = harness.get_by_label(regular::ARROW_LINE_DOWN).rect();
    let newest = format!("message {}", newest.get());
    let entry = harness.get_by_label(&newest).rect();
    assert!(entry.bottom() <= footer.top());
}

#[test]
fn the_header_holds_the_filters() {
    let logs = Logs::default();
    let mut table = table();
    let time = TimeRange::default();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 300.0))
        .build_ui(move |ui| table.ui(ui, &logs, &time));
    harness
        .get_by_label(&format!("Level {}", regular::CARET_DOWN))
        .click();
    harness.run();
    harness.get_by_label("ERROR");
    harness.get_by_label("TRACE");
}

#[test]
fn hidden_levels_are_filtered() {
    let mut table = table();
    let debug_entry = create_entry(LogLevel::Debug, "debug message");
    let warn_entry = create_entry(LogLevel::Warn, "warn message");

    assert!(table.matches(&debug_entry));
    assert!(table.matches(&warn_entry));

    table.filter.hidden_levels.insert(LogLevel::Debug);
    assert!(!table.matches(&debug_entry));
    assert!(table.matches(&warn_entry));
}

#[test]
fn the_columns_follow_the_properties() {
    let harness = |columns: LogColumns| {
        let mut logs = Logs::default();
        let entry = LogEntry {
            time: 1_700_000_000_000_000_000,
            ..create_entry(LogLevel::Info, "speed 100")
        };
        logs.push("", entry);
        let mut table = LogTable { columns, ..table() };
        // the time base comes from the model
        let mut model = crate::app::model::Model::default();
        let hello = proto::Message::Hello(proto::Hello {
            version: 1,
            name: None,
            clock: Some(proto::Clock::Epoch),
            sender: None,
        });
        model.receive("", &serde_cbor_2::to_vec(&hello).unwrap());
        let mut time = TimeRange::default();
        time.update(&model, 0.0);
        Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| table.ui(ui, &logs, &time))
    };
    let date = TimeBase::Unix.format_dated(1_700_000_000_000_000_000, 3);
    let shown = harness(LogColumns::default());
    shown.get_by_label("speed 100");
    assert!(shown.query_by_label(&date).is_none());
    assert!(
        shown
            .query_by_label(&format!("Component {}", regular::CARET_DOWN))
            .is_none()
    );

    let dated = harness(LogColumns { date: true });
    dated.get_by_label(&date);
}

#[test]
fn the_properties_show_the_date_and_no_component() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 300.0))
        .build_ui_state(
            |ui, (table, model): &mut (LogTable, crate::app::model::Model)| {
                let connected = std::collections::HashSet::new();
                let context = crate::app::view::PropertiesContext {
                    model,
                    connected: &connected,
                    plots: &super::super::NO_PLOTS,
                };
                table.properties_ui(ui, &context);
            },
            (LogTable::default(), crate::app::model::Model::default()),
        );
    harness.run();
    harness.get_by_label("Date");
    assert!(harness.query_by_label("Component").is_none());
}

#[test]
fn an_old_layout_with_components_loads_without_them() {
    use crate::app::view::{View, ViewKind};
    let json = r#"{"name": "Log", "kind": {"Logs": {"paths": ["log/imu"], "filter": {"hidden_levels": ["Debug"], "hidden_components": ["adc"], "message_regex": "", "case_sensitive": false, "auto_scroll": true}, "columns": {"date": true, "component": false}}}}"#;
    let view = serde_json::from_str::<View>(json).unwrap();
    let ViewKind::Logs(table) = view.kind else {
        panic!("not a log table");
    };
    assert_eq!(table.paths, BTreeSet::from(["log/imu".to_owned()]));
    assert_eq!(
        table.filter.hidden_levels,
        BTreeSet::from([LogLevel::Debug])
    );
    assert!(table.columns.date);
    assert!(table.filter.hidden_paths.is_empty());
}

#[test]
fn clearing_the_filters_keeps_autoscroll() {
    let mut filter = LogFilter {
        auto_scroll: false,
        message_regex: "x".to_string(),
        ..LogFilter::default()
    };
    assert!(filter.is_active());
    filter.clear();
    assert!(!filter.is_active());
    assert!(!filter.auto_scroll);
    let mut filter = LogFilter {
        hidden_paths: paths_of(&["log/imu"]),
        ..LogFilter::default()
    };
    assert!(filter.is_active());
    filter.clear();
    assert!(filter.hidden_paths.is_empty());
    assert!(!filter.is_active());
}

#[test]
fn test_regex_filter() {
    let mut table = table();
    let entry1 = create_entry(LogLevel::Info, "connection established to ws://localhost");
    let entry2 = create_entry(LogLevel::Warn, "failed connection retrying");

    table.filter.message_regex = r"^connection.*".to_string();
    table.update_regex_cache();
    assert!(table.matches(&entry1));
    assert!(!table.matches(&entry2));

    // Case sensitivity check
    table.filter.message_regex = r"ESTABLISHED".to_string();
    table.filter.case_sensitive = true;
    table.update_regex_cache();
    assert!(!table.matches(&entry1));

    table.filter.case_sensitive = false;
    table.update_regex_cache();
    assert!(table.matches(&entry1));

    // an invalid pattern is searched as text
    let entry3 = create_entry(LogLevel::Info, "retrying (3 left)");
    table.filter.message_regex = r"(3".to_string();
    table.update_regex_cache();
    assert!(matches!(table.compiled_regex.as_deref(), Some(Err(_))));
    assert!(table.matches(&entry3));
    assert!(!table.matches(&entry1));
}
