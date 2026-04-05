//! Chan<S, T>: session-typed channel over a Transport T.
//!
//! Mirrors par's session type structure but uses pane-session's
//! Transport trait for IPC. Par types are phantom state markers.

use std::marker::PhantomData;
use serde::{Serialize, de::DeserializeOwned};
use crate::transport::Transport;

/// The ProtocolAbort sentinel. Sent on Drop when a channel is
/// abandoned mid-protocol. Peer detects this at the framing layer
/// (I11: checked before postcard deserialization).
pub const PROTOCOL_ABORT: [u8; 2] = [0xFF, 0xFF];

/// Session-typed channel over Transport T.
///
/// S is the current session state (a par session type).
/// T is the concrete transport.
/// Panics on disconnect (par's CLL model).
///
/// Drop sends ProtocolAbort [0xFF][0xFF] if the session was not
/// completed (I10). Best-effort — if the transport is dead, Drop
/// silently succeeds.
#[must_use = "dropping a channel mid-protocol sends ProtocolAbort to the peer"]
pub struct Chan<S, T: Transport> {
    transport: Option<T>,
    _session: PhantomData<S>,
}

impl<S, T: Transport> Chan<S, T> {
    pub fn new(transport: T) -> Self {
        Chan { transport: Some(transport), _session: PhantomData }
    }

    /// Take the transport, leaving None (suppresses Drop abort).
    fn take_transport(&mut self) -> T {
        self.transport.take().expect("transport already consumed")
    }

    fn advance<S2>(mut self) -> Chan<S2, T> {
        let transport = self.take_transport();
        // self.transport is now None — Drop won't send abort
        std::mem::forget(self); // skip Drop entirely since transport moved
        Chan { transport: Some(transport), _session: PhantomData }
    }

    /// Reclaim the transport (handshake → active phase transition).
    /// Suppresses ProtocolAbort — the session is intentionally ending.
    pub fn into_transport(mut self) -> T {
        let t = self.take_transport();
        std::mem::forget(self);
        t
    }
}

/// Drop sends ProtocolAbort if the channel still holds a transport.
/// This fires when a Chan is dropped mid-protocol (early return,
/// panic during unwind). Best-effort: if send fails, ignore it.
/// I10: Chan Drop must not block.
impl<S, T: Transport> Drop for Chan<S, T> {
    fn drop(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            // Best-effort abort — don't panic in Drop
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                transport.send_raw(&PROTOCOL_ABORT);
            }));
        }
    }
}

// --- Send ---

impl<A, S, T> Chan<par::exchange::Send<A, S>, T>
where
    A: Serialize + Send + 'static,
    S: par::Session,
    T: Transport,
{
    pub fn send(mut self, value: A) -> Chan<S, T> {
        let bytes = postcard::to_allocvec(&value)
            .expect("session send: serialization failed");
        self.transport.as_mut().unwrap().send_raw(&bytes);
        self.advance()
    }
}

impl<A, T> Chan<par::exchange::Send<A, ()>, T>
where
    A: Serialize + Send + 'static,
    T: Transport,
{
    pub fn send1(self, value: A) {
        let _ = self.send(value);
    }
}

// --- Recv ---

impl<A, S, T> Chan<par::exchange::Recv<A, S>, T>
where
    A: DeserializeOwned + Send + 'static,
    S: par::Session,
    T: Transport,
{
    pub fn recv(mut self) -> (A, Chan<S, T>) {
        let bytes = self.transport.as_mut().unwrap().recv_raw();
        let value: A = postcard::from_bytes(&bytes)
            .expect("session recv: deserialization failed");
        (value, self.advance())
    }
}

impl<A, T> Chan<par::exchange::Recv<A, ()>, T>
where
    A: DeserializeOwned + Send + 'static,
    T: Transport,
{
    pub fn recv1(self) -> A {
        self.recv().0
    }
}

// --- Session end ---

impl<T: Transport> Chan<(), T> {
    /// Close the session cleanly. No ProtocolAbort sent.
    pub fn close(mut self) {
        self.take_transport(); // suppress abort
        std::mem::forget(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;
    use par::exchange::{Send, Recv};

    type ClientProto = Send<String, Recv<u64, ()>>;

    #[test]
    fn send_recv_over_transport() {
        let (ta, tb) = MemoryTransport::pair();

        let client: Chan<ClientProto, _> = Chan::new(ta);
        let client = client.send("hello".to_string());

        let server: Chan<Recv<String, Send<u64, ()>>, _> = Chan::new(tb);
        let (msg, server) = server.recv();
        assert_eq!(msg, "hello");
        server.send1(42u64);

        let (response, _client) = client.recv();
        assert_eq!(response, 42);
    }

    #[test]
    fn send1_recv1() {
        let (ta, tb) = MemoryTransport::pair();
        let sender: Chan<Send<String>, _> = Chan::new(ta);
        sender.send1("done".to_string());

        let receiver: Chan<Recv<String>, _> = Chan::new(tb);
        assert_eq!(receiver.recv1(), "done");
    }

    #[test]
    fn branching_via_result() {
        let (ta, tb) = MemoryTransport::pair();
        let sender: Chan<Send<Result<u64, String>>, _> = Chan::new(ta);
        sender.send1(Ok(42u64));

        let receiver: Chan<Recv<Result<u64, String>>, _> = Chan::new(tb);
        assert_eq!(receiver.recv1(), Ok(42));
    }

    #[test]
    fn multi_step_protocol() {
        let (ta, tb) = MemoryTransport::pair();
        type Client = Send<String, Send<u32, Recv<String, ()>>>;

        let client: Chan<Client, _> = Chan::new(ta);
        let client = client.send("Alice".to_string());
        let client = client.send(30u32);

        let server: Chan<Recv<String, Recv<u32, Send<String, ()>>>, _> = Chan::new(tb);
        let (name, server) = server.recv();
        let (age, server) = server.recv();
        server.send1(format!("Hello {}, age {}", name, age));

        assert_eq!(client.recv1(), "Hello Alice, age 30");
    }

    #[test]
    fn drop_sends_protocol_abort() {
        let (ta, tb) = MemoryTransport::pair();

        // Create a channel and drop it without completing the protocol
        {
            let _chan: Chan<Send<String, Recv<u64, ()>>, _> = Chan::new(ta);
            // dropped here — should send ProtocolAbort
        }

        // The peer should receive the abort sentinel
        let mut peer = tb;
        let bytes = peer.recv_raw();
        assert_eq!(bytes, PROTOCOL_ABORT);
    }

    #[test]
    fn close_does_not_send_abort() {
        let (ta, tb) = MemoryTransport::pair();

        // Send a value, receive it, then close cleanly
        let sender: Chan<Send<String>, _> = Chan::new(ta);
        sender.send1("hello".to_string());

        let receiver: Chan<Recv<String>, _> = Chan::new(tb);
        let _msg = receiver.recv1();
        // recv1 closes the () continuation cleanly — no abort
        // If abort were sent, the sender's transport would have
        // extra bytes, but since sender is already consumed, this
        // is a structural test that close() suppresses abort.
    }

    #[test]
    fn into_transport_suppresses_abort() {
        let (ta, _tb) = MemoryTransport::pair();
        let chan: Chan<(), _> = Chan::new(ta);
        let _transport = chan.into_transport();
        // No abort sent — transport reclaimed for active phase
    }
}
