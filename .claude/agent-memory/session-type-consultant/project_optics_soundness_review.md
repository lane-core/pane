---
name: Optics layer soundness review (2026-03-30)
description: Session-type and concurrency theory review of pane-optic design brief. Conditionally sound verdict with four invariants.
type: project
---

Reviewed the pane-optic design brief against session-type theory on 2026-03-30.

**Verdict:** Conditionally sound.

**Key findings:**

1. `dyn Any` downcast boundary in `DynOptic` is computation-level, not protocol-level (Ferrite Section 3.1 distinction). Protocol guarantees (reply discipline) are preserved. Invariant: downcast failure must return Err, not panic.

2. `CompletionReplyPort` ownership-handle pattern works across wire boundary. Same affine gap compensation as ReplyPort. Implementation should use `Option<mpsc::Sender<ClientToComp>>` with take() pattern.

3. `resolve_chain` safety checks (max depth, cycle detection, same-pane) are necessarily runtime. Dependent types (TLL+C style) would be needed for static enforcement. Rust can't express this.

4. `ScriptReply(ReplyPort)` composition is correct -- newtype is transparent to Drop chain. Panic in `optic.set()` kills the pane (BLooper fault domain model), ReplyFailed sent via panic unwinding Drop.

**Critical invariant:** `panic = unwind` required. With `panic = abort`, all affine gap compensation (ReplyPort, CompletionReplyPort, ScriptReply) fails silently.

**Why:** This review establishes the theoretical grounding for the optic implementation phase.

**How to apply:** Reference these findings when implementing pane-optic. The proc macro must generate downcast-or-error patterns mechanically. ScriptReply must NOT implement custom Drop.
