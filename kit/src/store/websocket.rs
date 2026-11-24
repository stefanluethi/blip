//! The WebSocket source: a connection to an outpost or a target that speaks the blip
//! protocol, natively and in the browser.

use super::source::{Event, Source};

/// A WebSocket connection as a [`Source`]. It is being opened until it tells
/// [`Event::Opened`].
pub struct WebSocket {
    sender: ewebsock::WsSender,
    receiver: ewebsock::WsReceiver,
}

impl WebSocket {
    /// Connects to `address`, e.g. `ws://127.0.0.1:3000/ws/outpost`. `wake_up` is called
    /// from another thread whenever an event arrived, e.g. to repaint.
    pub fn connect(
        address: &str,
        wake_up: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let options = ewebsock::Options::default();
        let (sender, receiver) = ewebsock::connect_with_wakeup(address, options, wake_up)?;
        Ok(Self { sender, receiver })
    }
}

impl Source for WebSocket {
    fn poll(&mut self) -> Option<Event> {
        loop {
            return Some(match self.receiver.try_recv()? {
                ewebsock::WsEvent::Opened => Event::Opened,
                ewebsock::WsEvent::Message(ewebsock::WsMessage::Binary(frame)) => {
                    Event::Frame(frame)
                }
                ewebsock::WsEvent::Message(ewebsock::WsMessage::Text(text)) => {
                    log::info!("{text}");
                    continue;
                }
                ewebsock::WsEvent::Message(_) => {
                    log::warn!("ws unknown message received");
                    continue;
                }
                ewebsock::WsEvent::Error(error) => Event::Failed(error),
                ewebsock::WsEvent::Closed => Event::Closed,
            });
        }
    }

    fn send(&mut self, request: &proto::Request) {
        let packet = serde_cbor_2::to_vec(request).expect("requests encode");
        self.sender.send(ewebsock::WsMessage::Binary(packet));
    }

    fn close(&mut self) {
        self.sender.close();
    }
}
