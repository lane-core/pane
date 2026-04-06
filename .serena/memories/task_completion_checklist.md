# Task Completion Checklist

Run after any coding task. Technical verification only — process decisions (commit, workflow steps) are elsewhere.

1. `cargo test -p <crate>` for affected crates
2. `cargo check` for workspace
3. `just lint` — no clippy warnings
4. Verify test count hasn't decreased
5. Tee long output to /tmp: `| tee /tmp/test.log | tail -40`
