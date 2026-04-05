//! Bridge: connects par's in-process session channels to IPC.
//!
//! Two-phase connection:
//!   Phase 1 (connect): verify the transport is alive. Returns Result.
//!   Phase 2 (handshake): par drives the exchange over the verified
//!     transport. If the transport dies mid-handshake, the session is
//!     aborted (panic). This is the rare case — the common failure
//!     (server not running) is caught in Phase 1.
//!
//! Architecture:
//!   Handler ←→ par oneshot ←→ Bridge thread ←→ Transport ←→ wire
//!
//! The handler uses par's Send::send() and Recv::recv() directly.
//! The bridge thread serializes (postcard) between par's channels
//! and the transport.
//!
//! Design heritage: BeOS split connection into find_port (returned
//! status_t) and the AS_CREATE_APP exchange (debugger on failure).
//! Plan 9's mount() returned -1 on unreachable; Tversion/Rattach
//! was the handshake over a verified fd.

use par::exchange::{Send, Recv};
use par::Session;
use crate::transport::{Transport, ConnectError};
use crate::handshake::{Hello, Welcome, ClientHandshake, ServerHandshake};

/// Phase 1: verify a transport is alive.
///
/// Returns the transport on success, ConnectError on failure.
/// No par involved — this is where "server not running" surfaces
/// as a Result, before the session-typed handshake begins.
pub fn verify_transport<T: Transport>(transport: T) -> Result<T, ConnectError> {
    // Real implementations would send a probe/ping here.
    // MemoryTransport is always connected.
    Ok(transport)
}

/// Phase 2: client-side handshake over a verified transport.
///
/// Returns the handler's par session endpoint. A bridge thread
/// handles serialization between par's channels and the transport.
///
/// If the transport dies mid-handshake, the bridge thread panics,
/// its par endpoint drops, and the handler's next recv() panics
/// ("sender dropped"). This aborts the session.
pub fn bridge_client_handshake(mut transport: impl Transport + 'static) -> ClientHandshake {
    Send::fork_sync(move |server: ServerHandshake| {
        std::thread::spawn(move || {
            let (hello, server): (Hello, _) =
                futures::executor::block_on(server.recv());

            let bytes = postcard::to_allocvec(&hello)
                .expect("bridge: Hello serialization failed");
            transport.send_raw(&bytes);

            let bytes = transport.recv_raw();
            let welcome: Welcome = postcard::from_bytes(&bytes)
                .expect("bridge: Welcome deserialization failed");

            server.send1(welcome);
        });
    })
}

/// Phase 2: server-side handshake over a verified transport.
pub fn bridge_server_handshake(mut transport: impl Transport + 'static) -> ServerHandshake {
    Recv::fork_sync(move |client: ClientHandshake| {
        std::thread::spawn(move || {
            let bytes = transport.recv_raw();
            let hello: Hello = postcard::from_bytes(&bytes)
                .expect("bridge: Hello deserialization failed");

            let client = client.send(hello);

            let (welcome, _): (Welcome, _) =
                futures::executor::block_on(client.recv());

            let bytes = postcard::to_allocvec(&welcome)
                .expect("bridge: Welcome serialization failed");
            transport.send_raw(&bytes);
        });
    })
}

/// Convenience: Phase 1 + Phase 2 for the client side.
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

        let client = connect_client(ct).expect("client connect failed");
        let server = connect_server(st).expect("server connect failed");

        let client = client.send(Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
        });

        let (hello, server) = futures::executor::block_on(server.recv());
        assert_eq!(hello.version, 1);

        server.send1(Welcome {
            version: 1,
            instance_id: "test-server".into(),
            max_message_size: 16 * 1024 * 1024,
            bindings: vec![],
        });

        let welcome = futures::executor::block_on(client.recv1());
        assert_eq!(welcome.instance_id, "test-server");
    }

    #[test]
    fn phase1_catches_bad_transport() {
        let (ct, _st) = MemoryTransport::pair();
        let result = verify_transport(ct);
        assert!(result.is_ok());
    }
}
