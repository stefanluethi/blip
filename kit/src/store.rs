//! What the targets send, kept for the views.
//!
//! In: an application feeds a store through [`Ingest`] with an [`Input`], the typed
//! `value` of a sample, or takes a [`session::Session`] with its [`source::Source`]s,
//! an [`source::Inbox`] they hand their messages to, and the [`recording::Recorder`]
//! that writes what arrives to a file. Out: the [`Store`] trait, which the views read
//! through. [`MemoryStore`] implements both. The reader, the parser and the tables
//! behind it are private.

pub mod actions;
mod cbor;
mod flatten;
mod growing;
pub mod ingest;
pub mod logs;
pub mod numbers;
mod protocol;
pub mod query;
pub mod recording;
mod said;
pub mod session;
pub(crate) mod sorted;
pub mod source;
pub mod time_base;
pub mod traces;
pub mod value;
#[cfg(feature = "websocket")]
pub mod websocket;

use crate::store::actions::Actions;
use crate::store::logs::{LogEntry, Logs};
use crate::store::sorted::lower_bound;
use crate::store::time_base::TimeBase;
use crate::store::traces::Traces;
use std::collections::HashMap;

pub use ingest::{Ingest, Input, Times};
pub use query::{Loaded, Store, Window};
use std::sync::atomic::{AtomicU64, Ordering};

/// Identifies a state of a part of a model, e.g. the paths of its traces: a new one
/// is unlike all before, also of other models.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Version(u64);

impl Default for Version {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// How serious a notice for the user is, see [`MemoryStore::take_notices`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    /// Something did not work out completely, e.g. a recording was opened in part.
    Warning,
    /// Something failed, e.g. connecting.
    Error,
}

/// A store that keeps everything it receives in memory: it takes input through
/// [`Ingest`] and answers the queries of [`Store`].
#[derive(Default)]
pub struct MemoryStore {
    /// The traces with their samples.
    pub traces: Traces,
    /// The log records.
    pub logs: Logs,
    /// The actions the targets announced.
    pub actions: Actions,
    /// Changes whenever a trace, a log path or an announcement of actions arrives.
    paths_version: Version,
    /// Changes whenever a value or log record arrives.
    samples_version: Version,
    /// The leaves of the value being applied.
    flat: flatten::Flat,
    /// The timestamp of the first value or log record, from which times count.
    origin: Option<u64>,
    /// Whether a sample was dropped as its time base is not the one of the model.
    warned_time_base: bool,
    /// The clock of each source from its latest Hello.
    clocks: HashMap<String, TimeBase>,
    /// What the timestamps of the data count from, set by the first accepted sample or
    /// log record or by a Hello with the epoch clock.
    base: Option<TimeBase>,
    /// Whether the different clocks of the sources were warned of. Kept when the data
    /// is cleared.
    warned_clocks: bool,
    /// The latest value of each path, also while not recording, so that actions show
    /// their readbacks without recording. Kept when the data is cleared, as the target
    /// is as it was.
    latest: HashMap<Box<str>, Latest>,
    /// What went wrong or needs telling since the last `take_notices`, without repeats.
    /// Kept when the data is cleared.
    notices: Vec<(Level, String)>,
}

/// The latest value of a path.
#[derive(Default)]
struct Latest {
    /// The newest value, copied.
    value: value::Value,
    /// When it was taken.
    timestamp: u64,
    /// The values received at the path, a block counted once.
    count: usize,
    /// The traces of the value, with only its latest sample, if it holds the readback
    /// of an announced action.
    traces: Option<Traces>,
}

impl MemoryStore {
    /// Removes the received data but what describes the targets: their units, actions
    /// and clocks, with which the recording starts again.
    pub fn clear(&mut self) {
        self.drop_samples();
        self.warned_time_base = false;
        let has_unix = self.clocks.values().any(|clock| *clock == TimeBase::Unix);
        self.base = has_unix.then_some(TimeBase::Unix);
    }

    /// Returns what went wrong or needs telling since the last call, to show it.
    pub fn take_notices(&mut self) -> Vec<(Level, String)> {
        std::mem::take(&mut self.notices)
    }

    /// Adds a notice, unless it is pending already.
    pub(crate) fn notify(&mut self, level: Level, text: String) {
        session::notify(&mut self.notices, level, text);
    }

