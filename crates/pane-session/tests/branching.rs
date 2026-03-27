use pane_session::types::{Send, Recv, Branch, End, Chan};
use pane_session::{Offer, SessionError};
use pane_session::transport::memory;
use pane_session::transport::unix;

/// A protocol with a branch: client sends a request, server selects
/// accept (sends u64) or reject (sends String).
type ClientSide = Send<String, Branch<Recv<u64, End>, Recv<String, End>>>;
// Server dual: Recv<String, Select<Send<u64, End>, Send<String, End>>>

#[test]
fn branch_left() {
    let (client, server): (Chan<ClientSide, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "hello");
        // Server selects left (accept)
        let server = server.select_left().unwrap();
        let server = server.send(42u64).unwrap();
        server.close();
    });

    let client = client.send("hello".to_string()).unwrap();
    match client.offer().unwrap() {
        Offer::Left(client) => {
            let (val, client) = client.recv().unwrap();
            assert_eq!(val, 42);
            client.close();
        }
        Offer::Right(_) => panic!("expected left branch"),
    }

    server_handle.join().unwrap();
}

#[test]
fn branch_right() {
    let (client, server): (Chan<ClientSide, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        assert_eq!(msg, "bad request");
        // Server selects right (reject)
        let server = server.select_right().unwrap();
        let server = server.send("rejected: bad input".to_string()).unwrap();
        server.close();
    });

    let client = client.send("bad request".to_string()).unwrap();
    match client.offer().unwrap() {
        Offer::Left(_) => panic!("expected right branch"),
        Offer::Right(client) => {
            let (reason, client) = client.recv().unwrap();
            assert_eq!(reason, "rejected: bad input");
            client.close();
        }
    }

    server_handle.join().unwrap();
}

#[test]
fn branch_crash_before_selection() {
    let (client, server): (Chan<ClientSide, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (_msg, server) = server.recv().unwrap();
        // Server crashes before selecting a branch
        drop(server);
    });

    let client = client.send("hello".to_string()).unwrap();
    server_handle.join().unwrap();

    let result = client.offer();
    assert!(matches!(result, Err(SessionError::Disconnected)));
}

/// Test the request() combinator — send + recv in one call.
#[test]
fn request_combinator() {
    type Protocol = Send<String, Recv<u64, End>>;
    let (client, server): (Chan<Protocol, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        let server = server.send(msg.len() as u64).unwrap();
        server.close();
    });

    // request() = send + recv in one call
    let (len, client) = client.request("hello world".to_string()).unwrap();
    assert_eq!(len, 11);
    client.close();

    server_handle.join().unwrap();
}

/// Test finish() — reclaim transport after session ends.
#[test]
fn finish_reclaims_transport() {
    type Protocol = Send<String, Recv<String, End>>;
    let (client, server): (Chan<Protocol, _>, _) = memory::pair();

    let server_handle = std::thread::spawn(move || {
        let (msg, server) = server.recv().unwrap();
        let server = server.send(format!("echo: {}", msg)).unwrap();
        let _transport = server.finish(); // reclaim, don't close
    });

    let (response, client) = client.request("test".to_string()).unwrap();
    assert_eq!(response, "echo: test");
    let _transport = client.finish(); // reclaim, don't close

    server_handle.join().unwrap();
}

/// Test branch over unix sockets.
#[test]
fn branch_over_unix_socket() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("branch-test.sock");
    let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let client_handle = std::thread::spawn(move || {
        let client: Chan<ClientSide, _> = unix::connect_session(&path).unwrap();
        let client = client.send("request".to_string()).unwrap();
        match client.offer().unwrap() {
            Offer::Left(client) => {
                let (val, client) = client.recv().unwrap();
                assert_eq!(val, 99);
                client.close();
            }
            Offer::Right(_) => panic!("expected left"),
        }
    });

    use pane_session::Dual;
    let server: Chan<Dual<ClientSide>, _> = unix::accept_session(&listener).unwrap();
    let (msg, server) = server.recv().unwrap();
    assert_eq!(msg, "request");
    let server = server.select_left().unwrap();
    let server = server.send(99u64).unwrap();
    server.close();

    client_handle.join().unwrap();
}

/// Test unix_pair — connected pair in same process.
#[test]
fn unix_pair_works() {
    type Protocol = Send<u32, Recv<u32, End>>;
    let (client, server): (Chan<Protocol, _>, Chan<_, _>) = unix::unix_pair().unwrap();

    let server_handle = std::thread::spawn(move || {
        let (val, server) = server.recv().unwrap();
        let server = server.send(val * 2).unwrap();
        server.close();
    });

    let (doubled, client) = client.request(21u32).unwrap();
    assert_eq!(doubled, 42);
    client.close();

    server_handle.join().unwrap();
}
