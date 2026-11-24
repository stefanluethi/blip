use super::*;

fn at(time: u64, message: &str) -> LogEntry {
    LogEntry::new(time, 2, message, None)
}

#[test]
fn records_are_kept_by_path() {
    let mut logs = Logs::default();
    assert!(logs.push("motor", at(1, "a")));
    assert!(!logs.push("motor", at(2, "b")));
    // without a path, of the target as a whole
    assert!(logs.push("", at(3, "c")));
    assert_eq!(logs.paths().collect::<Vec<_>>(), [DEFAULT_PATH, "motor"]);
    assert!(logs.has_path("motor") && !logs.has_path("robot"));
    assert_eq!(logs.get("motor").unwrap().entries.len(), 2);
    assert_eq!(logs.get(DEFAULT_PATH).unwrap().entries[0].message, "c");
    assert_eq!(logs.time_span(), Some((1, 3)));
}

#[test]
fn move_all_to_keeps_no_span_without_records() {
    let mut logs = Logs::default();
    logs.move_all_to(7);
    assert_eq!(logs.time_span(), None);
    logs.push("", at(1, "a"));
    logs.push("", at(3, "b"));
    logs.move_all_to(7);
    assert_eq!(logs.time_span(), Some((7, 7)));
}

#[test]
fn a_late_record_changes_the_order_of_its_path_only() {
    let mut logs = Logs::default();
    logs.push("motor", at(2, "a"));
    logs.push("robot", at(1, "b"));
    let motor = logs.get("motor").unwrap().order();
    let robot = logs.get("robot").unwrap().order();
    // appended
    logs.push("motor", at(3, "c"));
    assert_eq!(logs.get("motor").unwrap().order(), motor);
    // before the others, after those of the same time
    logs.push("motor", at(2, "d"));
    let entries = &logs.get("motor").unwrap().entries;
    let messages: Vec<_> = entries.iter().map(|entry| entry.message.as_str()).collect();
    assert_eq!(messages, ["a", "d", "c"]);
    assert_ne!(logs.get("motor").unwrap().order(), motor);
    assert_eq!(logs.get("robot").unwrap().order(), robot);
}

#[test]
fn records_between_two_times_are_found_in_each_path() {
    let mut logs = Logs::default();
    for (path, time) in [("motor", 1), ("motor", 5), ("robot", 2), ("robot", 9)] {
        logs.push(path, at(time, ""));
    }
    let times = |start, end| -> Vec<Vec<u64>> {
        (logs.between(start, end))
            .map(|entries| entries.iter().map(|entry| entry.time).collect())
            .collect()
    };
    assert_eq!(times(0, 6), [vec![1, 5], vec![2]]);
    assert_eq!(times(2, 5), [vec![2]]);
    assert!(times(10, 20).is_empty());
}

#[test]
fn the_first_and_the_most_severe_records_are_of_all_paths() {
    let mut logs = Logs::default();
    let record = |time, level, message| LogEntry::new(time, level, message, None);
    logs.push("motor", record(1, 2, "a"));
    logs.push("motor", record(4, 4, "b"));
    logs.push("motor", record(6, 4, "c"));
    logs.push("robot", record(2, 3, "d"));
    logs.push("robot", record(3, 4, "e"));
    let messages = |entries: Vec<&LogEntry>| -> Vec<String> {
        entries.iter().map(|entry| entry.message.clone()).collect()
    };
    let (first, all) = logs.first_between(0, 10, 3);
    assert_eq!(
        (messages(first), all),
        (vec!["a".into(), "d".into(), "e".into()], 5)
    );
    let (first, all) = logs.first_between(2, 5, 10);
    assert_eq!(
        (messages(first), all),
        (vec!["d".into(), "e".into(), "b".into()], 3)
    );
    // the first of the errors, of the other path
    assert_eq!(logs.most_severe_between(0, 10).unwrap().message, "e");
    assert_eq!(logs.most_severe_between(0, 3).unwrap().message, "d");
    assert!(logs.most_severe_between(7, 10).is_none());
}

#[test]
fn a_record_without_a_path_is_at_log_default() {
    assert_eq!(DEFAULT_PATH, "log/default");
    let mut logs = Logs::default();
    for path in ["", "log/imu", "motor", "robot/arm"] {
        logs.push(path, at(1, ""));
    }
    assert_eq!(
        logs.paths().collect::<Vec<_>>(),
        ["log/default", "log/imu", "motor", "robot/arm"]
    );
    assert!(logs.get("log").is_none());
}

#[test]
fn records_at_0_are_placed_and_levels_above_4_are_errors() {
    assert_eq!(
        LogEntry::new(0, 2, "x", Some(7_000_000_000)).time,
        7_000_000_000
    );
    let before = chrono::Utc::now().timestamp_nanos_opt().unwrap() as u64;
    let time = LogEntry::new(0, 2, "x", None).time;
    let after = chrono::Utc::now().timestamp_nanos_opt().unwrap() as u64;
    assert!((before..=after).contains(&time));

    let levels: Vec<_> = (proto::level::TRACE..=proto::level::ERROR)
        .map(|level| LogEntry::new(1, level, "x", None).level)
        .collect();
    assert_eq!(
        levels,
        [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error
        ]
    );
    for level in [5, 255] {
        assert_eq!(LogEntry::new(1, level, "x", None).level, LogLevel::Error);
    }
}
