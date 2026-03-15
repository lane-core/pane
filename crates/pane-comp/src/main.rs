mod glyph_atlas;
mod pane_renderer;

use tracing::info;

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
    // Phase 2: smithay winit backend + cell grid rendering
    // Stubbed for now — will be filled in tasks 2.1-2.4
    info!("compositor skeleton — not yet implemented");
    Ok(())
}
