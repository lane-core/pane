use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;

use pane_session::types::{Send, Recv, End, Chan};
use pane_session::transport::unix::UnixTransport;
use pane_session::error::SessionError;

/// A simple request-response protocol over unix sockets.
/// Client sends a string, server sends back a u64, session ends.
type ClientProtocol = Send<String, Recv<u64, End>>;
// Server side: Recv<String, Send<u64, End>> (the dual)

#[test]
fn unix_socket_request_response() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("pane-test.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    // Server thread — accepts one connection, runs the dual protocol
    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = UnixTransport::from_stream(stream);

        // Server session type is the dual: Recv<String, Send<u64, End>>
        let server: Chan<Recv<String, Send<u64, End>>, _> = Chan::new(transport);

        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "hello from client");

        let server = server.send(42u64).unwrap();
        server.close();
    });

    // Client side — connects and runs the protocol
    let stream = UnixStream::connect(&sock_path).unwrap();
    let transport = UnixTransport::from_stream(stream);
    let client: Chan<ClientProtocol, _> = Chan::new(transport);

    let client = client.send("hello from client".to_string()).unwrap();
    let (response, client) = client.recv().unwrap();
    assert_eq!(response, 42);
    client.close();

    server_handle.join().unwrap();
}

/// Multi-step protocol over unix sockets.
type MultiStep = Send<String, Recv<u32, Send<Vec<u8>, Recv<bool, End>>>>;

#[test]
fn unix_socket_multi_step() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("pane-test-multi.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = UnixTransport::from_stream(stream);
        let server: Chan<Recv<String, Send<u32, Recv<Vec<u8>, Send<bool, End>>>>, _> =
            Chan::new(transport);

        let (name, server) = server.recv().unwrap();
        assert_eq!(name, "test-pane");

        let server = server.send(7u32).unwrap();

        let (data, server) = server.recv().unwrap();
        assert_eq!(data, vec![1, 2, 3, 4, 5]);

        let server = server.send(true).unwrap();
        server.close();
    });

    let stream = UnixStream::connect(&sock_path).unwrap();
    let transport = UnixTransport::from_stream(stream);
    let client: Chan<MultiStep, _> = Chan::new(transport);

    let client = client.send("test-pane".to_string()).unwrap();
    let (id, client) = client.recv().unwrap();
    assert_eq!(id, 7);

    let client = client.send(vec![1u8, 2, 3, 4, 5]).unwrap();
    let (ack, client) = client.recv().unwrap();
    assert!(ack);
    client.close();

    server_handle.join().unwrap();
}

/// Crash recovery: server dies mid-conversation, client gets Err(Disconnected).
#[test]
fn unix_socket_crash_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("pane-test-crash.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    // Server accepts, does one exchange, then crashes (drops the channel)
    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = UnixTransport::from_stream(stream);
        let server: Chan<Recv<String, Send<String, Recv<String, End>>>, _> =
            Chan::new(transport);

        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "step 1");

        let server = server.send("step 2".to_string()).unwrap();

        // Server crashes here — drops the channel before receiving step 3
        drop(server);
    });

    let stream = UnixStream::connect(&sock_path).unwrap();
    let transport = UnixTransport::from_stream(stream);
    type CrashProtocol = Send<String, Recv<String, Send<String, End>>>;
    let client: Chan<CrashProtocol, _> = Chan::new(transport);

    // Steps 1-2 succeed
    let client = client.send("step 1".to_string()).unwrap();
    let (msg, client) = client.recv().unwrap();
    assert_eq!(msg, "step 2");

    // Wait for server to die
    server_handle.join().unwrap();

    // Step 3: client tries to send, server is dead
    let result = client.send("step 3".to_string());
    assert!(
        matches!(result, Err(SessionError::Disconnected)),
        "expected Disconnected, got {:?}",
        result.as_ref().map(|_| "Ok")
    );
}

/// Cross-thread crash: server thread panics, client gets Err(Disconnected).
/// This simulates what happens when a pane handler thread panics in the compositor.
#[test]
fn unix_socket_server_panic_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("pane-test-panic.sock");

    let listener = UnixListener::bind(&sock_path).unwrap();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = UnixTransport::from_stream(stream);
        let server: Chan<Recv<String, Send<u64, End>>, _> = Chan::new(transport);

        let (_msg, _server) = server.recv().unwrap();

        // Server panics — simulates a bug in the pane handler
        panic!("simulated compositor handler bug");
    });

    let stream = UnixStream::connect(&sock_path).unwrap();
    let transport = UnixTransport::from_stream(stream);
    let client: Chan<Send<String, Recv<u64, End>>, _> = Chan::new(transport);

    let client = client.send("hello".to_string()).unwrap();

    // Server panicked — the thread is dead, the socket is closed
    let _ = server_handle.join(); // join returns Err because of panic

    // Client's recv should get Disconnected, not propagate the panic
    let result = client.recv();
    assert!(
        matches!(result, Err(SessionError::Disconnected)),
        "expected Disconnected after server panic, got {:?}",
        result.as_ref().map(|v| format!("{:?}", v.0))
    );
}

/// Security: oversized length prefix is rejected, not allocated.
#[test]
fn unix_socket_rejects_oversized_message() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("pane-test-oversize.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    // Malicious sender: writes a length prefix claiming 4GB
    let path = sock_path.clone();
    let sender_handle = thread::spawn(move || {
        let mut stream = UnixStream::connect(&path).unwrap();
        use std::io::Write;
        // Write a 4-byte length prefix of 0xFFFFFFFF (4GB)
        stream.write_all(&0xFFFF_FFFFu32.to_le_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let (stream, _) = listener.accept().unwrap();
    let transport = UnixTransport::from_stream(stream);
    let server: Chan<Recv<String, End>, _> = Chan::new(transport);

    let result = server.recv();
    // Should be an I/O error (InvalidData), not an allocation attempt
    assert!(result.is_err());
    match result.unwrap_err() {
        SessionError::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::InvalidData),
        other => panic!("expected Io(InvalidData), got {:?}", other),
    }

    sender_handle.join().unwrap();
}
