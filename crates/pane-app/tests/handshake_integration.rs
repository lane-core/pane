//! Integration tests for the session-typed handshake with MockCompositor.
//!
//! Validates that the handshake runs correctly before the active phase,
//! and that pane creation/event delivery work after handshake completion.

use std::thread;

use pane_app::{App, Tag, Message, run_client_handshake};
use pane_app::mock::MockCompositor;

/// Full lifecycle: handshake → create pane → receive events → close.
#[test]
fn handshake_then_hello_pane() {
    // 1. Create mock with handshake support
    let (client_chan, conn, mock) = MockCompositor::pair_with_handshake();

    // 2. Start mock (runs server handshake, then active loop)
    let mock_handle = thread::spawn(move || mock.run());

    // 3. Run client handshake
    let hs = run_client_handshake(client_chan, "com.example.hello").unwrap();
    eprintln!("[test] handshake complete, accepted caps: {:?}", "(handshake ok)");

    // 4. Connect app using active-phase channels
    let app = App::connect_test("com.example.hello", conn).unwrap();

    // 5. Create pane and run
    let pane = app.create_pane(Tag::new("Hello")).unwrap();
    // Pane exits on Ready — validates the full handshake→create→run flow
    pane.run(|_proxy, event| {
        match event {
            Message::Ready(_) => {
                eprintln!("[test] got Ready after handshake — success!");
                Ok(false) // exit immediately after Ready
            }
            _ => Ok(true),
        }
    }).unwrap();

    drop(app);
    mock_handle.join().unwrap();
}

/// Handshake then multi-pane creation.
#[test]
fn handshake_then_multi_pane() {
    let (client_chan, conn, mock) = MockCompositor::pair_with_handshake();
    let mock_handle = thread::spawn(move || mock.run());

    run_client_handshake(client_chan, "com.test.multi").unwrap();
    let app = App::connect_test("com.test.multi", conn).unwrap();

    // Create 3 panes — validates that the active-phase channels
    // work correctly after the handshake
    let pane1 = app.create_pane(Tag::new("One")).unwrap();
    let pane2 = app.create_pane(Tag::new("Two")).unwrap();
    let pane3 = app.create_pane(Tag::new("Three")).unwrap();

    // Each pane gets a unique ID
    assert_ne!(pane1.id(), pane2.id());
    assert_ne!(pane2.id(), pane3.id());
    assert_ne!(pane1.id(), pane3.id());

    // Exit all panes immediately on Ready
    let h1 = thread::spawn(move || {
        pane1.run(|_, event| {
            if matches!(event, Message::Ready(_)) { Ok(false) } else { Ok(true) }
        }).unwrap();
    });
    let h2 = thread::spawn(move || {
        pane2.run(|_, event| {
            if matches!(event, Message::Ready(_)) { Ok(false) } else { Ok(true) }
        }).unwrap();
    });
    let h3 = thread::spawn(move || {
        pane3.run(|_, event| {
            if matches!(event, Message::Ready(_)) { Ok(false) } else { Ok(true) }
        }).unwrap();
    });

    h1.join().unwrap();
    h2.join().unwrap();
    h3.join().unwrap();

    drop(app);
    mock_handle.join().unwrap();
}
