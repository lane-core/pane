# Command Reference

## Build & Test
- `just test` — run all workspace tests
- `just check` — check all crates without building
- `just lint` — clippy with `-D warnings`
- `just fmt` — format Rust (cargo fmt) and Nix (nixpkgs-fmt)
- `just doc` — generate API docs
- `cargo test -p pane-app` — test a specific crate
- `cargo test -p pane-app -- filter_wants` — test name filter

## Long output
- `cargo test 2>&1 | tee /tmp/test.log | tail -40`

## Cross-build (macOS → Linux)
- `just build-linux` — build via nix linux-builder
- `just vm-fresh` — build and boot test VM
- `just vm-ssh` — SSH into test VM

## System
- Shell: ksh. Tools: git, cargo, nix, just (all via direnv).
