pub mod actions;
pub mod cbor;
pub mod decode;
pub mod growing;
pub mod logs;
pub mod numbers;
pub mod recording;
pub mod traces;

use crate::app::model::actions::Actions;
use crate::app::model::logs::{LogEntry, Logs};
use crate::app::model::recording::Recording;
use crate::app::model::traces::Traces;
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
    /// Changes whenever a trace or an announcement of actions arrives.
    paths_version: Version,
    /// Changes whenever a value or log record arrives.
    samples_version: Version,
    /// The value being applied, kept to apply the next one without allocating.
    flat: decode::Flat,
    /// The timestamp of the first value or log record, from which times count.
    origin: Option<u64>,
}

/// The latest frame of `source` with its actions or the unit of a path.
struct Description {
    source: String,
    /// The path of the unit, `None` for the actions.
    path: Option<String>,
    frame: Vec<u8>,
}

impl Model {
    /// Removes the received data but the descriptions of the targets, their units and
    /// actions, with which the recording starts again.
    pub fn clear(&mut self) {
        let descriptions = std::mem::take(&mut self.descriptions);
        *self = Model::default();
        for description in &descriptions {
            self.receive(&description.source, &description.frame);
        }
    }

    /// Keeps `frame` from `source` as its latest description of the unit of `path`, or
    /// of its actions if `None`.
    fn describe(&mut self, source: &str, path: Option<&str>, frame: &[u8]) {
        let known = self
            .descriptions
            .iter_mut()
            .find(|known| known.source == source && known.path.as_deref() == path);
        match known {
            Some(known) => frame.clone_into(&mut known.frame),
            None => self.descriptions.push(Description {
                source: source.to_owned(),
                path: path.map(str::to_owned),
                frame: frame.to_vec(),
            }),
        }
    }

