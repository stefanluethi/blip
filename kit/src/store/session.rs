//! The session: the store with the invoker of actions, whether it is recording, and
//! the sources it receives from. It does what happens each frame apart from the GUI:
//! receive, apply, and send the invocations.

use super::actions::Invoker;
use super::source::{Event, Source};
use super::{Level, MemoryStore};
use std::collections::HashSet;

/// A source of a session with the name its messages are kept under.
struct Attached {
    name: String,
    source: Box<dyn Source>,
    /// Whether the source told that it is open.
    open: bool,
}

/// The store, the invoker and the sources, and the rules between them: samples are
/// kept only while recording, a recording starts only while connected and clears the
/// data, and the first source to open starts afresh.
#[derive(Default)]
pub struct Session {
    pub store: MemoryStore,
    pub invoker: Invoker,
    /// Whether the samples received are kept, else they are dropped. See
    /// [`Self::set_recording`].
    pub recording: bool,
    sources: Vec<Attached>,
    /// What went wrong with the sources since [`Self::take_notices`].
    notices: Vec<(Level, String)>,
}

impl Session {
    /// A session that records from the start, for an application without a record
    /// button.
    pub fn recording() -> Self {
        Self {
            recording: true,
            ..Self::default()
        }
    }

    /// Adds `source`, whose messages are kept under `name` and whose actions are
    /// invoked by that name. Replaces the source of the same name.
    pub fn add_source(&mut self, name: &str, source: impl Source + 'static) {
        self.remove_source(name);
        self.sources.push(Attached {
            name: name.to_owned(),
            source: Box::new(source),
            open: false,
        });
    }

    /// Closes and removes the source named `name`, if there is one.
    pub fn remove_source(&mut self, name: &str) {
        self.sources.retain_mut(|attached| {
            if attached.name != name {
                return true;
            }
            attached.source.close();
            false
        });
    }

    /// The names of the open sources of the session.
    pub fn open_sources(&self) -> HashSet<String> {
        self.sources
            .iter()
            .filter(|attached| attached.open)
            .map(|attached| attached.name.clone())
            .collect()
    }

    /// Does the work of a frame with the sources of the session at the time `now` in
    /// seconds: sends the invocations queued since the last frame, receives from the
    /// sources and updates the invoker. Call it once a frame before the widgets, and
    /// [`Self::send_invocations`] after them to send what they queued without waiting
    /// for the next frame.
    pub fn update(&mut self, now: f64) {
        self.send_invocations();
        self.poll_sources(now);
        let connected = self.open_sources();
        self.update_invoker(connected, now);
    }

    /// Receives what the sources of the session have, and drops those that ended.
    fn poll_sources(&mut self, now: f64) {
        let mut sources = std::mem::take(&mut self.sources);
        let mut open = sources.iter().filter(|attached| attached.open).count();
        sources.retain_mut(|attached| {
            while let Some(event) = attached.source.poll() {
                match event {
                    Event::Opened => {
                        if open == 0 {
                            self.first_opened();
                        }
                        open += usize::from(!attached.open);
                        attached.open = true;
                    }
                    Event::Frame(frame) => self.receive_frame(&attached.name, &frame, now),
                    Event::Closed => {
                        let text = format!("{} closed the connection", attached.name);
                        self.notices.push((Level::Error, text));
                        open -= usize::from(attached.open);
                        attached.source.close();
                        return false;
                    }
                    Event::Failed(error) => {
                        let text = if attached.open {
                            format!("The connection to {} broke: {error}", attached.name)
                        } else {
                            format!("Cannot connect to {}: {error}", attached.name)
                        };
                        self.notices.push((Level::Error, text));
                        open -= usize::from(attached.open);
                        attached.source.close();
                        return false;
                    }
                }
            }
            true
        });
        // sources added while receiving, by none of the calls above, would be lost
        sources.append(&mut self.sources);
        self.sources = sources;
    }

    /// Tells the invoker which sources are `connected` and whether the session is
    /// recording, and lets it send what is due and give up on what was not answered at
    /// the time `now`.
    pub fn update_invoker(&mut self, connected: HashSet<String>, now: f64) {
        self.invoker.connected = connected;
        self.invoker.recording = self.recording;
        self.invoker.update(now);
    }

    /// Takes the requests the invoker queued, each with the name of the source it goes
    /// to, without those for actions their source no longer announces.
    pub fn take_requests(&mut self) -> Vec<(String, proto::Request)> {
        let mut requests = self.invoker.take_outbox();
        requests.retain(|(source, request)| {
            let proto::Request::Invoke(invocation) = request;
            let announced = (self.store.actions)
                .announced(source, &invocation.path)
                .is_some();
            if !announced {
                log::warn!(
                    "dropping an invocation of {} on {source:?}, which no longer announces it",
                    invocation.path
                );
            }
            announced
        });
        requests
    }

    /// Sends the requests the invoker queued to the sources of the session.
    pub fn send_invocations(&mut self) {
        for (name, request) in self.take_requests() {
            let source =
                (self.sources.iter_mut()).find(|attached| attached.open && attached.name == name);
            match source {
                Some(attached) => attached.source.send(&request),
                None => log::warn!("dropping a request to {name:?}, which is not connected"),
            }
        }
    }

    /// Applies the messages of `frame`, received from `source` at the time `now`. A
    /// frame holds one message or several as a CBOR sequence.
    pub fn receive_frame(&mut self, source: &str, frame: &[u8], now: f64) {
        for message in super::cbor::items(frame) {
            match message {
                Ok(message) => self.receive(source, message, now),
                Err(error) => log::warn!("error reading a message of {source}: {error}"),
            }
        }
    }

    /// Applies the one message `message`, received from `source` at the time `now`:
    /// records it while recording, and passes answers to the invoker.
    pub fn receive(&mut self, source: &str, message: &[u8], now: f64) {
        let result = if self.recording {
            self.store.receive(source, message)
        } else {
            self.store.receive_unrecorded(source, message)
        };
        if let Some(result) = result {
            self.invoker.receive(source, &result, now, &self.store);
        }
    }

    /// Tells that a source opened while no other was open, before any of its
    /// messages: starts afresh with no data, unless recording. Returns whether it did.
    pub fn first_opened(&mut self) -> bool {
        if self.recording {
            return false;
        }
        self.start_fresh(MemoryStore::default());
        true
    }

    /// Removes the received data, see [`MemoryStore::clear`]. The readbacks and their counts
    /// stay, so the invocations still tell which readback samples are newer.
    pub fn clear_data(&mut self) {
        self.store.clear();
    }

    /// Starts afresh with `store`, e.g. of an opened recording: drops all data, also the
    /// descriptions of the targets that [`Self::clear_data`] keeps, stops recording and
    /// forgets the invocations.
    pub fn start_fresh(&mut self, store: MemoryStore) {
        self.recording = false;
        self.invoker.reset();
        self.store = store;
    }

    /// Starts or stops recording. A recording starts only while `connected` and clears
    /// the data. Returns whether a recording started, e.g. to follow it live.
    pub fn set_recording(&mut self, recording: bool, connected: bool) -> bool {
        let starts = recording && !self.recording;
        if starts {
            if !connected {
                return false;
            }
            self.clear_data();
        }
        self.recording = recording;
        starts
    }

    /// Takes what to tell the user since the last call: what went wrong in the store
    /// and with the sources of the session.
    pub fn take_notices(&mut self) -> Vec<(Level, String)> {
        let mut notices = self.store.take_notices();
        notices.append(&mut self.notices);
        notices
    }
}

#[cfg(test)]
mod tests;
