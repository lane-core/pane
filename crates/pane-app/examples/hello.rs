//! Basic headless example — connect, create a pane, handle events.
//!
//! Run pane-headless first:
//!   cargo run -p pane-headless
//!
//! Then in another terminal:
//!   cargo run -p pane-app --example hello

use pane_app::{App, Tag, cmd, Message, KeyCombo};
use pane_proto::event::{Key, Modifiers, NamedKey};

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.pane.example.hello")?;

    let mut pane = app.create_pane(
        Tag::new("Hello, headless!")
            .command(cmd("save", "Save").shortcut("Ctrl+S"))
            .command(cmd("quit", "Quit").shortcut("Alt+Q")),
    )?.wait()?;

    pane.add_shortcut(KeyCombo::new(Key::Char('s'), Modifiers::CTRL), "save", "");
    pane.add_shortcut(KeyCombo::new(Key::Named(NamedKey::Escape), Modifiers::empty()), "quit", "");

    println!("pane {:?} created — waiting for events", pane.id());

    pane.run(|_messenger, msg| {
        println!("{msg:?}");
        match msg {
            Message::CommandExecuted { ref command, .. } if command == "quit" => Ok(false),
            Message::CloseRequested => Ok(false),
            _ => Ok(true),
        }
    })
}
