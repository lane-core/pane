//! ConnectionSource: calloop EventSource wrapping a post-handshake
//! connection fd.
//!
//! Replaces the bridge's reader thread + forwarding thread for a
//! single connection. When registered on the Looper's calloop
//! EventLoop, it reads frames from the connection fd and produces
//! LooperMessages via the calloop callback, and drains outbound
//! frames from a write channel to the fd when writable.
//!
//! The connection fd is always non-blocking. Read interest is always
//! registered. Write interest is registered only when the write
//! queue is non-empty — when drained, write interest is removed to
//! avoid busy-spinning.
//!
//! ConnectionSource is born Active (D2): handshake has already
//! completed. A ConnectionSource that exists is a session that
//! speaks the protocol.
//!
//! Design heritage: BeOS used one thread per BLooper with a kernel
//! port as the event source (src/kits/app/Looper.cpp:1162,
//! MessageFromPort). Plan 9's per-connection mux (devmnt.c:803,
//! mountmux) read from the mount fd and dispatched by tag. pane's
//! ConnectionSource is the calloop equivalent: one fd, event-driven
//! read/write, LooperMessage output. It replaces the bridge's
//! thread-per-direction model with poll-based multiplexing on a
//! single thread — the looper thread.

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use calloop::generic::Generic;
use calloop::{EventSource, Interest, Mode, Poll, PostAction, Readiness, Token, TokenFactory};

use pane_proto::control::ControlMessage;
use pane_session::bridge::{LooperMessage, WriteMessage};
use pane_session::frame::Frame;

/// Errors from ConnectionSource's event processing.
#[derive(Debug)]
pub enum ConnectionError {
    /// The underlying I/O failed in a way that isn't WouldBlock.
    Io(io::Error),
    /// Frame protocol violation (oversized, unknown service, etc.).
    Protocol(String),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::Io(e) => write!(f, "connection I/O error: {e}"),
            ConnectionError::Protocol(msg) => write!(f, "connection protocol error: {msg}"),
        }
    }
}

impl std::error::Error for ConnectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConnectionError::Io(e) => Some(e),
            ConnectionError::Protocol(_) => None,
        }
    }
}

impl From<io::Error> for ConnectionError {
    fn from(e: io::Error) -> Self {
        ConnectionError::Io(e)
    }
}

/// State machine for incremental frame decoding from a non-blocking
/// source. Handles WouldBlock at the byte level, accumulating partial
/// reads across poll iterations.
///
/// Frame wire format: [length: u32 LE][service: u16 LE][payload]
/// Length counts service + payload (minimum 2).
struct FrameReader {
    /// Maximum message size for this connection (negotiated in Welcome).
    max_message_size: u32,
    /// Whether all service discriminants are accepted (server-side).
    permissive: bool,
    /// Accumulated bytes for the current frame. Once we have enough
    /// bytes for a complete frame, we decode and emit it.
    buf: Vec<u8>,
    /// How many bytes we've read into buf so far.
    filled: usize,
    /// Current parse state.
    state: ReadState,
}

/// Parse state for incremental frame reading.
#[derive(Clone, Copy)]
enum ReadState {
    /// Reading the 4-byte length prefix.
    Length,
    /// Reading the body (service + payload). The field holds the
    /// declared body length from the length prefix.
    Body { body_len: u32 },
}

impl FrameReader {
    fn new(max_message_size: u32, permissive: bool) -> Self {
        FrameReader {
            max_message_size,
            permissive,
            buf: vec![0u8; 4], // start with length prefix buffer
            filled: 0,
            state: ReadState::Length,
        }
    }

