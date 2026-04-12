//! Wire protocol framing for pane sessions.
//!
//! Frame format: `[length: u32 LE][service: u16 LE][payload: postcard bytes]`
//!
//! The length field counts the service field (2 bytes) plus payload —
//! it does not include the 4-byte length prefix itself. Minimum valid
//! length is 2 (service field only, empty payload).
//!
//! Service 0 is the control protocol, always known from construction.
//! Service 0xFFFF is reserved for ProtocolAbort and cannot be
//! registered or sent via write_frame.
//!
//! Design heritage: Plan 9 9P framing: [size: u32][type: u8][tag: u16]
//! (intro(5), reference/plan9/man/5/0intro:91-100) — similar
//! structure, type byte discriminates message kinds. BeOS
//! LinkSender (headers/private/app/LinkSender.h:36-40) used
//! StartMessage(code)/Attach/EndMessage/Flush batched protocol
//! over kernel ports (headers/os/kernel/OS.h:133) — compact binary
//! where both sides agree on the schema. pane's framing follows
//! the same principle: no self-describing format, postcard + Rust
//! types are the schema.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// A decoded frame from the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// A normal message carrying a service discriminant and payload.
    Message { service: u16, payload: Vec<u8> },
    /// ProtocolAbort — the peer is tearing down the connection.
    Abort,
}

/// Errors from reading a frame off the wire.
///
/// These are connection-level errors. After any FrameError, the
/// connection must be considered dead — the stream may be desynced
/// (Oversized leaves body bytes unconsumed, so subsequent reads
/// interpret body as the next length prefix).
///
/// The codec self-poisons on any read error: all subsequent
/// read_frame calls return Poisoned without touching the stream.
#[derive(Debug)]
pub enum FrameError {
    /// Declared length exceeds the negotiated maximum.
    Oversized { declared: u32, limit: u32 },
    /// Service discriminant not registered with the codec.
    UnknownService(u16),
    /// Underlying transport failed (includes EOF).
    Transport(io::Error),
    /// Declared length is zero — no room for even the service byte.
    TooShort { declared: u32 },
    /// Codec poisoned by a prior error. No further reads possible.
    Poisoned,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::Oversized { declared, limit } => {
                write!(f, "frame too large: {declared} bytes (limit {limit})")
            }
            FrameError::UnknownService(s) => {
                write!(f, "unknown service discriminant: 0x{s:04X}")
            }
            FrameError::Transport(e) => write!(f, "transport error: {e}"),
            FrameError::TooShort { declared } => {
                write!(
                    f,
                    "frame too short: declared length {declared}, minimum is 2"
                )
            }
            FrameError::Poisoned => write!(f, "codec poisoned by prior error"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FrameError::Transport(e) => Some(e),
            FrameError::Oversized { .. }
            | FrameError::UnknownService(_)
            | FrameError::TooShort { .. }
            | FrameError::Poisoned => None,
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Transport(e)
    }
}

/// Length-prefixed frame codec with service validation.
///
/// Tracks which service discriminants are valid for this connection.
/// Service 0 (control) is always known. Service 0xFFFF is reserved
/// for ProtocolAbort and cannot be registered.
pub struct FrameCodec {
    max_message_size: u32,
    /// Services registered as valid. In permissive mode this is
    /// ignored — the `permissive` flag bypasses the check.
    known_services: std::collections::HashSet<u16>,
    /// When true, all service discriminants (except 0xFFFF) are
    /// accepted. Used server-side where session_ids are allocated
    /// dynamically by DeclareInterest.
    permissive: bool,
    /// Set on any read error. All subsequent read_frame calls
    /// return Poisoned without touching the stream. AtomicBool
    /// because the codec is shared via Arc between reader and
    /// writer threads (writer doesn't check this — writes can
    /// still go out on a poisoned-read connection).
    poisoned: AtomicBool,
}

impl FrameCodec {
    /// Create a new codec with the given maximum message size.
    ///
    /// Service 0 (control) is registered from construction.
    /// Client-side: only registered services are accepted.
    pub fn new(max_message_size: u32) -> Self {
        let mut known_services = std::collections::HashSet::new();
        known_services.insert(0);
        FrameCodec {
            max_message_size,
            known_services,
            permissive: false,
            poisoned: AtomicBool::new(false),
        }
    }

