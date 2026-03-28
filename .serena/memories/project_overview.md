# pane — BeOS-inspired Desktop Environment

A Wayland compositor, application kit, and Linux distribution inspired by BeOS's message-passing discipline, Plan 9's protocol uniformity, and session type theory.

## Tech Stack
- **Language:** Rust (stable toolchain)
- **Build:** Cargo workspace, Nix flakes (flake-parts + rust-flake), direnv
- **Compositor:** smithay (Linux only, cross-built from macOS via nix linux-builder)
- **Widget rendering:** Vello (planned)
- **Serialization:** postcard (positional binary format)
- **Session types:** Custom `Chan<S, T>` typestate (not par crate)
- **Init system:** s6 specifically
- **Filesystem:** btrfs exclusively

## Workspace Crates
- `pane-session` — Session-typed channels with pluggable transport (memory, unix)
- `pane-proto` — Wire protocol types (events, messages, handshake, tags)
- `pane-notify` — Filesystem change notification (fanotify on Linux, polling stub on macOS)
- `pane-app` — Application kit (BApplication/BLooper/BHandler equivalent)
- `pane-comp` — Compositor (Linux only, smithay + calloop)

## Current Phase
Phase 3 (pane-app) complete with 81 tests. Stage 5 (BeAPI modernization) landed.
Phase 4 (compositor integration) next, but blocked on Linux for visual testing.
