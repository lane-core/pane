//! Bridge: connects par's in-process session channels to IPC.
//!
//! Two-phase connection model:
//!   Phase 1 (connect): verify the transport is alive. Returns Result.
//!     "Server not running" is an error, not a crash. No par involved.
//!   Phase 2 (handshake): par drives the Hello/Welcome exchange over
//!     the verified transport. If the transport dies mid-handshake,
//!     par's CLL annihilation fires (panic). This is the exceptional
//!     case — the connection was verified and then broke.
//!
//! Architecture:
//!   Handler ←→ par oneshot ←→ Bridge thread ←→ Transport ←→ wire
//!
//! BeOS: Phase 1 = find_port + create_desktop_connection (returned status_t).
//! Phase 2 = AS_CREATE_APP exchange (debugger on failure).
//! Plan 9: Phase 1 = mount() returns -1. Phase 2 = Tversion/Rattach.

use par::exchange::{Send, Recv};
use par::Session;
use crate::transport::{Transport, ConnectError};
use crate::handshake::{Hello, Welcome, ClientHandshake, ServerHandshake};

/// Phase 1: verify a transport is alive by exchanging a probe.
///
/// Returns the transport on success, ConnectError on failure.
/// No par involved — this is a simple synchronous check.
///
/// For MemoryTransport (tests), this is a no-op — memory
/// transports are always connected. Real transports (unix, tcp)
/// would verify the socket is open and the peer responds.
pub fn verify_transport<T: Transport>(transport: T) -> Result<T, ConnectError> {
    // For now, the transport is assumed valid if it was constructed.
    // Real implementations would send a probe/ping here.
    // The point: this is where "server not running" surfaces as
    // Result::Err, before par is involved.
    Ok(transport)
}

/// Phase 2: create a client-side handshake session over a
/// verified transport.
///
/// Returns the handler's par session endpoint. A bridge thread
/// serializes between par's oneshot channels and the transport.
///
/// If the transport dies mid-handshake, par's CLL annihilation
/// fires: bridge thread panics → par endpoint dropped → handler's
/// recv() panics ("sender dropped"). This is the correct CLL
/// encoding — a session either completes or is annihilated.
pub fn bridge_client_handshake(mut transport: impl Transport + 'static) -> ClientHandshake {
    Send::fork_sync(move |server: ServerHandshake| {
        std::thread::spawn(move || {
            // Wait for handler to send Hello through par
            let (hello, server): (Hello, _) =
                futures::executor::block_on(server.recv());

            // Serialize and write to transport
            let bytes = postcard::to_allocvec(&hello)
                .expect("bridge: Hello serialization failed");
            transport.send_raw(&bytes);

            // Read Welcome from transport
            let bytes = transport.recv_raw();
            let welcome: Welcome = postcard::from_bytes(&bytes)
                .expect("bridge: Welcome deserialization failed");

            // Send Welcome back through par to the handler
            server.send1(welcome);
        });
    })
}

/// Phase 2: create a server-side handshake session over a
/// verified transport.
pub fn bridge_server_handshake(mut transport: impl Transport + 'static) -> ServerHandshake {
    Recv::fork_sync(move |client: ClientHandshake| {
        std::thread::spawn(move || {
            // Read Hello from transport
            let bytes = transport.recv_raw();
            let hello: Hello = postcard::from_bytes(&bytes)
                .expect("bridge: Hello deserialization failed");

            // Send Hello through par to the handler
            let client = client.send(hello);

            // Wait for handler to send Welcome through par
            let (welcome, _): (Welcome, _) =
                futures::executor::block_on(client.recv());

            // Serialize and write to transport
            let bytes = postcard::to_allocvec(&welcome)
                .expect("bridge: Welcome serialization failed");
            transport.send_raw(&bytes);
        });
    })
}

/// Convenience: Phase 1 + Phase 2 for the client side.
/// Returns Result — Phase 1 errors are recoverable.
/// Phase 2 panics are CLL annihilation (exceptional).
pub fn connect_client(
    transport: impl Transport + 'static,
) -> Result<ClientHandshake, ConnectError> {
    let transport = verify_transport(transport)?;
    Ok(bridge_client_handshake(transport))
}

/// Convenience: Phase 1 + Phase 2 for the server side.
pub fn connect_server(
    transport: impl Transport + 'static,
) -> Result<ServerHandshake, ConnectError> {
    let transport = verify_transport(transport)?;
    Ok(bridge_server_handshake(transport))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;

    #[test]
    fn two_phase_handshake_roundtrip() {
        let (ct, st) = MemoryTransport::pair();

        // Phase 1 + 2: connect and get par sessions
        let client = connect_client(ct).expect("client connect failed");
        let server = connect_server(st).expect("server connect failed");

        // Handler uses par's native API
        let client = client.send(Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
        });

        // Server receives through par (bridge deserializes from transport)
        let (hello, server) = futures::executor::block_on(server.recv());
        assert_eq!(hello.version, 1);

        // Server sends Welcome through par
        server.send1(Welcome {
            version: 1,
            instance_id: "test-server".into(),
            max_message_size: 16 * 1024 * 1024,
            bindings: vec![],
        });

        // Client receives through par (bridge deserializes from transport)
        let welcome = futures::executor::block_on(client.recv1());
        assert_eq!(welcome.instance_id, "test-server");
    }

    #[test]
    fn phase1_catches_bad_transport() {
        // When real transports exist, this would test connection
        // refusal. For MemoryTransport, verify_transport always
        // succeeds. This test documents the Phase 1 → Result path.
        let (ct, _st) = MemoryTransport::pair();
        let result = verify_transport(ct);
        assert!(result.is_ok());
    }
}
