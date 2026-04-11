---
name: Distributed protocol code review (2026-03-30)
description: Session-type soundness vet of four code reviewers' findings on TCP/TLS transport, handshake rejection, PaneRefused, and additional Chan concerns.
type: project
---

Vetted consolidated code review findings against session type theory on 2026-03-30.

**Key findings:**

1. Finding 2 (TCP active phase dropped) is **incorrect** — `connect_remote` correctly enters active phase via `from_tcp_stream` + pump threads. The three-phase model works as designed.

2. Handshake Branch with one arm unused (rejection) is sound — Gay & Hole (2005) Section 4: internal choice subtyping allows using fewer branches. Client handles both via exhaustive match. Risk is untested codepath, not unsoundness.

3. TLS lazy handshake confirmed: `StreamOwned::new` defers handshake. Doc comment on `from_stream` lies. Should force-complete TLS before entering session-typed phase (keep transport failures outside protocol).

4. `SessionSource::from_streams` does not enforce non-blocking. Violates BLooper single-thread progress assumption (DLfActRiS Theorem 5.4). Blocking TCP stream in calloop stalls all clients.

5. Duality of ClientHandshake/ServerHandshake manually verified correct. HasDual inversions sound.

6. `pending_creates` FIFO already fragile even without PaneRefused — out-of-order processing silently misroutes. UUID-keyed HashMap is the right fix (aligns with prior review's recommendation).

7. `offer()` without timeout on TCP: 25s hang possible (keepalive worst case). Blocks main thread during `connect_remote`.

**Why:** This review establishes which code review findings are real bugs vs false positives, and identifies which require session-type-level fixes vs runtime fixes.

**How to apply:** Finding 2 should be dropped from the issue list. Finding 4 (TLS) needs `complete_io` loop before `StreamOwned` construction. Finding 5 needs `set_nonblocking(true)` enforcement in `from_streams` or at call sites. Finding 6 and pending_creates migration are the same work item as PLAN.md's typestate handle task.
