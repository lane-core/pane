//! Handshake types for the pane wire protocol.
//!
//! The handshake is a par session-typed exchange:
//!   Client → Server: Hello
//!   Server → Client: Result<Welcome, Rejection>
//!
//! The server responds with Ok(Welcome) on success or
//! Err(Rejection) on failure. This is a value-level Result
//! inside a single Send/Recv exchange — both branches terminate
//! the par session. Not par's `choose` mechanism.
//!
//! Protocol types defined with par. Executed over a Transport
//! via the bridge module. Serialized as CBOR (self-describing)
//! for forward compatibility: new fields with `#[serde(default)]`
//! deserialize from older payloads that omit them. Data-plane
//! frames after the handshake use postcard (compact, positional).
//!
//! Design heritage: Haiku converged on the same two-phase format
//! split — BMessage (self-describing) for AS_GET_DESKTOP handshake
//! (src/servers/app/Desktop.cpp), link protocol (positional binary)
//! for data plane (headers/private/app/LinkSender.h:36-40).
//!
//! Design heritage: Plan 9 Tversion/Rversion negotiated protocol
//! version and max message size (version(5),
//! reference/plan9/man/5/version:19-48). Rerror provided explicit
//! rejection on any T-message (intro(5), 0intro:325-331). BeOS
//! AS_CREATE_APP sent team_id/port/signature
//! (src/kits/app/Application.cpp:1402-1416) and got back a status_t
//! via FlushWithReply (Application.cpp:1423). pane's explicit
//! Result<Welcome, Rejection> combines both: rich rejection reasons
//! (Plan 9 Rerror's explicitness) with typed structure (not Be's
//! bare status_t integer).

use pane_proto::ServiceId;
use serde::{Deserialize, Serialize};

/// The handshake protocol from the client's perspective.
/// Send Hello, receive either Welcome (accepted) or Rejection (declined).
pub type ClientHandshake =
    par::exchange::Send<Hello, par::exchange::Recv<Result<Welcome, Rejection>>>;

/// The handshake protocol from the server's perspective (dual).
pub type ServerHandshake = par::Dual<ClientHandshake>;

/// A service this pane implements for others.
///
/// Declared in Hello so the server's provider index is populated
/// at handshake time.
///
/// Design heritage: Plan 9's Tattach carried aname (the file tree
/// to mount) — the client declared what it offered in the
/// namespace. BeOS's AS_CREATE_APP carried the app signature,
/// which the roster used to index capabilities. ServiceProvision
/// combines both: a typed service identity with a version, declared
/// upfront for routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceProvision {
    pub service: ServiceId,
    pub version: u32,
}

/// Client → Server: initial connection message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub max_message_size: u32,
    /// Proposed cap on in-flight requests (D9). Server may reduce
    /// but never increase. 0 = unlimited (wire default for
    /// backwards compatibility).
    ///
    /// Design heritage: Plan 9 Tversion's msize was the only
    /// negotiated parameter (version(5),
    /// reference/plan9/man/5/version:19-48) — client proposed,
    /// server could reduce. pane adds a second knob for request
    /// concurrency. Haiku's port capacity was receiver-unilateral
    /// (B_LOOPER_PORT_DEFAULT_CAPACITY = 200); pane negotiates
    /// because cross-process IPC requires both sides to agree on
    /// flow control.
    #[serde(default)]
    pub max_outstanding_requests: u16,
    pub interests: Vec<ServiceInterest>,
    /// Services this pane provides for others.
    pub provides: Vec<ServiceProvision>,
}

/// Server → Client: handshake response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Welcome {
    pub version: u32,
    pub instance_id: String,
    pub max_message_size: u32,
    /// Effective cap on in-flight requests — at most the client's
    /// proposed value. 0 = unlimited.
    #[serde(default)]
    pub max_outstanding_requests: u16,
    pub bindings: Vec<ServiceBinding>,
}

/// A service the client wants to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInterest {
    pub service: ServiceId,
    pub expected_version: u32,
}

/// A service binding from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceBinding {
    pub service: ServiceId,
    pub session_id: u8,
    pub version: u32,
}

