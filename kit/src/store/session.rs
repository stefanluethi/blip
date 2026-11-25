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
    id: SourceId,
    name: String,
    /// What the notices call the source, the name until [`Session::set_label`].
    label: String,
    /// What to check when the source cannot be opened, told with that notice.
    hint: Option<&'static str>,
    source: Box<dyn Source>,
    /// Whether the source told that it is open.
    open: bool,
    /// When the source last announced its actions, by the session's count of
    /// announcements; `None` if it did not.
    announced: Option<u64>,
}

/// Which source of a session it is, given by [`Session::add_source`]: to remove the source,
/// label it, and tell its events apart from those of another source, also one of the same
/// name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceId(u64);

/// What the sources handed over in an update of the session, see [`Session::update`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    /// How many messages the sources handed over, e.g. to show a rate.
    pub messages: usize,
    /// Whether a source opened while none was open, so the session started afresh with no
    /// data (unless recording or the data is an unsaved recording), e.g. to reset what
    /// shows the old data.
    pub started_fresh: bool,
}

/// The store, the recorder of what arrives, the invoker and the sources, and the rules
/// between them: samples are kept only while recording, a recording starts only while
/// connected and clears the data, and the first source to open starts afresh, unless
/// that drops an unsaved recording. It works with any store that takes input and answers
/// queries, [`MemoryStore`] unless the application brings its own.
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
    /// Whether samples were recorded since the data was last saved, cleared or
    /// replaced. See [`Self::is_unsaved`].
    changed: bool,
    /// What went wrong with the sources since [`Self::take_notices`].
    notices: Vec<(Level, String)>,
    /// The last id given to a source.
    next_id: u64,
    /// How many times a source announced its actions.
    announcements: u64,
    /// What the sources told about themselves in the last update.
    events: Vec<(SourceId, SourceEvent)>,
    /// What reads the messages, kept to read the next without allocating.
    parser: Parser,
}

/// What happened while the session polled a source, apart from the messages.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Polled {
    /// What the source told about itself, in order.
    pub(crate) events: Vec<SourceEvent>,
    /// How many messages the source handed over.
    pub(crate) messages: usize,
    /// Whether the source opened first and so started afresh, with no data.
    pub(crate) started_fresh: bool,
    /// Whether the source announced its actions.
    pub(crate) announced: bool,
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
    /// Whether something was recorded since the data was last saved, cleared or
    /// replaced, see [`Session::is_unsaved`].
    pub(crate) changed: &'a mut bool,
}

impl Router<'_> {
    /// Routes what `source` said at the time `now`: to the recorder, then answers to
    /// the invoker and the rest to the store.
    pub(crate) fn said(&mut self, source: &str, now: f64, said: Said<'_>) {
        *self.changed |= self.recording;
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
    /// unless recording or the data is an unsaved recording, which stays until the user
    /// drops it. Returns whether it did.
    pub(crate) fn first_opened(&mut self) -> bool {
        if self.recording || is_unsaved(*self.changed, &*self.store) {
            return false;
        }
        self.store.reset();
        *self.recorder = Recorder::default();
        self.invoker.reset();
        *self.changed = false;
        true
    }
}

/// Whether `store` holds samples and something was recorded since it was last saved,
/// cleared or replaced (`changed`).
fn is_unsaved(changed: bool, store: &dyn Store) -> bool {
    changed && store.time_span().is_some()
}

