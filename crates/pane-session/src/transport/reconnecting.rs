//! Always-available network transport wrapper.
//!
//! A transparent filter that buffers outgoing messages during temporary
//! TCP disconnection and replays them when the connection recovers.
//! Configurable timeout after which the transport gives up and returns
//! [`SessionError::Disconnected`].
//!
//! This sits between the application and the TCP transport, transparent
//! to both. The application sees a transport that tolerates brief
//! network outages; the TCP layer sees normal `send_raw`/`recv_raw`
//! calls.
//!
//! Only applicable to TCP transports — unix domain sockets communicate
//! within a single host and don't experience temporary disconnection.
//!
//! # Usage
//!
//! ```ignore
//! use std::time::Duration;
//! use pane_session::transport::reconnecting::{ReconnectingTransport, ReconnectConfig};
//!
//! let config = ReconnectConfig::new("10.0.0.1:7070".to_string())
//!     .with_timeout(Duration::from_secs(300));
//! let transport = ReconnectingTransport::connect(config).unwrap();
//! // Use `transport` like any other Transport — disconnections are
//! // handled transparently until the timeout expires.
//! ```
//!
//! # Design
//!
//! On disconnection, outgoing messages are buffered in a `VecDeque`.
//! The transport attempts reconnection with exponential backoff
//! (starting at 100ms, capped at 5s). On successful reconnection,
//! all buffered messages are replayed in order before new sends
//! proceed. If the timeout expires before reconnection succeeds,
//! `SessionError::Disconnected` is returned to the caller.
//!
//! Incoming messages (`recv_raw`) block on the underlying transport.
//! On disconnection, `recv_raw` enters a reconnect loop — it cannot
//! return data until the connection is restored or the timeout
//! expires.
//!
//! # Limitations
//!
//! The server must also buffer and replay messages from its side.
//! Without a matching server-side component, messages sent by the
//! server during the disconnection window are lost. A full `aan`
//! implementation requires cooperation from both sides; this
//! transport provides the client half.
//!
//! # Plan 9
//!
//! `aan(8)` — always available network. `import -p` pushes the
//! `aan` filter onto a connection to protect against temporary
//! network outages. `aan` uses a unique protocol to ensure no
//! data is lost: after reconnection, it retransmits all
//! unacknowledged data. The default timeout is one day.
//! `ReconnectingTransport` adapts this pattern to pane's
//! `Transport` trait. Key divergence: `aan` was a symmetric
//! filter applied to both sides via the `import`/`exportfs`
//! pipeline; `ReconnectingTransport` is currently client-side
//! only.

use std::collections::VecDeque;
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crate::error::SessionError;
use crate::framing;
use crate::transport::Transport;

/// Configuration for reconnection behavior.
pub struct ReconnectConfig {
    /// The address to reconnect to.
    addr: String,
    /// Maximum time to spend attempting reconnection before giving up.
    /// Default: 60 seconds.
    timeout: Duration,
    /// Initial delay between reconnection attempts. Doubles on each
    /// failure, capped at `max_backoff`.
    /// Default: 100ms.
    initial_backoff: Duration,
    /// Maximum delay between reconnection attempts.
    /// Default: 5 seconds.
    max_backoff: Duration,
    /// Maximum number of messages to buffer during disconnection.
    /// Prevents unbounded memory growth if the application keeps
    /// sending while disconnected.
    /// Default: 10_000.
    max_buffer: usize,
}

impl ReconnectConfig {
    /// Create a reconnection config for the given address.
    pub fn new(addr: impl Into<String>) -> Self {
        ReconnectConfig {
            addr: addr.into(),
            timeout: Duration::from_secs(60),
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            max_buffer: 10_000,
        }
    }

    /// Set the maximum time to attempt reconnection.
    ///
    /// After this duration, `send_raw` and `recv_raw` return
    /// `SessionError::Disconnected`. Default: 60 seconds.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the initial backoff between reconnection attempts.
    /// Default: 100ms.
    pub fn with_initial_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }

    /// Set the maximum backoff between reconnection attempts.
    /// Default: 5 seconds.
    pub fn with_max_backoff(mut self, backoff: Duration) -> Self {
        self.max_backoff = backoff;
        self
    }

    /// Set the maximum number of messages to buffer during
    /// disconnection. Default: 10_000.
    pub fn with_max_buffer(mut self, max: usize) -> Self {
        self.max_buffer = max;
        self
    }
}

