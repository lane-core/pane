---
name: Plan 9 lineage documentation audit
description: Audit results for Plan 9 heritage annotations in pane codebase — what exists, what's needed, licensing for reference docs
type: project
---

Plan 9 lineage annotation audit completed 2026-03-31.

**Existing annotations (6):** connect_remote (app.rs), clipboard module, tcp.rs, tls.rs, pane-headless main.rs, PaneId (message.rs).

**New annotations identified (10):** PaneCreateFuture (clunk-on-abandon), App struct (connection model), looper.rs module (per-pane = per-process namespace), Messenger struct (transport-transparent handle), ExitBroadcaster (divergence from /proc polling), pane-session lib.rs (session types vs 9P convention), Transport trait (transport independence), SessionError (connection-drop robustness), unix transport (mount pipe parallel), PeerIdentity (Tauth/Rattach ordering). Also expand existing pane-headless annotation (cite rio, drawterm).

**Why:** pane systematically documents BeOS heritage with `# BeOS` sections but lacked parallel `# Plan 9` coverage for the distributed systems lineage. The annotations help developers understand why the distributed architecture has its shape.

**How to apply:** When implementing the annotations, follow the draft texts in the audit report. The kit-documentation-style.md needs a new "Heritage Annotations: `# Plan 9`" section defining format and citation conventions. A `pane/plan9_divergences` Serena memory should track adopted/diverged/rejected Plan 9 patterns (modeled after beapi_divergences).

**Licensing finding:** Plan 9 man pages + source tree papers are MIT licensed (Plan 9 Foundation, 2021). Safe to include as `reference/plan9/` directory paralleling `reference/haiku-book/`. Academic publisher copies of papers have separate copyright — use `/sys/doc/` versions.
