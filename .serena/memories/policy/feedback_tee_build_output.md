---
type: policy
status: current
supersedes: [pane/feedback_tee_build_output, auto-memory/feedback_tee_build_output]
created: 2026-03-27
last_updated: 2026-04-10
importance: normal
keywords: [build_output, tee, tmp, long_running, progress]
agents: [pane-architect]
---

# Tee Build Output to /tmp

**Rule:** When running builds or tests that produce long output,
always tee to a /tmp file even when piping through tail for
display. The user may want to check progress on a long-running
build.

**Why:** Builds via linux-builder can take minutes. The user wants
to be able to check what's happening without waiting for the tail
to complete.

**How to apply:** Use `nix build ... 2>&1 | tee /tmp/pane-build.log | tail -30`
instead of just `| tail -30`. Same for cargo test.
