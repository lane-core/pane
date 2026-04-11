---
type: analysis
status: archived
archived: 2026-04-11
created: 2026-04-05
last_updated: 2026-04-05
importance: low
keywords: [design_checkpoint, ergonomics, entry_point, messenger, fuse, roster, be_plan9_roundtable]
related: [analysis/verification/_hub, decision/vertical_slice_first_pane]
---

# Design Checkpoint: Dev API & Ergonomics Review (2026-04-05)

After PeerAuth implementation, both Be and Plan 9 agents reviewed the full codebase for trajectory alignment with classic design patterns.

## Verdict: infrastructure is sound, developer surface is missing

Core type designs (Protocol, Handles<P>, Handler, obligation handles, PeerAuth, MessageFilter, MonadicLens) are improvements over predecessors. Obligation handles called "unambiguously better" than BMessage reply. pane-fs namespace design confirmed faithful to Plan 9. Dual API (kit + filesystem) is the right call.

## Critical gaps

1. **No entry point.** No public way to get a Pane from userland. Need `pane::connect("com.example.hello")` as BApplication constructor equivalent.
2. **No inter-pane addressing.** Messenger is control-only (set_content etc.), not an addressing mechanism. Needs target concept — `Messenger::for_pane(id)` or construction from pane-fs path.
3. **FUSE mount is the validation surface.** PaneEntry/AttrSet/AttrReader exist but nothing serves them. Without it, the Plan 9 promise is a design doc. Schedule right after calloop-backed looper.

## High-priority items

4. **Flow ergonomics.** 99% of handler returns are Continue. `#[pane::protocol_handler]` macro should default to `()` → Continue.
5. **Blocking-read observer file.** `/pane/<n>/event` — one line per state change, blocks between. The rio wctl pattern for scriptability.
6. **Roster / discovery.** Launch-by-signature, lifecycle notification, MIME association.

## Be-specific findings
- 13-line hello world achievable once entry point exists
- ReplyPort must arrive in same method as request (not split callbacks) for macro ergonomics
- No ad-hoc data container (typed enums lack BMessage flexibility for unplanned cooperation)
- Observer pattern undecided (in-process StartWatching vs filesystem watches)

## Plan 9-specific findings
- pane-fs correctly avoids union directories (computed projections, not mounts)
- Per-process namespaces biggest philosophical loss — global FUSE mount, mitigated by filter views
- Need staleness metadata for remote panes (attrs/connected, errno differentiation)
- Need consent mechanism for remote connections (factotum confirm → .plan)
- Need connection resilience (aan equivalent — ReconnectingTransport)
- FrameCodec blocking reads need calloop integration strategy
