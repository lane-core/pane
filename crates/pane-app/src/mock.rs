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
    ClientHandshake, ServerHandshake,
    ServerHello, Accepted,
};
use pane_session::transport::memory;
use pane_session::types::Chan;

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
    /// If set, run the server-side handshake at the start of run().
    handshake_rx: Option<mpsc::Receiver<Chan<ServerHandshake, pane_session::transport::memory::MemoryTransport>>>,
}

impl MockCompositor {
    /// Create a connected pair: (kit connection, mock compositor).
    /// No handshake — raw active-phase channels. Use for unit tests.
    pub fn pair() -> (Connection, Self) {
        let (conn, mock_conn) = connection::test_pair();
        let mock = MockCompositor {
            conn: mock_conn,
            next_id: 1,
            injections: Vec::new(),
            log: Arc::new(Mutex::new(Vec::new())),
            handshake_rx: None,
        };
        (conn, mock)
    }

    /// Create a connected pair with session-typed handshake.
    ///
    /// Returns:
    /// - Client handshake channel (caller runs client side of handshake)
    /// - Active-phase Connection (pass to `App::connect_test()` after handshake)
    /// - MockCompositor (spawn on a thread — runs server handshake then active loop)
    ///
    /// The server handshake runs automatically at the start of `mock.run()`.
    /// The client must complete its handshake BEFORE calling `App::connect_test()`.
    pub fn pair_with_handshake() -> (
        Chan<ClientHandshake, pane_session::transport::memory::MemoryTransport>,
        Connection,
        Self,
    ) {
        let (client_chan, server_chan): (
            Chan<ClientHandshake, _>,
            Chan<ServerHandshake, _>,
        ) = memory::pair();

        // Server handshake runs in MockCompositor::run() before entering
        // the active-phase loop. Pass the server channel via oneshot.
        let (hs_tx, hs_rx) = mpsc::channel();
        hs_tx.send(server_chan).unwrap();

        let (conn, mock_conn) = connection::test_pair();
        let mock = MockCompositor {
            conn: mock_conn,
            next_id: 1,
            injections: Vec::new(),
            log: Arc::new(Mutex::new(Vec::new())),
            handshake_rx: Some(hs_rx),
        };

        (client_chan, conn, mock)
    }

    /// Schedule an event to be injected after a pane is created.
    /// The event is sent to the pane identified by `target` after
    /// a short delay.
    pub fn inject_after_create(&mut self, target: PaneId, event: CompToClient) {
        self.injections.push((target, event));
    }

    /// Schedule a Close event for the first pane created, after a delay.
    /// Convenience for the hello-pane test.
    pub fn close_first_pane_after(&mut self, _delay: Duration) {
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

    /// Get a sender for injecting events directly into the client connection.
    /// Use this to send CompToClient messages from test code.
    pub fn sender(&self) -> mpsc::Sender<CompToClient> {
        self.conn.sender.clone()
    }

    /// Run the mock compositor. Processes messages until the connection closes.
    ///
    /// If created via `pair_with_handshake()`, runs the server-side handshake
    /// first (recv ClientHello, send ServerHello, recv ClientCaps, accept).
    pub fn run(mut self) {
        // If a handshake channel was provided, run the server handshake first
        if let Some(hs_rx) = self.handshake_rx.take() {
            if let Ok(server_chan) = hs_rx.recv() {
                // Server handshake: recv hello, send hello, recv caps, accept
                let (hello, s) = server_chan.recv().expect("handshake: recv ClientHello");
                let s = s.send(ServerHello {
                    compositor: "pane-mock".into(),
                    version: 1,
                }).expect("handshake: send ServerHello");
                let (caps, s) = s.recv().expect("handshake: recv ClientCaps");
                // Always accept in mock
                let s = s.select_left().expect("handshake: select Accept");
                let s = s.send(Accepted { caps: caps.caps }).expect("handshake: send Accepted");
                let _ = hello; // used for logging if needed
                s.close();
            }
        }

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
                            // Send on a separate thread with delay to ensure
                            // the kit has time to register the pane's channel
                            let sender = self.conn.sender.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(200));
                                let _ = sender.send(event);
                            });
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
