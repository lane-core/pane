//! Chan<S, T>: session-typed channel over a Transport T.
//!
//! Mirrors par's session type structure but uses pane-session's
//! Transport trait for IPC.

use std::marker::PhantomData;
use serde::{Serialize, de::DeserializeOwned};
use crate::transport::Transport;

/// Session-typed channel over Transport T.
///
/// S is the current session state (a par session type).
/// T is the concrete transport.
/// Panics on disconnect (par's CLL model).
#[must_use = "dropping a channel may leave the peer waiting"]
pub struct Chan<S, T: Transport> {
    transport: T,
    _session: PhantomData<S>,
}

impl<S, T: Transport> Chan<S, T> {
    pub fn new(transport: T) -> Self {
        Chan { transport, _session: PhantomData }
    }

    fn advance<S2>(self) -> Chan<S2, T> {
        Chan { transport: self.transport, _session: PhantomData }
    }

    /// Reclaim the transport (handshake → active phase transition).
    pub fn into_transport(self) -> T {
        self.transport
    }
}

// --- Send ---

impl<A, S, T> Chan<par::exchange::Send<A, S>, T>
where
    A: Serialize + Send + 'static,
    S: par::Session,
    T: Transport,
{
    /// Send a value and advance. Panics on disconnect.
    pub fn send(mut self, value: A) -> Chan<S, T> {
        let bytes = postcard::to_allocvec(&value)
            .expect("session send: serialization failed");
        self.transport.send_raw(&bytes);
        self.advance()
    }
}

impl<A, T> Chan<par::exchange::Send<A, ()>, T>
where
    A: Serialize + Send + 'static,
    T: Transport,
{
    /// Terminal send — send and end session.
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
    /// Receive a value and advance. Panics on disconnect.
    pub fn recv(mut self) -> (A, Chan<S, T>) {
        let bytes = self.transport.recv_raw();
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
    /// Terminal recv — receive and end session.
    pub fn recv1(self) -> A {
        self.recv().0
    }
}

// --- Session end ---

impl<T: Transport> Chan<(), T> {
    pub fn close(self) {
        drop(self);
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

        let (response, _) = client.recv();
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
}