    /// Create a permissive codec that accepts all service discriminants.
    ///
    /// Used by the server, which validates frames against its routing
    /// table rather than a static service set. Session_ids are allocated
    /// dynamically by DeclareInterest — the codec can't know them in
    /// advance because it's behind Arc (no interior mutability).
    ///
    /// I12 (unknown discriminant → connection error) still holds for
    /// client-side codecs. The server-side equivalent is: unknown
    /// route → frame silently dropped (constraint 5, Cancel/Reply race).
    pub fn permissive(max_message_size: u32) -> Self {
        FrameCodec {
            max_message_size,
            known_services: std::collections::HashSet::new(),
            permissive: true,
            poisoned: AtomicBool::new(false),
        }
    }

    /// Update the maximum message size after negotiation.
    ///
    /// Called after handshake when the Welcome carries the agreed
    /// max_message_size.
    pub fn set_max_message_size(&mut self, max: u32) {
        self.max_message_size = max;
    }

    /// Register a service discriminant as valid for this connection.
    ///
    /// # Panics
    ///
    /// Panics if `service` is 0xFFFF (reserved for ProtocolAbort).
    pub fn register_service(&mut self, service: u16) {
        assert!(
            service != 0xFFFF,
            "service 0xFFFF is reserved for ProtocolAbort"
        );
        self.known_services.insert(service);
    }

