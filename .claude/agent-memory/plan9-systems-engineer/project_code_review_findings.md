---
name: Code review findings — three-crate rewrite
description: Review of pane-proto, pane-session, pane-app against docs/architecture.md (par+EAct spec) — 2026-04-03
type: project
---

Review of the clean-slate three-crate implementation (pane-proto, pane-session, pane-app) against the current architecture spec (par + EAct, three error channels, no phases). Prior review findings from 2026-03-31 are fully stale — old crate structure (pane-server, pane-headless) no longer exists.

## High priority

1. **ServiceId missing UUID** — spec says `{ uuid: Uuid, name: &'static str }` with UUIDv5 from reverse-DNS. Code has `{ name: &'static str }` only. Blocks wire protocol (InterestAccepted sends `service_uuid: Uuid`). Spec's functoriality principle explicitly warns against bare-string identity. Fix: `service_id!()` proc-macro for compile-time UUIDv5.

2. **Box::leak on ServiceId deserialize** — untrusted wire input leaks heap strings to get `&'static str`. Bounded only if peers send known names. DoS vector. Fix: match against known set or use interning table.

3. **Wire framing not implemented** — Transport is raw bytes, no `[length:u32][service:u8][payload]`, no ProtocolAbort sentinel, no max_message_size check. Main implementation gap for wire readiness.

## Medium priority

4. **Transport panics directly instead of returning Result** — spec separates layers: Transport returns Result, Chan panics (par model). Current MemoryTransport calls `.expect()` directly. Layers should be distinct.

5. **Select/Branch/Queue/Server session types not implemented** — Chan only has Send/Recv. CLL branching needed for control protocol (DeclareInterest choice). Expected gap but blocks protocol work.

## Tracked (expected, not divergences)

6. **Handles<P>::receive and Handler methods lack `proxy: &Messenger` parameter** — crate boundary (pane-proto can't depend on pane-app). Will resolve via `#[pane::protocol_handler]` macro adding proxy at the call site. Spec's Handler definition is really the macro-generated surface.

7. **Handler missing pane_exited, supported_properties, request_received** — depends on types not yet defined (PropertyInfo, ReplyPort). Will land with those subsystems.

8. **Handshake ServiceBinding uses ServiceId not Uuid** — follows from finding #1.

## Retracted from prior review

- All 14 findings from 2026-03-31 review are stale (old crate structure removed).
- "Handler returns bare Flow" was correct per spec, not a divergence.
- "Missing Messenger" was a crate boundary issue, not a bug.

**Why:** Ensures the new three-crate implementation is measured against the correct (current) spec, not the old phased design.

**How to apply:** High-priority items (1-3) need design decisions before wire protocol work. Medium items (4-5) can be addressed during implementation. Tracked items are expected gaps.
