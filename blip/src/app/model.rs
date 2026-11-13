pub mod actions;
pub mod cbor;
pub mod decode;
pub mod growing;
pub mod logs;
pub mod numbers;
pub mod recording;
pub mod sorted;
pub mod traces;

use crate::app::model::actions::Actions;
use crate::app::model::logs::{LogEntry, Logs};
use crate::app::model::recording::Recording;
use crate::app::model::sorted::lower_bound;
use crate::app::model::traces::Traces;
use crate::app::time::TimeBase;
use crate::app::toasts;
use std::collections::HashMap;
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

#[derive(Default)]
pub struct Model {
    pub traces: Traces,
    pub logs: Logs,
    /// The actions the targets announced.
    pub actions: Actions,
    /// The received frames, to save them.
    pub recording: Recording,
    /// The latest frames the targets describe themselves with, which they send only
    /// once or seldom, in the order they first arrived.
    descriptions: Vec<Description>,
    /// Changes whenever a trace, a log path or an announcement of actions arrives.
    paths_version: Version,
    /// Changes whenever a value or log record arrives.
    samples_version: Version,
    /// The value being applied, kept to apply the next one without allocating.
    flat: decode::Flat,
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
    notices: Vec<(toasts::Level, String)>,
}

/// The latest value of a path.
struct Latest {
    frame: Vec<u8>,
    /// The values received at the path, a block counted once.
    count: usize,
    /// The traces of the value, with only its latest sample, if it holds the readback
    /// of an announced action.
    traces: Option<Traces>,
}

/// What a [`Description`] describes.
#[derive(PartialEq)]
enum DescriptionKind {
    Hello,
    Actions,
    /// The unit of the trace at the path.
    Unit(String),
}

/// The latest frame of `source` with its Hello, its actions or the unit of a path.
struct Description {
    source: String,
    kind: DescriptionKind,
    frame: Vec<u8>,
}

impl Model {
    /// Removes the received data but the descriptions of the targets, their units and
    /// actions, with which the recording starts again.
    pub fn clear(&mut self) {
        let descriptions = std::mem::take(&mut self.descriptions);
        let latest = std::mem::take(&mut self.latest);
        let notices = std::mem::take(&mut self.notices);
        let warned_clocks = self.warned_clocks;
        *self = Model::default();
        self.notices = notices;
        self.warned_clocks = warned_clocks;
        for description in &descriptions {
            self.receive(&description.source, &description.frame);
        }
        self.latest = latest;
    }

    /// Returns what went wrong or needs telling since the last call, to show it.
    pub fn take_notices(&mut self) -> Vec<(toasts::Level, String)> {
        std::mem::take(&mut self.notices)
    }

    /// Adds a notice, unless it is pending already.
    fn notify(&mut self, level: toasts::Level, text: String) {
        if !self.notices.iter().any(|(l, t)| *l == level && *t == text) {
            self.notices.push((level, text));
        }
    }

    /// Keeps `frame` from `source` as its latest description of `kind`.
    fn describe(&mut self, source: &str, kind: DescriptionKind, frame: &[u8]) {
        let known = (self.descriptions.iter_mut())
            .find(|known| known.source == source && known.kind == kind);
        match known {
            Some(known) => frame.clone_into(&mut known.frame),
            None => self.descriptions.push(Description {
                source: source.to_owned(),
                kind,
                frame: frame.to_vec(),
            }),
        }
    }

