//! What the timestamps count from, and the seconds the time range and the views count
//! in.

pub const NANOS_PER_SECOND: f64 = 1.0e9;

/// Seconds from `origin` to `timestamp`, both in nanoseconds, negative before it.
pub fn seconds_since(origin: u64, timestamp: u64) -> f64 {
    timestamp.wrapping_sub(origin) as i64 as f64 / NANOS_PER_SECOND
}

/// The timestamp `seconds` after `origin` in nanoseconds, 0 before the epoch.
pub fn timestamp_at(origin: u64, seconds: f64) -> u64 {
    origin.saturating_add_signed((seconds * NANOS_PER_SECOND).round() as i64)
}

/// Timestamps from this one on are taken as Unix time (after 2001-09-09), earlier ones
/// as time since the device booted.
pub const UNIX_TIME_THRESHOLD: u64 = 1_000_000_000 * 1_000_000_000;

/// What the timestamps count from, as the Hello declares or guessed from their value.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TimeBase {
    /// Seconds since the Unix epoch, shown as local time.
    Unix,
    /// Seconds since the device booted.
    #[default]
    SinceBoot,
}

impl From<proto::Clock> for TimeBase {
    fn from(clock: proto::Clock) -> Self {
        match clock {
            proto::Clock::Epoch => TimeBase::Unix,
            proto::Clock::Boot => TimeBase::SinceBoot,
        }
    }
}

impl TimeBase {
    /// The time base of `timestamp` in nanoseconds.
    pub fn guess(timestamp: u64) -> Self {
        if timestamp >= UNIX_TIME_THRESHOLD {
            TimeBase::Unix
        } else {
            TimeBase::SinceBoot
        }
    }

    /// How many seconds the times that [`Self::format`] shows for `timestamp` are
    /// ahead of them: the offset of the local time zone from UTC for Unix time, else 0.
    pub fn display_offset(self, timestamp: u64) -> f64 {
        match self {
            TimeBase::Unix => {
                let seconds = (timestamp / 1_000_000_000) as i64;
                chrono::DateTime::from_timestamp(seconds, 0).map_or(0.0, |utc| {
                    let local: chrono::DateTime<chrono::Local> = utc.into();
                    f64::from(local.offset().local_minus_utc())
                })
            }
            TimeBase::SinceBoot => 0.0,
        }
    }

    /// Formats `timestamp` in nanoseconds as local time of day, or as hours, minutes
    /// and seconds since boot, with `decimals` decimals of the seconds, rounded.
    pub fn format(self, timestamp: u64, decimals: usize) -> String {
        self.format_as(timestamp, decimals, "%H:%M:%S")
    }

    /// Formats `timestamp` like [`Self::format`], local time with the date before it.
    pub fn format_dated(self, timestamp: u64, decimals: usize) -> String {
        self.format_as(timestamp, decimals, "%Y-%m-%d %H:%M:%S")
    }

    /// Formats `timestamp` like [`Self::format`], local time as `pattern` has it.
    fn format_as(self, timestamp: u64, decimals: usize, pattern: &str) -> String {
        let decimals = decimals.min(9);
        let unit = 10u64.pow(9 - decimals as u32);
        let rounded = timestamp.saturating_add(unit / 2) / unit * unit;
        let whole = rounded / 1_000_000_000;
        let fraction = if decimals > 0 {
            let digits = rounded % 1_000_000_000 / unit;
            format!(".{digits:0decimals$}")
        } else {
            String::new()
        };
        match self {
            TimeBase::Unix => match chrono::DateTime::from_timestamp(whole as i64, 0) {
                Some(utc) => {
                    let local: chrono::DateTime<chrono::Local> = utc.into();
                    format!("{}{fraction}", local.format(pattern))
                }
                None => format!("{timestamp}"),
            },
            TimeBase::SinceBoot => {
                format!(
                    "{}:{:02}:{:02}{fraction}",
                    whole / 3600,
                    whole / 60 % 60,
                    whole % 60
                )
            }
        }
    }
}