    /// Takes the clock of `source`: none removes it. The epoch clock wins over the boot
    /// one, as the time axis shows one base.
    fn greet(&mut self, source: &str, clock: Option<TimeBase>) {
        match clock {
            Some(clock) => {
                self.clocks.insert(source.to_owned(), clock);
            }
            None => {
                self.clocks.remove(source);
            }
        }
        if clock == Some(TimeBase::Unix) {
            if self.base == Some(TimeBase::SinceBoot) {
                self.drop_samples();
                self.warn_clocks();
            }
            self.base = Some(TimeBase::Unix);
        }
        let has = |base| self.clocks.values().any(|clock| *clock == base);
        if has(TimeBase::Unix) && has(TimeBase::SinceBoot) {
            self.warn_clocks();
        }
    }

    /// Warns once that the sources send different clocks.
    fn warn_clocks(&mut self) {
        if self.warned_clocks {
            return;
        }
        self.warned_clocks = true;
        let text = "Connections send different clocks: blip shows the time of day and \
                    drops the data timed since boot";
        log::warn!("{text}");
        self.notify(Level::Warning, text.to_owned());
    }

    /// Removes the samples and log records received, but keeps the metadata, actions,
    /// descriptions and the recording. The latest values stay, as the count of readback
    /// samples only grows.
    fn drop_samples(&mut self) {
        self.traces.clear_samples();
        self.logs = Logs::default();
        self.origin = None;
        self.paths_version = Version::default();
        self.samples_version = Version::default();
    }

    /// What the timestamps of the data count from, `None` before there is any data.
    pub fn time_base(&self) -> Option<TimeBase> {
        self.base
            .or_else(|| self.time_span().map(|(_, newest)| TimeBase::guess(newest)))
    }

    /// Applies the sample at `path`.
    fn sample(&mut self, source: &str, timestamp: u64, path: &str, value: value::ValueRef<'_>) {
        if !self.fits_time_base(source, timestamp, path) {
            return;
        }
        self.set_origin(timestamp);
        let generation = self.traces.generation();
        self.flat.fill(value);
        self.traces
            .insert(path, timestamp, &self.flat, value.texts());
        if self.traces.generation() != generation {
            self.paths_version = Version::default();
        }
        self.samples_version = Version::default();
    }

    /// Adds the log record, unless its time does not fit the time base of the model.
    fn log(&mut self, source: &str, timestamp: u64, path: &str, level: u8, message: &str) {
        if timestamp > 0 && !self.fits_time_base(source, timestamp, path) {
            return;
        }
        // a log without a timestamp happened about when the newest data did, in the time
        // domain of the device
        let now = self.time_span().map(|(_, newest)| newest);
        let entry = LogEntry::new(timestamp, level, message, now);
        if timestamp > 0 {
            // not the wall clock of a record without a timestamp, which may be far from
            // the time domain of the target
            self.set_origin(timestamp);
        }
        if self.logs.push(path, entry) {
            self.paths_version = Version::default();
        }
        self.samples_version = Version::default();
    }

    /// Makes `timestamp` the origin if there is none yet. The log records before it
    /// had no timestamp and got the wall clock; if that is not in the time base of
    /// `timestamp`, e.g. the uptime of a target, they move to `timestamp`, as they
    /// would be decades away from the data.
    fn set_origin(&mut self, timestamp: u64) {
        if self.origin.is_some() {
            return;
        }
        self.origin = Some(timestamp);
        let base = self.base.unwrap_or_else(|| TimeBase::guess(timestamp));
        if (self.logs.entries()).any(|entry| TimeBase::guess(entry.time) != base) {
            self.logs.move_all_to(timestamp);
        }
    }

    /// Whether `timestamp` from `source` has the time base of the model, the Unix epoch
    /// or the boot of the device, as the time axis shows only one. The time base of a
    /// timestamp is the clock of its source, else guessed from its value. The first one
    /// sets the model's. Warns of the first sample at `path` that does not fit, which is
    /// dropped, e.g. from a source with the time of its computer while the target sends
    /// its uptime.
    fn fits_time_base(&mut self, source: &str, timestamp: u64, path: &str) -> bool {
        let base = (self.clocks.get(source).copied()).unwrap_or_else(|| TimeBase::guess(timestamp));
        let expected = *self.base.get_or_insert(base);
        if base == expected {
            return true;
        }
        if !self.warned_time_base {
            self.warned_time_base = true;
            let counts = |base| match base {
                TimeBase::Unix => "the Unix epoch (UTC)",
                TimeBase::SinceBoot => "the boot of the target",
            };
            log::warn!(
                "{path}: dropped a sample timed since {}, as the data is timed since {}, \
                 and one time axis cannot show both (further ones are dropped silently)",
                counts(base),
                counts(expected),
            );
        }
        false
    }

