use super::Version;
use super::sorted::{between_by, upper_bound_by};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The path of the records without one, which are of the target as a whole.
pub const DEFAULT_PATH: &str = "log";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[allow(dead_code)]
impl LogLevel {
    pub fn as_index(&self) -> usize {
        match self {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    pub const ALL: [LogLevel; 5] = [
        LogLevel::Trace,
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ];
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    /// Nanoseconds in the time domain of the target.
    pub time: u64,
    pub component: String,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    /// A record without its path, which [`Logs`] keeps it under. A record without a
    /// timestamp gets `now`, the newest time of the received data, or the wall-clock
    /// time if there is none.
    pub fn new(
        timestamp: u64,
        level: u8,
        component: &str,
        message: &str,
        now: Option<u64>,
    ) -> Self {
        let level = match level {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            // more severe levels, e.g. fatal
            _ => LogLevel::Error,
        };

        let time = if timestamp > 0 {
            timestamp
        } else {
            now.unwrap_or_else(|| {
                chrono::Utc::now()
                    .timestamp_nanos_opt()
                    .map_or(0, |nanos| nanos as u64)
            })
        };

        let component = if component.is_empty() {
            "target"
        } else {
            component
        };

        Self {
            time,
            component: component.to_owned(),
            level,
            message: message.to_owned(),
        }
    }
}

/// The log records of one path, in the order of their times.
#[derive(Default)]
pub struct PathLogs {
    pub entries: Vec<LogEntry>,
    /// Changes when an entry is put before others rather than appended, so views that
    /// keep what they made of the entries make it anew.
    order: Version,
}

impl PathLogs {
    /// Changes when an entry is put before others rather than appended.
    pub fn order(&self) -> Version {
        self.order
    }

    /// The entries from `start` up to `end` nanoseconds, without `end`.
    pub fn between(&self, start: u64, end: u64) -> &[LogEntry] {
        &self.entries[between_by(&self.entries, (start, end), |entry| entry.time)]
    }
}

/// The log records received, by their paths, so that a view reads only the records of
/// the paths it shows.
#[derive(Default)]
pub struct Logs {
    by_path: BTreeMap<Arc<str>, PathLogs>,
    /// Times of the first and the newest entry of all paths.
    span: Option<(u64, u64)>,
}

impl Logs {
    /// Adds `entry` at `path`, [`DEFAULT_PATH`] if it is empty, in the order of the
    /// times, after the entries of the same time, e.g. also when another outpost sends
    /// it late. Returns whether its path is new.
    pub fn push(&mut self, path: &str, entry: LogEntry) -> bool {
        let path = if path.is_empty() { DEFAULT_PATH } else { path };
        self.span = Some(match self.span {
            Some((first, newest)) => (first.min(entry.time), newest.max(entry.time)),
            None => (entry.time, entry.time),
        });
        // the path is allocated once, not for each record
        let new_path = !self.by_path.contains_key(path);
        if new_path {
            self.by_path.insert(path.into(), PathLogs::default());
        }
        let Some(logs) = self.by_path.get_mut(path) else {
            unreachable!("the path was just added");
        };
        let at = upper_bound_by(&logs.entries, entry.time, |known| known.time);
        if at < logs.entries.len() {
            logs.order = Version::default();
        }
        logs.entries.insert(at, entry);
        new_path
    }

    /// Moves all entries to `time`, keeping their order.
    pub fn move_all_to(&mut self, time: u64) {
        for logs in self.by_path.values_mut() {
            for entry in &mut logs.entries {
                entry.time = time;
            }
            logs.order = Version::default();
        }
        self.span = self.span.map(|_| (time, time));
    }

    /// The records of all paths, those of each path in the order of their times.
    pub fn entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.by_path.values().flat_map(|logs| &logs.entries)
    }

    /// The records at `path`, if any arrived.
    pub fn get(&self, path: &str) -> Option<&PathLogs> {
        self.by_path.get(path)
    }

    /// The path and the records at `path`, if any arrived.
    pub fn get_key_value(&self, path: &str) -> Option<(&Arc<str>, &PathLogs)> {
        self.by_path.get_key_value(path)
    }

    /// Whether records at `path` arrived.
    pub fn has_path(&self, path: &str) -> bool {
        self.by_path.contains_key(path)
    }

    /// The paths of the entries, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.by_path.keys().map(|path| &**path)
    }

    /// The records of all paths from `start` up to `end` nanoseconds, without `end`, a
    /// slice in the order of the times for each path.
    pub fn between(&self, start: u64, end: u64) -> impl Iterator<Item = &[LogEntry]> {
        (self.by_path.values())
            .map(move |logs| logs.between(start, end))
            .filter(|entries| !entries.is_empty())
    }

    /// The first of the most severe records of all paths from `start` up to `end`
    /// nanoseconds, without `end`.
    pub fn most_severe_between(&self, start: u64, end: u64) -> Option<&LogEntry> {
        (self.between(start, end).flatten())
            .min_by_key(|entry| (std::cmp::Reverse(entry.level), entry.time))
    }

    /// The first `count` records of all paths from `start` up to `end` nanoseconds,
    /// without `end`, in the order of their times, and how many there are in all.
    pub fn first_between(&self, start: u64, end: u64, count: usize) -> (Vec<&LogEntry>, usize) {
        let mut all = 0;
        // the first ones of all paths are among the first ones of each
        let mut first = Vec::new();
        for entries in self.between(start, end) {
            all += entries.len();
            first.extend(entries.iter().take(count));
        }
        // stable, so that records of the same time keep the order of their paths
        first.sort_by_key(|entry| entry.time);
        first.truncate(count);
        (first, all)
    }

    /// Times of the first and the newest entry of all paths.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        self.span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(time: u64, message: &str) -> LogEntry {
        LogEntry::new(time, 2, "", message, None)
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
        let record = |time, level, message| LogEntry::new(time, level, "", message, None);
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
}
