//! Session-typed handshake protocol tests.
//!
//! Exercises the ClientHandshake/ServerHandshake type aliases
//! over memory transport. Validates the full three-step handshake
//! (ClientHello → ServerHello → ClientCaps → Accept/Reject).

use std::thread;

use pane_session::transport::memory;
use pane_session::types::{Chan, Offer};

use pane_proto::protocol::{
    ClientHandshake, ServerHandshake,
    ClientHello, ServerHello, ClientCaps, Accepted, Rejected, ConnectionTopology,
};

/// Run a successful handshake: client connects, server accepts.
#[test]
fn handshake_accept_flow() {
    let (client, server): (Chan<ClientHandshake, _>, Chan<ServerHandshake, _>) = memory::pair();

    let server_handle = thread::spawn(move || {
        // Server: recv ClientHello
        let (hello, server) = server.recv().unwrap();
        assert_eq!(hello.signature, "com.example.hello");
        assert_eq!(hello.version, 1);
        assert!(hello.identity.is_none());

        // Server: send ServerHello
        let server = server.send(ServerHello {
            compositor: "pane-test".into(),
            version: 1,
            instance_id: "test-instance".into(),
        }).unwrap();

        // Server: recv ClientCaps
        let (caps, server) = server.recv().unwrap();
        assert_eq!(caps.caps, vec!["clipboard"]);

        // Server: select Accept (left branch)
        let server = server.select_left().unwrap();
        let server = server.send(Accepted {
            caps: vec!["clipboard".into()],
            topology: ConnectionTopology::Local,
        }).unwrap();

        server.close();
    });

    // Client: send ClientHello
    let client = client.send(ClientHello {
        signature: "com.example.hello".into(),
        version: 1,
        identity: None,
    }).unwrap();

    // Client: recv ServerHello
    let (hello, client) = client.recv().unwrap();
    assert_eq!(hello.compositor, "pane-test");
    assert_eq!(hello.instance_id, "test-instance");

    // Client: send ClientCaps
    let client = client.send(ClientCaps {
        caps: vec!["clipboard".into()],
    }).unwrap();

    // Client: offer (branch on server's choice)
    match client.offer().unwrap() {
        Offer::Left(chan) => {
            let (accepted, chan) = chan.recv().unwrap();
            assert_eq!(accepted.caps, vec!["clipboard"]);
            assert_eq!(accepted.topology, ConnectionTopology::Local);
            chan.close();
        }
        Offer::Right(_) => panic!("expected Accept, got Reject"),
    }

    server_handle.join().unwrap();
}

/// Server rejects the client.
#[test]
fn handshake_reject_flow() {
    let (client, server): (Chan<ClientHandshake, _>, Chan<ServerHandshake, _>) = memory::pair();

    let server_handle = thread::spawn(move || {
        let (hello, server) = server.recv().unwrap();
        assert_eq!(hello.signature, "com.evil.malware");

        let server = server.send(ServerHello {
            compositor: "pane-test".into(),
            version: 1,
            instance_id: "test-instance".into(),
        }).unwrap();

        let (_caps, server) = server.recv().unwrap();

        // Server: select Reject (right branch)
        let server = server.select_right().unwrap();
        let server = server.send(Rejected {
            reason: "untrusted signature".into(),
        }).unwrap();

        server.close();
    });

    let client = client.send(ClientHello {
        signature: "com.evil.malware".into(),
        version: 1,
        identity: None,
    }).unwrap();

    let (_hello, client) = client.recv().unwrap();

    let client = client.send(ClientCaps {
        caps: vec![],
    }).unwrap();

    match client.offer().unwrap() {
        Offer::Left(_) => panic!("expected Reject, got Accept"),
        Offer::Right(chan) => {
            let (rejected, chan) = chan.recv().unwrap();
            assert_eq!(rejected.reason, "untrusted signature");
            chan.close();
        }
    }

    server_handle.join().unwrap();
}

/// Handshake with finish() — reclaim the transport for active phase.
#[test]
fn handshake_finish_reclaims_transport() {
    let (client, server): (Chan<ClientHandshake, _>, Chan<ServerHandshake, _>) = memory::pair();

    let server_handle = thread::spawn(move || {
        let (_hello, server) = server.recv().unwrap();
        let server = server.send(ServerHello {
            compositor: "pane-test".into(),
            version: 1,
            instance_id: "test-instance".into(),
        }).unwrap();
        let (_caps, server) = server.recv().unwrap();

        let server = server.select_left().unwrap();
        let server = server.send(Accepted {
            caps: vec![],
            topology: ConnectionTopology::Local,
        }).unwrap();

        // finish() reclaims the transport instead of closing
        let _transport = server.finish();
        // Transport is now available for active-phase use
    });

    let client = client.send(ClientHello {
        signature: "com.test".into(),
        version: 1,
        identity: None,
    }).unwrap();
    let (_hello, client) = client.recv().unwrap();
    let client = client.send(ClientCaps { caps: vec![] }).unwrap();

    match client.offer().unwrap() {
        Offer::Left(chan) => {
            let (_accepted, chan) = chan.recv().unwrap();
            let _transport = chan.finish();
            // Transport reclaimed on client side too
        }
        Offer::Right(_) => panic!("expected Accept"),
    }

    server_handle.join().unwrap();
}

/// Client crashes mid-handshake — server gets Disconnected, not panic.
#[test]
fn handshake_client_crash_mid_flow() {
    let (client, server): (Chan<ClientHandshake, _>, Chan<ServerHandshake, _>) = memory::pair();

    let server_handle = thread::spawn(move || {
        let result = server.recv();
        // Client dropped before sending ClientHello → Disconnected
        assert!(result.is_err(), "expected Disconnected from crashed client");
    });

    // Drop client immediately — simulates crash
    drop(client);

    server_handle.join().unwrap();
}
