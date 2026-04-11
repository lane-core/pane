# Linearity Gap: Three Paths and Debug Tools

From review of Jacobs/Hinrichsen/Krebbers, "Deadlock-Free Separation Logic" (POPL 2024, DLfActRiS/LinearActris).

## The gap

Rust's ownership is affine (can drop), not linear (must use). A crashed thread drops Chan endpoints silently, leaving peers blocked. LinearActris proves linearity is NECESSARY for deadlock freedom from types alone — affinity is insufficient.

## Three paths

**(a) Accept affinity + runtime recovery (current approach).** pane already does this: `SessionError::Disconnected` converts dropped channels to errors; `ReplyPort` Drop sends `ReplyFailed`. The paper validates this as principled engineering, not a hack. Cost: session types guarantee protocol adherence but not deadlock freedom.

**(b) Ferrite-style linear encoding.** CPS-based API where the session continuation is inside the API, not returned to the caller. Prevents dropping. Trade-off: fundamentally different API shape (`session.send(x, |s| s.recv(|y, s| ...))`) — conflicts with pane's goal of BeOS-familiar kit simplicity. Source: Chen/Balzer/Toninho, ECOOP 2022.

**(c) Runtime connectivity graph checker.** Implement LinearActris's connectivity graph invariant as a debug tool. Track channel ownership in a directed graph; check for cycles on thread exit or send_and_wait timeout. Doesn't prevent deadlocks but detects them immediately with clear diagnostics ("thread T1 blocked on channel C1, forms cycle with T2 via C2"). Most practical near-term application.

## When this matters

- Phase 2 inter-pane communication: when pane A sends to B and B sends back to A, you get the two-thread cross-wait pattern LinearActris identifies. The existing `WouldDeadlock` guard (rejecting send_and_wait from looper threads) is a coarse but effective mitigation.
- Phase 3 streaming: stream endpoints are affine in Rust. Dropped stream-send leaves the receiver blocked.
- Debug tooling: connectivity graph tracker could be a Phase 4 diagnostic, or earlier if deadlock issues emerge.

## Dependent separation protocols (scripting relevance)

LinearActris protocols carry logical conditions alongside messages, and continuations can depend on values sent. This is what pane's scripting protocol needs: "get property X" returns a type that depends on which X. In Rust, approximate with enum dispatch within a session step — each scripting step is `Send<ScriptQuery, Branch<...>>` where the branch is determined by the query value. The paper validates this pattern is sound for deadlock freedom as long as acyclicity holds.
