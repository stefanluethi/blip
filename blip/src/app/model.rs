pub mod logs;
pub mod streams;

use crate::app::model::logs::Logs;
use crate::app::model::streams::Streams;

pub struct Model {
    pub streams: Streams,
    pub logs: Logs,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            streams: Streams::new(),
            logs: Logs::new(),
        }
    }
}

impl Model {
    /// Timestamps of the first and the newest collected data.
    pub fn time_span(&self) -> Option<(u64, u64)> {
        [self.streams.time_span(), self.logs.time_span()]
            .into_iter()
            .flatten()
            .reduce(|(first_a, last_a), (first_b, last_b)| (first_a.min(first_b), last_a.max(last_b)))
    }
}
