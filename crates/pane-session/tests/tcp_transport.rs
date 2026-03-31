use std::net::{TcpListener, TcpStream};
use std::thread;

use pane_session::types::{Send, Recv, End, Chan};
use pane_session::transport::tcp::TcpTransport;
use pane_session::error::SessionError;

/// A simple request-response protocol over TCP.
/// Client sends a string, server sends back a u64, session ends.
type ClientProtocol = Send<String, Recv<u64, End>>;

#[test]
fn tcp_request_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = TcpTransport::from_stream(stream);
        let server: Chan<Recv<String, Send<u64, End>>, _> = Chan::new(transport);

        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "hello from client");

        let server = server.send(42u64).unwrap();
        server.close();
    });

    let stream = TcpStream::connect(addr).unwrap();
    let transport = TcpTransport::from_stream(stream);
    let client: Chan<ClientProtocol, _> = Chan::new(transport);

    let client = client.send("hello from client".to_string()).unwrap();
    let (response, client) = client.recv().unwrap();
    assert_eq!(response, 42);
    client.close();

    server_handle.join().unwrap();
}

/// Multi-step protocol over TCP.
type MultiStep = Send<String, Recv<u32, Send<Vec<u8>, Recv<bool, End>>>>;

#[test]
fn tcp_multi_step() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = TcpTransport::from_stream(stream);
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

    let stream = TcpStream::connect(addr).unwrap();
    let transport = TcpTransport::from_stream(stream);
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

/// Crash recovery: server dies mid-conversation, client eventually gets Err.
///
/// TCP differs from unix sockets here: a send to a closed TCP peer may
/// succeed (buffered in the kernel) before the RST arrives. The error
/// surfaces on the next recv or a subsequent send. The test verifies
/// that the error IS detected — not that it's detected on the first
/// operation after the crash.
#[test]
fn tcp_crash_recovery() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = TcpTransport::from_stream(stream);
        let server: Chan<Recv<String, Send<String, Recv<String, Send<String, End>>>>, _> =
            Chan::new(transport);

        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "step 1");

        let server = server.send("step 2".to_string()).unwrap();

        // Server crashes — drops the channel before receiving step 3
        drop(server);
    });

    let stream = TcpStream::connect(addr).unwrap();
    let transport = TcpTransport::from_stream(stream);
    type CrashProtocol = Send<String, Recv<String, Send<String, Recv<String, End>>>>;
    let client: Chan<CrashProtocol, _> = Chan::new(transport);

    let client = client.send("step 1".to_string()).unwrap();
    let (msg, client) = client.recv().unwrap();
    assert_eq!(msg, "step 2");

    server_handle.join().unwrap();

    // First send may succeed (TCP kernel buffer absorbs it before RST)
    match client.send("step 3".to_string()) {
        Err(e) => {
            // Detected immediately — good
            assert!(matches!(e, SessionError::Disconnected | SessionError::Io(_)));
        }
        Ok(client) => {
            // Buffered — the recv that follows will fail
            let result = client.recv();
            assert!(
                result.is_err(),
                "expected error after server crash, got {:?}",
                result.as_ref().map(|v| format!("{:?}", v.0))
            );
        }
    }
}

/// tcp_pair helper creates a connected pair for testing.
#[test]
fn tcp_pair_roundtrip() {
    use pane_session::transport::tcp::tcp_pair;

    type Proto = Send<String, Recv<u64, End>>;

    let (client, server) = tcp_pair::<Proto>().unwrap();

    let server_handle = thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "via tcp_pair");
        let server = server.send(99u64).unwrap();
        server.close();
    });

    let client = client.send("via tcp_pair".to_string()).unwrap();
    let (val, client) = client.recv().unwrap();
    assert_eq!(val, 99);
    client.close();

    server_handle.join().unwrap();
}

/// Security: oversized length prefix is rejected, not allocated.
#[test]
fn tcp_rejects_oversized_message() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let sender_handle = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).unwrap();
        use std::io::Write;
        // Write a 4-byte length prefix of 0xFFFFFFFF (4GB)
        stream.write_all(&0xFFFF_FFFFu32.to_le_bytes()).unwrap();
        stream.flush().unwrap();
    });

    let (stream, _) = listener.accept().unwrap();
    let transport = TcpTransport::from_stream(stream);
    let server: Chan<Recv<String, End>, _> = Chan::new(transport);

    let result = server.recv();
    assert!(result.is_err());
    match result.unwrap_err() {
        SessionError::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::InvalidData),
        other => panic!("expected Io(InvalidData), got {:?}", other),
    }

    sender_handle.join().unwrap();
}

/// Large message: near MAX_MESSAGE_SIZE payload over TCP.
#[test]
fn tcp_large_message() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // 1MB payload — well under MAX_MESSAGE_SIZE (16MB) but exercises the framing
    let payload: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
    let payload_clone = payload.clone();

    let server_handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let transport = TcpTransport::from_stream(stream);
        let server: Chan<Recv<Vec<u8>, Send<usize, End>>, _> = Chan::new(transport);

        let (data, server) = server.recv().unwrap();
        let len = data.len();
        assert_eq!(data, payload_clone);
        let server = server.send(len).unwrap();
        server.close();
    });

    let stream = TcpStream::connect(addr).unwrap();
    let transport = TcpTransport::from_stream(stream);
    let client: Chan<Send<Vec<u8>, Recv<usize, End>>, _> = Chan::new(transport);

    let client = client.send(payload.clone()).unwrap();
    let (len, client) = client.recv().unwrap();
    assert_eq!(len, 1_000_000);
    client.close();

    server_handle.join().unwrap();
}
