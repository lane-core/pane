# Task Completion Checklist

When a coding task is completed:

1. **Run tests:** `cargo test -p <crate>` for affected crates
2. **Check compilation:** `cargo check` for workspace
3. **Verify no clippy warnings:** `just lint`
4. **Count tests:** Verify test count hasn't decreased
5. **Tee output to /tmp:** Always `| tee /tmp/test.log | tail -40` for long output

Do NOT commit unless explicitly asked. Do NOT push unless explicitly asked.
