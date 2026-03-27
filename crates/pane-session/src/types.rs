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

/// A session type representing "this endpoint selects one of two continuations."
///
/// In linear logic: A ⊕ B (plus — internal choice, the selector decides).
pub struct Select<L, R>(PhantomData<(L, R)>);

/// A session type representing "this endpoint receives the peer's selection."
///
/// In linear logic: A & B (with — external choice, the offerer handles both).
pub struct Branch<L, R>(PhantomData<(L, R)>);

/// The result of receiving a branch selection from the peer.
pub enum Offer<L, R> {
    Left(L),
    Right(R),
}

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
    /// Create a new channel with the given transport and initial session type.
    ///
    /// The caller is responsible for ensuring the session type matches
    /// what the peer expects. For in-memory testing, use `memory::pair()`
    /// which creates correctly-typed pairs automatically.
    pub fn new(transport: T) -> Self {
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

// --- Branching (Select/Branch) ---

impl<L, R, T: Transport> Chan<Select<L, R>, T> {
    /// Select the left branch and advance to continuation L.
    /// Sends a 0x00 tag byte to the peer.
    pub fn select_left(mut self) -> Result<Chan<L, T>, SessionError> {
        self.transport.send_raw(&[0x00])?;
        Ok(self.advance())
    }

    /// Select the right branch and advance to continuation R.
    /// Sends a 0x01 tag byte to the peer.
    pub fn select_right(mut self) -> Result<Chan<R, T>, SessionError> {
        self.transport.send_raw(&[0x01])?;
        Ok(self.advance())
    }
}

impl<L, R, T: Transport> Chan<Branch<L, R>, T> {
    /// Receive the peer's branch selection.
    /// Returns `Offer::Left(chan)` or `Offer::Right(chan)` depending on
    /// the peer's choice. The caller must handle both cases — Rust's
    /// exhaustive match enforces this.
    pub fn offer(mut self) -> Result<Offer<Chan<L, T>, Chan<R, T>>, SessionError> {
        let tag = self.transport.recv_raw()?;
        match tag.as_slice() {
            [0x00] => Ok(Offer::Left(self.advance())),
            [0x01] => Ok(Offer::Right(self.advance())),
            _ => Err(SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid branch tag: expected 0x00 or 0x01, got {:?}", tag),
            ))),
        }
    }
}

// --- Combinators ---

impl<A, B, S, T> Chan<Send<A, Recv<B, S>>, T>
where
    A: Serialize,
    B: DeserializeOwned,
    T: Transport,
{
    /// Send a value, receive a response, and advance — the request-response
    /// pattern in one call. This is `BMessenger::SendMessage(&msg, &reply)`
    /// translated to session types.
    ///
    /// ```text
    /// // Before: 4 lines, 2 rebindings
    /// let chan = chan.send(request)?;
    /// let (response, chan) = chan.recv()?;
    ///
    /// // After: 1 line
    /// let (response, chan) = chan.request(value)?;
    /// ```
    pub fn request(self, value: A) -> Result<(B, Chan<S, T>), SessionError> {
        let chan = self.send(value)?;
        chan.recv()
    }
}

// --- Session termination ---

impl<T: Transport> Chan<End, T> {
    /// Close the session, dropping the transport.
    /// Use when the session is complete and the transport is no longer needed.
    pub fn close(self) {
        drop(self);
    }

    /// Complete the session and reclaim the underlying transport.
    /// Use for phase transitions: the session-typed handshake ends,
    /// and the transport continues into the active phase with typed
    /// enum messaging.
    ///
    /// ```text
    /// // Handshake complete → active phase
    /// let transport = chan.finish();
    /// let stream = transport.into_stream();
    /// let source = SessionSource::new(stream)?;
    /// ```
    pub fn finish(self) -> T {
        self.transport
    }
}