/// Handshake rejection — server explicitly declines the connection.
///
/// Sent as Err(Rejection) in the handshake Result. The client
/// receives this via recv1() and can inspect the reason and
/// optional human-readable message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rejection {
    pub reason: RejectReason,
    pub message: Option<String>,
}

/// Why the server rejected the handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RejectReason {
    VersionMismatch,
    Unauthorized,
    ServerFull,
    ServiceUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: serialize a value to CBOR bytes (handshake format).
    fn cbor_serialize<T: serde::Serialize>(val: &T) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(val, &mut buf).expect("CBOR serialize");
        buf
    }

    /// Helper: deserialize a value from CBOR bytes.
    fn cbor_deserialize<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
        ciborium::de::from_reader(bytes).expect("CBOR deserialize")
    }

    #[test]
    fn hello_roundtrip_with_max_outstanding_requests() {
        let hello = Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 128,
            interests: vec![],
            provides: vec![],
        };
        let bytes = cbor_serialize(&hello);
        let decoded: Hello = cbor_deserialize(&bytes);
        assert_eq!(decoded.max_outstanding_requests, 128);
    }

    #[test]
    fn welcome_roundtrip_with_max_outstanding_requests() {
        let welcome = Welcome {
            version: 1,
            instance_id: "test-server".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 64,
            bindings: vec![],
        };
        let bytes = cbor_serialize(&welcome);
        let decoded: Welcome = cbor_deserialize(&bytes);
        assert_eq!(decoded.max_outstanding_requests, 64);
    }

    #[test]
    fn hello_zero_means_unlimited() {
        let hello = Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        };
        let bytes = cbor_serialize(&hello);
        let decoded: Hello = cbor_deserialize(&bytes);
        assert_eq!(decoded.max_outstanding_requests, 0);
    }

    /// Forward-compatibility: a Hello serialized WITHOUT
    /// max_outstanding_requests (simulating an older client)
    /// deserializes into the current Hello struct with the field
    /// defaulting to 0. This is the whole point of D11 — CBOR's
    /// self-describing format makes #[serde(default)] functional.
    /// With postcard (positional binary), this would fail with
    /// "hit the end of buffer, expected more data."
    #[test]
    fn hello_forward_compat_missing_field_defaults_to_zero() {
        // Simulate a V1 Hello that lacks max_outstanding_requests.
        // CBOR encodes as a map with named keys, so omitting a key
        // is meaningful — the deserializer hits #[serde(default)].
        #[derive(Debug, serde::Serialize)]
        struct HelloV1 {
            version: u32,
            max_message_size: u32,
            interests: Vec<ServiceInterest>,
            provides: Vec<ServiceProvision>,
        }

        let old_hello = HelloV1 {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        };

        let bytes = cbor_serialize(&old_hello);
        let decoded: Hello = cbor_deserialize(&bytes);

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.max_message_size, 16 * 1024 * 1024);
        // The missing field defaults to 0 (unlimited).
        assert_eq!(decoded.max_outstanding_requests, 0);
    }

    /// Same forward-compatibility test for Welcome.
    #[test]
    fn welcome_forward_compat_missing_field_defaults_to_zero() {
        #[derive(Debug, serde::Serialize)]
        struct WelcomeV1 {
            version: u32,
            instance_id: String,
            max_message_size: u32,
            bindings: Vec<ServiceBinding>,
        }

        let old_welcome = WelcomeV1 {
            version: 1,
            instance_id: "old-server".into(),
            max_message_size: 16 * 1024 * 1024,
            bindings: vec![],
        };

        let bytes = cbor_serialize(&old_welcome);
        let decoded: Welcome = cbor_deserialize(&bytes);

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.instance_id, "old-server");
        assert_eq!(decoded.max_message_size, 16 * 1024 * 1024);
        assert_eq!(decoded.max_outstanding_requests, 0);
    }

    /// Session subtyping: a newer Hello with additional fields is a
    /// width subtype of the current Hello. Extra fields are silently
    /// ignored during deserialization.
    ///
    /// Design heritage: Plan 9 version(5) — unknown version strings
    /// are negotiated down, not rejected
    /// (reference/plan9/man/5/version:19-48). The CBOR self-describing
    /// format (D11) enables the same tolerance for unknown map keys.
    #[test]
    fn hello_extra_field_ignored() {
        use std::collections::BTreeMap;

        // Build a CBOR map with all standard Hello fields plus an
        // unknown "experimental_feature" key. A future Hello revision
        // might add this field; older deserializers must ignore it.
        let mut map = BTreeMap::<String, ciborium::Value>::new();
        map.insert("version".into(), ciborium::Value::Integer(1.into()));
        map.insert(
            "max_message_size".into(),
            ciborium::Value::Integer((16 * 1024 * 1024_i64).into()),
        );
        map.insert(
            "max_outstanding_requests".into(),
            ciborium::Value::Integer(32.into()),
        );
        map.insert("interests".into(), ciborium::Value::Array(vec![]));
        map.insert("provides".into(), ciborium::Value::Array(vec![]));
        map.insert("experimental_feature".into(), ciborium::Value::Bool(true));

        let bytes = cbor_serialize(&map);
        let decoded: Hello = cbor_deserialize(&bytes);

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.max_message_size, 16 * 1024 * 1024);
        assert_eq!(decoded.max_outstanding_requests, 32);
        assert!(decoded.interests.is_empty());
        assert!(decoded.provides.is_empty());
    }

    /// Same width-subtyping property for Welcome: extra fields from a
    /// newer server are silently ignored by an older client.
    ///
    /// Design heritage: Plan 9 version(5) — unknown capabilities
    /// negotiated away, not fatal.
    #[test]
    fn welcome_extra_field_ignored() {
        use std::collections::BTreeMap;

        let mut map = BTreeMap::<String, ciborium::Value>::new();
        map.insert("version".into(), ciborium::Value::Integer(1.into()));
        map.insert("instance_id".into(), ciborium::Value::Text("srv-42".into()));
        map.insert(
            "max_message_size".into(),
            ciborium::Value::Integer((16 * 1024 * 1024_i64).into()),
        );
        map.insert(
            "max_outstanding_requests".into(),
            ciborium::Value::Integer(64.into()),
        );
        map.insert("bindings".into(), ciborium::Value::Array(vec![]));
        // Unknown field from a hypothetical future server
        map.insert("compression".into(), ciborium::Value::Text("zstd".into()));

        let bytes = cbor_serialize(&map);
        let decoded: Welcome = cbor_deserialize(&bytes);

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.instance_id, "srv-42");
        assert_eq!(decoded.max_message_size, 16 * 1024 * 1024);
        assert_eq!(decoded.max_outstanding_requests, 64);
        assert!(decoded.bindings.is_empty());
    }

    /// Forward-compat in the other direction: an old client that omits
    /// max_outstanding_requests deserializes with the field defaulting
    /// to 0 (unlimited). Constructed via raw CBOR map to prove the
    /// #[serde(default)] annotation is exercised at the CBOR layer.
    ///
    /// Complements hello_forward_compat_missing_field_defaults_to_zero
    /// (which uses a surrogate struct) by going through the raw map
    /// path, proving the property holds regardless of how the bytes
    /// are produced.
    #[test]
    fn hello_missing_optional_field_defaults() {
        use std::collections::BTreeMap;

        let mut map = BTreeMap::<String, ciborium::Value>::new();
        map.insert("version".into(), ciborium::Value::Integer(1.into()));
        map.insert(
            "max_message_size".into(),
            ciborium::Value::Integer((16 * 1024 * 1024_i64).into()),
        );
        // max_outstanding_requests deliberately omitted
        map.insert("interests".into(), ciborium::Value::Array(vec![]));
        map.insert("provides".into(), ciborium::Value::Array(vec![]));

        let bytes = cbor_serialize(&map);
        let decoded: Hello = cbor_deserialize(&bytes);

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.max_message_size, 16 * 1024 * 1024);
        assert_eq!(decoded.max_outstanding_requests, 0);
        assert!(decoded.interests.is_empty());
        assert!(decoded.provides.is_empty());
    }
}
