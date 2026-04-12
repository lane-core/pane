//! pane-hello: canonical first pane app.
//!
//! Runs a ProtocolServer and a client in the same process, connected
//! over a unix socket. The client connects, receives Ready, prints
//! "Hello, world", and exits.
//!
//! This is the vertical slice proof — every layer of the stack
//! participates: UnixStream transport, FrameCodec framing, par
//! handshake, ProtocolServer routing, bridge reader/writer threads,
//! LooperCore dispatch, Handler lifecycle methods.

use std::os::unix::net::UnixListener;
use std::sync::mpsc;

use pane_app::dispatch::PeerScope;
use pane_app::exit_reason::ExitReason;
use pane_app::looper_core::LooperCore;
use pane_proto::{Flow, Handler};
use pane_session::bridge;
use pane_session::handshake::Hello;
use pane_session::server::ProtocolServer;

// -- Handler --

struct HelloHandler;

impl Handler for HelloHandler {
    fn ready(&mut self) -> Flow {
        println!("Hello, world");
        Flow::Stop
    }
}

// -- Main --

fn main() {
    let socket_path = std::env::temp_dir().join(format!("pane-hello-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path).expect("failed to bind unix socket");
    let server = ProtocolServer::new();

    // Accept one connection in background
    let server_thread = std::thread::spawn(move || {
        let (stream, _addr) = listener.accept().expect("accept failed");
        let reader = stream.try_clone().expect("stream clone failed");
        let conn = server.accept(reader, stream).expect("server accept failed");
        conn.wait();
    });

    // Connect as client
    let stream =
        std::os::unix::net::UnixStream::connect(&socket_path).expect("failed to connect to server");

    let client = bridge::connect_and_run(
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
        stream,
    )
    .expect("handshake failed");

    println!("client: connected to {}", client.welcome.instance_id);

    let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);
    let (exit_tx, _exit_rx) = mpsc::channel();
    let looper = LooperCore::new(
        HelloHandler,
        PeerScope(0),
        write_tx,
        exit_tx,
        pane_app::Messenger::stub(),
    );
    let reason = looper.run(client.rx);

    println!("client: exited with {reason:?}");

    // Close the write channel so the bridge writer thread exits,
    // which closes the transport, letting the server detect disconnect.
    drop(client.write_tx);

    server_thread.join().expect("server thread panicked");
    let _ = std::fs::remove_file(&socket_path);

    match reason {
        ExitReason::Graceful => std::process::exit(0),
        _ => std::process::exit(1),
    }
}
