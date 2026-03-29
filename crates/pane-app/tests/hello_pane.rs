use std::thread;
use std::time::Duration;

use pane_app::{App, Tag, cmd, Message};
use pane_app::mock::MockCompositor;

/// THE acceptance test. The hello-pane example running end-to-end
/// against a MockCompositor.
#[test]
fn hello_pane_lifecycle() {
    eprintln!("[test] creating mock compositor pair");
    let (conn, mock) = MockCompositor::pair();
    let inject_sender = mock.sender();

    eprintln!("[test] spawning mock compositor thread");
    let mock_handle = thread::spawn(move || mock.run());

    eprintln!("[test] connecting app");
    let app = App::connect_test("com.example.hello", conn).unwrap();

    eprintln!("[test] creating pane");
    let pane = app.create_pane(
        Tag::new("Hello")
            .command(cmd("close", "Close this pane").shortcut("Alt+W")),
    ).unwrap();

    let pane_id = pane.id();
    eprintln!("[test] pane created with id {:?}", pane_id);

    // Inject Close after a delay on a separate thread
    eprintln!("[test] scheduling close injection in 200ms");
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        eprintln!("[inject] sending Close to pane {:?}", pane_id);
        let _ = inject_sender.send(pane_proto::protocol::CompToClient::Close { pane: pane_id });
        eprintln!("[inject] Close sent");
    });

    eprintln!("[test] entering pane.run()");
    pane.run(|_proxy, event| {
        eprintln!("[run] received event: {:?}", event);
        match event {
            Message::Key(key) if key.is_escape() => Ok(false),
            Message::CloseRequested => {
                eprintln!("[run] got Close, exiting");
                Ok(false)
            }
            _ => Ok(true),
        }
    }).unwrap();

    eprintln!("[test] pane.run() returned, dropping app");
    drop(app);

    eprintln!("[test] joining mock thread");
    mock_handle.join().unwrap();

    eprintln!("[test] PASSED");
}
