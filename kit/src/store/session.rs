//! The session: the store with the invoker of actions, whether it is recording, and
//! the sources it receives from. It does what happens each frame apart from the GUI:
//! receive, apply, and send the invocations.

use super::actions::Invoker;
use super::protocol::Parser;
use super::query::Store;
use super::recording::Recorder;
use super::said::{self, Said};
use super::source::{Inbox, Source};
use super::{Ingest, Level, MemoryStore};
use std::collections::HashSet;

/// A source of a session with the name its messages are kept under.
struct Attached {
    name: String,
    source: Box<dyn Source>,
    /// Whether the source told that it is open.
    open: bool,
}

/// The store, the recorder of what arrives, the invoker and the sources, and the rules
/// between them: samples are kept only while recording, a recording starts only while
/// connected and clears the data, and the first source to open starts afresh. It
/// works with any store that takes input and answers queries, [`MemoryStore`] unless
/// the application brings its own.
pub struct Session<S = MemoryStore> {
    /// What the sources receive into and the views read from.
    pub store: S,
    /// What arrived while recording, to save it as a recording file.
    pub recorder: Recorder,
    /// Sends the invocations of actions to the sources.
    pub invoker: Invoker,
    /// Whether the samples received are kept, else they are dropped. See
    /// [`Self::set_recording`].
    pub recording: bool,
    sources: Vec<Attached>,
    /// What went wrong with the sources since [`Self::take_notices`].
    notices: Vec<(Level, String)>,
    /// What reads the messages, kept to read the next without allocating.
    parser: Parser,
}

/// What happened while the session polled a source, apart from the messages.
#[derive(Debug, Default, PartialEq)]
pub struct Polled {
    /// What the source told about itself, in order.
    pub events: Vec<SourceEvent>,
    /// How many messages the source handed over.
    pub messages: usize,
    /// Whether the source opened first and so started afresh, with no data.
    pub started_fresh: bool,
}

/// What a source told about itself while polled.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceEvent {
    /// The source is open.
    Opened,
    /// The source sent its hello.
    Hello(proto::Hello),
    /// The source ended.
    Closed,
    /// The source could not be opened or broke, with the reason.
    Failed(String),
}

/// What a store must do for the session to route messages to it: take input and answer
/// queries.
pub(crate) trait Target: Ingest + Store {}

impl<T: Ingest + Store> Target for T {}

/// The parts of a session that route a message, borrowed apart from its parser.
pub(crate) struct Router<'a> {
    pub(crate) store: &'a mut dyn Target,
    pub(crate) recorder: &'a mut Recorder,
    pub(crate) invoker: &'a mut Invoker,
    pub(crate) notices: &'a mut Vec<(Level, String)>,
    pub(crate) recording: bool,
}

impl Router<'_> {
    /// Routes what `source` said at the time `now`: to the recorder, then answers to
    /// the invoker and the rest to the store.
    pub(crate) fn said(&mut self, source: &str, now: f64, said: Said<'_>) {
        self.recorder.record(source, &said, self.recording);
        match said {
            Said::ActionResult(result) => {
                self.invoker.receive(source, result, now, &*self.store);
            }
            Said::Unreadable { name, error, .. } => {
                log::warn!("error parsing a message of {source}: {error}");
                if self.recording && name == Some("Samples") {
                    notify(self.notices, Level::Error, said::malformed_block(source));
                }
            }
            said => {
                if let Some(input) = said.input() {
                    self.store.ingest(source, input);
                }
            }
        }
    }

    /// Tells that a source opened while no other was open: starts afresh with no data,
    /// unless recording. Returns whether it did.
    pub(crate) fn first_opened(&mut self) -> bool {
        if self.recording {
            return false;
        }
        self.store.reset();
        *self.recorder = Recorder::default();
        self.invoker.reset();
        true
    }
}

/// Adds a notice, unless it is pending already.
pub(crate) fn notify(notices: &mut Vec<(Level, String)>, level: Level, text: String) {
    if !notices.iter().any(|(l, t)| *l == level && *t == text) {
        notices.push((level, text));
    }
}

impl Session<MemoryStore> {
    /// A session that records from the start, for an application without a record
    /// button.
    pub fn recording() -> Self {
        Self {
            recording: true,
            ..Self::default()
        }
    }
}

