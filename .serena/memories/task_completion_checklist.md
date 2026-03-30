# Task Completion Checklist

When a coding task is completed:

1. **Run tests:** `cargo test -p <crate>` for affected crates
2. **Check compilation:** `cargo check` for workspace
3. **Verify no clippy warnings:** `just lint`
4. **Count tests:** Verify test count hasn't decreased
5. **Tee output to /tmp:** Always `| tee /tmp/test.log | tail -40` for long output

After completing a planned multi-phase task where all tests and doc builds pass, commit the results with a descriptive message. For single-file or ad-hoc changes, check before committing. Do NOT push unless explicitly asked.
