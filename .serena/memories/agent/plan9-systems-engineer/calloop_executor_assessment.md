---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [calloop, executor, async, adapt_io, Scheduler, futures, ConnectionSource, FrameReader, handshake, bridge, mountmux, rio, xfid, thread2, alt, Smithay]
sources: [calloop 0.14.4 docs (executor feature, adapt_io, Async), pane-app/src/connection_source.rs, pane-session/src/bridge.rs, pane-session/src/frame.rs, devmnt.c (mountmux:934), rio.c (mousethread:461, xfidctl:102), Inferno Dis VM cooperative scheduling]
verified_against: [calloop 0.14.4 source via context7, pane-app ConnectionSource, pane-session bridge.rs + frame.rs]
related: [agent/plan9-systems-engineer/async_concurrency_assessment, agent/session-type-consultant/rumpsteak_smol_translation, decision/par_integration_architecture, dependency/par]
agents: [plan9-systems-engineer]
---

# calloop Executor Assessment (Path C Revised)

Assessed 2026-04-12 against calloop 0.14.4 executor feature.

## Verdict

Use calloop executor for two scoped purposes: async handshake
(eliminates blocking bridge thread) and par Recv integration
(par oneshots driven by calloop executor instead of block_on
in bridge thread). Do NOT use for ConnectionSource read/write
or FrameReader conversion.

## Key findings

### calloop executor = xfid pool
calloop executor + adapt_io maps to rio's xfid thread pool:
cooperative concurrency within a single OS process, futures
are lightweight threads with implicit stack state. Event loop
drives everything. !Send futures preserved by spawn_local.

### ConnectionSource IS mountmux — don't asyncify it
ConnectionSource's synchronous batch-read loop (try_read_frame
until WouldBlock) is structurally identical to devmnt.c
mountmux (934-967). Bidirectional read+write in one
EventSource is a strength. Splitting into async read + async
write futures adds coordination complexity, loses batch-read
efficiency (900K msg/sec baseline), fragments interest
management.

### FrameReader state machine is too small to justify async
4 states (ReadingLength, ReadingBody, partial tracking,
poisoning). Compiler-generated state machine from async would
be equivalent but loses: batch integration with
ConnectionSource, poisoning control, direct syscall path.
Plan 9's mntrdwr (devmnt.c:828-860) used same pattern — tight
loop, small state, never needed async.

### Async handshake IS justified
connect_unix currently blocks on Hello/Welcome exchange.
adapt_io(stream) wraps fd, async future reads/writes CBOR
frames, executor callback delivers Welcome. Eliminates one
blocking thread per connection during handshake. Sequential
two-message exchange is natural async fit.

### Smithay composition works
pane EventSources in compositor's calloop = mounted 9P
services in rio's namespace. Independent dispatch, no
cross-source ordering hazards. Batch limit (64 msgs / 8ms)
prevents starvation. Shutdown ordering requires explicit
future cleanup (executor drop cancels in-flight futures,
may trigger par panic-on-drop).

### Dependency cost
calloop executor feature adds async-task crate. futures-io
already transitively present. Architectural cost: spawned
futures are state across poll iterations — shutdown/error
path reasoning becomes more complex.

## Supersedes

Updates async_concurrency_assessment with specific calloop
executor evaluation. Prior assessment said "sync correct for
Phase 1, async deferred to Phase 2." This assessment
identifies two Phase 1 async uses (handshake, par bridge)
while confirming the sync ConnectionSource path stays.
