use pane_session::types::{Send, Recv, End, Chan};
use pane_session::transport::memory;
use pane_session::error::SessionError;

/// A simple request-response protocol:
/// Client sends a string, server sends back a u64, session ends.
type ClientProtocol = Send<String, Recv<u64, End>>;

// The server side is the dual — Recv<String, Send<u64, End>>
// (derived automatically by the type system)

#[test]
fn simple_request_response() {
    let (client, server): (Chan<ClientProtocol, _>, _) = memory::pair();

    // Server thread
    let server_handle = std::thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "hello");
        let server = server.send(42u64).unwrap();
        server.close();
    });

    // Client side
    let client = client.send("hello".to_string()).unwrap();
    let (response, client) = client.recv().unwrap();
    assert_eq!(response, 42);
    client.close();

    server_handle.join().unwrap();
}

#[test]
fn crash_produces_error_not_panic() {
    let (client, server): (Chan<ClientProtocol, _>, _) = memory::pair();

    // Drop the server immediately — simulates a crash
    drop(server);

    // Client should get Disconnected, not a panic
    let result = client.send("hello".to_string());
    assert!(result.is_err());
    match result.unwrap_err() {
        SessionError::Disconnected => {} // correct
        other => panic!("expected Disconnected, got {:?}", other),
    }
}

/// A multi-step protocol: send name, recv id, send data, recv ack, end.
type MultiStep = Send<String, Recv<u32, Send<Vec<u8>, Recv<bool, End>>>>;

#[test]
fn multi_step_protocol() {
    let (client, server): (Chan<MultiStep, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (name, server) = server.recv().unwrap();
        assert_eq!(name, "test-pane");

        let server = server.send(7u32).unwrap();

        let (data, server) = server.recv().unwrap();
        assert_eq!(data, vec![1, 2, 3]);

        let server = server.send(true).unwrap();
        server.close();
    });

    let client = client.send("test-pane".to_string()).unwrap();
    let (id, client) = client.recv().unwrap();
    assert_eq!(id, 7);

    let client = client.send(vec![1u8, 2, 3]).unwrap();
    let (ack, client) = client.recv().unwrap();
    assert!(ack);
    client.close();

    server_handle.join().unwrap();
}

/// Verify that mid-conversation crash is handled gracefully.
#[test]
fn mid_conversation_crash() {
    type Protocol = Send<String, Recv<String, Send<String, End>>>;
    let (client, server): (Chan<Protocol, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "step 1");
        let server = server.send("step 2".to_string()).unwrap();
        // Server crashes here — drops the channel before receiving step 3
        drop(server);
    });

    let client = client.send("step 1".to_string()).unwrap();
    let (msg, client) = client.recv().unwrap();
    assert_eq!(msg, "step 2");

    // Client tries to send step 3, but server is dead
    server_handle.join().unwrap();
    let result = client.send("step 3".to_string());
    assert!(matches!(result, Err(SessionError::Disconnected)));
}
