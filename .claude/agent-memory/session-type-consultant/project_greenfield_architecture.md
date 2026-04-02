---
name: Greenfield architecture decision (2026-03-31)
description: From-scratch architecture analysis — per-protocol traits over shared state, CompositorEvent/service split, Sigma<H> dispatch, builder registration
type: project
---

Clean-slate architecture analysis requested 2026-03-31. All prior expediency arguments stripped.

**Core decisions:**

1. **Handler structure**: Per-protocol traits (`PaneHandler`, `ClipboardHandler`, `ObserverHandler`, `DragHandler`) over shared `&mut self` state. NOT monolithic, NOT separate state machines. Grounded in EAct TH-Handler rule (Fig. 8): sigma indexed by session endpoint, all entries share actor state type A.

2. **Pane identity**: Infrastructure takes handler (`pane.run_with(handler)`). NOT Pane-as-trait. Grounded in EAct actor 4-tuple: actor *contains* sigma, sigma is not the actor.

3. **Message enum**: Split into `CompositorEvent` (fully Clone, filter-visible) and per-service event enums (`ClipboardEvent`, etc.). Eliminates 4 panic branches in Clone. Grounded in EAct: plain vs obligation-carrying messages are distinct types.

4. **Service channels**: Per-protocol typed calloop sources. Service events bypass filter chain, dispatch directly to service handler methods. Grounded in EAct per-session queues.

5. **Capability declaration**: Builder pattern at pane construction (`pane.with_clipboard().run_with(handler)`). Trait bounds checked at registration site, not globally. Grounded in EAct E-Suspend (sigma grows dynamically).

6. **Filters**: Operate on `CompositorEvent` only. Service events never filtered. Grounded in MPST: filters are channel transformers, not handler store entries.

7. **Looper**: `Sigma<H>` struct holds handler + service dispatch function pointers captured at registration time (monomorphized). EAct sigma defunctionalized into Rust.

**Key insight**: `ClipboardHandler: PaneHandler` as supertrait — every service handler is also a compositor handler. This lets the builder chain accumulate bounds: `pane.with_clipboard().run_with::<H: PaneHandler + ClipboardHandler>()`.

**Affine gap**: 4 Clone panics eliminated by design. All typestate handles preserved unchanged. Filter chain only sees Clone types.

**How to apply:** This supersedes the prior "commit register_channel now, defer trait split" decision. The architecture should be built as described here if starting fresh or when the next major refactor occurs.
