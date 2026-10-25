use super::Version;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
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
    /// A record without a timestamp gets `now`, the newest time of the received data,
    /// or the wall-clock time if there is none.
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

/// The log records received, all of them, in the order of their times.
#[derive(Default)]
pub struct Logs {
    pub entries: Vec<LogEntry>,
    /// Changes when an entry is put before others rather than appended, so views that
    /// keep what they made of the entries make it anew.
    order: Version,
}

impl Logs {
    /// Adds `entry` in the order of the times, after the entries of the same time, e.g.
    /// also when another outpost sends it late.
    pub fn push(&mut self, entry: LogEntry) {
        let at = self
            .entries
            .partition_point(|known| known.time <= entry.time);
        if at < self.entries.len() {
            self.order = Version::default();
        }
        self.entries.insert(at, entry);
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