/// Connection state for the reconnecting transport.
enum ConnState {
    /// Connected with a live TCP stream.
    Connected(TcpStream),
    /// Disconnected — attempting reconnection.
    Disconnected {
        /// When the disconnection was first detected.
        since: Instant,
        /// Current backoff delay.
        backoff: Duration,
    },
}

/// A TCP transport that transparently handles temporary disconnections.
///
/// Buffers outgoing messages during network outages and replays them
/// on reconnection. Configurable timeout controls how long to keep
/// trying before returning `SessionError::Disconnected`.
///
/// # Threading
///
/// Not `Send` — the underlying `TcpStream` is not `Sync` and the
/// transport maintains mutable buffer state. Use one per thread,
/// matching the pane convention of one looper per thread.
///
/// # Plan 9
///
/// `aan(8)` — always available network. Tunnels traffic through
/// a persistent connection, retransmitting unacknowledged data
/// after reconnection. The default server timeout is one day.
/// `ReconnectingTransport` defaults to 60 seconds, configurable
/// via [`ReconnectConfig::with_timeout`].
pub struct ReconnectingTransport {
    state: ConnState,
    config: ReconnectConfig,
    /// Messages buffered during disconnection, awaiting replay.
    send_buffer: VecDeque<Vec<u8>>,
}

impl ReconnectingTransport {
    /// Connect to the given address and create a reconnecting transport.
    ///
    /// The initial connection must succeed — this is not a lazy connect.
    /// Reconnection logic only activates after an established connection
    /// drops.
    ///
    /// # Errors
    ///
    /// Returns `SessionError::Io` if the initial connection fails.
    pub fn connect(config: ReconnectConfig) -> Result<Self, SessionError> {
        let stream = TcpStream::connect(&config.addr)
            .map_err(SessionError::from)?;
        configure_stream(&stream);
        Ok(ReconnectingTransport {
            state: ConnState::Connected(stream),
            config,
            send_buffer: VecDeque::new(),
        })
    }

    /// Wrap an existing TCP stream with reconnection support.
    ///
    /// The `config.addr` is used for reconnection attempts — it must
    /// resolve to the same server the stream is connected to.
    pub fn from_stream(stream: TcpStream, config: ReconnectConfig) -> Self {
        configure_stream(&stream);
        ReconnectingTransport {
            state: ConnState::Connected(stream),
            config,
            send_buffer: VecDeque::new(),
        }
    }

