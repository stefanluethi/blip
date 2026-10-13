use crate::app::time::{TimeBase, timestamp_to_seconds};
use std::collections::VecDeque;

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
    /// Nanoseconds since the Unix epoch.
    pub time: u64,
    pub timestamp: String,
    pub component: String,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    pub fn from_proto(record: &proto::LogRecord) -> Self {
        let level = match record.level {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            4 => LogLevel::Error,
            _ => LogLevel::Trace,
        };

        let time = if record.timestamp > 0 {
            record.timestamp
        } else {
            chrono::Utc::now()
                .timestamp_nanos_opt()
                .map_or(0, |nanos| nanos as u64)
        };
        let seconds = timestamp_to_seconds(time);
        let timestamp = TimeBase::guess(seconds).format(seconds, 3);

        let component = if record.component.is_empty() {
            "target".to_string()
        } else {
            record.component.clone()
        };

        Self {
            time,
            timestamp,
            component,
            level,
            message: record.message.clone(),
        }
    }
}

pub struct Logs {
    pub entries: VecDeque<LogEntry>,
    pub max_entries: usize,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 5000,
        }
    }
}

impl Logs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Times of the first and the newest entry.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        Some((self.entries.front()?.time, self.entries.back()?.time))
    }
}
