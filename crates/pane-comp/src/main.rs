mod cell;
mod glyph_atlas;
mod pane_renderer;
mod state;

use std::time::Duration;

use smithay::{
    backend::{
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    utils::{Size, Transform},
};
use calloop::{EventLoop, timer::{Timer, TimeoutAction}};
use anyhow::Result;
use bpaf::Bpaf;
use tracing::{info, warn};

use glyph_atlas::GlyphAtlas;
use pane_renderer::PaneRenderer;
use state::CompState;

/// Target frame interval (~60fps)
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

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
                .unwrap_or_else(|_| format!("pane_comp={}", opts.log_level).into()),
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
    info!("WAYLAND_DISPLAY={:?}", std::env::var("WAYLAND_DISPLAY").ok());
    info!("XDG_RUNTIME_DIR={:?}", std::env::var("XDG_RUNTIME_DIR").ok());

    // --- smithay winit backend ---
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

    // --- glyph atlas ---
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
    let cell_width = atlas.cell_width();
    let cell_height = atlas.cell_height();

    // --- compositor state ---
    let mut comp_state = CompState {
        backend,
        output,
        size: Size::from((size.w as i32, size.h as i32)),
        atlas,
        pane_renderer,
        running: true,
        cell_width,
        cell_height,
    };

    // --- calloop event loop ---
    let mut event_loop: EventLoop<'_, CompState> =
        EventLoop::try_new().map_err(|e| anyhow::anyhow!("calloop init: {e}"))?;

    let loop_handle = event_loop.handle();

    // Frame timer — triggers rendering at ~60fps
    let timer = Timer::from_duration(FRAME_INTERVAL);
    loop_handle.insert_source(timer, move |_deadline, _metadata, state: &mut CompState| {
        // Dispatch pending winit events
        winit_evt.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size: new_size, .. } => {
                state.size = Size::from((new_size.w as i32, new_size.h as i32));
                let new_mode = Mode {
                    size: state.size,
                    refresh: 60_000,
                };
                state.output.change_current_state(Some(new_mode), None, None, None);
                let cols = new_size.w as u16 / state.cell_width;
                let rows = new_size.h as u16 / state.cell_height;
                info!("resized: {}x{} px, {}x{} cells", new_size.w, new_size.h, cols, rows);
            }
            WinitEvent::Input(_) => {}
            WinitEvent::Focus(_) => {}
            WinitEvent::Redraw => {}
            WinitEvent::CloseRequested => {
                info!("close requested");
                state.running = false;
            }
        });

        if !state.running {
            return TimeoutAction::Drop;
        }

        // Render frame
        state.render_frame();

        // Reschedule for next frame
        TimeoutAction::ToDuration(FRAME_INTERVAL)
    }).map_err(|e| anyhow::anyhow!("timer source: {e}"))?;

    info!("entering calloop event loop");

    // Run the event loop
    while comp_state.running {
        event_loop.dispatch(Some(Duration::from_millis(100)), &mut comp_state)
            .map_err(|e| anyhow::anyhow!("calloop dispatch: {e}"))?;
    }

    info!("compositor exiting cleanly");
    Ok(())
}
