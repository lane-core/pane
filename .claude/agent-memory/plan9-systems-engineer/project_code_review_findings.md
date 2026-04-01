---
name: Code review findings from distributed computing implementation
description: Vetted findings from 4 reviewers plus independent investigation of TCP transport, headless server, TLS, handshake, ownership verification — 2026-03-31
type: project
---

Second review pass (2026-03-31): vetted 8 external findings, added 6 new findings.

## Critical

1. **Pane ownership verification missing** — handle_message in pane-server/src/lib.rs processes any PaneId without checking client_id ownership. PaneState.client_id exists but is never enforced. Unlike 9P's per-connection fid scoping, PaneIds are global UUIDs visible across connections. Fix: add pane_owned_by() check at top of handle_message.

2. **Identity discarded after handshake** — ServerHandshakeResult carries identity but pane-headless drops it. ClientSession has no identity field. Cannot implement any identity-based access control. Fix: add identity to ClientSession, store during register_client.

3. **Reconnecting transport has no session continuity** — after reconnection, raw TCP socket has not re-handshaked. Server expects ClientHello, client sends active-phase messages. Protocol state mismatch. Unlike aan(8) which was symmetric below 9P, this is client-only above the session layer. Currently unusable for the handshake-requiring path.

## Important

4. **TLS not wired into pane-headless** — transport exists in pane-session but TCP listener creates raw TcpTransport. PeerIdentity is self-reported and unverifiable without TLS. Plumbing exists, needs --tls-cert/--tls-key flags.

5. **No msize negotiation** — 16MB hardcoded on both sides. Over constrained links, large SetContent blocks all messages. 9P version(5) negotiated msize. Add to handshake caps or make MAX_MESSAGE_SIZE configurable per-connection.

6. **No handshake thread limit** — TCP listener spawns unbounded threads. Needs semaphore or connection limit.

7. **No connect timeout in App::connect_remote** — TcpStream::connect uses system default (75-120s). Use connect_timeout.

## Moderate

8. **Active-phase tracing receive-only** — trace_message called on << but send_to_client writes untraced. Unlike exportfs -d which logged all traffic.

9. **Error info loss in App::connect** — format!("{:?}") in error conversion discards source chain.

10. **PaneCreateFuture drop spawns 10s cleanup thread** — can outlive App. Clunk-on-abandon is correct, timeout is too long or needs cancel token.

11. **try_reconnect recurses** — bounded by timeout but mutual recursion with replay_buffer amplifies depth. Convert to iterative loop.

## Low

12. **MAX_MESSAGE_SIZE duplication** — real issue is write_framed function duplication between wire.rs (non-vectored) and framing.rs (vectored).

13. **Keepalive parameter discrepancy** — TcpTransport 10s/5s vs ReconnectingTransport 5s/2s. Rationale is sound but undocumented.

14. **send_to_client uses different write paths for Unix vs TCP** — correctness ok in single-threaded calloop but landmine if multi-threaded.

## Previously fixed (verified)
- pane_count TOCTOU race
- Condvar lost notification
- duration_since panic

**Why:** Comprehensive review from distributed systems perspective to ensure network-facing deployment is viable.

**How to apply:** Critical items (1, 2, 3) block secure remote usage. Important items (4-7) block production deployment. Moderate items (8-11) are quality/debuggability. Low items (12-14) are hygiene.
