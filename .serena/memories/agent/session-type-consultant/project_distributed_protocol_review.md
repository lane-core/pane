---
name: Distributed protocol extensions soundness review (2026-03-30)
description: Session-type review of reconnect variants, UUID PaneId, payload changes for distributed computing. Option A reconnect (corrected polarity) sound; Option B unsound; UUID correlation sound with HashMap.
type: project
---

Reviewed proposed distributed computing protocol extensions on 2026-03-30.

**Reconnect:** Option A (type-level Select/Branch) is sound with corrected polarity -- client must Select, server must Branch. Option B (payload-level) unsound: violates protocol fidelity (Ferrite Theorem 4.3). Reconnect continuation should use dedicated ReconnectResult type, not reuse Accepted/Rejected.

**UUID PaneId:** Fire-and-forget with UUID correlation (HashMap<Uuid, Sender>) replaces FIFO ghost state (VecDeque). No sub-protocol needed -- active-phase enum variants sufficient. Compositor needs cleanup on client disconnect.

**Payload changes:** Sound pre-1.0. Postcard non-self-describing format means new->old works (trailing bytes ignored), old->new fails hard (deserialization error). Version field in ClientHello must gate compatibility.

**Affine gap finding:** No Drop impl on Chan. Server blocks on offer() with no timeout -- half-open TCP connections hang indefinitely. Need transport-level timeout for remote connections.

**Why:** These decisions shape the distributed protocol foundation (PLAN.md Phase 1).

**How to apply:** Reference when implementing TcpTransport and protocol extensions. The polarity correction on reconnect Branch is critical -- getting it wrong means the server selects reconnection, which is nonsensical.