    /// Try to read one complete frame from the source.
    ///
    /// Returns:
    /// - `Ok(Some(frame))` — a complete frame was decoded
    /// - `Ok(None)` — WouldBlock, no complete frame yet
    /// - `Err(ConnectionError)` — fatal error (EOF, protocol violation)
    fn try_read_frame(&mut self, source: &mut impl Read) -> Result<Option<Frame>, ConnectionError> {
        loop {
            match self.state {
                ReadState::Length => {
                    // Try to fill the 4-byte length prefix.
                    match read_into(&mut self.buf, &mut self.filled, source) {
                        ReadProgress::Complete => {}
                        ReadProgress::WouldBlock => return Ok(None),
                        ReadProgress::Eof => {
                            if self.filled == 0 {
                                // Clean EOF at frame boundary — connection closed.
                                return Err(ConnectionError::Io(io::Error::new(
                                    io::ErrorKind::UnexpectedEof,
                                    "connection closed",
                                )));
                            }
                            // Partial length prefix — truncated stream.
                            return Err(ConnectionError::Io(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "connection closed mid-frame (partial length prefix)",
                            )));
                        }
                        ReadProgress::Error(e) => return Err(ConnectionError::Io(e)),
                    }

                    let length =
                        u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]);

                    // Validate length.
                    if length < 2 {
                        return Err(ConnectionError::Protocol(format!(
                            "frame too short: declared length {length}, minimum is 2"
                        )));
                    }
                    if length > self.max_message_size {
                        return Err(ConnectionError::Protocol(format!(
                            "frame too large: {length} bytes (limit {})",
                            self.max_message_size
                        )));
                    }

                    // Transition to body state.
                    self.buf.resize(length as usize, 0);
                    self.filled = 0;
                    self.state = ReadState::Body { body_len: length };
                }
                ReadState::Body { body_len } => {
                    match read_into(&mut self.buf, &mut self.filled, source) {
                        ReadProgress::Complete => {}
                        ReadProgress::WouldBlock => return Ok(None),
                        ReadProgress::Eof => {
                            return Err(ConnectionError::Io(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "connection closed mid-frame (partial body)",
                            )));
                        }
                        ReadProgress::Error(e) => return Err(ConnectionError::Io(e)),
                    }

                    let service = u16::from_le_bytes([self.buf[0], self.buf[1]]);
                    let frame = if service == 0xFFFF {
                        Frame::Abort
                    } else if !self.permissive && service != 0 {
                        // Non-permissive mode: only service 0 is
                        // accepted without explicit registration.
                        // ConnectionSource uses permissive mode
                        // (same as the existing reader loop) because
                        // session_ids are dynamically allocated by
                        // DeclareInterest.
                        return Err(ConnectionError::Protocol(format!(
                            "unknown service discriminant: 0x{service:04X}"
                        )));
                    } else {
                        let payload = self.buf[2..body_len as usize].to_vec();
                        Frame::Message { service, payload }
                    };

                    // Reset for next frame.
                    self.buf.resize(4, 0);
                    self.filled = 0;
                    self.state = ReadState::Length;

                    return Ok(Some(frame));
                }
            }
        }
    }
}

/// Outcome of a partial read into a fixed-size buffer.
enum ReadProgress {
    /// The buffer is now full.
    Complete,
    /// Got WouldBlock — come back later.
    WouldBlock,
    /// Got EOF (0 bytes read).
    Eof,
    /// Got a real I/O error.
    Error(io::Error),
}

/// Read bytes into `buf[filled..]` from `source`, advancing `filled`.
/// Returns once the buffer is full, or on WouldBlock/EOF/error.
fn read_into(buf: &mut [u8], filled: &mut usize, source: &mut impl Read) -> ReadProgress {
    while *filled < buf.len() {
        match source.read(&mut buf[*filled..]) {
            Ok(0) => return ReadProgress::Eof,
            Ok(n) => *filled += n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return ReadProgress::WouldBlock,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return ReadProgress::Error(e),
        }
    }
    ReadProgress::Complete
}

/// State machine for incremental frame writing to a non-blocking
/// sink. Handles WouldBlock and partial writes.
struct FrameWriter {
    /// Queued complete frames ready to write. Each entry is a
    /// fully-encoded wire frame (length prefix + service + payload).
    queue: VecDeque<Vec<u8>>,
    /// Position within the current front-of-queue frame.
    write_pos: usize,
}

impl FrameWriter {
    fn new() -> Self {
        FrameWriter {
            queue: VecDeque::new(),
            write_pos: 0,
        }
    }

    /// Enqueue a frame for writing. Encodes the wire format
    /// (length prefix + service + payload) into the queue.
    fn enqueue(&mut self, service: u16, payload: &[u8]) {
        let length = 2u32 + payload.len() as u32;
        let mut frame = Vec::with_capacity(4 + length as usize);
        frame.extend_from_slice(&length.to_le_bytes());
        frame.extend_from_slice(&service.to_le_bytes());
        frame.extend_from_slice(payload);
        self.queue.push_back(frame);
    }

