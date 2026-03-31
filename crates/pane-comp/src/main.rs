mod cell;
mod glyph_atlas;
mod pane_renderer;
mod server;
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

    // --- protocol server ---
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| anyhow::anyhow!("XDG_RUNTIME_DIR not set"))?;
    let (protocol_server, listener) = server::ProtocolServer::new(std::path::Path::new(&runtime_dir))
        .map_err(|e| anyhow::anyhow!("protocol server: {e}"))?;
    let comp_instance_id = protocol_server.instance_id.clone();

    let (handshake_tx, handshake_rx) = std::sync::mpsc::channel();

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
        server: protocol_server,
        handshake_rx,
    };

    // --- calloop event loop ---
    let mut event_loop: EventLoop<'_, CompState> =
        EventLoop::try_new().map_err(|e| anyhow::anyhow!("calloop init: {e}"))?;

    let loop_handle = event_loop.handle();

    // --- listener socket (accepts new client connections) ---
    let generic_listener = calloop::generic::Generic::new(
        listener,
        calloop::Interest::READ,
        calloop::Mode::Level,
    );
    let generic_listener: calloop::generic::Generic<std::os::unix::net::UnixListener> = generic_listener;

    let hs_sender = handshake_tx;
    let hs_instance_id = comp_instance_id;
    loop_handle.insert_source(generic_listener, move |_event, listener, state: &mut CompState| {
        // Accept all pending connections
        loop {
            let result: std::io::Result<_> = listener.accept();
            match result {
                Ok((stream, _addr)) => {
                    info!("new client connection");
                    let client_id = state.server.alloc_client_id();
                    let sender = hs_sender.clone();

                    let iid = hs_instance_id.clone();
                    std::thread::spawn(move || {
                        match server::run_server_handshake(stream, &iid) {
                            Ok(stream) => {
                                let _ = sender.send((client_id, stream));
                            }
                            Err(e) => {
                                warn!("handshake failed: {}", e);
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    warn!("accept error: {}", e);
                    break;
                }
            }
        }
        Ok(calloop::PostAction::Continue)
    }).map_err(|e| anyhow::anyhow!("listener source: {e}"))?;

    // Frame timer — triggers rendering at ~60fps.
    // Client messages dispatch immediately via calloop SessionSource
    // callbacks (registered by poll_handshakes); the frame timer only
    // handles handshake polling, winit events, and rendering.
    let timer = Timer::from_duration(FRAME_INTERVAL);
    let frame_loop_handle = loop_handle.clone();
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

        // --- Frame budget telemetry ---
        let frame_start = std::time::Instant::now();

        state.poll_handshakes(&frame_loop_handle);

        let protocol_elapsed = frame_start.elapsed();

        state.render_frame();

        let total_elapsed = frame_start.elapsed();
        if total_elapsed.as_millis() > 8 {
            warn!("frame budget: protocol {}ms + render {}ms = {}ms",
                protocol_elapsed.as_millis(),
                (total_elapsed - protocol_elapsed).as_millis(),
                total_elapsed.as_millis());
        }

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