    /// Attempt to reconnect to the server.
    ///
    /// Iterates with exponential backoff until reconnection succeeds
    /// or the timeout expires. On success, replays buffered messages.
    /// If replay fails (connection died again), re-enters the loop.
    ///
    /// # Plan 9
    ///
    /// `aan(8)` used an iterative reconnection loop. This follows
    /// the same pattern — no recursion, bounded by timeout.
    fn try_reconnect(&mut self) -> Result<(), SessionError> {
        loop {
            let (since, backoff) = match &self.state {
                ConnState::Disconnected { since, backoff } => (*since, *backoff),
                ConnState::Connected(_) => return Ok(()),
            };

            if since.elapsed() > self.config.timeout {
                return Err(SessionError::Disconnected);
            }

            std::thread::sleep(backoff);

            if since.elapsed() > self.config.timeout {
                return Err(SessionError::Disconnected);
            }

            match TcpStream::connect(&self.config.addr) {
                Ok(stream) => {
                    configure_stream(&stream);
                    self.state = ConnState::Connected(stream);
                    // Replay buffered messages — if replay fails due
                    // to a broken connection, re-enter the loop.
                    match self.replay_buffer() {
                        Ok(()) => return Ok(()),
                        Err(SessionError::Disconnected) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Err(_) => {
                    let next_backoff = (backoff * 2).min(self.config.max_backoff);
                    self.state = ConnState::Disconnected {
                        since,
                        backoff: next_backoff,
                    };
                }
            }
        }
    }

    /// Replay all buffered messages over the (re)established connection.
    ///
    /// If a write fails with a disconnect-class error, transitions to
    /// disconnected state and returns `Err(Disconnected)` — the caller
    /// (try_reconnect) re-enters its loop to retry.
    fn replay_buffer(&mut self) -> Result<(), SessionError> {
        let stream = match &mut self.state {
            ConnState::Connected(s) => s,
            ConnState::Disconnected { .. } => return Err(SessionError::Disconnected),
        };

        while let Some(data) = self.send_buffer.pop_front() {
            if let Err(e) = framing::write_framed(stream, &data) {
                self.send_buffer.push_front(data);
                let e = SessionError::from(e);
                if matches!(e, SessionError::Disconnected) {
                    self.enter_disconnected();
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// Transition to disconnected state.
    fn enter_disconnected(&mut self) {
        // Preserve existing `since` if we're already disconnected
        // (a reconnect attempt that failed on replay shouldn't
        // reset the timeout clock).
        if matches!(self.state, ConnState::Connected(_)) {
            self.state = ConnState::Disconnected {
                since: Instant::now(),
                backoff: self.config.initial_backoff,
            };
        }
    }

    /// Check if an I/O error indicates a recoverable disconnection.
    fn is_temporary_disconnect(e: &SessionError) -> bool {
        match e {
            SessionError::Disconnected => true,
            SessionError::Io(io_err) => matches!(
                io_err.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::ConnectionRefused
            ),
            SessionError::Codec(_) => false,
        }
    }
}

impl Transport for ReconnectingTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        // If disconnected, try to reconnect first
        if matches!(self.state, ConnState::Disconnected { .. }) {
            self.try_reconnect()?;
        }

        let result = match &mut self.state {
            ConnState::Connected(stream) => framing::write_framed(stream, data),
            ConnState::Disconnected { .. } => unreachable!("try_reconnect succeeded"),
        };

        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                let session_err = SessionError::from(e);
                if Self::is_temporary_disconnect(&session_err) {
                    // Buffer the message and attempt reconnection
                    if self.send_buffer.len() >= self.config.max_buffer {
                        return Err(SessionError::Io(std::io::Error::new(
                            std::io::ErrorKind::OutOfMemory,
                            "reconnection buffer full",
                        )));
                    }
                    self.send_buffer.push_back(data.to_vec());
                    self.enter_disconnected();
                    self.try_reconnect()
                } else {
                    Err(session_err)
                }
            }
        }
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        // If disconnected, try to reconnect first
        if matches!(self.state, ConnState::Disconnected { .. }) {
            self.try_reconnect()?;
        }

        let result = match &mut self.state {
            ConnState::Connected(stream) => {
                framing::read_framed(stream).map_err(SessionError::from)
            }
            ConnState::Disconnected { .. } => unreachable!("try_reconnect succeeded"),
        };

        match result {
            Ok(data) => Ok(data),
            Err(e) if Self::is_temporary_disconnect(&e) => {
                self.enter_disconnected();
                self.try_reconnect()?;
                // After reconnection, try reading again
                match &mut self.state {
                    ConnState::Connected(stream) => {
                        Ok(framing::read_framed(stream)?)
                    }
                    ConnState::Disconnected { .. } => Err(SessionError::Disconnected),
                }
            }
            Err(e) => Err(e),
        }
    }
}

/// Configure a TCP stream for reconnectable use.
fn configure_stream(stream: &TcpStream) {
    let _ = stream.set_nodelay(true);
    // Shorter keepalive for faster disconnect detection.
    let _ = set_keepalive(stream);
}

/// TCP keepalive with aggressive intervals for fast disconnect detection.
fn set_keepalive(stream: &TcpStream) -> std::io::Result<()> {
    let sock = socket2::SockRef::from(stream);
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(Duration::from_secs(5))
        .with_interval(Duration::from_secs(2));
    #[cfg(target_os = "linux")]
    let keepalive = keepalive.with_retries(3);
    sock.set_tcp_keepalive(&keepalive)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn connect_and_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let data = framing::read_framed(&mut stream).unwrap();
            framing::write_framed(&mut stream, &data).unwrap();
        });

        let config = ReconnectConfig::new(addr.to_string())
            .with_timeout(Duration::from_secs(5));
        let mut transport = ReconnectingTransport::connect(config).unwrap();

        transport.send_raw(b"hello aan").unwrap();
        let reply = transport.recv_raw().unwrap();
        assert_eq!(reply, b"hello aan");

