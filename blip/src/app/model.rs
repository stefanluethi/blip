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