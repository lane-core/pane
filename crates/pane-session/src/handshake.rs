//! Handshake types for the pane wire protocol.
//!
//! The handshake is a session-typed exchange:
//!   Client → Server: Hello
//!   Server → Client: Welcome
//!
//! Defined as par session types, run over Chan<S>.

use serde::{Serialize, Deserialize};
use pane_proto::ServiceId;

/// The handshake protocol from the client's perspective.
/// Send Hello, receive Welcome.
pub type ClientHandshake = par::exchange::Send<Hello, par::exchange::Recv<Welcome>>;

/// The handshake protocol from the server's perspective (dual).
pub type ServerHandshake = par::Dual<ClientHandshake>;

/// Client → Server: initial connection message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub max_message_size: u32,
    pub interests: Vec<ServiceInterest>,
}

/// Server → Client: handshake response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Welcome {
    pub version: u32,
    pub instance_id: String,
    pub max_message_size: u32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;
    use crate::Chan;

    #[test]
    fn handshake_roundtrip() {
        let (ta, tb) = MemoryTransport::pair();

        // Client sends Hello
        let client: Chan<ClientHandshake, _> = Chan::new(ta);
        let client = client.send(Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
        });

        // Server receives Hello, sends Welcome
        let server: Chan<ServerHandshake, _> = Chan::new(tb);
        let (hello, server) = server.recv();
        assert_eq!(hello.version, 1);

        server.send1(Welcome {
            version: 1,
            instance_id: "test-server-1".into(),
            max_message_size: 16 * 1024 * 1024,
            bindings: vec![],
        });

        // Client receives Welcome
        let welcome = client.recv1();
        assert_eq!(welcome.instance_id, "test-server-1");
    }
}
