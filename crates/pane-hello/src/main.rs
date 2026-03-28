//! hello-pane — the canonical first pane application.
//!
//! Connects to the compositor, creates a pane with a tag and command
//! vocabulary, prints all events, and exits on Close or Escape.

use pane_app::{App, Tag, cmd, PaneEvent};
use pane_proto::tag::BuiltIn;

fn main() {
    if let Err(e) = run() {
        eprintln!("pane-hello: {e:?}");
        std::process::exit(1);
    }
}

fn run() -> pane_app::Result<()> {
    let app = App::connect("com.pane.hello")?;

    let pane = app.create_pane(
        Tag::new("Hello, pane!")
            .commands(vec![
                cmd("close", "Close this pane")
                    .shortcut("Alt+W")
                    .built_in(BuiltIn::Close),
            ]),
    )?;

    println!("pane created: {:?}", pane.id());

    pane.run(|_proxy, event| {
        println!("event: {:?}", event);
        match event {
            PaneEvent::Key(key) if key.is_escape() => Ok(false),
            PaneEvent::Close => Ok(false),
            _ => Ok(true),
        }
    })
}
