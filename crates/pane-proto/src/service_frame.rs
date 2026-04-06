//! Wire envelope for service-channel messages.
//!
//! ServiceFrame wraps the inner payload bytes for services 1..=254.
//! Type safety lives at the edges: the sender serializes P::Message
//! into the payload, the receiver deserializes. The frame itself is
//! untyped — postcard bytes in, postcard bytes out.
//!
//! Service 0 (Control) uses ControlMessage directly, not ServiceFrame.
//! Service 0xFF is reserved for ProtocolAbort (see FrameCodec).
//!
//! Design heritage: Plan 9 9P carried tag[2] inside the message body
//! for request/reply correlation (intro(5),
//! reference/plan9/man/5/0intro:307-324). BeOS BMessage carried
//! reply_port/reply_target/reply_team inside the message for return
//! routing (headers/private/app/MessagePrivate.h:66-68).
//! ServiceFrame's token serves both purposes — correlation and return
//! routing — but lives in a framework envelope the developer never
//! sees. The developer works with typed P::Message; the framework
//! wraps and unwraps ServiceFrame transparently.
//!
//! In duploid terms (MMM25): all four variants are **positive** —
//! they are serialized values on the wire. Polarity crossings
//! happen at dispatch (LooperCore), not at framing. The Request
//! is a positive value offered to the receiver; the Reply is a
//! positive value satisfying a prior demand. The negative types
//! (Handles<P> handlers, ReplyPort continuations) live at the
//! endpoints, not in the frame.

use serde::{Serialize, Deserialize};

/// Wire envelope for service-channel messages (services 1..=254).
///
/// Untyped — inner payload is postcard bytes. Type safety at the
/// edges: sender serializes P::Message, receiver deserializes.
///
/// `#[non_exhaustive]` — Phase 3 streaming extensibility will add
/// `StreamChunk` and `StreamEnd` variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ServiceFrame {
    /// Typed request expecting a reply. Token correlates with Dispatch.
    Request { token: u64, payload: Vec<u8> },
    /// Reply to a prior request.
    Reply { token: u64, payload: Vec<u8> },
    /// Reply obligation failed (ReplyPort dropped without .reply()).
    Failed { token: u64 },
    /// Fire-and-forget message. No token, no reply expected.
    Notification { payload: Vec<u8> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_request() {
        let frame = ServiceFrame::Request {
            token: 42,
            payload: vec![1, 2, 3],
        };
        match &frame {
            ServiceFrame::Request { token, payload } => {
                assert_eq!(*token, 42);
                assert_eq!(payload, &[1, 2, 3]);
            }
            _ => panic!("expected Request"),
        }
    }

    #[test]
    fn construct_reply() {
        let frame = ServiceFrame::Reply {
            token: 7,
            payload: vec![0xFF],
        };
        assert!(matches!(frame, ServiceFrame::Reply { token: 7, .. }));
    }

    #[test]
    fn construct_failed() {
        let frame = ServiceFrame::Failed { token: 99 };
        assert!(matches!(frame, ServiceFrame::Failed { token: 99 }));
    }

    #[test]
    fn construct_notification() {
        let frame = ServiceFrame::Notification {
            payload: b"hello".to_vec(),
        };
        match frame {
            ServiceFrame::Notification { payload } => {
                assert_eq!(payload, b"hello");
            }
            _ => panic!("expected Notification"),
        }
    }

    #[test]
    fn roundtrip_request() {
        let original = ServiceFrame::Request {
            token: 0xDEAD_BEEF,
            payload: vec![10, 20, 30, 40],
        };
        let bytes = postcard::to_allocvec(&original).expect("serialize");
        let decoded: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ServiceFrame::Request { token, payload } => {
                assert_eq!(token, 0xDEAD_BEEF);
                assert_eq!(payload, vec![10, 20, 30, 40]);
            }
            _ => panic!("expected Request after roundtrip"),
        }
    }

    #[test]
    fn roundtrip_notification() {
        let original = ServiceFrame::Notification {
            payload: b"fire-and-forget".to_vec(),
        };
        let bytes = postcard::to_allocvec(&original).expect("serialize");
        let decoded: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ServiceFrame::Notification { payload } => {
                assert_eq!(payload, b"fire-and-forget");
            }
            _ => panic!("expected Notification after roundtrip"),
        }
    }

    #[test]
    fn roundtrip_reply() {
        let original = ServiceFrame::Reply {
            token: 1,
            payload: vec![42],
        };
        let bytes = postcard::to_allocvec(&original).expect("serialize");
        let decoded: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ServiceFrame::Reply { token, payload } => {
                assert_eq!(token, 1);
                assert_eq!(payload, vec![42]);
            }
            _ => panic!("expected Reply after roundtrip"),
        }
    }

    #[test]
    fn roundtrip_failed() {
        let original = ServiceFrame::Failed { token: 77 };
        let bytes = postcard::to_allocvec(&original).expect("serialize");
        let decoded: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize");
        assert!(matches!(decoded, ServiceFrame::Failed { token: 77 }));
    }

    #[test]
    fn empty_payload_request() {
        let frame = ServiceFrame::Request {
            token: 0,
            payload: vec![],
        };
        let bytes = postcard::to_allocvec(&frame).expect("serialize");
        let decoded: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ServiceFrame::Request { token, payload } => {
                assert_eq!(token, 0);
                assert!(payload.is_empty());
            }
            _ => panic!("expected Request with empty payload"),
        }
    }
}
