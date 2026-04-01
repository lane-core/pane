//! Compositor state — the single mutable struct passed to all calloop callbacks.
//!
//! This is the Desktop equivalent: it owns everything the main thread needs.
//! Pane threads communicate with it via channels, not shared references.

use smithay::{
    backend::{
        renderer::{
            gles::GlesRenderer,
            Frame, Renderer,
            Color32F,
        },
        winit::WinitGraphicsBackend,
    },
    output::{Mode, Output},
    utils::{Rectangle, Size, Transform},
};

use calloop::PostAction;
use pane_session::calloop::{SessionEvent, SessionSource};
use pane_proto::protocol::{ClientToComp, PaneGeometry};

use crate::glyph_atlas::GlyphAtlas;
use crate::pane_renderer::PaneRenderer;
use crate::server::ProtocolServer;

/// Background color — warm light grey, Frutiger Aero era
const BG_COLOR: Color32F = Color32F::new(0.82, 0.81, 0.78, 1.0);

/// All compositor state lives here. Passed as `&mut` in calloop callbacks.
/// calloop's design gives exclusive access — no Arc<Mutex<>> needed.
pub struct CompState {
    /// The smithay winit backend (owns the window + renderer).
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    /// The smithay output (display configuration).
    pub output: Output,
    /// Current window/output size in pixels.
    pub size: Size<i32, smithay::utils::Physical>,
    /// Glyph atlas for text rendering.
    pub atlas: GlyphAtlas,
    /// Pane renderer (chrome, borders, body).
    pub pane_renderer: PaneRenderer,
    /// Whether the compositor is running.
    pub running: bool,
    /// Cell dimensions from the atlas.
    pub cell_width: u16,
    pub cell_height: u16,
    /// Protocol server — client connections and pane state.
    pub server: ProtocolServer,
    /// Completed handshakes waiting to be registered with calloop.
    pub handshake_rx: std::sync::mpsc::Receiver<(usize, std::os::unix::net::UnixStream)>,
}

impl CompState {
    /// Render a frame.
    pub fn render_frame(&mut self) {
        let output_size = self.size;
        let output_rect = Rectangle::from_size(output_size);

        let render_result = (|| -> anyhow::Result<()> {
            let (renderer, mut target) = self.backend.bind()
                .map_err(|e| anyhow::anyhow!("bind: {e}"))?;
            let mut frame = renderer.render(&mut target, output_size, Transform::Normal)
                .map_err(|e| anyhow::anyhow!("render: {e}"))?;
            frame.clear(BG_COLOR, &[output_rect])
                .map_err(|e| anyhow::anyhow!("clear: {e}"))?;

            if let Err(e) = self.pane_renderer.render(&mut frame, &self.atlas, output_size) {
                tracing::warn!("pane render error: {e}");
            }

            frame.finish()
                .map_err(|e| anyhow::anyhow!("finish: {e}"))?;
            Ok(())
        })();

        if let Err(e) = render_result {
            tracing::warn!("frame render error: {e}");
        }

        if let Err(e) = self.backend.submit(Some(&[output_rect])) {
            tracing::warn!("submit error: {e}");
        }
    }
}

impl CompState {
    /// Poll for completed handshakes and register new clients.
    /// Each client's read stream is registered as a calloop SessionSource
    /// for event-driven message dispatch (no per-client threads).
    pub fn poll_handshakes(
        &mut self,
        loop_handle: &calloop::LoopHandle<'_, CompState>,
    ) {
        while let Ok((client_id, stream)) = self.handshake_rx.try_recv() {
            let write_stream = match stream.try_clone() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("clone stream for client {}: {}", client_id, e);
                    continue;
                }
            };

            self.server.register_client(client_id, pane_server::ClientStream::Unix(write_stream), None);

            let source = match SessionSource::new(stream) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("session source for client {}: {}", client_id, e);
                    self.server.remove_client(client_id);
                    continue;
                }
            };

            if let Err(e) = loop_handle.insert_source(source, move |event, _, state: &mut CompState| {
                match event {
                    SessionEvent::Message(bytes) => {
                        match pane_proto::deserialize::<ClientToComp>(&bytes) {
                            Ok(msg) => {
                                let geometry = PaneGeometry {
                                    width: state.size.w as u32,
                                    height: state.size.h as u32,
                                    cols: state.size.w as u16 / state.cell_width,
                                    rows: state.size.h as u16 / state.cell_height,
                                };
                                state.server.handle_message(client_id, msg, geometry);
                            }
                            Err(e) => {
                                tracing::warn!("bad message from client {}: {}", client_id, e);
                            }
                        }
                        Ok(PostAction::Continue)
                    }
                    SessionEvent::Disconnected => {
                        state.server.remove_client(client_id);
                        Ok(PostAction::Remove)
                    }
                }
            }) {
                tracing::warn!("register source for client {}: {}", client_id, e);
                self.server.remove_client(client_id);
            }
        }
    }
}

impl CompState {
    /// Update the output mode on resize.
    pub fn handle_resize(&mut self, width: i32, height: i32) {
        self.size = Size::from((width, height));
        let new_mode = Mode {
            size: self.size,
            refresh: 60_000,
        };
        self.output.change_current_state(Some(new_mode), None, None, None);

        let cols = width as u16 / self.cell_width;
        let rows = height as u16 / self.cell_height;
        tracing::info!("resized: {}x{} px, {}x{} cells", width, height, cols, rows);
    }
}
