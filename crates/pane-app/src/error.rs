use std::fmt;

/// Result type for pane-app operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors from pane-app operations.
#[derive(Debug)]
pub enum Error {
    /// Failed to connect to the compositor.
    Connect(ConnectError),
    /// Error during pane operations.
    Pane(PaneError),
    /// Scripting protocol error.
    Script(ScriptError),
    /// Session transport error.
    Session(pane_session::SessionError),
    /// I/O error.
    Io(std::io::Error),
}

/// Connection-specific errors.
#[derive(Debug)]
pub enum ConnectError {
    /// The compositor is not running.
    NotRunning,
    /// Handshake was rejected by the compositor.
    Rejected(String),
    /// Transport failure during handshake.
    Transport(pane_session::SessionError),
}

/// Pane operation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneError {
    /// Pane creation was refused by the compositor.
    Refused,
    /// Pane creation timed out (compositor didn't respond).
    Timeout,
    /// The pane's session was disconnected.
    Disconnected,
    /// Calling `send_and_wait` from a looper thread would deadlock.
    /// The reply arrives on the same channel the looper is blocking on.
    /// Use `send_request` (async) for looper-to-looper communication.
    WouldDeadlock,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connect(e) => write!(f, "connection error: {}", e),
            Error::Pane(e) => write!(f, "pane error: {}", e),
            Error::Script(e) => write!(f, "scripting error: {}", e),
            Error::Session(e) => write!(f, "session error: {}", e),
            Error::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::NotRunning => write!(f, "compositor not running"),
            ConnectError::Rejected(reason) => write!(f, "rejected: {}", reason),
            ConnectError::Transport(e) => write!(f, "transport: {}", e),
        }
    }
}

impl fmt::Display for PaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaneError::Refused => write!(f, "pane creation refused"),
            PaneError::Timeout => write!(f, "pane creation timed out"),
            PaneError::Disconnected => write!(f, "pane disconnected"),
            PaneError::WouldDeadlock => write!(f, "send_and_wait called from looper thread (would deadlock)"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Connect(e) => Some(e),
            Error::Pane(e) => Some(e),
            Error::Script(e) => Some(e),
            Error::Session(e) => Some(e),
            Error::Io(e) => Some(e),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConnectError::Transport(e) => Some(e),
            _ => None,
        }
    }
}

impl std::error::Error for PaneError {}

/// Scripting protocol errors.
///
/// Returned by `DynOptic` operations and the specifier resolution
/// chain. Each variant maps to a specific failure mode in the
/// property access path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// The requested property does not exist on this handler.
    PropertyNotFound,
    /// Value type does not match the property's declared type.
    TypeMismatch {
        expected: crate::scripting::ValueType,
        got: crate::scripting::ValueType,
    },
    /// Attempted to set a read-only property.
    ReadOnly,
    /// Index specifier out of range for a collection property.
    IndexOutOfRange,
    /// Specifier chain resolution failed.
    SpecifierFailed(String),
    /// Internal error: handler state type mismatch in DynOptic.
    /// This indicates a bug in the optic implementation, not a
    /// user error.
    StateMismatch,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::PropertyNotFound => write!(f, "property not found"),
            ScriptError::TypeMismatch { expected, got } =>
                write!(f, "type mismatch: expected {:?}, got {:?}", expected, got),
            ScriptError::ReadOnly => write!(f, "property is read-only"),
            ScriptError::IndexOutOfRange => write!(f, "index out of range"),
            ScriptError::SpecifierFailed(msg) => write!(f, "specifier failed: {}", msg),
            ScriptError::StateMismatch =>
                write!(f, "internal: handler state type mismatch in optic"),
        }
    }
}

impl std::error::Error for ScriptError {}

impl From<ScriptError> for Error {
    fn from(e: ScriptError) -> Self { Error::Script(e) }
}

impl From<ConnectError> for Error {
    fn from(e: ConnectError) -> Self { Error::Connect(e) }
}

impl From<PaneError> for Error {
    fn from(e: PaneError) -> Self { Error::Pane(e) }
}

impl From<pane_session::SessionError> for Error {
    fn from(e: pane_session::SessionError) -> Self { Error::Session(e) }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Error::Io(e) }
}
