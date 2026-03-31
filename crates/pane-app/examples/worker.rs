//! Worker thread example — post results back to the pane's event loop.
//!
//! Demonstrates `Messenger::post_app_message()` for sending results
//! from spawned threads to the handler, and `Handler::app_message()`
//! for receiving them with type-safe downcasting.
//!
//! Run pane-headless first:
//!   cargo run -p pane-headless
//!
//! Then:
//!   cargo run -p pane-app --example worker

use std::any::Any;
use std::thread;
use std::time::Duration;

use pane_app::{App, Tag, Handler, Messenger};
use pane_proto::protocol::PaneGeometry;

/// Result type sent from the worker thread to the handler.
struct ComputeResult {
    value: u64,
}

struct WorkerDemo {
    result: Option<u64>,
}

impl Handler for WorkerDemo {
    fn ready(&mut self, proxy: &Messenger, _geom: PaneGeometry) -> pane_app::Result<bool> {
        println!("spawning worker thread...");
        let proxy = proxy.clone();
        thread::spawn(move || {
            // Simulate expensive work
            thread::sleep(Duration::from_millis(500));
            let result = ComputeResult { value: 42 };
            println!("worker done — posting result");
            proxy.post_app_message(result).ok();
        });
        Ok(true)
    }

    fn app_message(&mut self, _proxy: &Messenger, msg: Box<dyn Any + Send>) -> pane_app::Result<bool> {
        if let Some(result) = msg.downcast_ref::<ComputeResult>() {
            println!("received result from worker: {}", result.value);
            self.result = Some(result.value);
            // Got our result — exit
            return Ok(false);
        }
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> pane_app::Result<bool> {
        Ok(false)
    }
}

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.pane.example.worker")?;
    let pane = app.create_pane(Tag::new("Worker"))?.wait()?;

    println!("pane {:?} created", pane.id());
    pane.run_with(WorkerDemo { result: None })
}
