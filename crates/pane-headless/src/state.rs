//! Headless server state.
//!
//! Manages the protocol server and client registration without
//! any rendering concerns. Analogous to `CompState` in pane-comp
//! minus smithay, the glyph atlas, and the renderer.

use std::fs::File;
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;

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
    /// Protocol trace file, shared across all client connections.
    /// `None` when `--protocol-trace` is not specified.
    pub trace_writer: Option<Arc<Mutex<File>>>,
    /// Epoch for trace timestamps.
    pub trace_epoch: Instant,
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

    /// Log an active-phase protocol message to the trace file.
    ///
    /// Best-effort — trace write failures are silently ignored.
    /// Tracing must never break the server.
    fn trace_message(&self, client_id: usize, direction: &str, bytes: &[u8], label: &str) {
        if let Some(ref tw) = self.trace_writer {
            let elapsed = self.trace_epoch.elapsed();
            let preview_len = bytes.len().min(64);
            let hex: String = bytes[..preview_len]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let truncated = if bytes.len() > 64 { "..." } else { "" };
            if let Ok(mut f) = tw.lock() {
                let _ = writeln!(
                    f,
                    "{:>4}.{:03} client-{} {} {} {} bytes [{}{}]",
                    elapsed.as_secs(),
                    elapsed.subsec_millis(),
                    client_id,
                    direction,
                    label,
                    bytes.len(),
                    hex,
                    truncated,
                );
            }
        }
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
                    state.trace_message(client_id, "<<", &bytes, "ClientToComp");
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
