use std::marker::PhantomData;

use serde::{Serialize, de::DeserializeOwned};

use crate::error::SessionError;
use crate::transport::Transport;

/// A session type representing "send a value of type A, then continue as S."
///
/// In linear logic: A ⊗ S (tensor — output A, continue as S).
pub struct Send<A, S>(PhantomData<(A, S)>);

/// A session type representing "receive a value of type A, then continue as S."
///
/// In linear logic: A ⅋ S (par — input A, continue as S).
pub struct Recv<A, S>(PhantomData<(A, S)>);

/// A session type representing "session terminated."
///
/// In linear logic: 1 (unit — close).
pub struct End;

/// A session-typed channel. `S` is the current session state (what operations
/// are valid). `T` is the transport (how messages are physically sent/received).
///
/// The typestate pattern: `Chan<Send<A, S>, T>` offers only `send()`, which
/// returns `Chan<S, T>`. `Chan<Recv<A, S>, T>` offers only `recv()`, which
/// returns `(A, Chan<S, T>)`. Invalid operations don't compile.
///
/// Crash-safe: all operations return `Result<_, SessionError>`. A dropped
/// or crashed peer produces `Err(SessionError::Disconnected)`, not a panic.
#[must_use = "session channels must be used — dropping a channel may leave the peer waiting"]
pub struct Chan<S, T: Transport> {
    transport: T,
    _session: PhantomData<S>,
}

impl<S, T: Transport> std::fmt::Debug for Chan<S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chan")
            .field("session", &std::any::type_name::<S>())
            .finish()
    }
}

impl<S, T: Transport> Chan<S, T> {
    /// Create a new channel with the given transport.
    /// Internal — users create channels via `Transport::connect()` or `Transport::pair()`.
    pub(crate) fn new(transport: T) -> Self {
        Chan {
            transport,
            _session: PhantomData,
        }
    }

    /// Transition to a new session state (internal).
    fn advance<S2>(self) -> Chan<S2, T> {
        Chan {
            transport: self.transport,
            _session: PhantomData,
        }
    }
}

impl<A, S, T> Chan<Send<A, S>, T>
where
    A: Serialize,
    T: Transport,
{
    /// Send a value and advance to the continuation session.
    ///
    /// Returns `Err(SessionError::Disconnected)` if the peer is gone.
    pub fn send(mut self, value: A) -> Result<Chan<S, T>, SessionError> {
        self.transport.send_raw(&postcard::to_allocvec(&value)?)?;
        Ok(self.advance())
    }
}

impl<A, S, T> Chan<Recv<A, S>, T>
where
    A: DeserializeOwned,
    T: Transport,
{
    /// Receive a value and advance to the continuation session.
    ///
    /// Returns `Err(SessionError::Disconnected)` if the peer is gone.
    /// This is the critical property: no panic, just an error.
    pub fn recv(mut self) -> Result<(A, Chan<S, T>), SessionError> {
        let bytes = self.transport.recv_raw()?;
        let value: A = postcard::from_bytes(&bytes)?;
        Ok((value, self.advance()))
    }
}

impl<T: Transport> Chan<End, T> {
    /// Close the session. Consumes the channel.
    pub fn close(self) {
        // Transport is dropped, closing the underlying connection.
        // No message sent — End is a type-level marker, not a wire message.
        drop(self);
    }
}
