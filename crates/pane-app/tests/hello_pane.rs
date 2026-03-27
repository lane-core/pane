use std::num::NonZeroU32;
use std::thread;

use pane_app::{App, Tag, cmd, BuiltIn, PaneEvent};
use pane_app::mock::MockCompositor;
use pane_proto::message::PaneId;
use pane_proto::protocol::CompToClient;

/// THE acceptance test. The hello-pane example running end-to-end
/// against a MockCompositor.
#[test]
fn hello_pane_lifecycle() {
    let (conn, mut mock) = MockCompositor::pair();

    // Schedule a Close event for the first pane after creation
    mock.close_first_pane_after(std::time::Duration::from_millis(100));

    let mock_handle = thread::spawn(move || mock.run());

    let app = App::connect_test("com.example.hello", conn).unwrap();

    let pane = app.create_pane(
        Tag::new("Hello").commands(vec![
            cmd("close", "Close this pane")
                .shortcut("Alt+W")
                .built_in(BuiltIn::Close),
        ]),
    ).unwrap();

    // The mock will inject Close, which causes the closure to return Ok(false)
    pane.run(|event| match event {
        PaneEvent::Key(key) if key.is_escape() => Ok(false),
        PaneEvent::Close => Ok(false),
        _ => Ok(true),
    }).unwrap();

    // The mock thread exits when the connection closes
    mock_handle.join().unwrap();
}

/// Test that multiple panes receive events independently.
#[test]
fn multi_pane_independent() {
    let (conn, mock) = MockCompositor::pair();
    let mock_log = mock.log();
    let mock_handle = thread::spawn(move || mock.run());

    let app = App::connect_test("com.example.multi", conn).unwrap();

    let pane1 = app.create_pane(Tag::new("Pane 1")).unwrap();
    let pane2 = app.create_pane(Tag::new("Pane 2")).unwrap();

    let id1 = pane1.id();
    let id2 = pane2.id();

    // Both panes should have different IDs
    assert_ne!(id1, id2);

    // Run both panes on threads, each exits immediately on any event
    // We just test that create_pane works for multiple panes
    let h1 = thread::spawn(move || {
        pane1.run(|_event| Ok(false)).unwrap();
    });
    let h2 = thread::spawn(move || {
        pane2.run(|_event| Ok(false)).unwrap();
    });

    // Both will get Disconnected when the app drops (channel closes)
    drop(app);
    h1.join().unwrap();
    h2.join().unwrap();
    mock_handle.join().unwrap();
}

/// Test that set_title reaches the compositor.
#[test]
fn set_title_reaches_compositor() {
    let (conn, mock) = MockCompositor::pair();
    let mock_log = mock.log();
    let mock_handle = thread::spawn(move || mock.run());

    let app = App::connect_test("com.example.title", conn).unwrap();
    let pane = app.create_pane(Tag::new("Original")).unwrap();

    pane.set_title(pane_proto::PaneTitle {
        text: "Updated".into(),
        short: None,
    }).unwrap();

    // Exit immediately
    pane.run(|_| Ok(false)).unwrap();

    drop(app);
    mock_handle.join().unwrap();

    // Check the mock received the SetTitle
    let log = mock_log.lock().unwrap();
    let has_set_title = log.iter().any(|msg| {
        matches!(msg, pane_proto::ClientToComp::SetTitle { title, .. } if title.text == "Updated")
    });
    assert!(has_set_title, "mock should have received SetTitle");
}
