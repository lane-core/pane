mod glyph_atlas;
mod pane_renderer;

use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            gles::GlesRenderer,
            Bind, Frame, Renderer,
            Color32F,
        },
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    utils::{Rectangle, Size, Transform},
};
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
        eprintln!("pane-comp fatal: {e}");
        std::process::exit(1);
    }

    info!("pane-comp shutdown");
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize winit backend — opens a window on the host desktop
    let (mut backend, mut winit_evt) = winit::init::<GlesRenderer>()?;

    let size = backend.window_size();
    info!("window size: {}x{}", size.w, size.h);

    // Create a virtual output matching the window
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

    // Initialize glyph atlas and pane renderer
    let renderer = backend.renderer();
    let mut atlas = GlyphAtlas::new(14.0)?;
    atlas.load_ascii(renderer)?;
    info!(
        "glyph atlas: cell {}x{}, {} glyphs loaded",
        atlas.cell_width(),
        atlas.cell_height(),
        atlas.glyph_count()
    );

    let _pane_renderer = PaneRenderer::new(&atlas);

    // Track window size for resize handling
    let mut current_size = size;

    // Event loop
    let mut running = true;
    while running {
        // Dispatch winit events
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
                info!("resized: {}x{} pixels, {}x{} cells", new_size.w, new_size.h, cols, rows);
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

        // Render frame
        let size = current_size;
        let output_size = Size::from((size.w as i32, size.h as i32));
        let output_rect = Rectangle::from_size(output_size);

        {
            let (renderer, mut target) = backend.bind()?;
            let mut frame = renderer.render(&mut target, output_size, Transform::Normal)?;
            frame.clear(BG_COLOR, &[output_rect])?;
            frame.finish()?;
        }

        backend.submit(Some(&[output_rect]))?;

        // Yield — don't spin
        std::thread::sleep(Duration::from_millis(16));
    }

    info!("compositor exiting cleanly");
    Ok(())
}
