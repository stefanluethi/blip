use super::*;
use crate::app::model::logs::DEFAULT_PATH;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A table showing the logs without a path.
fn table() -> LogTable {
    LogTable {
        paths: BTreeSet::from([DEFAULT_PATH.to_owned()]),
        ..LogTable::default()
    }
}

fn create_entry(level: LogLevel, component: &str, message: &str) -> LogEntry {
    LogEntry {
        time: 0,
        component: component.to_string(),
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
        ..create_entry(level, "core", "message")
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
        logs.push(path, create_entry(LogLevel::Info, "core", "message"));
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
        ..create_entry(LogLevel::Info, "core", "message")
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
    let counts: Vec<_> = (table.shown_paths().iter())
        .map(|path| (&*path.path, path.shown))
        .collect();
    assert_eq!(counts, [("motor", 3), ("robot", 3), ("arm", 1)]);
}

#[test]
fn the_path_is_shown_while_the_logs_of_several_paths_are() {
    let harness = |paths: &[&str]| {
        let logs = logs_of_two_paths();
        let mut table = LogTable {
            paths: paths.iter().map(|path| path.to_string()).collect(),
            ..LogTable::default()
        };
        let time = TimeRange::default();
        Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| table.ui(ui, &logs, &time))
    };
    let several = harness(&["motor", "robot/arm"]);
    several.get_by_label("Path");
    several.get_by_label("robot/arm");
    assert!(harness(&["motor"]).query_by_label("Path").is_none());
    // paths picked without shown entries tell nothing apart
    assert!(harness(&["motor", "log"]).query_by_label("Path").is_none());
}

#[test]
fn only_the_paths_of_the_shown_entries_count() {
    let mut logs = Logs::default();
    logs.push("", create_entry(LogLevel::Info, "core", "message"));
    logs.push("motor", create_entry(LogLevel::Info, "hidden", "message"));
    let mut table = LogTable {
        paths: BTreeSet::from([DEFAULT_PATH.to_owned(), "motor".to_owned()]),
        ..LogTable::default()
    };
    // the entry of "motor" is hidden
    table.filter.hidden_components.insert("hidden".to_owned());
    table.update_shown(&logs);
    assert!(!table.shows_several_paths());
    // an arriving entry of "motor" is filtered with those before
    logs.push("motor", create_entry(LogLevel::Info, "core", "later"));
    table.update_shown(&logs);
    assert!(table.shows_several_paths());
}

#[test]
fn autoscroll_button_stays_visible_in_a_small_view() {
    let mut logs = Logs::default();
    for _ in 0..100 {
        logs.push("", create_entry(LogLevel::Info, "core", "message"));
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
                logs.push("", create_entry(LogLevel::Info, "core", &message));
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
    let debug_entry = create_entry(LogLevel::Debug, "core", "debug message");
    let warn_entry = create_entry(LogLevel::Warn, "core", "warn message");

    assert!(table.matches(&debug_entry));
    assert!(table.matches(&warn_entry));

    table.filter.hidden_levels.insert(LogLevel::Debug);
    assert!(!table.matches(&debug_entry));
    assert!(table.matches(&warn_entry));
}

#[test]
fn hidden_components_are_filtered() {
    let mut table = table();
    let entry1 = create_entry(LogLevel::Info, "motor_ctrl", "speed 100");
    let entry2 = create_entry(LogLevel::Info, "sensor_nav", "heading 90");

    table
        .filter
        .hidden_components
        .insert("sensor_nav".to_string());
    assert!(table.matches(&entry1));
    assert!(!table.matches(&entry2));

    // not while the components are hidden
    table.columns.component = false;
    assert!(table.matches(&entry2));
    assert!(!table.filter.is_active(false));
    table.columns.component = true;

    table.filter.clear();
    assert!(table.matches(&entry1));
    assert!(table.matches(&entry2));
}

#[test]
fn the_columns_follow_the_properties() {
    let harness = |columns: LogColumns| {
        let mut logs = Logs::default();
        let entry = LogEntry {
            time: 1_700_000_000_000_000_000,
            ..create_entry(LogLevel::Info, "motor_ctrl", "speed 100")
        };
        logs.push("", entry);
        let mut table = LogTable { columns, ..table() };
        let time = TimeRange::default();
        Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui(move |ui| table.ui(ui, &logs, &time))
    };
    let shown = harness(LogColumns::default());
    shown.get_by_label("motor_ctrl");
    shown.get_by_label(&format!("Component {}", regular::CARET_DOWN));
    let date = TimeBase::Unix.format_dated(1_700_000_000_000_000_000, 3);
    assert!(shown.query_by_label(&date).is_none());

    let hidden = harness(LogColumns {
        date: true,
        component: false,
    });
    assert!(hidden.query_by_label("motor_ctrl").is_none());
    hidden.get_by_label(&date);
}

#[test]
fn clearing_the_filters_keeps_autoscroll() {
    let mut filter = LogFilter {
        auto_scroll: false,
        message_regex: "x".to_string(),
        ..LogFilter::default()
    };
    assert!(filter.is_active(true));
    filter.clear();
    assert!(!filter.is_active(true));
    assert!(!filter.auto_scroll);
}

#[test]
fn test_regex_filter() {
    let mut table = table();
    let entry1 = create_entry(
        LogLevel::Info,
        "ws",
        "connection established to ws://localhost",
    );
    let entry2 = create_entry(LogLevel::Warn, "ws", "failed connection retrying");

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
    let entry3 = create_entry(LogLevel::Info, "ws", "retrying (3 left)");
    table.filter.message_regex = r"(3".to_string();
    table.update_regex_cache();
    assert!(matches!(table.compiled_regex.as_deref(), Some(Err(_))));
    assert!(table.matches(&entry3));
    assert!(!table.matches(&entry1));
}
