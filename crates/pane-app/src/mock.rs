//! Mock compositor for testing the pane-app kit.
//!
//! Responds to protocol messages without any rendering or Wayland.
//! Enough to exercise the full kit API: create panes, send events,
//! receive content updates.

use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pane_proto::message::PaneId;
use pane_proto::protocol::{
    ClientToComp, CompToClient, PaneGeometry,
};

use crate::connection::{self, MockConnection};
use crate::connection::Connection;

/// A mock compositor that responds to CreatePane and RequestClose.
pub struct MockCompositor {
    conn: MockConnection,
    next_id: u32,
    /// Events to inject into specific panes after creation.
    injections: Vec<(PaneId, CompToClient)>,
    /// Log of received ClientToComp messages for assertions.
    log: Arc<Mutex<Vec<ClientToComp>>>,
}

impl MockCompositor {
    /// Create a connected pair: (kit connection, mock compositor).
    pub fn pair() -> (Connection, Self) {
        let (conn, mock_conn) = connection::test_pair();
        let mock = MockCompositor {
            conn: mock_conn,
            next_id: 1,
            injections: Vec::new(),
            log: Arc::new(Mutex::new(Vec::new())),
        };
        (conn, mock)
    }

    /// Schedule an event to be injected after a pane is created.
    /// The event is sent to the pane identified by `target` after
    /// a short delay.
    pub fn inject_after_create(&mut self, target: PaneId, event: CompToClient) {
        self.injections.push((target, event));
    }

    /// Schedule a Close event for the first pane created, after a delay.
    /// Convenience for the hello-pane test.
    pub fn close_first_pane_after(&mut self, delay: Duration) {
        // We'll handle this specially in run()
        self.injections.push((
            PaneId::new(NonZeroU32::new(1).unwrap()),
            CompToClient::Close { pane: PaneId::new(NonZeroU32::new(1).unwrap()) },
        ));
    }

    /// Get a handle to the message log for assertions.
    pub fn log(&self) -> Arc<Mutex<Vec<ClientToComp>>> {
        self.log.clone()
    }

    /// Run the mock compositor. Processes messages until the connection closes.
    pub fn run(mut self) {
        let default_geometry = PaneGeometry {
            width: 800,
            height: 600,
            cols: 80,
            rows: 24,
        };

        loop {
            let msg = match self.conn.receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(msg) => msg,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };

            self.log.lock().unwrap().push(msg.clone());

            match msg {
                ClientToComp::CreatePane { .. } => {
                    let id = PaneId::new(NonZeroU32::new(self.next_id).unwrap());
                    self.next_id += 1;

                    // Send PaneCreated response
                    let _ = self.conn.sender.send(CompToClient::PaneCreated {
                        pane: id,
                        geometry: default_geometry,
                    });

                    // Process any pending injections for this pane
                    let mut remaining = Vec::new();
                    for (target, event) in self.injections.drain(..) {
                        if target == id {
                            // Small delay to let the kit set up the pane
                            std::thread::sleep(Duration::from_millis(50));
                            let _ = self.conn.sender.send(event);
                        } else {
                            remaining.push((target, event));
                        }
                    }
                    self.injections = remaining;
                }

                ClientToComp::RequestClose { pane } => {
                    let _ = self.conn.sender.send(CompToClient::CloseAck { pane });
                }

                // Record everything else — tests can inspect the log
                _ => {}
            }
        }
    }
}
