//! Sources: where the messages of the targets come from and their requests go to, e.g.
//! a WebSocket to an outpost, a serial port read in the application or a simulation.

use super::ingest::Times;
use super::protocol::Parser;
use super::said::Said;
use super::session::{Polled, Router, SourceEvent};
use super::value::AsValue;

/// A source of messages. A [`Session`](super::session::Session) asks its sources each
/// frame, so none of the calls may block.
pub trait Source {
    /// Hands what arrived since the last call to `inbox`, in order. Must not block.
    /// Tell [`Inbox::opened`] once the source is open, and [`Inbox::closed`] or
    /// [`Inbox::failed`] when it ends: it is not polled again after that.
    fn poll(&mut self, inbox: &mut Inbox<'_>);

    /// Sends `request` to the target, e.g. the invocation of an action. A source
    /// without a way back drops it.
    fn send(&mut self, request: &proto::Request);

    /// Ends the source, called before it is dropped.
    fn close(&mut self) {}
}

/// What a source hands its messages to: made by the session for one source, for one
/// poll. Hand over what the source has with the call that fits its data, the session
/// does the rest: keeps it while recording, keeps the latest value only otherwise, and
/// answers invocations.
///
/// A source whose data never was CBOR hands it over typed, without encoding it and
/// without building protocol messages. A source of bytes in the protocol hands them to
/// [`Self::frame`]. After [`Self::closed`] or [`Self::failed`] the inbox ignores every
/// call.
///
/// ```
/// use blip_kit::store::Times;
/// use blip_kit::store::session::Session;
///
/// let mut session = Session::recording();
/// let mut inbox = session.inbox("sim", 0.0);
/// inbox.opened();
/// inbox.sample(1_000, "motor.speed", 3.2);
/// inbox.samples("motor.current", Times::Period { timestamp: 2_000, period: 1_000 }, [0.5, 0.6]);
/// inbox.log(3_000, "motor", 2, "started");
/// ```
pub struct Inbox<'a> {
    source: &'a str,
    now: f64,
    router: Router<'a>,
    parser: &'a mut Parser,
    events: Vec<SourceEvent>,
    messages: usize,
    fresh_when_opened: bool,
    started_fresh: bool,
    ended: bool,
}

impl<'a> Inbox<'a> {
    pub(crate) fn new(
        source: &'a str,
        now: f64,
        router: Router<'a>,
        parser: &'a mut Parser,
        fresh_when_opened: bool,
    ) -> Self {
        Self {
            source,
            now,
            router,
            parser,
            events: Vec::new(),
            messages: 0,
            fresh_when_opened,
            started_fresh: false,
            ended: false,
        }
    }

    /// What happened during the poll, for the session.
    pub(crate) fn finish(self) -> Polled {
        Polled {
            events: self.events,
            messages: self.messages,
            started_fresh: self.started_fresh,
        }
    }

    /// The parser, whether it reads in full (recording) and what gives what it says to
    /// the router, counting a message if `count`; `None` once the source ended.
    #[inline]
    fn reading(&mut self, count: bool) -> Option<(&mut Parser, bool, impl FnMut(Said<'_>))> {
        if self.ended {
            return None;
        }
        self.messages += usize::from(count);
        let full = self.router.recording;
        let Self {
            source,
            now,
            router,
            parser,
            events,
            ..
        } = self;
        let (source, now) = (*source, *now);
        // a hello is also an event of the source
        let say = move |said: Said<'_>| {
            if let Said::Hello(hello) = said {
                events.push(SourceEvent::Hello(hello.clone()));
            }
            router.said(source, now, said);
        };
        Some((&mut **parser, full, say))
    }

    /// Tells that the source is open: it delivers messages and takes requests from now
    /// on. Call it before the first message.
    pub fn opened(&mut self) {
        if self.ended {
            return;
        }
        self.events.push(SourceEvent::Opened);
        if self.fresh_when_opened && !self.started_fresh {
            self.started_fresh = self.router.first_opened();
        }
    }

    /// Tells that the source ended. The session drops it and tells the user.
    pub fn closed(&mut self) {
        if !self.ended {
            self.events.push(SourceEvent::Closed);
            self.ended = true;
        }
    }

    /// Tells that the source could not be opened or broke, with the reason the user
    /// reads. The session drops the source.
    pub fn failed(&mut self, reason: impl Into<String>) {
        if !self.ended {
            self.events.push(SourceEvent::Failed(reason.into()));
            self.ended = true;
        }
    }

    /// Hands over bytes in the blip protocol: one message as CBOR, or several one after
    /// another as a CBOR sequence (RFC 8742). A message that cannot be read is dropped
    /// and logged. The source holds no parser.
    pub fn frame(&mut self, frame: &[u8]) {
        let source = self.source;
        let read = match self.reading(false) {
            Some((parser, full, say)) => parser.frame(source, frame, full, say),
            None => return,
        };
        self.messages += read;
    }

    /// Hands over the value of `path` at `timestamp`, in nanoseconds. `value` is
    /// anything that can be a value: a number, a bool, text, an array, a record.
    pub fn sample(&mut self, timestamp: u64, path: &str, value: impl AsValue) {
        if let Some((parser, full, say)) = self.reading(true) {
            parser.sample(timestamp, path, value, full, say);
        }
    }

    /// Hands over a block of values of `path`, taken at `times`. A value without a
    /// timestamp, or a timestamp without a value, is dropped with a warning. A block
    /// ends with its timestamps for `Times::Each` and with its values for
    /// `Times::Period`, so an endless iterator needs `Times::Each`.
    pub fn samples<V: AsValue>(
        &mut self,
        path: &str,
        times: Times<'_>,
        values: impl IntoIterator<Item = V>,
    ) {
        if let Some((parser, full, say)) = self.reading(true) {
            parser.samples(path, times, values, full, say);
        }
    }

    /// Hands over a log record of `path`, empty for the target as a whole, with a
    /// `level` of [`proto::level`]. Logs are kept only while recording.
    pub fn log(&mut self, timestamp: u64, path: &str, level: u8, message: &str) {
        if let Some((parser, full, say)) = self.reading(true) {
            parser.log(timestamp, path, level, message, full, say);
        }
    }

    /// Hands over what a trace is, e.g. its unit and expected range.
    pub fn metadata(&mut self, metadata: &proto::TraceMetadata) {
        if let Some((_, _, mut say)) = self.reading(true) {
            say(Said::Metadata(metadata));
        }
    }

    /// Hands over all actions the target can carry out, replacing those announced
    /// before.
    pub fn actions(&mut self, actions: &[proto::ActionDescriptor]) {
        if let Some((_, _, mut say)) = self.reading(true) {
            say(Said::Actions(actions));
        }
    }

    /// Hands over what the target and its connection are, first on a connection.
    pub fn hello(&mut self, hello: &proto::Hello) {
        if let Some((_, _, mut say)) = self.reading(true) {
            say(Said::Hello(hello));
        }
    }

    /// Hands over the answer of the target to an invocation.
    pub fn action_result(&mut self, result: &proto::ActionResult) {
        if let Some((_, _, mut say)) = self.reading(true) {
            say(Said::ActionResult(result));
        }
    }

    /// Hands over a protocol message built by the source.
    pub fn message(&mut self, message: &proto::Message) {
        if let Some((parser, full, say)) = self.reading(true) {
            parser.message(message, full, say);
        }
    }
}
