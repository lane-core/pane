use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::time::Duration;

use calloop::{EventLoop, PostAction};

use pane_session::calloop::{SessionSource, SessionEvent, write_message};
use pane_session::transport::unix::UnixTransport;
use pane_session::types::{Send, Recv, End, Chan};

type ClientProtocol = Send<String, Recv<u64, End>>;

#[test]
fn calloop_receives_session_message() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("calloop-test.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let client_handle = thread::spawn(move || {
        let stream = UnixStream::connect(&path).unwrap();
        let transport = UnixTransport::from_stream(stream);
        let client: Chan<ClientProtocol, _> = Chan::new(transport);

        let client = client.send("hello from calloop test".to_string()).unwrap();
        let (len, client) = client.recv().unwrap();
        assert_eq!(len, "hello from calloop test".len() as u64);
        client.close();
    });

    let (stream, _) = listener.accept().unwrap();
    let source = SessionSource::new(stream.try_clone().unwrap()).unwrap();
    let response_stream = stream;

    let mut event_loop: EventLoop<'_, bool> = EventLoop::try_new().unwrap();
    let handle = event_loop.handle();

    handle
        .insert_source(source, move |event, _, got_message: &mut bool| {
            match event {
                SessionEvent::Message(bytes) => {
                    let msg: String = postcard::from_bytes(&bytes).unwrap();
                    assert_eq!(msg, "hello from calloop test");

                    let response = postcard::to_allocvec(&(msg.len() as u64)).unwrap();
                    write_message(&response_stream, &response).unwrap();

                    *got_message = true;
                    Ok(PostAction::Remove)
                }
                SessionEvent::Disconnected => Ok(PostAction::Remove),
            }
        })
        .unwrap();

    let mut got_message = false;
    event_loop.dispatch(Duration::from_secs(2), &mut got_message).unwrap();
    assert!(got_message, "calloop should have received the session message");
    client_handle.join().unwrap();
}

#[test]
fn calloop_handles_client_crash() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("calloop-crash.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let client_handle = thread::spawn(move || {
        let stream = UnixStream::connect(&path).unwrap();
        let transport = UnixTransport::from_stream(stream);
        type CrashProtocol = Send<String, Send<String, End>>;
        let client: Chan<CrashProtocol, _> = Chan::new(transport);

        let client = client.send("before crash".to_string()).unwrap();
        drop(client);
    });

    let (stream, _) = listener.accept().unwrap();
    client_handle.join().unwrap();

    let source = SessionSource::new(stream).unwrap();

    let mut event_loop: EventLoop<'_, (u32, bool)> = EventLoop::try_new().unwrap();
    let handle = event_loop.handle();

    handle
        .insert_source(source, |event, _, state: &mut (u32, bool)| {
            match event {
                SessionEvent::Message(bytes) => {
                    let _msg: String = postcard::from_bytes(&bytes).unwrap();
                    state.0 += 1;
                    Ok(PostAction::Continue)
                }
                SessionEvent::Disconnected => {
                    state.1 = true;
                    Ok(PostAction::Remove)
                }
            }
        })
        .unwrap();

    let mut state = (0u32, false);
    event_loop.dispatch(Duration::from_secs(1), &mut state).unwrap();

    assert_eq!(state.0, 1, "should have received one message before crash");
    assert!(state.1, "should have detected disconnect");
}

/// Test that the accumulation buffer handles fragmented writes correctly.
/// Writes the length prefix and body in separate TCP-style chunks.
#[test]
fn calloop_handles_fragmented_writes() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("calloop-fragment.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let sender_handle = thread::spawn(move || {
        let mut stream = UnixStream::connect(&path).unwrap();
        use std::io::Write;

        let payload = postcard::to_allocvec(&"fragmented message".to_string()).unwrap();
        let len = (payload.len() as u32).to_le_bytes();

        // Write length prefix first, flush
        stream.write_all(&len).unwrap();
        stream.flush().unwrap();

        // Small delay to increase chance of separate reads
        std::thread::sleep(Duration::from_millis(50));

        // Write body separately
        stream.write_all(&payload).unwrap();
        stream.flush().unwrap();

        // Keep connection alive briefly for calloop to process
        std::thread::sleep(Duration::from_millis(200));
    });

    let (stream, _) = listener.accept().unwrap();
    let source = SessionSource::new(stream).unwrap();

    let mut event_loop: EventLoop<'_, Option<String>> = EventLoop::try_new().unwrap();
    let handle = event_loop.handle();

    handle
        .insert_source(source, |event, _, result: &mut Option<String>| {
            match event {
                SessionEvent::Message(bytes) => {
                    let msg: String = postcard::from_bytes(&bytes).unwrap();
                    *result = Some(msg);
                    Ok(PostAction::Remove)
                }
                SessionEvent::Disconnected => Ok(PostAction::Remove),
            }
        })
        .unwrap();

    let mut result: Option<String> = None;
    // Multiple dispatches to handle the fragmented arrival
    for _ in 0..5 {
        event_loop.dispatch(Duration::from_millis(100), &mut result).unwrap();
        if result.is_some() { break; }
    }

    assert_eq!(result, Some("fragmented message".to_string()));
    sender_handle.join().unwrap();
}