    /// Read one frame from the wire.
    ///
    /// Blocks until a complete frame is available or the transport fails.
    /// Returns `Frame::Abort` if the service field is 0xFFFF.
    pub fn read_frame(&self, reader: &mut impl Read) -> Result<Frame, FrameError> {
        if self.poisoned.load(Ordering::Relaxed) {
            return Err(FrameError::Poisoned);
        }

        let result = self.read_frame_inner(reader);
        if result.is_err() {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        result
    }

    fn read_frame_inner(&self, reader: &mut impl Read) -> Result<Frame, FrameError> {
        // Step 1: read length prefix
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let length = u32::from_le_bytes(len_buf);

        // Step 2: validate length (minimum 2 for the service field)
        if length < 2 {
            return Err(FrameError::TooShort { declared: length });
        }
        if length > self.max_message_size {
            return Err(FrameError::Oversized {
                declared: length,
                limit: self.max_message_size,
            });
        }

        // Step 3: read body (service u16 LE + payload)
        let mut body = vec![0u8; length as usize];
        reader.read_exact(&mut body)?;

        let service = u16::from_le_bytes([body[0], body[1]]);

        // Step 4: check for abort
        if service == 0xFFFF {
            return Ok(Frame::Abort);
        }

        // Step 5: validate service
        if !self.permissive && !self.known_services.contains(&service) {
            return Err(FrameError::UnknownService(service));
        }

        // Step 6: extract payload
        let payload = body[2..].to_vec();
        Ok(Frame::Message { service, payload })
    }

    /// Write a framed message to the wire.
    ///
    /// # Panics
    ///
    /// Panics if `service` is 0xFFFF (reserved for ProtocolAbort).
    pub fn write_frame(
        &self,
        writer: &mut impl Write,
        service: u16,
        payload: &[u8],
    ) -> io::Result<()> {
        assert!(
            service != 0xFFFF,
            "service 0xFFFF is reserved for ProtocolAbort"
        );

        let length = 2u32 + payload.len() as u32;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(&service.to_le_bytes())?;
        writer.write_all(payload)?;
        Ok(())
    }

    /// Encode a frame into a byte buffer without writing to a stream.
    ///
    /// Used by the server's per-connection writer threads (D12):
    /// the actor thread encodes frames, enqueues the bytes, and the
    /// writer thread writes them to the fd.
    ///
    /// # Panics
    ///
    /// Panics if `service` is 0xFFFF (reserved for ProtocolAbort).
    pub fn encode_frame(&self, service: u16, payload: &[u8]) -> Vec<u8> {
        assert!(
            service != 0xFFFF,
            "service 0xFFFF is reserved for ProtocolAbort"
        );

        let length = 2u32 + payload.len() as u32;
        let mut buf = Vec::with_capacity(4 + length as usize);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&service.to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    /// Write a ProtocolAbort frame. Best-effort — does not panic on
    /// write failure.
    pub fn write_abort(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(&2u32.to_le_bytes())?;
        writer.write_all(&0xFFFFu16.to_le_bytes())?;
        Ok(())
    }
}

// ── Non-blocking frame I/O ────────────────────────────────────
//
// FrameReader and FrameWriter are the non-blocking equivalents of
// FrameCodec. FrameCodec blocks on read_exact/write_all — correct
// for bridge and server threads. FrameReader/FrameWriter handle
// WouldBlock at the byte level, accumulating partial reads/writes
// across poll iterations — correct for calloop EventSources.
//
// Design heritage: same wire format, same validation. The split is
// purely about I/O model, not protocol semantics.

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

/// Parse state for incremental frame reading.
#[derive(Clone, Copy)]
enum ReadState {
    /// Reading the 4-byte length prefix.
    Length,
    /// Reading the body (service + payload). The field holds the
    /// declared body length from the length prefix.
    Body { body_len: u32 },
}

/// State machine for incremental frame decoding from a non-blocking
/// source. Handles WouldBlock at the byte level, accumulating partial
/// reads across poll iterations.
///
/// Frame wire format: `[length: u32 LE][service: u16 LE][payload]`
/// Length counts service + payload (minimum 2).
pub struct FrameReader {
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

impl FrameReader {
    /// Create a new non-blocking frame reader.
    ///
    /// `max_message_size` is the negotiated limit from the Welcome.
    /// When `permissive` is true, all service discriminants (except
    /// 0xFFFF) are accepted — used server-side where session_ids are
    /// dynamically allocated by DeclareInterest.
    pub fn new(max_message_size: u32, permissive: bool) -> Self {
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
    /// - `Err(FrameError)` — fatal error (EOF, protocol violation)
    ///
    /// After any error, the connection must be considered dead — the
    /// internal state may be desynced.
    pub fn try_read_frame(&mut self, source: &mut impl Read) -> Result<Option<Frame>, FrameError> {
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
                                return Err(FrameError::Transport(io::Error::new(
                                    io::ErrorKind::UnexpectedEof,
                                    "connection closed",
                                )));
                            }
                            // Partial length prefix — truncated stream.
                            return Err(FrameError::Transport(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "connection closed mid-frame (partial length prefix)",
                            )));
                        }
                        ReadProgress::Error(e) => return Err(FrameError::Transport(e)),
                    }

                    let length =
                        u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]);

                    // Validate length.
                    if length < 2 {
                        return Err(FrameError::TooShort { declared: length });
                    }
                    if length > self.max_message_size {
                        return Err(FrameError::Oversized {
                            declared: length,
                            limit: self.max_message_size,
                        });
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
                            return Err(FrameError::Transport(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "connection closed mid-frame (partial body)",
                            )));
                        }
                        ReadProgress::Error(e) => return Err(FrameError::Transport(e)),
                    }

                    let service = u16::from_le_bytes([self.buf[0], self.buf[1]]);
                    let frame = if service == 0xFFFF {
                        Frame::Abort
                    } else if !self.permissive && service != 0 {
                        return Err(FrameError::UnknownService(service));
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

/// Contiguous write buffer for non-blocking frame output.
///
/// Frames are encoded directly into a single `Vec<u8>` (length
/// prefix + service + payload as contiguous bytes). Flush writes
/// the entire pending region in one `write()` syscall, reducing
/// syscall overhead compared to per-frame writes.
///
/// Haiku precedent: LinkSender used a contiguous buffer with a
/// kWatermark flush trigger (src/kits/app/LinkSender.cpp). pane
/// adopts the contiguous buffer layout; watermark-gated write
/// interest registration is a future optimization (currently,
/// write interest is registered whenever any bytes are pending).
pub struct FrameWriter {
    /// Contiguous wire bytes for all pending frames.
    buf: Vec<u8>,
    /// How far into buf we've flushed. Bytes in `buf[..flush_pos]`
    /// have been written to the fd; bytes in `buf[flush_pos..]` are
    /// pending. When fully flushed, both buf and flush_pos reset to
    /// zero.
    flush_pos: usize,
}

/// Byte-based highwater cap for the write buffer. When the buffer
/// holds this many unflushed bytes, callers should stop pulling
/// from upstream channels. This preserves backpressure: socket-level
/// WouldBlock keeps the buffer full, which keeps bounded channels
/// full, which stalls cross-thread senders. Without this cap, the
/// buffer would absorb unbounded data and the sender would never
/// block.
///
/// 16KB is ~4 max-sized control frames or ~16 typical 1KB messages —
/// enough for batch efficiency without absorbing the entire channel
/// capacity.
pub const WRITE_HIGHWATER_BYTES: usize = 16 * 1024;

impl FrameWriter {
    /// Create a new empty write buffer.
    pub fn new() -> Self {
        FrameWriter {
            buf: Vec::new(),
            flush_pos: 0,
        }
    }

    /// Encode a frame directly into the contiguous buffer.
    ///
    /// Wire format: [length: u32 LE][service: u16 LE][payload]
    /// where length = 2 + payload.len() (counts service + payload).
    pub fn enqueue(&mut self, service: u16, payload: &[u8]) {
        let frame_len = 2 + payload.len();
        self.buf
            .extend_from_slice(&(frame_len as u32).to_le_bytes());
        self.buf.extend_from_slice(&service.to_le_bytes());
        self.buf.extend_from_slice(payload);
    }

    /// Whether the buffer has no pending data.
    pub fn is_empty(&self) -> bool {
        self.flush_pos >= self.buf.len()
    }

    /// Number of unflushed bytes in the buffer.
    pub fn pending_bytes(&self) -> usize {
        self.buf.len() - self.flush_pos
    }

    /// Drain the buffer to the sink in as few write() calls as
    /// possible.
    ///
    /// Returns:
    /// - `Ok(true)` — buffer fully drained
    /// - `Ok(false)` — WouldBlock, more to write later
    /// - `Err(e)` — fatal write error
    pub fn try_flush(&mut self, sink: &mut impl Write) -> Result<bool, io::Error> {
        while self.flush_pos < self.buf.len() {
            match sink.write(&self.buf[self.flush_pos..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write returned 0 bytes",
                    ));
                }
                Ok(n) => self.flush_pos += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        // All flushed — reclaim buffer memory.
        self.buf.clear();
        self.flush_pos = 0;
        Ok(true)
    }
}

impl Default for FrameWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // --- I10: ProtocolAbort must not block ---

    /// I10 says ProtocolAbort "must not block." That's a caller-site
    /// obligation — the framing layer provides a fallible write, and
    /// the caller does `let _ = codec.write_abort(...)`. This test
    /// verifies the framing layer propagates the error so the caller
    /// can discard it.
    #[test]
    fn abort_write_propagates_error() {
        // A writer that always fails.
        struct FailWriter;
        impl Write for FailWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken"))
            }
        }

        let codec = FrameCodec::new(1024);
        let result = codec.write_abort(&mut FailWriter);
        assert!(result.is_err());
    }

    #[test]
    fn abort_write_format() {
        let codec = FrameCodec::new(1024);
        let mut buf = Vec::new();
        codec.write_abort(&mut buf).unwrap();
        assert_eq!(buf, vec![0x02, 0x00, 0x00, 0x00, 0xFF, 0xFF]);
    }

    // --- I11: ProtocolAbort checked at framing layer ---

    #[test]
    fn read_frame_detects_abort() {
        let codec = FrameCodec::new(1024);
        // length=2, service=0xFFFF LE
        let data = vec![0x02, 0x00, 0x00, 0x00, 0xFF, 0xFF];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(frame, Frame::Abort);
    }

    #[test]
    fn abort_with_trailing_bytes_still_aborts() {
        let codec = FrameCodec::new(1024);
        // length=4, service=0xFFFF, two garbage bytes
        let data = vec![0x04, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xAA, 0xBB];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(frame, Frame::Abort);
    }

    #[test]
    #[should_panic(expected = "reserved for ProtocolAbort")]
    fn service_0xffff_cannot_be_registered() {
        let mut codec = FrameCodec::new(1024);
        codec.register_service(0xFFFF);
    }

    #[test]
    #[should_panic(expected = "reserved for ProtocolAbort")]
    fn write_frame_rejects_service_0xffff() {
        let codec = FrameCodec::new(1024);
        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 0xFFFF, b"hello").unwrap();
    }

    // --- I12: Unknown service discriminant ---

    #[test]
    fn unknown_service_is_connection_error() {
        let codec = FrameCodec::new(1024);
        // service=7 (u16 LE: 0x07,0x00), payload=[0xAA]
        let data = vec![0x03, 0x00, 0x00, 0x00, 0x07, 0x00, 0xAA];
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::UnknownService(7)) => {}
            other => panic!("expected UnknownService(7), got {other:?}"),
        }
    }

    #[test]
    fn known_service_accepted() {
        let mut codec = FrameCodec::new(1024);
        codec.register_service(5);
        // service=5 (u16 LE: 0x05,0x00), payload=[0x42]
        let data = vec![0x03, 0x00, 0x00, 0x00, 0x05, 0x00, 0x42];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 5,
                payload: vec![0x42],
            }
        );
    }

    #[test]
    fn control_service_always_known() {
        let codec = FrameCodec::new(1024);
        // service=0 (u16 LE: 0x00,0x00), payload=[0x01, 0x02]
        let data = vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 0,
                payload: vec![0x01, 0x02],
            }
        );
    }

    #[test]
    fn revoked_service_still_known_at_framing_layer() {
        // Service registration is monotonic — no unregister method exists.
        // This test documents that once registered, a service stays known.
        let mut codec = FrameCodec::new(1024);
        codec.register_service(10);
        // service=10 (u16 LE: 0x0A,0x00), empty payload
        let data = vec![0x02, 0x00, 0x00, 0x00, 0x0A, 0x00];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 10,
                payload: vec![],
            }
        );
    }

    // --- Max message size ---

    #[test]
    fn oversized_frame_rejected() {
        let codec = FrameCodec::new(100);
        // length=101, exceeds limit of 100
        let length_bytes = 101u32.to_le_bytes();
        let mut data = Vec::new();
        data.extend_from_slice(&length_bytes);
        // Don't need the body — error triggers before read
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::Oversized {
                declared: 101,
                limit: 100,
            }) => {}
            other => panic!("expected Oversized, got {other:?}"),
        }
    }

    #[test]
    fn frame_at_exact_limit_accepted() {
        let codec = FrameCodec::new(4);
        // length=4 (2-byte service + 2 payload bytes), limit=4
        let data = vec![0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAA, 0xBB];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 0,
                payload: vec![0xAA, 0xBB],
            }
        );
    }

    #[test]
    fn zero_length_frame_rejected() {
        let codec = FrameCodec::new(1024);
        let data = vec![0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::TooShort { declared: 0 }) => {}
            other => panic!("expected TooShort, got {other:?}"),
        }
    }

    #[test]
    fn length_one_frame_rejected() {
        // length=1 is too short for the 2-byte service field
        let codec = FrameCodec::new(1024);
        let data = vec![0x01, 0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::TooShort { declared: 1 }) => {}
            other => panic!("expected TooShort, got {other:?}"),
        }
    }

    // --- Roundtrip ---

    #[test]
    fn encode_decode_roundtrip() {
        let mut codec = FrameCodec::new(1024);
        codec.register_service(42);

        let payload = b"hello, pane";
        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 42, payload).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 42,
                payload: payload.to_vec(),
            }
        );
    }

    #[test]
    fn empty_payload_roundtrip() {
        let codec = FrameCodec::new(1024);

        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 0, &[]).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 0,
                payload: vec![],
            }
        );
    }

    // --- Transport errors ---

    #[test]
    fn eof_during_length_is_transport_error() {
        let codec = FrameCodec::new(1024);
        // Only 2 bytes — not enough for a u32 length
        let data = vec![0x01, 0x00];
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::Transport(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {}
            other => panic!("expected Transport(UnexpectedEof), got {other:?}"),
        }
    }

    #[test]
    fn eof_during_body_is_transport_error() {
        let codec = FrameCodec::new(1024);
        // length=10, but only 4 body bytes follow (need 10)
        let data = vec![0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02];
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::Transport(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {}
            other => panic!("expected Transport(UnexpectedEof), got {other:?}"),
        }
    }

    // --- Boundary and sequencing ---

    #[test]
    fn service_65534_boundary() {
        // 0xFFFE is the maximum assignable service discriminant.
        // 0xFFFF is reserved for ProtocolAbort. This test verifies
        // the boundary: 65534 works, 65535 would be abort.
        let mut codec = FrameCodec::new(1024);
        codec.register_service(0xFFFE);

        let payload = b"boundary";
        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 0xFFFE, payload).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 0xFFFE,
                payload: payload.to_vec(),
            }
        );
    }

    #[test]
    fn payload_containing_0xffff_is_not_abort() {
        // 0xFF bytes in the payload must not trigger abort detection.
        // Abort is identified solely by the service field (first two
        // bytes after the length prefix), not by payload contents.
        let codec = FrameCodec::new(1024);

        let payload = vec![0xFF, 0xFF, 0xFF];
        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 0, &payload).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 0,
                payload: vec![0xFF, 0xFF, 0xFF],
            }
        );
    }

    #[test]
    fn multi_frame_sequencing() {
        // Two frames written into one buffer must be readable
        // sequentially — the cursor advances correctly past each
        // frame boundary.
        let mut codec = FrameCodec::new(1024);
        codec.register_service(1);
        codec.register_service(2);

        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 1, b"first").unwrap();
        codec.write_frame(&mut buf, 2, b"second").unwrap();

        let mut cursor = Cursor::new(buf);

        let frame1 = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame1,
            Frame::Message {
                service: 1,
                payload: b"first".to_vec(),
            }
        );

        let frame2 = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame2,
            Frame::Message {
                service: 2,
                payload: b"second".to_vec(),
            }
        );

        // Cursor should be exhausted — next read hits EOF.
        let result = codec.read_frame(&mut cursor);
        assert!(matches!(result, Err(FrameError::Transport(_))));
    }

    // ── FrameReader unit tests ──────────���──────────────────────

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

    #[test]
    fn frame_reader_decodes_single_frame() {
        let mut reader = FrameReader::new(16 * 1024 * 1024, true);

        let wire = encode_frame(5, &[0xAA, 0xBB]);
        let mut cursor = Cursor::new(wire);

        let frame = reader.try_read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 5,
                payload: vec![0xAA, 0xBB],
            }
        );
    }

    #[test]
    fn frame_reader_decodes_multiple_frames() {
        let mut reader = FrameReader::new(16 * 1024 * 1024, true);

        let mut wire = Vec::new();
        wire.extend_from_slice(&encode_frame(0, &[0x01]));
        wire.extend_from_slice(&encode_frame(7, &[0x02, 0x03]));

        let mut cursor = Cursor::new(wire);
        let mut frames = Vec::new();
        loop {
            match reader.try_read_frame(&mut cursor) {
                Ok(Some(frame)) => frames.push(frame),
                // Cursor returns EOF (not WouldBlock) when exhausted.
                Ok(None) => break,
                Err(_) => break,
            }
        }

        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0],
            Frame::Message {
                service: 0,
                payload: vec![0x01],
            }
        );
        assert_eq!(
            frames[1],
            Frame::Message {
                service: 7,
                payload: vec![0x02, 0x03],
            }
        );
    }

    #[test]
    fn frame_reader_detects_abort() {
        let mut reader = FrameReader::new(1024, true);

        // ProtocolAbort: length=2, service=0xFFFF.
        let wire = vec![0x02, 0x00, 0x00, 0x00, 0xFF, 0xFF];
        let mut cursor = Cursor::new(wire);

        let frame = reader.try_read_frame(&mut cursor).unwrap().unwrap();
        assert!(matches!(frame, Frame::Abort));
    }

    #[test]
    fn frame_reader_oversized_frame_rejected() {
        let mut reader = FrameReader::new(100, true);

        let data = 101u32.to_le_bytes();
        let mut cursor = Cursor::new(data.to_vec());

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(
            result,
            Err(FrameError::Oversized {
                declared: 101,
                limit: 100,
            })
        ));
    }

    #[test]
    fn frame_reader_too_short_rejected() {
        let mut reader = FrameReader::new(1024, true);

        let data = 1u32.to_le_bytes();
        let mut cursor = Cursor::new(data.to_vec());

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(FrameError::TooShort { declared: 1 })));
    }

    #[test]
    fn frame_reader_zero_length_rejected() {
        let mut reader = FrameReader::new(1024, true);

        let data = 0u32.to_le_bytes();
        let mut cursor = Cursor::new(data.to_vec());

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(FrameError::TooShort { declared: 0 })));
    }

    #[test]
    fn frame_reader_eof_at_frame_boundary_is_error() {
        let mut reader = FrameReader::new(1024, true);

        // Empty input — EOF before any bytes.
        let mut cursor = Cursor::new(Vec::new());
        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(FrameError::Transport(_))));
    }

    #[test]
    fn frame_reader_eof_mid_body_is_error() {
        let mut reader = FrameReader::new(1024, true);

        // Length says 10 bytes, but only 3 body bytes follow.
        let mut wire = Vec::new();
        wire.extend_from_slice(&10u32.to_le_bytes());
        wire.extend_from_slice(&[0x00, 0x00, 0xAA]); // 3 of 10
        let mut cursor = Cursor::new(wire);

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(FrameError::Transport(_))));
    }

    #[test]
    fn frame_reader_unknown_service_non_permissive() {
        // Non-permissive mode rejects unknown service discriminants.
        let mut reader = FrameReader::new(1024, false);

        let wire = encode_frame(7, &[0xAA]);
        let mut cursor = Cursor::new(wire);

        let result = reader.try_read_frame(&mut cursor);
        assert!(matches!(result, Err(FrameError::UnknownService(7))));
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
        impl io::Write for SlowWriter {
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

    #[test]
    fn frame_writer_contiguous_buffer_layout() {
        // Multiple enqueues produce one contiguous buffer — no
        // per-frame allocation boundaries visible in the internal
        // state.
        let mut writer = FrameWriter::new();
        writer.enqueue(1, &[0xAA]);
        writer.enqueue(2, &[0xBB, 0xCC]);

        // Internal buffer contains both frames contiguously.
        let expected = vec![
            0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0xAA, // frame 1
            0x04, 0x00, 0x00, 0x00, 0x02, 0x00, 0xBB, 0xCC, // frame 2
        ];
        assert_eq!(writer.buf, expected);
        assert_eq!(writer.flush_pos, 0);
        assert_eq!(writer.pending_bytes(), expected.len());
    }

    #[test]
    fn frame_writer_partial_flush_resumes_on_wouldblock() {
        // Simulate a sink that accepts a few bytes then returns
        // WouldBlock. After the first partial flush, enqueue more
        // data, then resume flushing. The buffer state must be
        // consistent across the resume.
        let mut writer = FrameWriter::new();
        writer.enqueue(0, &[0x01, 0x02]); // 8 bytes wire

        // Sink that accepts 3 bytes then WouldBlock.
        struct WouldBlockAfter {
            buf: Vec<u8>,
            remaining: usize,
        }
        impl io::Write for WouldBlockAfter {
            fn write(&mut self, data: &[u8]) -> io::Result<usize> {
                if self.remaining == 0 {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                let n = data.len().min(self.remaining);
                self.buf.extend_from_slice(&data[..n]);
                self.remaining -= n;
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut sink = WouldBlockAfter {
            buf: Vec::new(),
            remaining: 3,
        };

        // First flush: writes 3 bytes, then WouldBlock.
        let drained = writer.try_flush(&mut sink).unwrap();
        assert!(!drained);
        assert_eq!(writer.flush_pos, 3);
        assert_eq!(writer.pending_bytes(), 5); // 8 - 3

        // Enqueue a second frame while partially flushed.
        writer.enqueue(1, &[0xAA]);

        // Resume flush with fresh capacity.
        sink.remaining = 100;
        let drained = writer.try_flush(&mut sink).unwrap();
        assert!(drained);

        // Sink received all bytes from both frames.
        let expected = vec![
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, // frame 1
            0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0xAA, // frame 2
        ];
        assert_eq!(sink.buf, expected);
        assert_eq!(writer.pending_bytes(), 0);
        assert!(writer.is_empty());
    }

    #[test]
    fn frame_writer_pending_bytes_tracks_correctly() {
        let mut writer = FrameWriter::new();
        assert_eq!(writer.pending_bytes(), 0);
        assert!(writer.is_empty());

        // Enqueue: 4 (length) + 2 (service) + 3 (payload) = 9 bytes
        writer.enqueue(0, &[0x01, 0x02, 0x03]);
        assert_eq!(writer.pending_bytes(), 9);
        assert!(!writer.is_empty());

        // Enqueue another: 4 + 2 + 1 = 7 bytes
        writer.enqueue(1, &[0xFF]);
        assert_eq!(writer.pending_bytes(), 16);

        // Flush all.
        let mut buf = Vec::new();
        writer.try_flush(&mut buf).unwrap();
        assert_eq!(writer.pending_bytes(), 0);
        assert!(writer.is_empty());
    }
}
