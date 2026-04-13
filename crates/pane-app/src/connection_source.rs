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

use std::cell::RefCell;
use std::io;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

use calloop::generic::Generic;
use calloop::{EventSource, Interest, Mode, Poll, PostAction, Readiness, Token, TokenFactory};

use pane_proto::control::ControlMessage;
use pane_session::bridge::{LooperMessage, WriteMessage};
use pane_session::frame::{Frame, FrameError, FrameReader, FrameWriter, WRITE_HIGHWATER_BYTES};

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

impl From<FrameError> for ConnectionError {
    fn from(e: FrameError) -> Self {
        match e {
            FrameError::Transport(io_err) => ConnectionError::Io(io_err),
            FrameError::Oversized { declared, limit } => ConnectionError::Protocol(format!(
                "frame too large: {declared} bytes (limit {limit})"
            )),
            FrameError::TooShort { declared } => ConnectionError::Protocol(format!(
                "frame too short: declared length {declared}, minimum is 2"
            )),
            FrameError::UnknownService(s) => {
                ConnectionError::Protocol(format!("unknown service discriminant: 0x{s:04X}"))
            }
            FrameError::Poisoned => {
                ConnectionError::Protocol("codec poisoned by prior error".into())
            }
        }
    }
}

/// Shared write queue for looper-thread direct writes (D12 Part 2).
///
/// Wraps `Rc<RefCell<FrameWriter>>` — single-threaded shared
/// mutability is correct because both enqueue (from handler dispatch)
/// and flush (from ConnectionSource::process_events) happen on the
/// looper thread (I6 single-thread guarantee). RefCell enforces this
/// at runtime.
///
/// Two write paths:
/// - **Looper-thread (90%):** `ServiceHandle::send_request` →
///   `DispatchCtx::enqueue_frame` → `SharedWriter::enqueue` →
///   `FrameWriter::enqueue` → fd. Direct, no synchronization.
/// - **Cross-thread (10%):** `send_and_wait` / external
///   `SubscriberSender` → mpsc → `ConnectionSource::drain_write_channel`
///   → `FrameWriter::enqueue` → fd. Bounded channel for genuinely
///   cross-thread sends.
///
/// Design heritage: Haiku BDirectMessageTarget bypassed the port for
/// same-process sends — message went directly to BMessageQueue
/// (unbounded linked list), port got a zero-byte poke to wake the
/// looper. Same principle: trust your own thread, bound your neighbors.
/// Plan 9 mountio wrote directly to the kernel pipe — one buffer, no
/// intermediate channels.
#[derive(Clone)]
pub struct SharedWriter {
    inner: Rc<RefCell<FrameWriter>>,
}

impl SharedWriter {
    fn new(writer: FrameWriter) -> Self {
        SharedWriter {
            inner: Rc::new(RefCell::new(writer)),
        }
    }

    /// Encode a frame into the shared contiguous write buffer.
    ///
    /// Called from handler dispatch on the looper thread. The frame
    /// will be flushed to the fd during the next
    /// `ConnectionSource::process_events` call.
    pub fn enqueue(&self, service: u16, payload: &[u8]) {
        self.inner.borrow_mut().enqueue(service, payload);
    }

    /// Whether the write queue has pending data.
    pub fn has_pending(&self) -> bool {
        !self.inner.borrow().is_empty()
    }
}

