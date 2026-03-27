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
#[derive(Debug)]
pub enum PaneError {
    /// Pane creation was refused by the compositor.
    Refused,
    /// Pane creation timed out (compositor didn't respond).
    Timeout,
    /// The pane's session was disconnected.
    Disconnected,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connect(e) => write!(f, "connection error: {}", e),
            Error::Pane(e) => write!(f, "pane error: {}", e),
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
        }
    }
}

impl std::error::Error for Error {}
impl std::error::Error for ConnectError {}
impl std::error::Error for PaneError {}

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