// --- Write path tests ---

/// Validate that write_message produces correct length-prefixed framing
/// that read_framed can parse.
#[test]
fn write_message_produces_valid_framing() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("write-framing.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let writer_handle = thread::spawn(move || {
        let stream = UnixStream::connect(&path).unwrap();
        write_message(&stream, b"hello").unwrap();
        write_message(&stream, b"world").unwrap();
        write_message(&stream, b"").unwrap(); // zero-length message
    });

    let (mut stream, _) = listener.accept().unwrap();

    // Read raw bytes and verify framing manually
    let mut buf = [0u8; 4];

    // Message 1: "hello" (5 bytes)
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(u32::from_le_bytes(buf), 5);
    let mut payload = vec![0u8; 5];
    stream.read_exact(&mut payload).unwrap();
    assert_eq!(payload, b"hello");

    // Message 2: "world" (5 bytes)
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(u32::from_le_bytes(buf), 5);
    let mut payload = vec![0u8; 5];
    stream.read_exact(&mut payload).unwrap();
    assert_eq!(payload, b"world");

    // Message 3: empty (0 bytes)
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(u32::from_le_bytes(buf), 0);

    writer_handle.join().unwrap();
}

/// Concurrent read via calloop + write via write_message on the same connection.
#[test]
fn concurrent_read_write_on_same_connection() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("read-write.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let client_handle = thread::spawn(move || {
        let stream = UnixStream::connect(&path).unwrap();
        let transport = UnixTransport::from_stream(stream);
        let client: Chan<ClientProtocol, _> = Chan::new(transport);

        // Send a request
        let client = client.send("ping".to_string()).unwrap();
        // Receive response
        let (response, client) = client.recv().unwrap();
        assert_eq!(response, 4); // "ping".len()
        client.close();
    });

    let (stream, _) = listener.accept().unwrap();
    let write_stream = stream.try_clone().unwrap();
    let source = SessionSource::new(stream).unwrap();

    let mut event_loop: EventLoop<'_, bool> = EventLoop::try_new().unwrap();
    let handle = event_loop.handle();

    handle
        .insert_source(source, move |event, _, done: &mut bool| {
            match event {
                SessionEvent::Message(bytes) => {
                    let msg: String = postcard::from_bytes(&bytes).unwrap();
                    // Write response on the same connection (via cloned stream)
                    let response = postcard::to_allocvec(&(msg.len() as u64)).unwrap();
                    write_message(&write_stream, &response).unwrap();
                    *done = true;
                    Ok(PostAction::Remove)
                }
                SessionEvent::Disconnected => {
                    *done = true;
                    Ok(PostAction::Remove)
                }
            }
        })
        .unwrap();

    let mut done = false;
    event_loop.dispatch(Duration::from_secs(2), &mut done).unwrap();
    assert!(done, "should have processed the request-response");
    client_handle.join().unwrap();
}

/// Multiple rapid writes followed by calloop reading all of them.
#[test]
fn write_burst_read_all() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("burst.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();

    let path = sock_path.clone();
    let writer_handle = thread::spawn(move || {
        let stream = UnixStream::connect(&path).unwrap();
        for i in 0u32..100 {
            let payload = postcard::to_allocvec(&i).unwrap();
            write_message(&stream, &payload).unwrap();
        }
        // Keep alive briefly
        thread::sleep(Duration::from_millis(200));
    });

    let (stream, _) = listener.accept().unwrap();
    let source = SessionSource::new(stream).unwrap();

    let mut event_loop: EventLoop<'_, u32> = EventLoop::try_new().unwrap();
    let handle = event_loop.handle();

    handle
        .insert_source(source, |event, _, count: &mut u32| {
            match event {
                SessionEvent::Message(bytes) => {
                    let _val: u32 = postcard::from_bytes(&bytes).unwrap();
                    *count += 1;
                    Ok(PostAction::Continue)
                }
                SessionEvent::Disconnected => Ok(PostAction::Remove),
            }
        })
        .unwrap();

    let mut count = 0u32;
    for _ in 0..10 {
        event_loop.dispatch(Duration::from_millis(100), &mut count).unwrap();
        if count >= 100 { break; }
    }

    assert_eq!(count, 100, "should have received all 100 messages, got {}", count);
    writer_handle.join().unwrap();
}
