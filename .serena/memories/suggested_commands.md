# Development Commands

## Build & Test
- `just test` — Run all workspace tests
- `just check` — Check all crates without building
- `just lint` — Run clippy with `-D warnings`
- `just fmt` — Format Rust (cargo fmt) and Nix (nixpkgs-fmt)
- `just doc` — Generate API docs
- `cargo test -p pane-app` — Test a specific crate
- `cargo test -p pane-app -- filter_wants` — Run tests matching a name

## Cross-build (macOS → Linux)
- `just build-linux` — Build compositor via nix linux-builder
- `just vm-fresh` — Build and boot test VM
- `just vm-ssh` — SSH into test VM

## System (Darwin/macOS)
- `git`, `cargo`, `nix`, `just` — all available in direnv shell
- Shell: ksh (Korn shell)
- Always tee long build output to /tmp: `cargo test 2>&1 | tee /tmp/test.log | tail -40`
