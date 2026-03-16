mod glyph_atlas;
mod pane_renderer;

use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            gles::GlesRenderer,
            Frame, Renderer,
            Color32F,
        },
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    utils::{Rectangle, Size, Transform},
};
use anyhow::{Context, Result};
use bpaf::Bpaf;
use tracing::{info, warn};

use glyph_atlas::GlyphAtlas;
use pane_renderer::PaneRenderer;

/// Background color — dark grey, 90s-inspired
/// Desktop background — warm light grey, Frutiger Aero era
const BG_COLOR: Color32F = Color32F::new(0.82, 0.81, 0.78, 1.0);

/// pane desktop environment compositor
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
struct Opts {
    /// font family name
    #[bpaf(long, fallback("monospace".to_string()))]
    font: String,

    /// font size in points
    #[bpaf(long, fallback(14.0))]
    font_size: f32,

    /// log level (error, warn, info, debug, trace)
    #[bpaf(long("log"), fallback("info".to_string()))]
    log_level: String,
}

fn main() {
    let opts = opts().run();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("pane_comp={},smithay=debug", opts.log_level).into()),
        )
        .init();

    info!("pane-comp starting");
    info!("font: {} {}pt", opts.font, opts.font_size);

    if let Err(e) = run(&opts) {
        eprintln!("pane-comp fatal: {e:?}");
        std::process::exit(1);
    }

    info!("pane-comp shutdown");
}

fn run(opts: &Opts) -> Result<()> {
    // Log Wayland environment for debugging
    info!("WAYLAND_DISPLAY={:?}", std::env::var("WAYLAND_DISPLAY").ok());
    info!("XDG_RUNTIME_DIR={:?}", std::env::var("XDG_RUNTIME_DIR").ok());
    info!("DISPLAY={:?}", std::env::var("DISPLAY").ok());

    let (mut backend, mut winit_evt): (smithay::backend::winit::WinitGraphicsBackend<GlesRenderer>, _) =
        winit::init()
            .map_err(|e| anyhow::anyhow!("winit backend init: {e:?}"))?;

    let size = backend.window_size();
    info!("window size: {}x{}", size.w, size.h);

    let output = Output::new(
        "pane-winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "pane".to_string(),
            model: "winit".to_string(),
        },
    );

    let mode = Mode {
        size: (size.w as i32, size.h as i32).into(),
        refresh: 60_000,
    };
    output.change_current_state(Some(mode), Some(Transform::Normal), None, Some((0, 0).into()));
    output.set_preferred(mode);

    // Initialize glyph atlas
    let mut atlas = GlyphAtlas::new(opts.font_size)?;
    let renderer = backend.renderer();
    atlas.load_ascii(renderer)?;
    info!(
        "glyph atlas: cell {}x{}, {} glyphs loaded",
        atlas.cell_width(),
        atlas.cell_height(),
        atlas.glyph_count()
    );

    let pane_renderer = PaneRenderer::new(&atlas);

    let mut current_size = size;
    let mut running = true;

    while running {
        winit_evt.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size: new_size, .. } => {
                current_size = new_size;
                let new_mode = Mode {
                    size: (new_size.w as i32, new_size.h as i32).into(),
                    refresh: 60_000,
                };
                output.change_current_state(Some(new_mode), None, None, None);
                let (cols, rows) = (
                    new_size.w as u16 / atlas.cell_width(),
                    new_size.h as u16 / atlas.cell_height(),
                );
                info!("resized: {}x{} px, {}x{} cells", new_size.w, new_size.h, cols, rows);
            }
            WinitEvent::Input(_) => {}
            WinitEvent::Focus(_) => {}
            WinitEvent::Redraw => {}
            WinitEvent::CloseRequested => {
                info!("close requested");
                running = false;
            }
        });

        if !running {
            break;
        }

        // Render
        let output_size = Size::from((current_size.w as i32, current_size.h as i32));
        let output_rect = Rectangle::from_size(output_size);

        {
            let (renderer, mut target) = backend.bind()
                .map_err(|e| anyhow::anyhow!("bind: {e}"))?;
            let mut frame = renderer.render(&mut target, output_size, Transform::Normal)
                .map_err(|e| anyhow::anyhow!("render: {e}"))?;
            frame.clear(BG_COLOR, &[output_rect])
                .map_err(|e| anyhow::anyhow!("clear: {e}"))?;

            if let Err(e) = pane_renderer.render(&mut frame, &atlas, output_size) {
                warn!("pane render error: {e}");
            }

            frame.finish()
                .map_err(|e| anyhow::anyhow!("finish: {e}"))?;
        }

        backend.submit(Some(&[output_rect]))
            .map_err(|e| anyhow::anyhow!("submit: {e}"))?;

        std::thread::sleep(Duration::from_millis(16));
    }

    info!("compositor exiting cleanly");
    Ok(())
}
