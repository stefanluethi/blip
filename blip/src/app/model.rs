pub mod logs;
pub mod numbers;
pub mod recording;
pub mod traces;

use crate::app::model::logs::{LogEntry, Logs};
use crate::app::model::recording::Recording;
use crate::app::model::traces::Traces;

#[derive(Default)]
pub struct Model {
    pub traces: Traces,
    pub logs: Logs,
    /// The received frames, to save them.
    pub recording: Recording,
}

impl Model {
    /// Records a frame received from `source`, the name of its connection, and applies
    /// the message in it. The paths of its traces and the components of its logs start
    /// with the source, unless it has no name.
    pub fn receive(&mut self, source: &str, frame: &[u8]) {
        self.recording.push(source, frame);
        match serde_cbor_2::from_slice::<proto::Message>(frame) {
            Ok(proto::Message::Value(mut value)) => {
                value.path = sourced(source, &value.path);
                self.traces.append(&value);
            }
            Ok(proto::Message::ValueMetadata(metadata)) => {
                let path = sourced(source, &metadata.path);
                self.traces.set_unit(&path, &metadata.unit);
            }
            Ok(proto::Message::Log(record)) => {
                let mut entry = LogEntry::from_proto(&record);
                entry.component = sourced(source, &entry.component);
                self.logs.push(entry);
            }
            Err(error) => log::warn!("error parsing message: {error}"),
        }
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

/// `name` below `source`, i.e. `"{source}/{name}"`, or `name` if the source has no name.
fn sourced(source: &str, name: &str) -> String {
    if source.is_empty() {
        name.to_owned()
    } else {
        format!("{source}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(message: &proto::Message) -> Vec<u8> {
        serde_cbor_2::to_vec(message).unwrap()
    }

    #[test]
    fn traces_and_logs_are_put_below_their_source() {
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
        for path in ["cpu", "a/cpu", "b/cpu"] {
            assert!(model.traces.get(path).is_some(), "{path} is missing");
        }
        assert_eq!(model.logs.entries[0].component, "b/motor");
        assert_eq!(model.recording.len(), 4);
    }
}
