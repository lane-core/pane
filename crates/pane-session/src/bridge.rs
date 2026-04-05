//! Bridge: connects par's in-process session channels to a wire Transport.
//!
//! Par's runtime uses oneshot channels (futures). The bridge thread
//! translates between par's async oneshot world and pane's synchronous
//! Transport. This means par's runtime is ACTUALLY driving the
//! protocol — the handler uses par::exchange::Send::send() and
//! par::exchange::Recv::recv() directly.
//!
//! Architecture:
//!   Handler ←→ par oneshot ←→ Bridge thread ←→ Transport ←→ wire
//!
//! The bridge is protocol-specific: for each concrete protocol type,
//! a bridge function reads/writes the expected sequence. The handshake
//! bridge is provided below as the canonical example.

use par::exchange::{Send, Recv};
use par::Session;
use serde::{Serialize, de::DeserializeOwned};
use crate::transport::Transport;
use crate::handshake::{Hello, Welcome, ClientHandshake, ServerHandshake};

/// Create a client-side handshake session backed by a transport.
///
/// Returns the handler's par session endpoint. A bridge thread
/// reads from par's channels, serializes to the transport, reads
/// from the transport, and feeds back into par's channels.
///
/// The handler uses par's native API:
/// ```ignore
/// let client: ClientHandshake = bridge_client_handshake(transport);
/// let client = client.send(hello);                    // par's send
/// let (welcome, _) = block_on(client.recv());         // par's recv
/// ```
pub fn bridge_client_handshake(mut transport: impl Transport + 'static) -> ClientHandshake {
    // fork_sync creates a par session pair:
    //   - returns ClientHandshake (Send<Hello, Recv<Welcome>>) to the caller
    //   - passes ServerHandshake (Recv<Hello, Send<Welcome>>) to the closure
    Send::fork_sync(move |server: ServerHandshake| {
        // Bridge: this closure runs synchronously in fork_sync.
        // We spawn a thread that blocks on par's async recv and
        // bridges to the transport.
        std::thread::spawn(move || {
            // Step 1: wait for handler to send Hello through par
            let (hello, server): (Hello, _) =
                futures::executor::block_on(server.recv());

            // Step 2: serialize and write to transport
            let bytes = postcard::to_allocvec(&hello)
                .expect("bridge: Hello serialization failed");
            transport.send_raw(&bytes);

            // Step 3: read Welcome from transport
            let bytes = transport.recv_raw();
            let welcome: Welcome = postcard::from_bytes(&bytes)
                .expect("bridge: Welcome deserialization failed");

            // Step 4: send Welcome back through par to the handler
            server.send1(welcome);
        });
    })
}

/// Create a server-side handshake session backed by a transport.
///
/// Returns the handler's par session endpoint. A bridge thread
/// handles the transport side.
pub fn bridge_server_handshake(mut transport: impl Transport + 'static) -> ServerHandshake {
    Recv::fork_sync(move |client: ClientHandshake| {
        std::thread::spawn(move || {
            // Step 1: read Hello from transport
            let bytes = transport.recv_raw();
            let hello: Hello = postcard::from_bytes(&bytes)
                .expect("bridge: Hello deserialization failed");

            // Step 2: send Hello through par to the handler
            let client = client.send(hello);

            // Step 3: wait for handler to send Welcome through par
            let (welcome, _): (Welcome, _) =
                futures::executor::block_on(client.recv());

            // Step 4: serialize and write to transport
            let bytes = postcard::to_allocvec(&welcome)
                .expect("bridge: Welcome serialization failed");
            transport.send_raw(&bytes);
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;
    use crate::handshake::{ServiceInterest, ServiceBinding};

    #[test]
    fn bridged_handshake_roundtrip() {
        let (client_transport, server_transport) = MemoryTransport::pair();

        // Client side: par session backed by transport
        let client: ClientHandshake = bridge_client_handshake(client_transport);

        // Server side: par session backed by transport
        let server: ServerHandshake = bridge_server_handshake(server_transport);

        // Client sends Hello through par
        let client = client.send(Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
        });

        // Server receives Hello through par (bridge deserializes from transport)
        let (hello, server) = futures::executor::block_on(server.recv());
        assert_eq!(hello.version, 1);

        // Server sends Welcome through par
        server.send1(Welcome {
            version: 1,
            instance_id: "test-server".into(),
            max_message_size: 16 * 1024 * 1024,
            bindings: vec![],
        });

        // Client receives Welcome through par (bridge deserializes from transport)
        let welcome = futures::executor::block_on(client.recv1());
        assert_eq!(welcome.instance_id, "test-server");
    }
}
