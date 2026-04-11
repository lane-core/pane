---
name: Plan 9 reference audit completed
description: Deep reading of vendored Plan 9 man pages and papers, insights in serena pane/plan9_reference_insights, divergences tracker updated
type: project
---

Completed deep reading of vendored Plan 9 reference material in `reference/plan9/`. Key documents read:
- All specified man pages (5/0intro, 5/attach, 5/walk, 5/clunk, 4/namespace, 4/import, 4/exportfs, 4/factotum, 4/rio, 4/plumber, 4/srv, 2/thread, 2/sleep, 3/draw, 3/proc, 3/srv, 3/cons, 6/namespace, 6/plumb, 1/bind, 8/drawterm)
- All specified papers (names.ms full, plumb.ms full, 8½.ms full, auth.ms partial, net.ms partial)

**Why:** Pane needed concrete citations from primary sources rather than secondhand summaries.

**How to apply:** Agents implementing Plan 9-influenced features should read `pane/plan9_reference_insights` in serena memory before starting. The divergences tracker (`pane/plan9_divergences`) was expanded with new sections: diagnostic/debugging patterns, terminal/window architecture, updated observer/resilience/export sections, factotum detail.

**Key discoveries not previously tracked:**
1. rio's wctl blocking-read IS Plan 9's observer pattern — dual-path (blocking read + push protocol) confirmed as right design
2. plumber(4) did multicast natively — pane routing must preserve this
3. plumber `click` attribute for context refinement — not yet adopted, should be
4. factotum `confirm`/`needkey` interactive consent patterns — worth adopting for sensitive remote operations
5. `iostats` transparent proxy — maps to Transport trait for diagnostic wrapping
6. rio consctl revert-on-close — lease pattern, perfect for Rust RAII
7. aan(8) session resilience — should inform ReconnectingTransport wrapper design