    /// The timestamp of the first value or log record in nanoseconds, from which the
    /// times of the views count, `None` before there is one.
    pub fn origin(&self) -> Option<u64> {
        self.origin
    }

    /// Changes whenever a trace, a log path or an announcement of actions arrives.
    pub fn paths_version(&self) -> Version {
        self.paths_version
    }

    /// Changes whenever a value or log record arrives.
    pub fn samples_version(&self) -> Version {
        self.samples_version
    }

    /// Keeps `value` as the latest at `path`, with the traces of it if it holds a
    /// readback.
    fn keep_latest(&mut self, path: &str, timestamp: u64, value: value::ValueRef<'_>) {
        let MemoryStore {
            latest,
            actions,
            flat,
            ..
        } = self;
        let latest = match latest.get_mut(path) {
            Some(latest) => latest,
            None => latest.entry(path.into()).or_default(),
        };
        latest.value.copy_from(value);
        latest.timestamp = timestamp;
        latest.count += 1;
        latest.traces = actions
            .holds_readback(path)
            .then(|| latest_traces(path, latest, flat));
    }

    /// The latest value at the path that holds the trace at `readback`, the longest if
    /// several do.
    fn latest_holding(&self, readback: &str) -> Option<&Latest> {
        self.latest
            .iter()
            .filter(|(path, _)| actions::holds(path, readback))
            .max_by_key(|(path, _)| path.len())
            .map(|(_, latest)| latest)
    }

    /// The readback trace at `readback` with only its latest sample, also while not
    /// recording, if its value arrived since it is the readback of an announced action.
    pub fn readback(&self, readback: &str) -> Option<traces::Trace<'_>> {
        self.latest_holding(readback)?
            .traces
            .as_ref()?
            .get(readback)
    }

    /// The number of values received at the path holding the readback trace at
    /// `readback`, also while not recording, 0 if there is none. It only grows, also
    /// when the data is cleared.
    pub fn readback_samples(&self, readback: Option<&str>) -> usize {
        readback
            .and_then(|readback| self.latest_holding(readback))
            .map_or(0, |latest| latest.count)
    }

    /// Whether values or log records were received, to save or clear them.
    pub fn has_samples(&self) -> bool {
        self.time_span().is_some()
    }

    /// Timestamps of the first and the newest collected data.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        [self.traces.time_span(), self.logs.time_span()]
            .into_iter()
            .flatten()
            .reduce(|(first_a, last_a), (first_b, last_b)| {
                (first_a.min(first_b), last_a.max(last_b))
            })
    }

    /// What arrived in each of `bins` bins of `step` nanoseconds from `start`
    /// nanoseconds on: the values, a record once, and the log records by level.
    pub fn activity(&self, start: u64, step: u64, bins: usize) -> Vec<Activity> {
        let mut activity = vec![Activity::default(); bins];
        let step = step.max(1);
        let edge = |bin: usize| start.saturating_add(step.saturating_mul(bin as u64));
        for (_, timestamps) in self.traces.tables() {
            let mut from = lower_bound(timestamps, start);
            for (bin, activity) in activity.iter_mut().enumerate() {
                let to = from + lower_bound(&timestamps[from..], edge(bin + 1));
                activity.values += (to - from) as u32;
                from = to;
            }
        }
        for entry in self.logs.between(start, edge(bins)).flatten() {
            if let Some(activity) = usize::try_from((entry.time - start) / step)
                .ok()
                .and_then(|bin| activity.get_mut(bin))
            {
                activity.logs[entry.level.as_index()] += 1;
            }
        }
        activity
    }
}

impl Store for MemoryStore {
    fn paths_version(&self) -> Version {
        self.paths_version
    }

    fn samples_version(&self) -> Version {
        self.samples_version
    }

    fn origin(&self) -> Option<u64> {
        self.origin
    }

    fn time_base(&self) -> Option<TimeBase> {
        MemoryStore::time_base(self)
    }

    fn time_span(&self) -> Option<(u64, u64)> {
        MemoryStore::time_span(self)
    }

    fn activity(&self, start: u64, step: u64, bins: usize) -> Vec<Activity> {
        MemoryStore::activity(self, start, step, bins)
    }

