//! Crash monitoring example — one pane watches another.
//!
//! Demonstrates `Messenger::monitor()` for Erlang-style crash
//! propagation. When the monitored pane exits, the watcher receives
//! `Message::PaneExited { pane, reason }`.
//!
//! Run pane-headless first:
//!   cargo run -p pane-headless
//!
//! Then:
//!   cargo run -p pane-app --example monitor

use std::thread;

use pane_app::{App, Tag, Message, Handler, Messenger};
use pane_proto::message::PaneId;
use pane_proto::protocol::PaneGeometry;

/// Watcher pane — monitors another pane and prints when it exits.
struct Watcher;

impl Handler for Watcher {
    fn ready(&mut self, _proxy: &Messenger, _geom: PaneGeometry) -> pane_app::Result<bool> {
        println!("[watcher] ready — monitoring target pane");
        Ok(true)
    }

    fn pane_exited(
        &mut self,
        _proxy: &Messenger,
        pane: PaneId,
        reason: pane_app::ExitReason,
    ) -> pane_app::Result<bool> {
        println!("[watcher] monitored pane {pane:?} exited: {reason:?}");
        // Our target died — exit too
        Ok(false)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> pane_app::Result<bool> {
        Ok(false)
    }
}

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.pane.example.monitor")?;

    // Create the target pane (will exit quickly)
    let target = app.create_pane(Tag::new("Target"))?.wait()?;
    let target_id = target.id();
    println!("[main] target pane: {target_id:?}");

    // Create the watcher pane
    let watcher = app.create_pane(Tag::new("Watcher"))?.wait()?;
    println!("[main] watcher pane: {:?}", watcher.id());

    // Set up monitoring: watcher watches target
    let watcher_messenger = watcher.messenger();
    target.messenger().monitor(&watcher_messenger);

    // Run target on a thread — exits after Ready
    let target_handle = thread::spawn(move || {
        target.run(|_proxy, msg| {
            match msg {
                Message::Ready(_) => {
                    println!("[target] ready — exiting immediately");
                    Ok(false)
                }
                _ => Ok(true),
            }
        })
    });

    // Run watcher — will receive PaneExited when target dies
    println!("[main] running watcher (will exit when target dies)");
    let result = watcher.run_with(Watcher);

    target_handle.join().unwrap()?;
    result
}
