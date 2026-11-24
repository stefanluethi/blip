//! Sources: where the messages of the targets come from and their requests go to, e.g.
//! a WebSocket to an outpost, a serial port read in the application or a simulation.

/// What a [`Source`] tells since it was last asked.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Event {
    /// The source is open: it delivers frames and takes requests from now on.
    Opened,
    /// A frame: one message as CBOR, or several one after another as a CBOR sequence
    /// (RFC 8742), e.g. a batch of a target.
    Frame(Vec<u8>),
    /// The source ended. It is not asked again.
    Closed,
    /// The source could not be opened or broke, with the reason. It is not asked again.
    Failed(String),
}

impl Event {
    /// The frame of `message`, for a source that makes its messages itself.
    pub fn message(message: &proto::Message) -> Self {
        Self::Frame(serde_cbor_2::to_vec(message).expect("messages encode"))
    }
}

/// A source of messages. A [`Session`](super::session::Session) asks its sources each
/// frame, so none of the calls may block.
pub trait Source {
    /// The next event since the last call, `None` when there is none for now.
    fn poll(&mut self) -> Option<Event>;

    /// Sends `request` to the target, e.g. the invocation of an action. A source
    /// without a way back drops it.
    fn send(&mut self, request: &proto::Request);

    /// Ends the source, called before it is dropped.
    fn close(&mut self) {}
}
