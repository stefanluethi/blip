pub mod actions;
pub mod logs;
pub mod numbers;
pub mod recording;
pub mod traces;

use crate::app::model::actions::Actions;
use crate::app::model::logs::{LogEntry, Logs};
use crate::app::model::recording::Recording;
use crate::app::model::traces::Traces;

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
        let message = serde_cbor_2::from_slice::<proto::Message>(frame);
        let sample = matches!(
            message,
            Ok(proto::Message::Value(_) | proto::Message::Log(_))
        );
        if sample && !recording {
            return None;
        }
        if recording {
            self.recording.push(source, frame);
        }
        match message {
            Ok(proto::Message::Value(value)) => self.traces.append(&value),
            Ok(proto::Message::ValueMetadata(metadata)) => {
                self.describe(source, Some(&metadata.path), frame);
                self.traces.set_unit(&metadata.path, &metadata.unit);
            }
            Ok(proto::Message::Log(record)) => self.logs.push(LogEntry::from_proto(&record)),
            Ok(proto::Message::Actions(actions)) => {
                self.describe(source, None, frame);
                self.actions.set(source, actions);
            }
            Ok(proto::Message::ActionResult(result)) => return Some(result),
            Err(error) => log::warn!("error parsing message: {error}"),
        }
        None
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
}

/// The items of the CBOR sequence (RFC 8742) `bytes`, each as its bytes, e.g. the
/// messages of a frame. Ends after the first item that cannot be read, with its error.
pub fn cbor_items(bytes: &[u8]) -> impl Iterator<Item = Result<&[u8], serde_cbor_2::Error>> {
    let mut items =
        serde_cbor_2::Deserializer::from_slice(bytes).into_iter::<serde::de::IgnoredAny>();
    let mut start = 0;
    std::iter::from_fn(move || {
        if start > bytes.len() {
            return None;
        }
        match items.next()? {
            Ok(_) => {
                let end = items.byte_offset();
                let item = &bytes[start..end];
                start = end;
                Some(Ok(item))
            }
            Err(error) => {
                // the rest cannot be told apart into items
                start = usize::MAX;
                Some(Err(error))
            }
        }
    })
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
    fn a_cbor_sequence_is_split_into_its_items() {
        let value = frame(&proto::Message::Value(proto::Value {
            timestamp: 1,
            path: "cpu".to_owned(),
            value: proto::Data::Float(12.5),
        }));
        let log = frame(&proto::Message::Log(proto::LogRecord {
            timestamp: 2,
            level: 2,
            component: String::new(),
            message: "hi".to_owned(),
        }));
        let batch = [value.clone(), log.clone()].concat();
        let items: Vec<&[u8]> = cbor_items(&batch).collect::<Result<_, _>>().unwrap();
        assert_eq!(items, [&value[..], &log[..]]);
        assert_eq!(cbor_items(&value).count(), 1);
        assert_eq!(cbor_items(&[]).count(), 0);

        // the items end with the first that cannot be read
        let cut = &batch[..batch.len() - 1];
        let items: Vec<_> = cbor_items(cut).collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_ref().ok(), Some(&&value[..]));
        assert!(items[1].is_err());
        let broken = [&value[..], &[0xff], &log[..]].concat();
        let items: Vec<_> = cbor_items(&broken).collect();
        assert_eq!(items.len(), 2);
        assert!(items[1].is_err());
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
