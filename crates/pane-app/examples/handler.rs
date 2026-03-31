//! Handler trait example — stateful pane with the Handler pattern.
//!
//! Demonstrates `run_with()` instead of the closure-based `run()`.
//! The handler struct holds state across events.
//!
//! Run pane-headless first:
//!   cargo run -p pane-headless
//!
//! Then:
//!   cargo run -p pane-app --example handler

use pane_app::{App, Tag, cmd, Handler, Messenger};
use pane_proto::protocol::PaneGeometry;

struct Counter {
    count: u32,
}

impl Handler for Counter {
    fn ready(&mut self, _proxy: &Messenger, geom: PaneGeometry) -> pane_app::Result<bool> {
        println!("ready — geometry: {geom:?}");
        Ok(true)
    }

    fn key(&mut self, _proxy: &Messenger, key: pane_proto::event::KeyEvent) -> pane_app::Result<bool> {
        self.count += 1;
        println!("key #{}: {:?}", self.count, key.key);
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> pane_app::Result<bool> {
        println!("close requested — {count} keys received", count = self.count);
        Ok(false)
    }

    fn quit_requested(&self) -> bool {
        println!("quit requested — count={}", self.count);
        true // allow quit
    }
}

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.pane.example.handler")?;

    let pane = app.create_pane(
        Tag::new("Counter")
            .command(cmd("close", "Close").shortcut("Alt+W")),
    )?.wait()?;

    println!("pane {:?} created", pane.id());

    pane.run_with(Counter { count: 0 })
}