/// The notice of the source `label` that could not be opened with `error`, with what to
/// check, `hint`, if there is one.
pub(crate) fn cannot_connect(label: &str, error: &str, hint: Option<&str>) -> String {
    match hint {
        Some(hint) => {
            let error = error.trim_end_matches('.');
            format!("Cannot connect to {label}: {error}. {hint}")
        }
        None => format!("Cannot connect to {label}: {error}"),
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
            changed: false,
            notices: Vec::new(),
            next_id: 0,
            announcements: 0,
            events: Vec::new(),
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
            changed: &mut self.changed,
        };
        (router, &mut self.parser)
    }

    /// Adds a notice for the user, unless it is pending already.
    #[cfg(feature = "workbench")]
    pub(crate) fn notify(&mut self, level: Level, text: String) {
        notify(&mut self.notices, level, text);
    }

    /// Adds `source`, whose messages are kept under `name` and whose actions are invoked by
    /// that name. Sources may share a name, e.g. two connections without one; the id tells
    /// them apart. Its notices call it by `name` until [`Self::set_label`].
    pub fn add_source(&mut self, name: &str, source: impl Source + 'static) -> SourceId {
        self.next_id += 1;
        let id = SourceId(self.next_id);
        self.sources.push(Attached {
            id,
            name: name.to_owned(),
            label: name.to_owned(),
            hint: None,
            source: Box::new(source),
            open: false,
            announced: None,
        });
        id
    }

    /// Closes and removes the source `id`, if the session holds it. Nothing is told about it:
    /// no event, no notice.
    pub fn remove_source(&mut self, id: SourceId) {
        if let Some(index) = self.sources.iter().position(|attached| attached.id == id) {
            self.sources.remove(index).source.close();
        }
    }

    /// Calls the source `id` `label` in the notices to the user, e.g. the address of a
    /// connection: "Cannot connect to {label}: ...", "The connection to {label} broke: ...",
    /// "{label} closed the connection".
    pub fn set_label(&mut self, id: SourceId, label: &str) {
        if let Some(attached) = self.sources.iter_mut().find(|attached| attached.id == id) {
            label.clone_into(&mut attached.label);
        }
    }

    /// Tells the user `hint` when the source `id` cannot be opened, what to check, e.g.
    /// whether the outpost runs.
    #[cfg(feature = "workbench")]
    pub(crate) fn set_hint(&mut self, id: SourceId, hint: &'static str) {
        if let Some(attached) = self.sources.iter_mut().find(|attached| attached.id == id) {
            attached.hint = Some(hint);
        }
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
    /// for the next frame. Returns how many messages arrived and whether the session
    /// started afresh; what the sources told about themselves is in
    /// [`Self::source_events`].
    pub fn update(&mut self, now: f64) -> Update {
        self.send_invocations();
        let update = self.poll_sources(now);
        let connected = self.open_sources();
        self.update_invoker(connected, now);
        update
    }

    /// What the sources told about themselves in the last [`Self::update`], in order, each
    /// with its id: opened, their `Hello`, closed or failed. A source that closed or failed
    /// is no longer in the session. Empty before the first update; [`Self::inbox`] adds
    /// nothing.
    pub fn source_events(&self) -> &[(SourceId, SourceEvent)] {
        &self.events
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
    /// whether its opening starts afresh, i.e. no other source is open.
    pub(crate) fn poll_source(
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
    fn poll_sources(&mut self, now: f64) -> Update {
        self.events.clear();
        let mut update = Update::default();
        let mut sources = std::mem::take(&mut self.sources);
        let mut open = sources.iter().filter(|attached| attached.open).count();
        sources.retain_mut(|attached| {
            let polled = self.poll_source(&attached.name, &mut *attached.source, now, open == 0);
            update.messages += polled.messages;
            update.started_fresh |= polled.started_fresh;
            if polled.announced {
                self.announcements += 1;
                attached.announced = Some(self.announcements);
            }
            let mut keep = true;
            for event in polled.events {
                match &event {
                    SourceEvent::Opened => {
                        open += usize::from(!attached.open);
                        attached.open = true;
                    }
                    SourceEvent::Hello(_) => {}
                    SourceEvent::Closed => {
                        let text = format!("{} closed the connection", attached.label);
                        self.notices.push((Level::Error, text));
                        open -= usize::from(attached.open);
                        attached.source.close();
                        keep = false;
                    }
                    SourceEvent::Failed(error) => {
                        let text = if attached.open {
                            format!("The connection to {} broke: {error}", attached.label)
                        } else {
                            cannot_connect(&attached.label, error, attached.hint)
                        };
                        self.notices.push((Level::Error, text));
                        open -= usize::from(attached.open);
                        attached.source.close();
                        keep = false;
                    }
                }
                self.events.push((attached.id, event));
                if !keep {
                    break;
                }
            }
            keep
        });
        // sources added while receiving, by none of the calls above, would be lost
        sources.append(&mut self.sources);
        self.sources = sources;
        update
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

    /// The index of the source a request to `name` goes to: of the open sources of that
    /// name, the one that announced its actions last, the actions the store holds; else the
    /// first of them.
    fn addressee(&self, name: &str) -> Option<usize> {
        let named = |attached: &&Attached| attached.open && attached.name == name;
        let last = (self.sources.iter().enumerate())
            .filter(|(_, attached)| named(attached))
            .filter_map(|(index, attached)| Some((attached.announced?, index)))
            .max()
            .map(|(_, index)| index);
        last.or_else(|| self.sources.iter().position(|attached| named(&attached)))
    }

    /// Sends the requests the invoker queued to the sources of the session.
    pub fn send_invocations(&mut self) {
        for (name, request) in self.take_requests() {
            match self.addressee(&name) {
                Some(index) => self.sources[index].source.send(&request),
                None => log::warn!("dropping a request to {name:?}, which is not connected"),
            }
        }
    }

    /// Tells that a source opened while no other was open, before any of its
    /// messages: starts afresh with no data, unless recording or the data is an
    /// unsaved recording, which stays until the user drops it. Returns whether it did.
    pub fn first_opened(&mut self) -> bool {
        self.parts().0.first_opened()
    }

    /// Removes the received data, see [`MemoryStore::clear`], and starts a new recording
    /// with the descriptions of the targets. The readbacks and their counts stay, so the
    /// invocations still tell which readback samples are newer.
    pub fn clear_data(&mut self) {
        self.store.clear();
        self.changed = false;
        self.recorder.restart();
    }

    /// Starts afresh with `store` and `recorder`, e.g. of an opened recording: drops all
    /// data, also the descriptions of the targets that [`Self::clear_data`] keeps, stops
    /// recording and forgets the invocations.
    pub fn start_fresh(&mut self, store: S, recorder: Recorder) {
        self.recording = false;
        self.changed = false;
        self.invoker.reset();
        self.store = store;
        self.recorder = recorder;
    }

    /// Whether the data holds samples recorded since it was last saved, cleared or
    /// replaced, e.g. by an opened recording. An application asks before it drops
    /// them, e.g. to record anew or to quit.
    pub fn is_unsaved(&self) -> bool {
        is_unsaved(self.changed, &self.store)
    }

    /// Tells that the data was saved as it is now, so it is no longer unsaved until
    /// more is recorded.
    pub fn mark_saved(&mut self) {
        self.changed = false;
    }

    /// Whether samples arrive but are dropped as nothing records them: a source is
    /// connected, the session is not recording and has no samples to show. An
    /// application with a record button tells the user to press it.
    pub fn waiting_for_record(&self) -> bool {
        !self.recording && !self.invoker.connected.is_empty() && self.store.time_span().is_none()
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