    /// Records a frame received from `source`, the name of its connection, and applies
    /// the message in it. The paths of its traces and actions and the components of its
    /// logs are kept as the outpost sends them, without the source: how they are
    /// organized is up to the user, also when outposts send the same paths. Returns
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
        // samples are dropped while not recording, without decoding them
        if !recording && decode::is_sample(frame) {
            return None;
        }
        let message = decode::decode(frame, &mut self.flat);
        if recording {
            self.recording.push(source, frame);
        }
        match message {
            Ok(decode::Message::Value { timestamp, path }) => {
                self.origin.get_or_insert(timestamp);
                let generation = self.traces.generation();
                self.traces.insert(path, timestamp, &self.flat, frame);
                if self.traces.generation() != generation {
                    self.paths_version = Version::default();
                }
                self.samples_version = Version::default();
            }
            Ok(decode::Message::Log {
                timestamp,
                level,
                component,
                message,
            }) => {
                // a log without a timestamp happened about when the newest data did, in
                // the time domain of the device
                let now = self.time_span().map(|(_, newest)| newest);
                let entry = LogEntry::new(timestamp, level, component, message, now);
                if timestamp > 0 {
                    // not the wall clock of a record without a timestamp, which may be
                    // far from the time domain of the target
                    self.origin.get_or_insert(timestamp);
                }
                self.logs.push(entry);
                self.samples_version = Version::default();
            }
            Ok(decode::Message::Other(proto::Message::ValueMetadata(metadata))) => {
                self.describe(source, Some(&metadata.path), frame);
                self.traces.set_unit(&metadata.path, &metadata.unit);
            }
            Ok(decode::Message::Other(proto::Message::Actions(actions))) => {
                self.describe(source, None, frame);
                self.actions.set(source, actions);
                self.paths_version = Version::default();
            }
            Ok(decode::Message::Other(proto::Message::ActionResult(result))) => {
                return Some(result);
            }
            Ok(decode::Message::Other(proto::Message::Value(_) | proto::Message::Log(_))) => {
                unreachable!("values and logs are decoded apart")
            }
            Err(error) => log::warn!("error parsing message: {error}"),
        }
        None
    }

    /// The timestamp of the first value or log record in nanoseconds, from which the
    /// times of the views count, `None` before there is one.
    pub fn origin(&self) -> Option<u64> {
        self.origin
    }

    /// Changes whenever a trace or an announcement of actions arrives.
    pub fn paths_version(&self) -> Version {
        self.paths_version
    }

    /// Changes whenever a value or log record arrives.
    pub fn samples_version(&self) -> Version {
        self.samples_version
    }

    /// The number of samples of the readback trace at `readback`, 0 if there is none.
    pub fn readback_samples(&self, readback: Option<&str>) -> usize {
        readback
            .and_then(|readback| self.traces.get(readback))
            .map_or(0, |trace| trace.timestamps.len())
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
            let mut from = timestamps.partition_point(|&t| t < start);
            for (bin, activity) in activity.iter_mut().enumerate() {
                let to = from + timestamps[from..].partition_point(|&t| t < edge(bin + 1));
                activity.values += (to - from) as u32;
                from = to;
            }
        }
        let entries = &self.logs.entries;
        let first = entries.partition_point(|entry| entry.time < start);
        for entry in &entries[first..] {
            let Some(activity) = usize::try_from((entry.time - start) / step)
                .ok()
                .and_then(|bin| activity.get_mut(bin))
            else {
                break;
            };
            activity.logs[entry.level.as_index()] += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(message: &proto::Message) -> Vec<u8> {
        serde_cbor_2::to_vec(message).unwrap()
    }

    #[test]
    fn paths_are_kept_as_the_outposts_send_them() {
        let value = |path: &str| {
            frame(&proto::Message::Value(proto::Value {
                timestamp: 1,
                path: path.to_owned(),
                value: proto::Data::Uint(1),
            }))
        };
        let log = frame(&proto::Message::Log(proto::LogRecord {
            timestamp: 1,
            level: 2,
            component: "motor".to_owned(),
            message: "hi".to_owned(),
        }));
        let mut model = Model::default();
        model.receive("", &value("cpu"));
        model.receive("a", &value("cpu"));
        model.receive("b", &value("cpu"));
        model.receive("b", &log);
        // the same path from several outposts is one trace
        assert_eq!(model.traces.iter().count(), 1);
        assert_eq!(model.traces.get("cpu").unwrap().timestamps.len(), 3);
        assert_eq!(model.logs.entries[0].component, "motor");
        assert_eq!(model.recording.messages(), 4);
    }

    #[test]
    fn activity_is_counted_per_bin_with_records_once() {
        let value = |timestamp: u64, path: &str, value: proto::Data| {
            frame(&proto::Message::Value(proto::Value {
                timestamp,
                path: path.to_owned(),
                value,
            }))
        };
        let imu = proto::Data::Record(
            [("x", 1.0), ("y", 2.0)]
                .into_iter()
                .map(|(name, x)| (name.to_owned(), proto::Data::Float(x)))
                .collect(),
        );
        let mut model = Model::default();
        for t in [0, 10, 19, 20, 39, 40] {
            model.receive("", &value(t, "cpu", proto::Data::Uint(1)));
        }
        model.receive("", &value(25, "imu", imu.clone()));
        model.receive("", &value(35, "imu", imu));
        model.receive(
            "",
            &frame(&proto::Message::Log(proto::LogRecord {
                timestamp: 5,
                level: 2,
                component: String::new(),
                message: "hi".to_owned(),
            })),
        );
        let values = |activity: Vec<Activity>| -> Vec<u32> {
            activity.iter().map(|bin| bin.values).collect()
        };
        // [0, 20), [20, 40)
        let halves = model.activity(0, 20, 2);
        assert_eq!(values(halves.clone()), [3, 4]);
        assert_eq!(halves[0].logs, [0, 0, 1, 0, 0]);
        assert_eq!(halves[1].log_count(), 0);
        assert_eq!(values(model.activity(0, 10, 4)), [1, 2, 2, 2]);
        assert_eq!(values(model.activity(10, 10, 1)), [2]);
        assert!(model.activity(0, 10, 0).is_empty());

        // a record without a field of the first is dropped and does not count
        let partial = proto::Data::Record(
            [("y".to_owned(), proto::Data::Float(3.0))]
                .into_iter()
                .collect(),
        );
        model.receive("", &value(45, "imu", partial));
        assert_eq!(values(model.activity(40, 10, 1)), [1]);
    }

    #[test]
    fn actions_are_kept_with_their_source_and_answers_returned() {
        let actions = frame(&proto::Message::Actions(vec![proto::ActionDescriptor {
            path: "motor/speed".to_owned(),
            description: String::new(),
            params: vec![],
            readback: Some("motor/rpm".to_owned()),
            confirm: false,
            enabled: true,
            disabled_reason: None,
        }]));
        let result = proto::ActionResult {
            id: 3,
            result: Ok(()),
        };
        let mut model = Model::default();
        assert_eq!(model.receive("b", &actions), None);
        let speed = model.actions.announced("b", "motor/speed").unwrap();
        assert_eq!(speed.readback.as_deref(), Some("motor/rpm"));
        assert!(model.actions.announced("a", "motor/speed").is_none());
        let answer = frame(&proto::Message::ActionResult(result.clone()));
        assert_eq!(model.receive("b", &answer), Some(result));
    }

    /// Measures how many messages a second the model takes in: scalars on 100 paths
    /// and a record of a float, an array and a string.
    #[test]
    #[ignore = "a benchmark, run with `cargo test --release -p blip ingest -- --ignored --nocapture`"]
    fn ingest_rate() {
        let scalar = |timestamp: u64, path: &str| {
            frame(&proto::Message::Value(proto::Value {
                timestamp,
                path: path.to_owned(),
                value: proto::Data::Float(timestamp as f64 * 0.5),
            }))
        };
        let imu = |timestamp: u64| {
            frame(&proto::Message::Value(proto::Value {
                timestamp,
                path: "sensors/imu".to_owned(),
                value: proto::Data::Record(vec![
                    ("t".to_owned(), proto::Data::Float(21.5)),
                    (
                        "acc".to_owned(),
                        proto::Data::Array(vec![proto::Data::Float(0.1); 3]),
                    ),
                    ("mode".to_owned(), proto::Data::from("run")),
                ]),
            }))
        };
        let paths: Vec<String> = (0..100).map(|n| format!("cpu/core{n}")).collect();
        let frames: Vec<Vec<u8>> = (0..1_000_000u64)
            .map(|n| match n % 10 {
                0 => imu(n),
                _ => scalar(n, &paths[n as usize % 100]),
            })
            .collect();
        let mut model = Model::default();
        let start = std::time::Instant::now();
        for frame in &frames {
            model.receive("a", frame);
        }
        let seconds = start.elapsed().as_secs_f64();
        println!(
            "{} messages in {seconds:.3} s: {:.0} messages/s",
            frames.len(),
            frames.len() as f64 / seconds
        );
        assert_eq!(
            model
                .traces
                .get("sensors/imu.mode")
                .unwrap()
                .timestamps
                .len(),
            100_000
        );
    }

    #[test]
    fn the_origin_is_the_first_timestamp_a_target_sent() {
        let log = |timestamp| {
            frame(&proto::Message::Log(proto::LogRecord {
                timestamp,
                level: 2,
                component: String::new(),
                message: "hi".to_owned(),
            }))
        };
        let mut model = Model::default();
        // a record without a timestamp gets the wall clock, which is no origin
        model.receive("a", &log(0));
        assert_eq!(model.origin(), None);
        model.receive(
            "a",
            &frame(&proto::Message::Value(proto::Value {
                timestamp: 5_000_000_000,
                path: "rpm".to_owned(),
                value: proto::Data::Uint(1),
            })),
        );
        model.receive("a", &log(7_000_000_000));
        assert_eq!(model.origin(), Some(5_000_000_000));
    }

    #[test]
    fn samples_are_dropped_while_not_recording() {
        let value = frame(&proto::Message::Value(proto::Value {
            timestamp: 1,
            path: "rpm".to_owned(),
            value: proto::Data::Uint(1),
        }));
        let log = frame(&proto::Message::Log(proto::LogRecord {
            timestamp: 1,
            level: 2,
            component: "motor".to_owned(),
            message: "hi".to_owned(),
        }));
        let metadata = frame(&proto::Message::ValueMetadata(proto::ValueMetadata {
            path: "rpm".to_owned(),
            unit: "rpm".to_owned(),
        }));
        let actions = frame(&proto::Message::Actions(vec![]));
        let result = proto::ActionResult {
            id: 3,
            result: Ok(()),
        };
        let answer = frame(&proto::Message::ActionResult(result.clone()));

        let mut model = Model::default();
        model.receive_unrecorded("b", &value);
        model.receive_unrecorded("b", &log);
        model.receive_unrecorded("b", &metadata);
        model.receive_unrecorded("b", &actions);
        assert_eq!(model.receive_unrecorded("b", &answer), Some(result));
        assert_eq!(model.time_span(), None);
        assert!(model.logs.entries.is_empty());
        // nothing is recorded, a recording starts with the kept units and actions
        assert_eq!(model.recording.messages(), 0);
        model.clear();
        assert_eq!(model.recording.messages(), 2);

        model.receive("b", &value);
        assert_eq!(model.traces.get("rpm").unwrap().timestamps.len(), 1);
        assert_eq!(model.traces.unit("rpm"), Some("rpm"));
    }

    #[test]
    fn clearing_keeps_the_latest_units_and_actions_and_records_them_again() {
        let unit = |unit: &str| {
            frame(&proto::Message::ValueMetadata(proto::ValueMetadata {
                path: "rpm".to_owned(),
                unit: unit.to_owned(),
            }))
        };
        let value = frame(&proto::Message::Value(proto::Value {
            timestamp: 1,
            path: "rpm".to_owned(),
            value: proto::Data::Uint(1),
        }));
        let actions = frame(&proto::Message::Actions(vec![proto::ActionDescriptor {
            path: "motor/speed".to_owned(),
            description: String::new(),
            params: vec![],
            readback: None,
            confirm: false,
            enabled: true,
            disabled_reason: None,
        }]));
        let mut model = Model::default();
        model.receive("a", &unit("rpm"));
        model.receive("b", &actions);
        model.receive("a", &value);
        model.receive("a", &unit("1/min"));
        model.clear();

        assert_eq!(model.time_span(), None);
        assert_eq!(model.traces.unit("rpm"), Some("1/min"));
        assert!(model.actions.announced("b", "motor/speed").is_some());
        let mut recorded = vec![];
        recording::read_file(&model.recording.to_file(), |source, frame| {
            recorded.push((source.to_owned(), frame.to_vec()));
        })
        .unwrap();
        assert_eq!(
            recorded,
            [("a".to_owned(), unit("1/min")), ("b".to_owned(), actions)]
        );
    }
}
