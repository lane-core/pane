//! Wire protocol framing for pane sessions.
//!
//! Frame format: `[length: u32 LE][service: u8][payload: postcard bytes]`
//!
//! The length field counts the service byte plus payload — it does not
//! include the 4-byte length prefix itself. Minimum valid length is 1
//! (service byte only, empty payload).
//!
//! Service 0 is the control protocol, always known from construction.
//! Service 0xFF is reserved for ProtocolAbort and cannot be registered
//! or sent via write_frame.
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
    Message { service: u8, payload: Vec<u8> },
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
    UnknownService(u8),
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
                write!(f, "unknown service discriminant: 0x{s:02X}")
            }
            FrameError::Transport(e) => write!(f, "transport error: {e}"),
            FrameError::TooShort { declared } => {
                write!(
                    f,
                    "frame too short: declared length {declared}, minimum is 1"
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
/// Service 0 (control) is always known. Service 0xFF is reserved for
/// ProtocolAbort and cannot be registered.
pub struct FrameCodec {
    max_message_size: u32,
    /// Indexed 0..=254. Index i corresponds to service i.
    /// 0xFF is not representable — it's handled as a special case.
    known_services: [bool; 255],
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
        let mut known_services = [false; 255];
        known_services[0] = true;
        FrameCodec {
            max_message_size,
            known_services,
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
            known_services: [true; 255],
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
    /// Panics if `service` is 0xFF (reserved for ProtocolAbort).
    pub fn register_service(&mut self, service: u8) {
        assert!(
            service != 0xFF,
            "service 0xFF is reserved for ProtocolAbort"
        );
        self.known_services[service as usize] = true;
    }

    /// Read one frame from the wire.
    ///
    /// Blocks until a complete frame is available or the transport fails.
    /// Returns `Frame::Abort` if the service byte is 0xFF.
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

        // Step 2: validate length
        if length < 1 {
            return Err(FrameError::TooShort { declared: length });
        }
        if length > self.max_message_size {
            return Err(FrameError::Oversized {
                declared: length,
                limit: self.max_message_size,
            });
        }

        // Step 3: read body (service byte + payload)
        let mut body = vec![0u8; length as usize];
        reader.read_exact(&mut body)?;

        let service = body[0];

        // Step 4: check for abort
        if service == 0xFF {
            return Ok(Frame::Abort);
        }

        // Step 5: validate service
        if !self.known_services[service as usize] {
            return Err(FrameError::UnknownService(service));
        }

        // Step 6: extract payload
        let payload = body[1..].to_vec();
        Ok(Frame::Message { service, payload })
    }

    /// Write a framed message to the wire.
    ///
    /// # Panics
    ///
    /// Panics if `service` is 0xFF (reserved for ProtocolAbort).
    pub fn write_frame(
        &self,
        writer: &mut impl Write,
        service: u8,
        payload: &[u8],
    ) -> io::Result<()> {
        assert!(
            service != 0xFF,
            "service 0xFF is reserved for ProtocolAbort"
        );

        let length = 1u32 + payload.len() as u32;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(&[service])?;
        writer.write_all(payload)?;
        Ok(())
    }

    /// Write a ProtocolAbort frame. Best-effort — does not panic on
    /// write failure.
    pub fn write_abort(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(&1u32.to_le_bytes())?;
        writer.write_all(&[0xFF])?;
        Ok(())
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
        assert_eq!(buf, vec![0x01, 0x00, 0x00, 0x00, 0xFF]);
    }

    // --- I11: ProtocolAbort checked at framing layer ---

    #[test]
    fn read_frame_detects_abort() {
        let codec = FrameCodec::new(1024);
        let data = vec![0x01, 0x00, 0x00, 0x00, 0xFF];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(frame, Frame::Abort);
    }

    #[test]
    fn abort_with_trailing_bytes_still_aborts() {
        let codec = FrameCodec::new(1024);
        // length=3, service=0xFF, two garbage bytes
        let data = vec![0x03, 0x00, 0x00, 0x00, 0xFF, 0xAA, 0xBB];
        let mut cursor = Cursor::new(data);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(frame, Frame::Abort);
    }

    #[test]
    #[should_panic(expected = "reserved for ProtocolAbort")]
    fn service_0xff_cannot_be_registered() {
        let mut codec = FrameCodec::new(1024);
        codec.register_service(0xFF);
    }

    #[test]
    #[should_panic(expected = "reserved for ProtocolAbort")]
    fn write_frame_rejects_service_0xff() {
        let codec = FrameCodec::new(1024);
        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 0xFF, b"hello").unwrap();
    }

    // --- I12: Unknown service discriminant ---

    #[test]
    fn unknown_service_is_connection_error() {
        let codec = FrameCodec::new(1024);
        // service=7, not registered
        let data = vec![0x03, 0x00, 0x00, 0x00, 0x07, 0xAA, 0xBB];
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
        // service=5, payload=[0x42]
        let data = vec![0x02, 0x00, 0x00, 0x00, 0x05, 0x42];
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
        // service=0, payload=[0x01, 0x02]
        let data = vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02];
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
        // No way to unregister; just verify it stays accepted.
        let data = vec![0x01, 0x00, 0x00, 0x00, 0x0A];
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
        let codec = FrameCodec::new(3);
        // length=3 (service + 2 payload bytes), limit=3
        let data = vec![0x03, 0x00, 0x00, 0x00, 0x00, 0xAA, 0xBB];
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
        // length=10, but only 3 body bytes follow
        let data = vec![0x0A, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02];
        let mut cursor = Cursor::new(data);
        let result = codec.read_frame(&mut cursor);
        match result {
            Err(FrameError::Transport(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {}
            other => panic!("expected Transport(UnexpectedEof), got {other:?}"),
        }
    }

    // --- Boundary and sequencing ---

    #[test]
    fn service_254_boundary() {
        // 0xFE is the maximum assignable service discriminant.
        // 0xFF is reserved for ProtocolAbort. This test verifies
        // the boundary: 254 works, 255 would be abort.
        let mut codec = FrameCodec::new(1024);
        codec.register_service(254);

        let payload = b"boundary";
        let mut buf = Vec::new();
        codec.write_frame(&mut buf, 254, payload).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = codec.read_frame(&mut cursor).unwrap();
        assert_eq!(
            frame,
            Frame::Message {
                service: 254,
                payload: payload.to_vec(),
            }
        );
    }

    #[test]
    fn payload_containing_0xff_is_not_abort() {
        // 0xFF in the payload must not trigger abort detection.
        // Abort is identified solely by the service byte (first
        // byte after the length prefix), not by payload contents.
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
}
