use std::fmt;
use std::io;

/// Errors that can occur during a session-typed conversation.
///
/// The critical property: a crashed or disconnected peer produces
/// `SessionError::Disconnected`, not a panic. The compositor can
/// handle client death as a typed event and continue serving others.
#[derive(Debug)]
pub enum SessionError {
    /// The peer disconnected — crashed, closed the socket, or was killed.
    /// This is the normal "client died" case. The compositor cleans up
    /// the dead client's panes and continues.
    Disconnected,

    /// A message failed to serialize or deserialize.
    /// This indicates a protocol mismatch — the two sides disagree
    /// about what message type is expected. In a well-typed system
    /// this should not happen; if it does, it's a bug.
    Codec(postcard::Error),

    /// An I/O error on the transport layer.
    /// Socket errors, pipe breaks, etc.
    Io(io::Error),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Disconnected => write!(f, "session peer disconnected"),
            SessionError::Codec(e) => write!(f, "session codec error: {}", e),
            SessionError::Io(e) => write!(f, "session I/O error: {}", e),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SessionError::Disconnected => None,
            SessionError::Codec(e) => Some(e),
            SessionError::Io(e) => Some(e),
        }
    }
}

impl From<io::Error> for SessionError {
    fn from(e: io::Error) -> Self {
        // A broken pipe or connection reset is a disconnect, not an I/O error
        match e.kind() {
            io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::UnexpectedEof => SessionError::Disconnected,
            _ => SessionError::Io(e),
        }
    }
}

impl From<postcard::Error> for SessionError {
    fn from(e: postcard::Error) -> Self {
        SessionError::Codec(e)
    }
}
