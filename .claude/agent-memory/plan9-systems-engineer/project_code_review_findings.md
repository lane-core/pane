---
name: Code review findings from distributed computing implementation
description: Vetted findings from 4 reviewers plus independent investigation of TCP transport, headless server, TLS, handshake — 2026-03-30
type: project
---

Post-implementation review of Phase 1 distributed computing work. Key findings:

1. **identity: None on TCP** — connect_remote never populates PeerIdentity. run_client_handshake hardcodes None. Critical gap — no identity on remote connections.

2. **TCP active phase dropped** — pane-headless handshake channel typed as UnixStream, TCP streams can't pass through. TCP handshakes succeed but clients immediately disconnect. Same root cause as register_client being UnixStream-only.

3. **instance_id is UUID, not hostname+generation** — code uses uuid::Uuid::new_v4() but prior design decision was hostname+generation counter. ServerHello doc comment says UUID. Needs reconciliation.

4. **Nagle not disabled, framing does two write_all calls** — write_framed sends length prefix and body separately. flush() mitigates but doesn't fully solve under load. Vectored write preferred over TCP_NODELAY.

5. **TLS handshake lazy** — rustls StreamOwned defers handshake to first I/O. Doc claims eager. Works by accident but error reporting is wrong.

6. **Server always accepts** — run_server_handshake_generic never takes the Reject branch. No version/identity/signature validation.

7. **No handshake timeout or thread limit** — every connection spawns a thread, no bounds. DoS vector on network-facing deployments.

**Why:** Four code reviewers flagged issues; vetted from Plan 9 perspective to assess severity and propose fixes grounded in distributed systems experience.

**How to apply:** Reference when implementing fixes. Critical items (1, 2) block all remote usage. Important items (3-7) are security/correctness issues for network-facing deployment. The enum-transport refactor (fixing 2) should be done once, solving both the channel typing and register_client problems.
