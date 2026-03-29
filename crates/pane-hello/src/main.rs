//! hello-pane — the canonical first pane application.
//!
//! Connects to the compositor, creates a pane with a tag and command
//! vocabulary, prints all events, and exits on Close or Escape.

use pane_app::{App, Tag, cmd, Message, KeyCombo};
use pane_proto::event::{Key, Modifiers, NamedKey};
use pane_proto::tag::BuiltIn;

fn main() {
    if let Err(e) = run() {
        eprintln!("pane-hello: {e:?}");
        std::process::exit(1);
    }
}

fn run() -> pane_app::Result<()> {
    let app = App::connect("com.pane.hello")?;

    let mut pane = app.create_pane(
        Tag::new("Hello, pane!")
            .command(cmd("save", "Save").shortcut("Ctrl+S").client("save"))
            .command(cmd("close", "Close").shortcut("Alt+W").built_in(BuiltIn::Close)),
    )?;

    // Register keyboard shortcuts — Ctrl+S and Escape
    pane.add_shortcut(KeyCombo::new(Key::Char('s'), Modifiers::CTRL), "save", "");
    pane.add_shortcut(KeyCombo::new(Key::Named(NamedKey::Escape), Modifiers::empty()), "close", "");

    println!("pane created: {:?}", pane.id());

    pane.run(|_messenger, msg| {
        println!("msg: {:?}", msg);
        match msg {
            Message::CommandExecuted { ref command, .. } if command == "close" => Ok(false),
            Message::Close => Ok(false),
            _ => Ok(true),
        }
    })
}
