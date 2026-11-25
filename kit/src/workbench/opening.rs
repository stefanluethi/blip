//! Opening a recording a part at a time, across frames, so that the view stays
//! responsive while a long one is read and shows how far it got.

use crate::store::session::Session;
use crate::store::{MemoryStore, recording};

/// How long the messages of a recording are read for in a frame, in seconds, leaving
/// the rest of a frame at 60 Hz to show the views.
const BUDGET: f64 = 0.008;

/// A recording being opened: read into a store of its own, which replaces the shown
/// one once all is read.
pub struct Opening {
    /// The name of the file, to tell the user.
    pub name: String,
    reader: recording::FileReader,
    store: MemoryStore,
}

impl Opening {
    /// Starts opening the recording `bytes` of the file `name`, or tells why it is
    /// none.
    pub fn new(name: String, bytes: Vec<u8>) -> Result<Self, String> {
        let reader = recording::FileReader::new(bytes)
            .map_err(|error| format!("{name} cannot be opened: {error}"))?;
        Ok(Self {
            name,
            reader,
            store: MemoryStore::default(),
        })
    }

    /// Reads the next part of the recording for about `BUDGET`. Returns whether all
    /// is read.
    pub fn step(&mut self) -> bool {
        let deadline = now() + BUDGET;
        self.read_while(|| now() < deadline)
    }

    /// Reads the next part of the recording while `more` tells there is time for more.
    /// Returns whether all is read.
    pub fn read_while(&mut self, more: impl FnMut() -> bool) -> bool {
        self.reader.read(more, &mut self.store)
    }

    /// How much of the file is read, from 0 to 1.
    pub fn progress(&self) -> f32 {
        self.reader.progress()
    }

    /// Puts what was read into `session`, which starts afresh with it: the store of the
    /// messages, the recorder that saves the file as it was opened, and the notices of
    /// the opening. Returns what was found in the file.
    pub fn finish(self, session: &mut Session) -> recording::Loaded {
        let (recorder, loaded) = self.reader.finish();
        session.start_fresh(self.store, recorder);
        for (level, text) in &loaded.notices {
            session.notify(*level, text.clone());
        }
        loaded
    }
}

/// The time in seconds from some point on, to measure how long reading takes.
#[cfg(not(target_arch = "wasm32"))]
fn now() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// The time in seconds from some point on, to measure how long reading takes.
#[cfg(target_arch = "wasm32")]
fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or_else(js_sys::Date::now, |performance| performance.now())
        / 1000.0
}
