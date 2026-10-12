use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
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


#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct LogEntry {
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

        let timestamp = if record.timestamp > 0 {
            let secs = (record.timestamp / 1_000_000_000) as i64;
            let nsecs = (record.timestamp % 1_000_000_000) as u32;
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                let local_dt: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
                local_dt.format("%H:%M:%S%.3f").to_string()
            } else {
                format!("{}", record.timestamp)
            }
        } else {
            chrono::Local::now().format("%H:%M:%S%.3f").to_string()
        };

        let component = if record.component.is_empty() {
            "target".to_string()
        } else {
            record.component.clone()
        };

        Self {
            timestamp,
            component,
            level,
            message: record.message.clone(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
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
}
