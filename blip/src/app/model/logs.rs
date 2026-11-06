use super::Version;
use std::collections::BTreeSet;
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
    /// Where the record comes from, [`DEFAULT_PATH`] if it has no path. Entries of the
    /// same path share it, see [`Logs::shared_path`].
    pub path: Arc<str>,
    pub component: String,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    /// A record at `path`, see [`Logs::shared_path`]. A record without a timestamp gets
    /// `now`, the newest time of the received data, or the wall-clock time if there is
    /// none.
    pub fn new(
        timestamp: u64,
        path: Arc<str>,
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
            path,
            component: component.to_owned(),
            level,
            message: message.to_owned(),
        }
    }
}

/// The log records received, all of them, in the order of their times.
#[derive(Default)]
pub struct Logs {
    pub entries: Vec<LogEntry>,
    /// The paths of the entries.
    paths: BTreeSet<Arc<str>>,
    /// Changes when an entry is put before others rather than appended, so views that
    /// keep what they made of the entries make it anew.
    order: Version,
}

impl Logs {
    /// Adds `entry` in the order of the times, after the entries of the same time, e.g.
    /// also when another outpost sends it late. Returns whether its path is new.
    pub fn push(&mut self, mut entry: LogEntry) -> bool {
        let new_path = match self.paths.get(&entry.path) {
            Some(known) => {
                entry.path = known.clone();
                false
            }
            None => self.paths.insert(entry.path.clone()),
        };
        let at = self
            .entries
            .partition_point(|known| known.time <= entry.time);
        if at < self.entries.len() {
            self.order = Version::default();
        }
        self.entries.insert(at, entry);
        new_path
    }

    /// Moves all entries to `time`, keeping their order.
    pub fn move_all_to(&mut self, time: u64) {
        for entry in &mut self.entries {
            entry.time = time;
        }
        self.order = Version::default();
    }

    /// The path to give a record received with `path`, [`DEFAULT_PATH`] if it is empty,
    /// shared with the entries of the same path, so that it is allocated once per path.
    pub fn shared_path(&self, path: &str) -> Arc<str> {
        let path = if path.is_empty() { DEFAULT_PATH } else { path };
        self.paths.get(path).cloned().unwrap_or_else(|| path.into())
    }

    /// Whether records at `path` arrived.
    pub fn has_path(&self, path: &str) -> bool {
        self.paths.contains(path)
    }

    /// The paths of the entries, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.iter().map(|path| &**path)
    }

    /// Changes when an entry is put before others rather than appended.
    pub fn order(&self) -> Version {
        self.order
    }

    /// Times of the first and the newest entry.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        Some((self.entries.first()?.time, self.entries.last()?.time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_of_a_path_share_it() {
        let mut logs = Logs::default();
        let entry = |logs: &Logs, path| LogEntry::new(1, logs.shared_path(path), 2, "", "hi", None);
        let first = entry(&logs, "motor");
        assert!(logs.push(first));
        let second = entry(&logs, "motor");
        assert!(Arc::ptr_eq(&second.path, &logs.entries[0].path));
        assert!(!logs.push(second));
        // without a path, of the target as a whole
        assert!(logs.push(entry(&logs, "")));
        assert_eq!(logs.paths().collect::<Vec<_>>(), [DEFAULT_PATH, "motor"]);
        assert!(logs.has_path("motor") && !logs.has_path("robot"));
    }
}