    /// Takes the clock of the Hello of `source`: a Hello without a known one removes
    /// it. The epoch clock wins over the boot one, as the time axis shows one base.
    fn greet(&mut self, source: &str, hello: &proto::Hello) {
        match hello.clock {
            Some(clock) => {
                self.clocks.insert(source.to_owned(), clock.into());
            }
            None => {
                self.clocks.remove(source);
            }
        }
        if hello.clock == Some(proto::Clock::Epoch) {
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
        self.notify(toasts::Level::Warning, text.to_owned());
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

    /// Records a frame received from `source`, the name of its connection, and applies
    /// the message in it. The paths of its traces,
    /// actions and logs are kept as the outpost sends them, without the source: how they
    /// are organized is up to the user, also when outposts send the same paths. Returns
    /// the answer to an invocation, for the invoker.
    pub fn receive(&mut self, source: &str, frame: &[u8]) -> Option<proto::ActionResult> {
        self.apply(source, frame, true)
    }

    /// Like [`Self::receive`] while not recording: records nothing and drops the
    /// samples, i.e. values and log records, but keeps what the targets send only once
    /// or in answer, their units, their actions and the answers to invocations. A
    /// recording starts with the units and actions, see [`Self::clear`].
    pub fn receive_unrecorded(
        &mut self,
        source: &str,
        frame: &[u8],
    ) -> Option<proto::ActionResult> {
        self.apply(source, frame, false)
    }

    fn apply(
        &mut self,
        source: &str,
        frame: &[u8],
        recording: bool,
    ) -> Option<proto::ActionResult> {
        // samples are dropped while not recording, without decoding them, except that
        // the latest value of each path is kept, decoded if it holds a readback
        if let Some(path) = decode::sample_path(frame) {
            self.keep_latest(path, frame);
        }
        if !recording && decode::is_sample(frame) {
            return None;
        }
        let message = decode::decode(frame, &mut self.flat);
        if recording {
            self.recording.push(source, frame);
        }
        match message {
            Ok(decode::Message::Sample { timestamp, path }) => {
                self.sample(source, timestamp, path, frame);
            }
            Ok(decode::Message::Samples(mut block)) => {
                while let Some(timestamp) = block.next(frame, &mut self.flat) {
                    self.sample(source, timestamp, block.path, frame);
                }
            }
            Ok(decode::Message::Log {
                timestamp,
                path,
                level,
                message,
            }) => {
                if timestamp > 0 && !self.fits_time_base(source, timestamp, path) {
                    return None;
                }
                // a log without a timestamp happened about when the newest data did, in
                // the time domain of the device
                let now = self.time_span().map(|(_, newest)| newest);
                let entry = LogEntry::new(timestamp, level, message, now);
                if timestamp > 0 {
                    // not the wall clock of a record without a timestamp, which may be
                    // far from the time domain of the target
                    self.set_origin(timestamp);
                }
                if self.logs.push(path, entry) {
                    self.paths_version = Version::default();
                }
                self.samples_version = Version::default();
            }
            Ok(decode::Message::Other(proto::Message::TraceMetadata(metadata))) => {
                self.describe(source, DescriptionKind::Unit(metadata.path.clone()), frame);
                self.traces.set_metadata(&metadata);
            }
            Ok(decode::Message::Other(proto::Message::Actions(actions))) => {
                self.describe(source, DescriptionKind::Actions, frame);
                self.actions.set(source, actions);
                self.paths_version = Version::default();
                // values that arrived before they became readbacks
                let Model {
                    latest,
                    actions,
                    flat,
                    ..
                } = self;
                for (path, latest) in latest {
                    match (actions.holds_readback(path), &latest.traces) {
                        (true, None) => latest.traces = latest_traces(&latest.frame, flat),
                        (false, Some(_)) => latest.traces = None,
                        _ => {}
                    }
                }
            }
            Ok(decode::Message::Other(proto::Message::ActionResult(result))) => {
                return Some(result);
            }
            Ok(decode::Message::Other(proto::Message::Hello(hello))) => {
                self.describe(source, DescriptionKind::Hello, frame);
                self.greet(source, &hello);
            }
            Ok(decode::Message::Other(
                proto::Message::Sample(_) | proto::Message::Samples(_) | proto::Message::Log(_),
            )) => unreachable!("samples and logs are decoded apart"),
            Err(error) => {
                log::warn!("error parsing message: {error}");
                if decode::message_name(frame) == Some("Samples") {
                    let text = if source.is_empty() {
                        "Dropped a malformed block of samples".to_owned()
                    } else {
                        format!("Dropped a malformed block of samples from {source}")
                    };
                    self.notify(toasts::Level::Error, text);
                }
            }
        }
        None
    }

    /// Applies a sample at `path`, flattened into `self.flat` from `frame`.
    fn sample(&mut self, source: &str, timestamp: u64, path: &str, frame: &[u8]) {
        if !self.fits_time_base(source, timestamp, path) {
            return;
        }
        self.set_origin(timestamp);
        let generation = self.traces.generation();
        self.traces.insert(path, timestamp, &self.flat, frame);
        if self.traces.generation() != generation {
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

    /// Keeps `frame` as the latest value at `path`, decoded if it holds a readback.
    fn keep_latest(&mut self, path: &str, frame: &[u8]) {
        let latest = match self.latest.get_mut(path) {
            Some(latest) => {
                frame.clone_into(&mut latest.frame);
                latest
            }
            None => self.latest.entry(path.into()).or_insert(Latest {
                frame: frame.to_vec(),
                count: 0,
                traces: None,
            }),
        };
        latest.count += 1;
        latest.traces = if self.actions.holds_readback(path) {
            latest_traces(frame, &mut self.flat)
        } else {
            None
        };
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

/// What arrived in a stretch of time, see [`Model::activity`].
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Activity {
    /// The number of values, a record counted once.
    pub values: u32,
    /// The number of log records by [`LogLevel::as_index`].
    pub logs: [u32; 5],
}

impl Activity {
    pub fn log_count(&self) -> u32 {
        self.logs.iter().sum()
    }
}

/// The traces of the value in `frame`, with only that sample, the last of a block.
fn latest_traces(frame: &[u8], flat: &mut decode::Flat) -> Option<Traces> {
    let (timestamp, path) = match decode::decode(frame, flat).ok()? {
        decode::Message::Sample { timestamp, path } => (timestamp, path),
        decode::Message::Samples(mut block) => {
            for _ in 1..block.len {
                block.next(frame, flat)?;
            }
            (block.next(frame, flat)?, block.path)
        }
        _ => return None,
    };
    let mut traces = Traces::default();
    traces.insert(path, timestamp, flat, frame);
    Some(traces)
}

#[cfg(test)]
mod tests;