impl pane_session::NonBlockingSend for SharedWriter {
    /// Enqueue a frame into the shared write buffer.
    ///
    /// Always succeeds — `FrameWriter::enqueue` is a `Vec` append
    /// on the looper thread. The `FrameWriter::enqueue` debug_assert
    /// guards against service 0xFFFF (ProtocolAbort).
    fn try_send_frame(
        &self,
        service: u16,
        payload: &[u8],
    ) -> Result<(), pane_session::Backpressure> {
        self.enqueue(service, payload);
        Ok(())
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
    /// Shared contiguous write buffer (D12): accessible from both
    /// the EventSource (for flushing) and DispatchCtx (for direct
    /// looper-thread enqueue). The Rc<RefCell<FrameWriter>> is
    /// safe because both paths run on the looper thread (I6).
    shared_writer: SharedWriter,
    /// Receiver for outbound frames from cross-thread senders.
    /// Used by send_and_wait (non-looper threads) and external
    /// SubscriberSender. Looper-thread sends bypass this channel
    /// and write directly to shared_writer (D12 Part 2).
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
            shared_writer: SharedWriter::new(FrameWriter::new()),
            write_rx,
            write_interest: false,
            connection_id,
            registered_token: None,
        })
    }

    /// Get the shared writer for looper-thread direct writes (D12).
    ///
    /// The returned SharedWriter is `Rc`-based (not Send) — it can
    /// only be used from the looper thread. This is by design:
    /// looper-thread sends bypass the mpsc channel and write directly
    /// to the shared queue.
    pub fn shared_writer(&self) -> SharedWriter {
        self.shared_writer.clone()
    }

    /// Drain pending messages from the write channel into the
    /// contiguous write buffer. Non-blocking: uses try_recv.
    ///
    /// The write buffer has a byte-based highwater cap
    /// (`WRITE_HIGHWATER_BYTES`). When unflushed bytes exceed this
    /// cap, no more messages are pulled from the channel. This
    /// preserves backpressure: socket-level WouldBlock keeps the
    /// buffer full, which keeps the bounded mpsc full, which stalls
    /// the SyncSender. Without this cap, the buffer would absorb
    /// unbounded data and the sender would never block.
    fn drain_write_channel(&mut self) {
        // Don't drain if the buffer is already above highwater.
        // The queued data hasn't been flushed to the socket yet
        // (WouldBlock), so pulling more from the channel just moves
        // the buffer from bounded (mpsc) to unbounded (Vec).
        {
            let writer = self.shared_writer.inner.borrow();
            if writer.pending_bytes() >= WRITE_HIGHWATER_BYTES {
                return;
            }
        }

        loop {
            {
                let writer = self.shared_writer.inner.borrow();
                if writer.pending_bytes() >= WRITE_HIGHWATER_BYTES {
                    break;
                }
            }
            match self.write_rx.try_recv() {
                Ok((service, payload)) => {
                    self.shared_writer.enqueue(service, &payload);
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
                    Err(FrameError::Transport(ref e))
                        if e.kind() == io::ErrorKind::UnexpectedEof =>
                    {
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
            match self.shared_writer.inner.borrow_mut().try_flush(stream) {
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

        // Register write interest when data is pending. Haiku's
        // LinkSender used a watermark (kWatermark = 2048) to defer
        // write interest below a threshold; pane currently registers
        // on any pending bytes and may adopt watermark-gated
        // registration as a future optimization.
        if self.shared_writer.inner.borrow().pending_bytes() > 0 && !self.write_interest {
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
        if !self.shared_writer.has_pending() {
            if let Ok((service, payload)) = self.write_rx.try_recv() {
                self.shared_writer.enqueue(service, &payload);
            }
        }

        if self.shared_writer.has_pending() {
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

    // ── Highwater cap ─────────────────────────────────────────

    #[test]
    fn frame_writer_highwater_cap_stops_drain() {
        // Verify that drain_write_channel stops pulling from the
        // mpsc when the buffer exceeds WRITE_HIGHWATER_BYTES.
        let (stream, _peer) = UnixStream::pair().unwrap();
        let (write_tx, write_rx) = mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
        let mut source = ConnectionSource::new(stream, 16 * 1024 * 1024, write_rx, 1).unwrap();

        // Send enough 1KB frames to exceed 16KB highwater.
        let payload = vec![0u8; 1024];
        for _ in 0..20 {
            write_tx.send((1, payload.clone())).unwrap();
        }

        // First drain: pulls frames until highwater.
        source.drain_write_channel();
        let pending = source.shared_writer.inner.borrow().pending_bytes();
        assert!(
            pending >= WRITE_HIGHWATER_BYTES,
            "should have reached highwater: {pending}"
        );

        // Channel should still have remaining frames.
        // (20 frames × ~1030 wire bytes each ≈ 20600, highwater is 16384)
        let remaining = write_tx.try_send((1, payload));
        assert!(
            remaining.is_ok(),
            "channel should still have capacity — not all frames were drained"
        );
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
        assert!(source.shared_writer.has_pending());

        let stream = unsafe { source.fd.get_mut() };
        let drained = source
            .shared_writer
            .inner
            .borrow_mut()
            .try_flush(stream)
            .unwrap();
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
        let drained = source
            .shared_writer
            .inner
            .borrow_mut()
            .try_flush(stream)
            .unwrap();
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
        assert!(matches!(result, Err(FrameError::Transport(_))));
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

    // ── NonBlockingSend ──────────────────────────────────────

    #[test]
    fn shared_writer_non_blocking_send_enqueues_frame() {
        use pane_session::NonBlockingSend;

        let writer = SharedWriter::new(pane_session::FrameWriter::new());
        let result = writer.try_send_frame(1, &[0xAA, 0xBB]);
        assert!(result.is_ok());

        // Verify the frame is in the buffer by checking pending bytes.
        // Wire format: 4 (length) + 2 (service) + 2 (payload) = 8 bytes.
        assert_eq!(writer.inner.borrow().pending_bytes(), 8);
        assert!(writer.has_pending());
    }

    #[test]
    fn shared_writer_non_blocking_send_multiple_frames() {
        use pane_session::NonBlockingSend;

        let writer = SharedWriter::new(pane_session::FrameWriter::new());
        writer.try_send_frame(1, &[0x01]).unwrap();
        writer.try_send_frame(2, &[0x02, 0x03]).unwrap();

        // Frame 1: 4 + 2 + 1 = 7. Frame 2: 4 + 2 + 2 = 8. Total: 15.
        assert_eq!(writer.inner.borrow().pending_bytes(), 15);
    }
}
