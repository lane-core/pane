---
name: Architecture spec review findings (2026-03-20)
description: Key gaps and recommendations from deep architecture spec review — scripting protocol absent, optics ungrounded, kit decomposition thin, pane-route artifacts
type: project
---

Major findings from architecture spec review against foundations spec:

1. **Scripting protocol entirely absent** — foundations makes "session types + optics = scripting protocol" the convergence point of three theoretical commitments, architecture spec says nothing about it. This is the single largest gap.

2. **Optics not grounded in architecture** — foundations dedicates section 4 to optics as multi-view consistency mechanism, architecture mentions views but never connects to optics or lens laws.

3. **Kit decomposition too thin** — missing Media Kit, Input Kit, Translation Kit equivalent. pane-ui description anemic compared to its importance. pane-ai disproportionately large (UX design mixed with kit architecture).

4. **pane-route artifacts** — routing subsection under Servers despite being a kit concern. Research documents still reference pane-route as a server.

5. **Missing server interactions** — startup ordering, server discovery/recovery, clipboard, MIME/file type recognition all unspecified.

6. **Input server tradeoff unacknowledged** — BeOS had separate input_server; pane folds input into compositor without justifying the design decision.

**Why:** Architecture spec appears written before foundations reached current form (before scripting protocol insight and router elimination).

**How to apply:** The architecture spec rewrite should start from foundations commitments and build architecture to serve them, not revise existing text.

Review written to: `openspec/changes/spec-tightening/review-architecture-be-engineer.md`