impl Default for Session<MemoryStore> {
    fn default() -> Self {
        Self::new(MemoryStore::default())
    }
}

impl<S: Ingest + Store> Session<S> {
    /// A session over `store`, not recording, with no sources.
    pub fn new(store: S) -> Self {
        Self {
            store,
            recorder: Recorder::default(),
            invoker: Invoker::default(),
            recording: false,
            sources: Vec::new(),
            notices: Vec::new(),
            parser: Parser::default(),
        }
    }

    /// The parts that route a message, and the parser that reads it.
    fn parts(&mut self) -> (Router<'_>, &mut Parser) {
        let router = Router {
            store: &mut self.store,
            recorder: &mut self.recorder,
            invoker: &mut self.invoker,
            notices: &mut self.notices,
            recording: self.recording,
        };
        (router, &mut self.parser)
    }

    /// Adds a notice for the user, unless it is pending already.
    #[cfg(feature = "workbench")]
    pub(crate) fn notify(&mut self, level: Level, text: String) {
        notify(&mut self.notices, level, text);
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

    /// The inbox of `source`, to hand over its messages at the time `now` in seconds
    /// without a [`Source`] to poll, e.g. from an application that has its data at
    /// hand. Opening does not start afresh and the events are dropped; a source that is
    /// polled by the session gets its inbox in [`Source::poll`].
    pub fn inbox<'a>(&'a mut self, source: &'a str, now: f64) -> Inbox<'a> {
        self.inbox_with(source, now, false)
    }

    fn inbox_with<'a>(&'a mut self, source: &'a str, now: f64, fresh: bool) -> Inbox<'a> {
        let (router, parser) = self.parts();
        Inbox::new(source, now, router, parser, fresh)
    }

    /// Polls `source`, named `name`, until it has nothing more or ends, and hands what it
    /// gave to the store through an inbox at the time `now`. The messages are counted and
    /// what the source told about itself is in the events; `fresh_when_opened` tells
    /// whether its opening starts afresh, i.e. no other source is open. The caller acts on
    /// the events, e.g. ends the source.
    pub fn poll_source(
        &mut self,
        name: &str,
        source: &mut dyn Source,
        now: f64,
        fresh_when_opened: bool,
    ) -> Polled {
        let mut inbox = self.inbox_with(name, now, fresh_when_opened);
        source.poll(&mut inbox);
        inbox.finish()
    }

    /// Receives what the sources of the session have, and drops those that ended.
    fn poll_sources(&mut self, now: f64) {
        let mut sources = std::mem::take(&mut self.sources);
        let mut open = sources.iter().filter(|attached| attached.open).count();
        sources.retain_mut(|attached| {
            let polled = self.poll_source(&attached.name, &mut *attached.source, now, open == 0);
            for event in polled.events {
                match event {
                    SourceEvent::Opened => {
                        open += usize::from(!attached.open);
                        attached.open = true;
                    }
                    SourceEvent::Hello(_) => {}
                    SourceEvent::Closed => {
                        let text = format!("{} closed the connection", attached.name);
                        self.notices.push((Level::Error, text));
                        open -= usize::from(attached.open);
                        attached.source.close();
                        return false;
                    }
                    SourceEvent::Failed(error) => {
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
            let announced = (self.store.actions())
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

    /// Tells that a source opened while no other was open, before any of its
    /// messages: starts afresh with no data, unless recording. Returns whether it did.
    pub fn first_opened(&mut self) -> bool {
        self.parts().0.first_opened()
    }

    /// Removes the received data, see [`MemoryStore::clear`], and starts a new recording
    /// with the descriptions of the targets. The readbacks and their counts stay, so the
    /// invocations still tell which readback samples are newer.
    pub fn clear_data(&mut self) {
        self.store.clear();
        self.recorder.restart();
    }

    /// Starts afresh with `store` and `recorder`, e.g. of an opened recording: drops all
    /// data, also the descriptions of the targets that [`Self::clear_data`] keeps, stops
    /// recording and forgets the invocations.
    pub fn start_fresh(&mut self, store: S, recorder: Recorder) {
        self.recording = false;
        self.invoker.reset();
        self.store = store;
        self.recorder = recorder;
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
