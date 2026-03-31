//! Tests for App::connect() over real unix sockets.
//!
//! Spawns a minimal server that runs the compositor handshake and
//! active-phase protocol, then tests that App::connect() works
//! end-to-end over a real socket.

use std::os::unix::net::UnixListener;
use std::thread;

use pane_app::{App, Tag, Message};
use pane_proto::protocol::{
    ServerHello, Accepted, ClientToComp, CompToClient, PaneGeometry,
    ServerHandshake, ConnectionTopology,
};
use pane_session::framing::{read_framed, write_framed};
use pane_session::transport::unix::UnixTransport;
use pane_session::types::Chan;

// uuid re-exported through pane-proto isn't directly available;
// use the instance_id as a plain string.

/// A minimal unix socket server that runs the compositor handshake then
/// processes active-phase messages on the same socket.
fn run_test_server(listener: UnixListener) {
    let (stream, _) = listener.accept().unwrap();

    // Phase 1: session-typed handshake on the socket
    let transport = UnixTransport::from_stream(stream);
    let server: Chan<ServerHandshake, _> = Chan::new(transport);

    let (_hello, server) = server.recv().unwrap();
    let server = server.send(ServerHello {
        compositor: "test-server".into(),
        version: 1,
        instance_id: "test-server-instance".into(),
    }).unwrap();
    let (_caps, server) = server.recv().unwrap();
    let server = server.select_left().unwrap();
    let server = server.send(Accepted {
        caps: vec![],
        topology: ConnectionTopology::Local,
    }).unwrap();

    // finish() reclaims the transport → into_stream() gets the socket back
    let stream = server.finish().into_stream();

    // Phase 2: active-phase framed messaging on the same socket
    // Client-proposed PaneIds — server echoes them back
    loop {
        let bytes = match read_framed(&mut &stream) {
            Ok(b) => b,
            Err(_) => break,
        };

        let msg: ClientToComp = match pane_proto::deserialize(&bytes) {
            Ok(m) => m,
            Err(_) => break,
        };

        match msg {
            ClientToComp::CreatePane { pane, .. } => {
                // Echo back the client-proposed PaneId
                let response = CompToClient::PaneCreated {
                    pane,
                    geometry: PaneGeometry { width: 800, height: 600, cols: 80, rows: 24 },
                };
                let bytes = pane_proto::serialize(&response).unwrap();
                if write_framed(&mut &stream, &bytes).is_err() { break; }
            }
            ClientToComp::RequestClose { pane } => {
                let response = CompToClient::CloseAck { pane };
                let bytes = pane_proto::serialize(&response).unwrap();
                if write_framed(&mut &stream, &bytes).is_err() { break; }
            }
            _ => {}
        }
    }
}

/// Full lifecycle over unix socket: connect → handshake → create pane → run → exit.
#[test]
fn connect_over_unix_socket() {
    let dir = tempfile::tempdir().unwrap();
    let sock_dir = dir.path().join("pane");
    std::fs::create_dir_all(&sock_dir).unwrap();
    let sock_path = sock_dir.join("compositor.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    // Set XDG_RUNTIME_DIR so App::connect() finds the socket
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", dir.path()); }

    let server_handle = thread::spawn(move || run_test_server(listener));

    // App::connect() finds the socket, runs handshake, enters active phase
    let app = App::connect("com.test.unix").unwrap();

    let pane = app.create_pane(Tag::new("Unix Test")).unwrap().wait().unwrap();
    // UUID PaneId — just verify it's valid (non-nil)
    assert!(!pane.id().uuid().is_nil());

    // Validate geometry from PaneCreated arrives as Ready event
    pane.run(|_proxy, event| {
        match event {
            Message::Ready(g) => {
                assert_eq!(g.width, 800);
                assert_eq!(g.height, 600);
                Ok(false)
            }
            _ => Ok(true),
        }
    }).unwrap();

    drop(app);
    server_handle.join().unwrap();
}
