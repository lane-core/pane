---
name: pane-session crate code review findings
description: Code review of pane-session (2026-04-03) — par drives protocol directly (no Chan wrapper), bridge thread pattern, transport death propagation analysis
type: project
---

Reviewed pane-session after redesign where par is the direct session channel (no Chan<S,T> wrapper).

**Architecture:** Handler <-> par oneshot <-> bridge thread <-> Transport <-> wire. Bridge created via par::fork_sync, spawns std::thread, uses futures::executor::block_on for par's async recv.

**Core verdict:** Bridge pattern is sound and has good BeOS lineage (structurally identical to app_server's per-client ServerApp thread). Thread-per-bridge is correct and acceptable cost. fork_sync + block_on is mechanically correct.

**Key issue — transport death propagation:** Transport panics on disconnect. Bridge thread unwinds, dropping par session endpoint. Par's internal `.expect("sender dropped")` / `.expect("receiver dropped")` causes handler thread to panic. This is correct for protocol violations but wrong for expected transport failures (network drop, server restart). Be's ports returned B_BAD_PORT_ID for recoverable disconnects. Current design is defensible IF watchdog/restart story is solid, but should be an explicit documented decision. For ephemeral connections (network services, remote pane), will eventually need Result-based error propagation.

**Par internals (v0.3.10):** Each send() calls fork_sync internally, creating a fresh oneshot pair for the continuation. No custom Drop impl on Send/Recv — relies on futures::oneshot::Canceled for peer-drop detection. #[must_use] on both types. send is non-blocking, recv is async (oneshot wait).

**Previous issues resolved by redesign:** Old Chan wrapper, calloop blocking mode, framing duplication — all eliminated. Transport trait is now minimal (send_raw/recv_raw, panics on disconnect).

**Why:** Session layer is the foundation for all pane IPC. The par-direct design is cleaner than the old Chan wrapper and gets real CLL guarantees. The transport death issue is the main thing to resolve before production use.

**How to apply:** When reviewing protocol extensions beyond handshake, verify bridge functions march through protocol in lockstep. When transport error handling is designed, reference the panic propagation analysis from this review.
