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
    /// Records a received frame and applies the message in it.
    pub fn receive(&mut self, frame: &[u8]) {
        self.recording.push(frame);
        match serde_cbor_2::from_slice::<proto::Message>(frame) {
            Ok(proto::Message::Value(value)) => self.traces.append(&value),
            Ok(proto::Message::ValueMetadata(metadata)) => {
                self.traces.set_unit(&metadata.path, &metadata.unit);
            }
            Ok(proto::Message::Log(record)) => self.logs.push(LogEntry::from_proto(&record)),
            Ok(proto::Message::Actions(_) | proto::Message::ActionResult(_)) => {}
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
