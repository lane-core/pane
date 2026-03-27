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

use crate::glyph_atlas::GlyphAtlas;
use crate::pane_renderer::PaneRenderer;

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
