//! pane-hello: canonical first pane app.
//!
//! Runs a stub server and a client in the same process, connected
//! over a unix socket. The client connects, receives Ready, prints
//! "Hello, world", receives CloseRequested, and exits.
//!
//! This is the vertical slice proof — every layer of the stack
//! participates: UnixStream transport, FrameCodec framing, par
//! handshake, bridge reader loop, LooperCore dispatch, Handler
//! lifecycle methods.

use std::os::unix::net::UnixListener;
use std::sync::mpsc;

use pane_proto::control::ControlMessage;
use pane_proto::protocols::lifecycle::LifecycleMessage;
use pane_proto::{Flow, Handler};
use pane_session::bridge::{self, HANDSHAKE_MAX_MESSAGE_SIZE};
use pane_session::frame::FrameCodec;
use pane_session::handshake::{Hello, Welcome};
use pane_session::peer_cred;
use pane_app::looper_core::LooperCore;
use pane_app::exit_reason::ExitReason;
use pane_app::dispatch::PeerScope;

// -- Handler --

struct HelloHandler;

impl Handler for HelloHandler {
    fn ready(&mut self) -> Flow {
        println!("Hello, world");
        Flow::Continue
    }

    fn close_requested(&mut self) -> Flow {
        println!("Goodbye");
        Flow::Stop
    }
}

// -- Stub server --

/// Run a minimal server that accepts one client, performs the
/// handshake, sends Ready and CloseRequested, then exits.
fn run_stub_server(listener: UnixListener) {
    let (stream, _addr) = listener.accept().expect("accept failed");

    // Derive PeerAuth from the connection.
    let auth = peer_cred::peer_cred(&stream)
        .expect("peer_cred failed");
    println!("server: accepted peer {auth}");

    let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

    // Read Hello
    let frame = codec.read_frame(&mut &stream)
        .expect("server: read Hello failed");
    let payload = match frame {
        pane_session::frame::Frame::Message { service: 0, payload } => payload,
        other => panic!("server: unexpected frame: {other:?}"),
    };
    let hello: Hello = postcard::from_bytes(&payload)
        .expect("server: Hello deserialization failed");
    println!("server: received Hello v{}", hello.version);

    // Send Welcome
    let decision: Result<Welcome, pane_session::handshake::Rejection> = Ok(Welcome {
        version: 1,
        instance_id: "pane-hello-server".into(),
        max_message_size: hello.max_message_size,
        bindings: vec![],
    });
    let bytes = postcard::to_allocvec(&decision)
        .expect("server: Welcome serialization failed");
    codec.write_frame(&mut &stream, 0, &bytes)
        .expect("server: write Welcome failed");

    // Active phase: send Ready
    let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
    let bytes = postcard::to_allocvec(&msg)
        .expect("server: Ready serialization failed");
    codec.write_frame(&mut &stream, 0, &bytes)
        .expect("server: write Ready failed");

    // Give the client a moment to process Ready, then send CloseRequested
    std::thread::sleep(std::time::Duration::from_millis(50));

    let msg = ControlMessage::Lifecycle(LifecycleMessage::CloseRequested);
    let bytes = postcard::to_allocvec(&msg)
        .expect("server: CloseRequested serialization failed");
    codec.write_frame(&mut &stream, 0, &bytes)
        .expect("server: write CloseRequested failed");

    // Wait for client to close the connection
    std::thread::sleep(std::time::Duration::from_millis(100));
}

// -- Main --

fn main() {
    // Create a temporary unix socket
    let socket_path = std::env::temp_dir().join(format!("pane-hello-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)
        .expect("failed to bind unix socket");

    // Start the stub server in a background thread
    let server_handle = std::thread::spawn(move || {
        run_stub_server(listener);
    });

    // Connect as a client
    let stream = std::os::unix::net::UnixStream::connect(&socket_path)
        .expect("failed to connect to server");

    let client = bridge::connect_and_run(
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        },
        stream,
    ).expect("handshake failed");

    println!("client: connected to {}", client.welcome.instance_id);

    // Wire the channel to the looper
    let (exit_tx, _exit_rx) = mpsc::channel();
    let looper = LooperCore::new(
        HelloHandler,
        PeerScope(0),
        exit_tx,
    );

    let reason = looper.run(client.rx);
    println!("client: exited with {reason:?}");

    // Clean up
    server_handle.join().expect("server thread panicked");
    let _ = std::fs::remove_file(&socket_path);

    match reason {
        ExitReason::Graceful => std::process::exit(0),
        _ => std::process::exit(1),
    }
}