    fn traces(&self) -> Box<dyn Iterator<Item = traces::Trace<'_>> + '_> {
        Box::new(self.traces.iter())
    }

    fn kinds(&self) -> u64 {
        self.traces.kinds()
    }

    // all samples are at hand: the windows need no work, and nothing is copied

    fn trace(&self, path: &str, _window: Window) -> Loaded<Option<traces::Trace<'_>>> {
        Loaded::complete(self.traces.get(path))
    }

    fn series(&self, path: &str, _window: Window) -> Loaded<Option<traces::Series<'_>>> {
        Loaded::complete(self.traces.series(path))
    }

    fn field_paths<'a>(&'a self, path: &str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        Box::new(self.traces.field_paths(path))
    }

    fn metadata(&self, path: &str) -> Option<&proto::TraceMetadata> {
        self.traces.metadata(path)
    }

    fn unit(&self, path: &str) -> Option<&str> {
        self.traces.unit(path)
    }

    fn description(&self, path: &str) -> Option<&str> {
        self.traces.description(path)
    }

    fn labels(&self, path: &str) -> Option<&numbers::Labels> {
        self.traces.labels(path)
    }

    fn expected(&self, path: &str) -> Option<(f64, f64)> {
        self.traces.expected(path)
    }

    fn logs(&self, _window: Window) -> Loaded<&Logs> {
        Loaded::complete(&self.logs)
    }

    fn actions(&self) -> &Actions {
        &self.actions
    }

    fn readback(&self, readback: &str) -> Option<traces::Trace<'_>> {
        MemoryStore::readback(self, readback)
    }

    fn readback_samples(&self, readback: Option<&str>) -> usize {
        MemoryStore::readback_samples(self, readback)
    }
}

/// What arrived in a stretch of time, see [`MemoryStore::activity`].
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Activity {
    /// The number of values, a record counted once.
    pub values: u32,
    /// The number of log records by `LogLevel::as_index`.
    pub logs: [u32; 5],
}

impl Activity {
    pub fn log_count(&self) -> u32 {
        self.logs.iter().sum()
    }
}

/// The traces of the latest value at `path`, with only that sample.
fn latest_traces(path: &str, latest: &Latest, flat: &mut flatten::Flat) -> Traces {
    let mut traces = Traces::default();
    flat.fill(latest.value.as_ref());
    traces.insert(path, latest.timestamp, flat, latest.value.texts());
    traces
}

impl Ingest for MemoryStore {
    fn ingest(&mut self, source: &str, input: Input<'_>) {
        match input {
            Input::Sample {
                timestamp,
                path,
                value,
            } => {
                self.keep_latest(path, timestamp, value);
                self.sample(source, timestamp, path, value);
            }
            Input::Samples {
                path,
                times,
                values,
            } => {
                let last = values.len().checked_sub(1);
                if let (Some(last), Some(value)) = (last, values.last())
                    && let Some(timestamp) = times.timestamp(last)
                {
                    self.keep_latest(path, timestamp, value);
                }
                for (row, value) in values.iter().enumerate() {
                    let Some(timestamp) = times.timestamp(row) else {
                        break;
                    };
                    self.sample(source, timestamp, path, value);
                }
            }
            Input::Latest {
                timestamp,
                path,
                value,
            } => self.keep_latest(path, timestamp, value),
            Input::Log {
                timestamp,
                path,
                level,
                message,
            } => self.log(source, timestamp, path, level, message),
            Input::Metadata(metadata) => self.traces.set_metadata(metadata),
            Input::Actions(actions) => {
                self.actions.set(source, actions.to_vec());
                self.paths_version = Version::default();
                // values that arrived before they became readbacks
                let MemoryStore {
                    latest,
                    actions,
                    flat,
                    ..
                } = self;
                for (path, latest) in latest.iter_mut() {
                    match (actions.holds_readback(path), &latest.traces) {
                        (true, None) => {
                            latest.traces = Some(latest_traces(path, latest, flat));
                        }
                        (false, Some(_)) => latest.traces = None,
                        _ => {}
                    }
                }
            }
            Input::Clock(clock) => self.greet(source, clock),
        }
    }

    fn clear(&mut self) {
        MemoryStore::clear(self);
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn take_notices(&mut self) -> Vec<(Level, String)> {
        MemoryStore::take_notices(self)
    }
}

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
