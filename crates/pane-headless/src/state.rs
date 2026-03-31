//! Headless server state.
//!
//! Manages the protocol server and client registration without
//! any rendering concerns. Analogous to `CompState` in pane-comp
//! minus smithay, the glyph atlas, and the renderer.

use std::os::unix::net::UnixStream;

use tracing::{info, warn};

use pane_proto::protocol::{ClientToComp, PaneGeometry};
use pane_session::calloop::{SessionEvent, SessionSource};
use calloop::PostAction;

/// Server state for the headless compositor.
pub struct HeadlessState {
    /// The protocol server — client/pane management, message routing.
    pub server: pane_server::ProtocolServer,
    /// Whether the server is running (false = exit the event loop).
    pub running: bool,
    /// Default geometry for headless panes.
    pub default_geometry: PaneGeometry,
}

impl HeadlessState {
    /// Register a client after handshake completes.
    ///
    /// Clones the stream (one for write registration in ProtocolServer,
    /// one for read registration as calloop SessionSource). This is the
    /// same pattern as `CompState::poll_handshakes` in pane-comp.
    pub fn register_client(
        &mut self,
        client_id: usize,
        stream: UnixStream,
        loop_handle: &calloop::LoopHandle<'_, HeadlessState>,
    ) {
        let write_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                warn!("clone stream for client {}: {}", client_id, e);
                return;
            }
        };

        self.server.register_client(client_id, write_stream);

        let source = match SessionSource::new(stream) {
            Ok(s) => s,
            Err(e) => {
                warn!("session source for client {}: {}", client_id, e);
                self.server.remove_client(client_id);
                return;
            }
        };

        let geometry = self.default_geometry;
        if let Err(e) = loop_handle.insert_source(source, move |event, _, state: &mut HeadlessState| {
            match event {
                SessionEvent::Message(bytes) => {
                    match pane_proto::deserialize::<ClientToComp>(&bytes) {
                        Ok(msg) => {
                            state.server.handle_message(client_id, msg, geometry);
                        }
                        Err(e) => {
                            warn!("bad message from client {}: {}", client_id, e);
                        }
                    }
                    Ok(PostAction::Continue)
                }
                SessionEvent::Disconnected => {
                    info!("client {} disconnected", client_id);
                    state.server.remove_client(client_id);
                    Ok(PostAction::Remove)
                }
            }
        }) {
            warn!("insert session source for client {}: {}", client_id, e);
            self.server.remove_client(client_id);
        } else {
            info!("client {} registered (headless)", client_id);
        }
    }
}