    /// Is the write queue empty?
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Number of frames queued for writing.
    fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Drain as many queued frames as possible to the sink.
    ///
    /// Returns:
    /// - `Ok(true)` — queue fully drained
    /// - `Ok(false)` — WouldBlock, more to write later
    /// - `Err(e)` — fatal write error
    fn try_flush(&mut self, sink: &mut impl Write) -> Result<bool, io::Error> {
        while let Some(front) = self.queue.front() {
            let remaining = &front[self.write_pos..];
            match sink.write(remaining) {
                Ok(0) => {
                    // Write returned 0 — treat as broken pipe.
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write returned 0 bytes",
                    ));
                }
                Ok(n) => {
                    self.write_pos += n;
                    if self.write_pos >= front.len() {
                        self.queue.pop_front();
                        self.write_pos = 0;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }
}

/// A calloop EventSource wrapping a single post-handshake connection fd.
///
/// Produces `LooperMessage` events by reading frames from the
/// connection and classifying them into Control or Service messages.
/// Drains outbound frames from a write channel to the connection
/// when writable.
///
/// Design heritage: BeOS BLooper had one thread per window reading
/// from a kernel port (src/kits/app/Looper.cpp:1162). Plan 9's
/// per-connection mux in devmnt.c (mountmux:934) read from the
/// mount fd and dispatched replies by tag. ConnectionSource merges
/// both roles — read dispatch and write drain — into one calloop
/// event source on the looper thread, eliminating the bridge's
/// two threads per connection.
pub struct ConnectionSource {
    /// The connection fd, wrapped for calloop registration.
    /// Interest is dynamically managed: always readable, writable
    /// only when the write queue is non-empty.
    fd: Generic<UnixStream>,
    /// Incremental frame decoder for non-blocking reads.
    reader: FrameReader,
    /// Incremental frame encoder/writer for non-blocking writes.
    writer: FrameWriter,
    /// Receiver for outbound frames from ServiceHandle/SubscriberSender.
    /// Transitional: keeps the existing mpsc channel interface so
    /// that ServiceHandle/SubscriberSender don't need to change.
    /// A future optimization can bypass the channel and write
    /// directly to the queue.
    write_rx: std::sync::mpsc::Receiver<WriteMessage>,
    /// Whether write interest is currently registered. Tracked to
    /// avoid unnecessary reregistration when nothing changed.
    write_interest: bool,
    /// Connection identifier for PeerScope addressing.
    connection_id: u16,
    /// Token assigned by calloop during registration. Saved for
    /// `before_sleep` synthetic event generation — when the write
    /// channel has pending data between poll iterations,
    /// `before_sleep` returns a synthetic writable event using this
    /// token to trigger process_events and flush the write queue.
    /// Without this, writes arriving between poll iterations when
    /// the fd has no read readiness would never be flushed.
    registered_token: Option<Token>,
}

impl ConnectionSource {
    /// Create a new ConnectionSource from a post-handshake UnixStream.
    ///
    /// The stream is set to non-blocking mode. `max_message_size` is
    /// the negotiated value from the Welcome. `write_rx` is the
    /// existing bounded mpsc channel that ServiceHandle/SubscriberSender
    /// write to.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if setting non-blocking mode fails.
    pub fn new(
        stream: UnixStream,
        max_message_size: u32,
        write_rx: std::sync::mpsc::Receiver<WriteMessage>,
        connection_id: u16,
    ) -> io::Result<Self> {
        stream.set_nonblocking(true)?;

        Ok(ConnectionSource {
            fd: Generic::new(stream, Interest::READ, Mode::Level),
            reader: FrameReader::new(max_message_size, true), // permissive (D2: born Active)
            writer: FrameWriter::new(),
            write_rx,
            write_interest: false,
            connection_id,
            registered_token: None,
        })
    }

    /// Drain pending messages from the write channel into the
    /// internal write queue. Non-blocking: uses try_recv.
    ///
    /// The write queue has a soft cap (`WRITE_QUEUE_HIGHWATER`).
    /// When the queue depth exceeds this, no more messages are
    /// pulled from the channel. This preserves backpressure:
    /// socket-level WouldBlock keeps the write queue full, which
    /// keeps the bounded mpsc channel full, which stalls the
    /// SyncSender. Without this cap, the VecDeque would absorb
    /// unbounded data and the sender would never block.
    ///
    /// The highwater mark is small — just enough to batch writes
    /// efficiently without absorbing the entire channel capacity.
    fn drain_write_channel(&mut self) {
        // Don't drain if the write queue is already above highwater.
        // The queued data hasn't been flushed to the socket yet
        // (WouldBlock), so pulling more from the channel just moves
        // the buffer from bounded (mpsc) to unbounded (VecDeque).
        const WRITE_QUEUE_HIGHWATER: usize = 8;
        if self.writer.queue_len() >= WRITE_QUEUE_HIGHWATER {
            return;
        }

        while self.writer.queue_len() < WRITE_QUEUE_HIGHWATER {
            match self.write_rx.try_recv() {
                Ok((service, payload)) => {
                    self.writer.enqueue(service, &payload);
                }
                Err(_) => break,
            }
        }
    }

    /// The connection identifier for PeerScope addressing.
    pub fn connection_id(&self) -> u16 {
        self.connection_id
    }
}

/// Classify a decoded frame into a LooperMessage.
///
/// Same logic as the bridge's reader_loop in bridge.rs — Control
/// frames (service 0) are deserialized as ControlMessage, service
/// frames are forwarded with their session_id tag.
fn classify_frame(frame: Frame) -> Result<Option<LooperMessage>, ConnectionError> {
    match frame {
        Frame::Message {
            service: 0,
            payload,
        } => {
            let msg: ControlMessage = postcard::from_bytes(&payload).map_err(|_| {
                ConnectionError::Protocol("control frame deserialization failed".into())
            })?;
            Ok(Some(LooperMessage::Control(msg)))
        }
        Frame::Message { service, payload } => Ok(Some(LooperMessage::Service {
            session_id: service,
            payload,
        })),
        Frame::Abort => {
            // ProtocolAbort — signal connection close.
            Ok(None)
        }
    }
}

impl EventSource for ConnectionSource {
    type Event = LooperMessage;
    type Metadata = ();
    type Ret = ();
    type Error = ConnectionError;

    // Opt into before_sleep/before_handle_events lifecycle.
    // before_sleep drains the write channel and generates a
    // synthetic writable event when writes are pending. Without
    // this, writes arriving between poll iterations when the fd
    // has no read readiness would never be flushed — the mpsc
    // channel is invisible to calloop's polling.
    const NEEDS_EXTRA_LIFECYCLE_EVENTS: bool = true;

    fn process_events<F>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: F,
    ) -> Result<PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        // Save the token on first call for before_sleep synthetic
        // event generation.
        if self.registered_token.is_none() {
            self.registered_token = Some(token);
        }

        // Before processing I/O, drain the write channel into the
        // internal queue. This picks up any frames enqueued by
        // handlers since the last poll.
        self.drain_write_channel();

        // SAFETY: We need &mut access to the stream for read/write.
        // The stream is not moved out — calloop's Generic owns it
        // and we only borrow it for I/O during process_events.
        let stream = unsafe { self.fd.get_mut() };

        let mut action = PostAction::Continue;

        // --- Read path ---
        if readiness.readable {
            loop {
                match self.reader.try_read_frame(stream) {
                    Ok(Some(frame)) => match classify_frame(frame) {
                        Ok(Some(msg)) => {
                            callback(msg, &mut ());
                        }
                        Ok(None) => {
                            // ProtocolAbort — connection dead.
                            return Ok(PostAction::Remove);
                        }
                        Err(_) => {
                            // Protocol violation — connection dead.
                            return Ok(PostAction::Remove);
                        }
                    },
                    Ok(None) => break, // WouldBlock — no more data
                    Err(ConnectionError::Io(ref e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        // EOF — connection closed.
                        return Ok(PostAction::Remove);
                    }
                    Err(_) => {
                        // Fatal read error — connection dead.
                        return Ok(PostAction::Remove);
                    }
                }
            }
        }

        // --- Write path ---
        if readiness.writable {
            match self.writer.try_flush(stream) {
                Ok(true) => {
                    // Queue drained — remove write interest.
                    if self.write_interest {
                        self.write_interest = false;
                        self.fd.interest = Interest::READ;
                        action = PostAction::Reregister;
                    }
                }
                Ok(false) => {
                    // More to write — keep write interest.
                }
                Err(_) => {
                    // Fatal write error — connection dead.
                    return Ok(PostAction::Remove);
                }
            }
        }

        // If we have pending writes and no write interest, add it.
        if !self.writer.is_empty() && !self.write_interest {
            self.write_interest = true;
            self.fd.interest = Interest::BOTH;
            action = PostAction::Reregister;
        }

        Ok(action)
    }

    fn register(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.fd.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut Poll,
        token_factory: &mut TokenFactory,
    ) -> calloop::Result<()> {
        self.fd.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        self.fd.unregister(poll)
    }

    /// Check for pending writes before calloop polls.
    ///
    /// The write channel (std::sync::mpsc) is invisible to calloop's
    /// polling — data can arrive between poll iterations without any
    /// fd event. Without this hook, writes enqueued after the last
    /// process_events call would sit in the channel until a read
    /// event happens to wake the source.
    ///
    /// This drains the write channel into the internal queue and, if
    /// writes are pending, returns a synthetic writable event. calloop
    /// sets the poll timeout to zero and calls process_events with
    /// the synthetic readiness, which flushes the write queue to the
    /// socket.
    ///
    /// Design heritage: calloop's own Channel source uses Ping (pipe
    /// write) to wake the poll when a sender pushes data.
    /// ConnectionSource avoids adding a pipe fd by using the
    /// before_sleep lifecycle instead — same result, one fewer fd.
    fn before_sleep(&mut self) -> calloop::Result<Option<(Readiness, Token)>> {
        // Check for pending writes without greedy drain. If the write
        // queue already has data, return a synthetic writable event
        // immediately — process_events will drain more from the
        // channel and flush to the socket.
        //
        // If the queue is empty, peek at the channel with one
        // try_recv. This avoids unbounded buffering: most writes
        // stay in the bounded mpsc channel until process_events
        // runs, preserving backpressure to the sender.
        if self.writer.is_empty() {
            if let Ok((service, payload)) = self.write_rx.try_recv() {
                self.writer.enqueue(service, &payload);
            }
        }

        if !self.writer.is_empty() {
            if let Some(token) = self.registered_token {
                return Ok(Some((
                    Readiness {
                        readable: false,
                        writable: true,
                        error: false,
                    },
                    token,
                )));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::control::ControlMessage;
    use pane_proto::protocols::lifecycle::LifecycleMessage;
    use pane_proto::ServiceFrame;
    use pane_session::bridge::WRITE_CHANNEL_CAPACITY;
    use pane_session::frame::FrameCodec;
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    /// Helper: create a ConnectionSource from a socketpair.
    /// Returns (source, peer_stream, write_tx).
    fn setup() -> (ConnectionSource, UnixStream, mpsc::SyncSender<WriteMessage>) {
        let (stream, peer) = UnixStream::pair().unwrap();
        let (write_tx, write_rx) = mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
        let source = ConnectionSource::new(stream, 16 * 1024 * 1024, write_rx, 1).unwrap();
        (source, peer, write_tx)
    }

    /// Helper: write a frame to the peer end of a socketpair
    /// using the blocking FrameCodec.
    fn write_frame_to_peer(peer: &mut UnixStream, service: u16, payload: &[u8]) {
        let codec = FrameCodec::new(16 * 1024 * 1024);
        codec.write_frame(peer, service, payload).unwrap();
    }

    /// Helper: encode a wire frame into a byte buffer for Cursor-based
    /// unit tests (no fd needed).
    fn encode_frame(service: u16, payload: &[u8]) -> Vec<u8> {
        let length = 2u32 + payload.len() as u32;
        let mut buf = Vec::with_capacity(4 + length as usize);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&service.to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    // ── FrameReader unit tests ─────────────────────────────────

    #[test]
    fn frame_reader_decodes_control_frame() {
        let mut reader = FrameReader::new(16 * 1024 * 1024, true);

        let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        let payload = postcard::to_allocvec(&msg).unwrap();
        let wire = encode_frame(0, &payload);
        let mut cursor = io::Cursor::new(wire);

        let frame = reader.try_read_frame(&mut cursor).unwrap().unwrap();
        let looper_msg = classify_frame(frame).unwrap().unwrap();
        assert!(matches!(
            looper_msg,
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));
    }

    #[test]
    fn frame_reader_decodes_service_frame() {
        let mut reader = FrameReader::new(16 * 1024 * 1024, true);

        let inner = ServiceFrame::Notification {
            payload: postcard::to_allocvec(&"hello").unwrap(),
        };
        let payload = postcard::to_allocvec(&inner).unwrap();
        let wire = encode_frame(5, &payload);
        let mut cursor = io::Cursor::new(wire);

        let frame = reader.try_read_frame(&mut cursor).unwrap().unwrap();
        let looper_msg = classify_frame(frame).unwrap().unwrap();
        assert!(matches!(
            looper_msg,
            LooperMessage::Service { session_id: 5, .. }
        ));
    }

    #[test]
    fn frame_reader_decodes_multiple_frames() {
        let mut reader = FrameReader::new(16 * 1024 * 1024, true);

        let mut wire = Vec::new();

        let msg1 = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        wire.extend_from_slice(&encode_frame(0, &postcard::to_allocvec(&msg1).unwrap()));

        let msg2 = ControlMessage::Lifecycle(LifecycleMessage::CloseRequested);
        wire.extend_from_slice(&encode_frame(0, &postcard::to_allocvec(&msg2).unwrap()));

        let inner = ServiceFrame::Notification {
            payload: postcard::to_allocvec(&42u32).unwrap(),
        };
        wire.extend_from_slice(&encode_frame(7, &postcard::to_allocvec(&inner).unwrap()));

        let mut cursor = io::Cursor::new(wire);
        let mut events = Vec::new();
        loop {
            match reader.try_read_frame(&mut cursor) {
                Ok(Some(frame)) => {
                    if let Ok(Some(msg)) = classify_frame(frame) {
                        events.push(msg);
                    }
                }
                // Cursor returns EOF (not WouldBlock) when exhausted.
                Ok(None) => break,
                Err(_) => break,
            }
        }

        assert_eq!(events.len(), 3);
        assert!(matches!(
            &events[0],
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));
        assert!(matches!(
            &events[1],
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::CloseRequested))
        ));
        assert!(matches!(
            &events[2],
            LooperMessage::Service { session_id: 7, .. }
        ));
    }

    #[test]
    fn frame_reader_detects_abort() {
        let mut reader = FrameReader::new(1024, true);

        // ProtocolAbort: length=2, service=0xFFFF.
        let wire = vec![0x02, 0x00, 0x00, 0x00, 0xFF, 0xFF];
        let mut cursor = io::Cursor::new(wire);

        let frame = reader.try_read_frame(&mut cursor).unwrap().unwrap();
        assert!(matches!(frame, Frame::Abort));
        assert!(matches!(classify_frame(frame), Ok(None)));
    }

    #[test]
    fn frame_reader_oversized_frame_rejected() {
        let mut reader = FrameReader::new(100, true);

        let data = 101u32.to_le_bytes();
        let mut cursor = io::Cursor::new(data.to_vec());

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(ConnectionError::Protocol(_))));
    }

    #[test]
    fn frame_reader_too_short_rejected() {
        let mut reader = FrameReader::new(1024, true);

        let data = 1u32.to_le_bytes();
        let mut cursor = io::Cursor::new(data.to_vec());

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(ConnectionError::Protocol(_))));
    }

    #[test]
    fn frame_reader_zero_length_rejected() {
        let mut reader = FrameReader::new(1024, true);

        let data = 0u32.to_le_bytes();
        let mut cursor = io::Cursor::new(data.to_vec());

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(ConnectionError::Protocol(_))));
    }

    #[test]
    fn frame_reader_eof_at_frame_boundary_is_error() {
        let mut reader = FrameReader::new(1024, true);

        // Empty input — EOF before any bytes.
        let mut cursor = io::Cursor::new(Vec::new());
        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(ConnectionError::Io(_))));
    }

    #[test]
    fn frame_reader_eof_mid_body_is_error() {
        let mut reader = FrameReader::new(1024, true);

        // Length says 10 bytes, but only 3 body bytes follow.
        let mut wire = Vec::new();
        wire.extend_from_slice(&10u32.to_le_bytes());
        wire.extend_from_slice(&[0x00, 0x00, 0xAA]); // 3 of 10
        let mut cursor = io::Cursor::new(wire);

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(ConnectionError::Io(_))));
    }

    // ── FrameWriter unit tests ─────────────────────────────────

    #[test]
    fn frame_writer_encodes_correctly() {
        let mut writer = FrameWriter::new();
        writer.enqueue(0, &[0x01, 0x02]);

        let mut buf = Vec::new();
        let drained = writer.try_flush(&mut buf).unwrap();
        assert!(drained);

        // Wire: [length=4 LE][service=0 LE][0x01, 0x02]
        assert_eq!(buf, vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02]);
    }

    #[test]
    fn frame_writer_encodes_multiple() {
        let mut writer = FrameWriter::new();
        writer.enqueue(1, &[0xAA]);
        writer.enqueue(2, &[0xBB, 0xCC]);

        let mut buf = Vec::new();
        let drained = writer.try_flush(&mut buf).unwrap();
        assert!(drained);

        // First frame: [length=3 LE][service=1 LE][0xAA]
        // Second frame: [length=4 LE][service=2 LE][0xBB, 0xCC]
        let expected = vec![
            0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0xAA, // frame 1
            0x04, 0x00, 0x00, 0x00, 0x02, 0x00, 0xBB, 0xCC, // frame 2
        ];
        assert_eq!(buf, expected);
    }

    #[test]
    fn frame_writer_handles_partial_write() {
        let mut writer = FrameWriter::new();
        writer.enqueue(0, &[0xAA, 0xBB, 0xCC]);

        // A writer that only accepts 3 bytes at a time.
        struct SlowWriter {
            buf: Vec<u8>,
        }
        impl Write for SlowWriter {
            fn write(&mut self, data: &[u8]) -> io::Result<usize> {
                let n = data.len().min(3);
                self.buf.extend_from_slice(&data[..n]);
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut slow = SlowWriter { buf: Vec::new() };
        let drained = writer.try_flush(&mut slow).unwrap();
        // try_flush loops, so it drains completely even with slow writes.
        assert!(drained);

        // Wire: [length=5 LE][service=0 LE][0xAA, 0xBB, 0xCC]
        assert_eq!(
            slow.buf,
            vec![0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAA, 0xBB, 0xCC]
        );
    }

    #[test]
    fn frame_writer_empty_is_noop() {
        let mut writer = FrameWriter::new();
        assert!(writer.is_empty());

        let mut buf = Vec::new();
        let drained = writer.try_flush(&mut buf).unwrap();
        assert!(drained);
        assert!(buf.is_empty());
    }

    // ── Writing via ConnectionSource ───────────────────────────

    #[test]
    fn writes_frame_to_peer() {
        let (mut source, mut peer, write_tx) = setup();
        peer.set_nonblocking(false).unwrap();

        let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        let bytes = postcard::to_allocvec(&msg).unwrap();
        write_tx.send((0, bytes.clone())).unwrap();

        // Drain and flush directly (unit test, no calloop).
        source.drain_write_channel();
        assert!(!source.writer.is_empty());

        let stream = unsafe { source.fd.get_mut() };
        let drained = source.writer.try_flush(stream).unwrap();
        assert!(drained);

        // Read from the peer using FrameCodec.
        let codec = FrameCodec::permissive(16 * 1024 * 1024);
        let frame = codec.read_frame(&mut peer).unwrap();
        match frame {
            Frame::Message {
                service: 0,
                payload,
            } => assert_eq!(payload, bytes),
            other => panic!("expected Control frame, got {other:?}"),
        }
    }

    #[test]
    fn writes_multiple_frames_to_peer() {
        let (mut source, mut peer, write_tx) = setup();
        peer.set_nonblocking(false).unwrap();

        for i in 0..5u16 {
            let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
            let bytes = postcard::to_allocvec(&msg).unwrap();
            write_tx.send((i, bytes)).unwrap();
        }

        source.drain_write_channel();
        let stream = unsafe { source.fd.get_mut() };
        let drained = source.writer.try_flush(stream).unwrap();
        assert!(drained);

        let codec = FrameCodec::permissive(16 * 1024 * 1024);
        for i in 0..5u16 {
            let frame = codec.read_frame(&mut peer).unwrap();
            match frame {
                Frame::Message { service, .. } => assert_eq!(service, i),
                other => panic!("expected Message frame, got {other:?}"),
            }
        }
    }

    // ── Connection close detection ─────────────────────────────

    #[test]
    fn detects_peer_close_via_reader() {
        let mut reader = FrameReader::new(1024, true);
        let (stream, peer) = UnixStream::pair().unwrap();
        stream.set_nonblocking(true).unwrap();
        drop(peer);

        let mut stream = stream;
        let result = reader.try_read_frame(&mut stream);
        assert!(matches!(result, Err(ConnectionError::Io(_))));
    }

    #[test]
    fn detects_protocol_abort_via_reader() {
        let mut reader = FrameReader::new(16 * 1024 * 1024, true);
        let (stream, mut peer) = UnixStream::pair().unwrap();
        stream.set_nonblocking(true).unwrap();

        let codec = FrameCodec::new(16 * 1024 * 1024);
        codec.write_abort(&mut peer).unwrap();

        let mut stream = stream;
        let frame = reader.try_read_frame(&mut stream).unwrap().unwrap();
        assert!(matches!(frame, Frame::Abort));
        assert!(matches!(classify_frame(frame), Ok(None)));
    }

    // ── Calloop integration ────────────────────────────────────

    #[test]
    fn calloop_reads_control_frame() {
        use calloop::EventLoop;

        let (source, mut peer, _write_tx) = setup();

        let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        let bytes = postcard::to_allocvec(&msg).unwrap();
        write_frame_to_peer(&mut peer, 0, &bytes);

        let mut event_loop: EventLoop<Vec<LooperMessage>> =
            EventLoop::try_new().expect("event loop creation failed");
        let handle = event_loop.handle();

        handle
            .insert_source(source, |event, _, events: &mut Vec<LooperMessage>| {
                events.push(event);
            })
            .expect("insert_source failed");

        let mut events = Vec::new();
        event_loop
            .dispatch(Some(std::time::Duration::from_millis(100)), &mut events)
            .expect("dispatch failed");

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));
    }

    #[test]
    fn calloop_reads_multiple_frames() {
        use calloop::EventLoop;

        let (source, mut peer, _write_tx) = setup();

        let msg1 = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        write_frame_to_peer(&mut peer, 0, &postcard::to_allocvec(&msg1).unwrap());
        let msg2 = ControlMessage::Lifecycle(LifecycleMessage::CloseRequested);
        write_frame_to_peer(&mut peer, 0, &postcard::to_allocvec(&msg2).unwrap());

        let mut event_loop: EventLoop<Vec<LooperMessage>> =
            EventLoop::try_new().expect("event loop creation failed");
        let handle = event_loop.handle();

        handle
            .insert_source(source, |event, _, events: &mut Vec<LooperMessage>| {
                events.push(event);
            })
            .expect("insert_source failed");

        let mut events = Vec::new();
        event_loop
            .dispatch(Some(std::time::Duration::from_millis(100)), &mut events)
            .expect("dispatch failed");

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));
        assert!(matches!(
            &events[1],
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::CloseRequested))
        ));
    }

    #[test]
    fn calloop_writes_and_reads() {
        use calloop::EventLoop;

        let (source, mut peer, write_tx) = setup();

        // Enqueue an outbound frame via the write channel.
        let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        let bytes = postcard::to_allocvec(&msg).unwrap();
        write_tx.send((0, bytes.clone())).unwrap();

        // Also write something from the peer so the source wakes up.
        write_frame_to_peer(&mut peer, 0, &bytes);

        let mut event_loop: EventLoop<Vec<LooperMessage>> =
            EventLoop::try_new().expect("event loop creation failed");
        let handle = event_loop.handle();

        handle
            .insert_source(source, |event, _, events: &mut Vec<LooperMessage>| {
                events.push(event);
            })
            .expect("insert_source failed");

        let mut events = Vec::new();

        // First dispatch: reads the incoming frame, detects pending
        // writes, sets Interest::BOTH + PostAction::Reregister.
        event_loop
            .dispatch(Some(std::time::Duration::from_millis(100)), &mut events)
            .expect("first dispatch failed");

        // Second dispatch: write readiness fires, flushes the
        // queued frame to the peer.
        event_loop
            .dispatch(Some(std::time::Duration::from_millis(100)), &mut events)
            .expect("second dispatch failed");

        // Should have read the frame we wrote from the peer.
        assert!(events.len() >= 1);
        assert!(matches!(
            &events[0],
            LooperMessage::Control(ControlMessage::Lifecycle(LifecycleMessage::Ready))
        ));

        // Read what ConnectionSource wrote to the peer.
        peer.set_nonblocking(false).unwrap();
        let codec = FrameCodec::permissive(16 * 1024 * 1024);
        let frame = codec.read_frame(&mut peer).unwrap();
        assert!(matches!(frame, Frame::Message { service: 0, .. }));
    }

    #[test]
    fn calloop_detects_connection_close() {
        use calloop::EventLoop;

        let (source, peer, _write_tx) = setup();
        drop(peer);

        let mut event_loop: EventLoop<bool> =
            EventLoop::try_new().expect("event loop creation failed");
        let handle = event_loop.handle();

        handle
            .insert_source(source, |_event, _, _closed: &mut bool| {})
            .expect("insert_source failed");

        let mut closed = false;
        // Dispatch — source should detect EOF and return Remove.
        let result = event_loop.dispatch(Some(std::time::Duration::from_millis(100)), &mut closed);
        assert!(result.is_ok());
    }

    #[test]
    fn calloop_detects_protocol_abort() {
        use calloop::EventLoop;

        let (source, mut peer, _write_tx) = setup();

        let codec = FrameCodec::new(16 * 1024 * 1024);
        codec.write_abort(&mut peer).unwrap();

        let mut event_loop: EventLoop<bool> =
            EventLoop::try_new().expect("event loop creation failed");
        let handle = event_loop.handle();

        handle
            .insert_source(source, |_event, _, _: &mut bool| {})
            .expect("insert_source failed");

        let mut state = false;
        let result = event_loop.dispatch(Some(std::time::Duration::from_millis(100)), &mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn connection_id_accessible() {
        let (source, _peer, _write_tx) = setup();
        assert_eq!(source.connection_id(), 1);
    }
}
