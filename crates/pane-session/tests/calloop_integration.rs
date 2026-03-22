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
