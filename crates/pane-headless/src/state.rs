//! Headless server state.
//!
//! Manages the protocol server and client registration without
//! any rendering concerns. Analogous to `CompState` in pane-comp
//! minus smithay, the glyph atlas, and the renderer.

use std::net::TcpStream;
use std::os::unix::net::UnixStream;

use tracing::{info, warn};

use pane_proto::protocol::{ClientToComp, PaneGeometry};
use pane_server::ClientStream;
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
    /// Register a unix client after handshake completes.
    pub fn register_unix_client(
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

        self.server.register_client(client_id, ClientStream::Unix(write_stream));

        let source = match SessionSource::new(stream) {
            Ok(s) => s,
            Err(e) => {
                warn!("session source for client {}: {}", client_id, e);
                self.server.remove_client(client_id);
                return;
            }
        };

        self.insert_client_source(client_id, source, loop_handle);
    }

    /// Register a TCP client after handshake completes.
    pub fn register_tcp_client(
        &mut self,
        client_id: usize,
        stream: TcpStream,
        loop_handle: &calloop::LoopHandle<'_, HeadlessState>,
    ) {
        let write_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                warn!("clone tcp stream for client {}: {}", client_id, e);
                return;
            }
        };

        self.server.register_client(client_id, ClientStream::Tcp(write_stream));

        // Set non-blocking for calloop
        if let Err(e) = stream.set_nonblocking(true) {
            warn!("set_nonblocking for tcp client {}: {}", client_id, e);
            self.server.remove_client(client_id);
            return;
        }

        let read_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                warn!("clone tcp read stream for client {}: {}", client_id, e);
                self.server.remove_client(client_id);
                return;
            }
        };

        let source = match SessionSource::from_streams(stream, read_stream) {
            Ok(s) => s,
            Err(e) => {
                warn!("tcp session source for client {}: {}", client_id, e);
                self.server.remove_client(client_id);
                return;
            }
        };

        self.insert_client_source(client_id, source, loop_handle);
    }

    /// Insert a calloop SessionSource for a client and wire up message dispatch.
    fn insert_client_source<S>(
        &mut self,
        client_id: usize,
        source: SessionSource<S>,
        loop_handle: &calloop::LoopHandle<'_, HeadlessState>,
    )
    where
        S: std::os::unix::io::AsFd + std::io::Read + 'static,
    {
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
