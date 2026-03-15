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
use tracing::{info, warn};

use glyph_atlas::GlyphAtlas;
use pane_renderer::PaneRenderer;

/// Background color — dark grey, 90s-inspired
const BG_COLOR: Color32F = Color32F::new(0.12, 0.12, 0.14, 1.0);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pane_comp=info".into()),
        )
        .init();

    info!("pane-comp starting");

    if let Err(e) = run() {
        eprintln!("pane-comp fatal: {e:?}");
        std::process::exit(1);
    }

    info!("pane-comp shutdown");
}

fn run() -> Result<()> {
    // Log Wayland environment for debugging
    info!("WAYLAND_DISPLAY={:?}", std::env::var("WAYLAND_DISPLAY").ok());
    info!("XDG_RUNTIME_DIR={:?}", std::env::var("XDG_RUNTIME_DIR").ok());
    info!("DISPLAY={:?}", std::env::var("DISPLAY").ok());

    let (mut backend, mut winit_evt) = winit::init::<GlesRenderer>()
        .context("winit backend init")?;

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
    let mut atlas = GlyphAtlas::new(14.0)?;
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
            let (renderer, mut target) = backend.bind()?;
            let mut frame = renderer.render(&mut target, output_size, Transform::Normal)?;
            frame.clear(BG_COLOR, &[output_rect])?;

            if let Err(e) = pane_renderer.render(&mut frame, &atlas, output_size) {
                warn!("pane render error: {e}");
            }

            frame.finish()?;
        }

        backend.submit(Some(&[output_rect]))?;

        std::thread::sleep(Duration::from_millis(16));
    }

    info!("compositor exiting cleanly");
    Ok(())
}