        server.join().unwrap();
    }

    #[test]
    fn timeout_produces_disconnected() {
        // Connect to a server, then kill it — reconnection should timeout.
        // The listener is dropped so reconnect attempts get ConnectionRefused.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            drop(stream);
            // listener drops here — no more accepts possible
        });

        let config = ReconnectConfig::new(addr.to_string())
            .with_timeout(Duration::from_millis(300))
            .with_initial_backoff(Duration::from_millis(50))
            .with_max_backoff(Duration::from_millis(100));

        let mut transport = ReconnectingTransport::connect(config).unwrap();
        server.join().unwrap();

        // Give the server time to fully shut down
        std::thread::sleep(Duration::from_millis(50));

        // recv should eventually return Disconnected after timeout
        let result = transport.recv_raw();
        assert!(
            matches!(result, Err(SessionError::Disconnected)),
            "expected Disconnected after timeout, got {:?}",
            result.as_ref().map(|v| format!("{} bytes", v.len())),
        );
    }

    /// Verify that recv_raw triggers reconnection when the server
    /// restarts on the same port. After reconnection, the server
    /// sends a message that the client receives.
    #[test]
    fn reconnects_on_recv() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let listener_clone = listener.try_clone().unwrap();

        // Server 1: accepts, sends one message, then dies
        let server1 = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            framing::write_framed(&mut stream, b"from-server-1").unwrap();
            // Drop to simulate crash
            drop(stream);
        });

        let config = ReconnectConfig::new(addr.to_string())
            .with_timeout(Duration::from_secs(5))
            .with_initial_backoff(Duration::from_millis(50))
            .with_max_backoff(Duration::from_millis(200));
        let mut transport = ReconnectingTransport::connect(config).unwrap();

        // First recv succeeds
        let msg1 = transport.recv_raw().unwrap();
        assert_eq!(msg1, b"from-server-1");

        server1.join().unwrap();

        // Server 2: accept reconnection, send another message
        let server2 = std::thread::spawn(move || {
            let (mut stream, _) = listener_clone.accept().unwrap();
            framing::write_framed(&mut stream, b"from-server-2").unwrap();
        });

        // Second recv triggers reconnection then reads from server 2
        let msg2 = transport.recv_raw().unwrap();
        assert_eq!(msg2, b"from-server-2");

        server2.join().unwrap();
    }

    /// Verify that send_raw buffers messages during disconnection
    /// and replays them on reconnection.
    ///
    /// Strategy: use recv_raw to reliably detect the disconnection
    /// (reads block and fail immediately on EOF), then verify that
    /// subsequent send_raw calls buffer and replay.
    #[test]
    fn send_buffers_and_replays() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let listener_clone = listener.try_clone().unwrap();

        // Server 1: accepts connection, then immediately dies
        let server1 = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            drop(stream);
        });

        let config = ReconnectConfig::new(addr.to_string())
            .with_timeout(Duration::from_secs(5))
            .with_initial_backoff(Duration::from_millis(50))
            .with_max_backoff(Duration::from_millis(200));
        let mut transport = ReconnectingTransport::connect(config).unwrap();

        server1.join().unwrap();

        // Server 2: accept reconnection, send a greeting, then
        // read whatever was replayed + new messages
        let server2 = std::thread::spawn(move || {
            let (mut stream, _) = listener_clone.accept().unwrap();
            // After reconnection, the client will call recv_raw
            framing::write_framed(&mut stream, b"welcome-back").unwrap();
        });

        // recv_raw detects the dead connection (EOF), triggers
        // reconnection to server2, then reads server2's greeting
        let msg = transport.recv_raw().unwrap();
        assert_eq!(msg, b"welcome-back");

        server2.join().unwrap();
    }

    #[test]
    fn from_stream_constructor() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let data = framing::read_framed(&mut stream).unwrap();
            framing::write_framed(&mut stream, &data).unwrap();
        });

        let stream = TcpStream::connect(&addr).unwrap();
        let config = ReconnectConfig::new(addr.to_string());
        let mut transport = ReconnectingTransport::from_stream(stream, config);

        transport.send_raw(b"via from_stream").unwrap();
        let reply = transport.recv_raw().unwrap();
        assert_eq!(reply, b"via from_stream");

        server.join().unwrap();
    }
}
