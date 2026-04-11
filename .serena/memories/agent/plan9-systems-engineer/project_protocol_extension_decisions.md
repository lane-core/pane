---
name: Protocol extension design decisions (Phase 1)
description: PeerIdentity in ClientHello, hostname+generation instance_id, calloop channel for handshake, Cancel deferred — decisions from 2026-03-30 consultation
type: project
---

Four decisions made for the distributed computing Phase 1 protocol extensions:

1. **PeerIdentity goes in ClientHello** (not ClientCaps). Identity precedes capability negotiation so ServerHello can be identity-aware. Optional<PeerIdentity> — None for local unix connections where SO_PEERCRED provides identity. Follows 9P's Tauth-before-Tattach ordering.

2. **instance_id is hostname + generation counter** (not UUID). Hostnames are meaningful in the unified namespace (`/pane/remote/<host>/`). Generation counter (u64, epoch millis) detects restarts. Multi-instance on one host uses `hostname:port`. Follows Plan 9's `import` convention.

3. **Handshake completion via calloop::channel::Channel** (not timer polling). Both pane-comp and pane-headless should use pipe-backed event notification instead of polling on the frame timer. pane-comp's `poll_handshakes()` on frame timer is a workaround to be cleaned up.

4. **Cancel variant reserved in ClientToComp, implementation deferred.** No current active-phase operation needs cancellation (fire-and-forget or fast sync). CompletionRequest staleness handled by token supersession. Full cancel semantics wait for remote long-running operations or reconnection.

**Why:** Lane asked for specific guidance on four protocol questions before implementing Phase 1 of distributed computing.

**How to apply:** Reference when implementing the handshake extension types in pane-proto/src/protocol.rs and when building pane-headless's calloop loop.
