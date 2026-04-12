---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [adversarial, partial_frame, deadlock, hol_blocking, reader_loop, FrameReader, WRITER_CHANNEL_CAPACITY, lib9p, exportfs, devmnt, mountmux, plumber]
related: [decision/connection_source_design, reference/plan9/man_pages_insights, reference/plan9/papers_insights, agent/plan9-systems-engineer/project_connectionsource_review_r2]
agents: [plan9-systems-engineer]
sources: [sys/src/9/port/devmnt.c, sys/src/cmd/exportfs, lib9p/srv.c, sys/src/cmd/plumber, crates/pane-session/src/server.rs, crates/pane-session/src/frame.rs, crates/pane-app/src/connection_source.rs]
---

# Adversarial bugs analysis (2026-04-12)

Three bugs from adversarial testing, analyzed against Plan 9 precedent.

## Bug 1: Partial frame hangs server reader

Plan 9 had the same bug. lib9p's `read9pmsg` and exportfs both did blocking reads with no timeout. devmnt.c's `mountmux` blocked on pipe reads with no deadline. The only escape was fd close (client death) or admin kill via /proc/N/ctl.

**Fix:** Extract FrameReader state machine from connection_source.rs:79 to pane-session::frame as shared non-blocking reader. Server readers use poll + per-connection deadline on incomplete frames. Deadline ticks only on partial frames, not idle connections. Priority: now. Currently leaks one thread per adversarial connection.

## Bug 2: Bidirectional buffer deadlock

Plan 9 avoided structurally: devmnt.c's mountrpc wrote one T-message then slept waiting for reply. mountmux read replies independently. Never burst-without-reading because each request blocked the calling process. MAXRPC=128 bounded concurrent outstanding by tag pool, not flow control.

pane's D12 overflow-teardown already handles this: writer thread blocks → channel fills → try_send fails → overflow_teardown → process_disconnect. Not a deadlock — a cascading failure, which is the correct response per session-type analysis.

**Fix:** Reduce WRITER_CHANNEL_CAPACITY from static 4096 to derive from max_outstanding_requests (D9). Currently 32x oversized, delays slow-client detection. Phase 2: consider ctl-priority channel so Cancel/ServiceTeardown survive data bursts.

## Bug 3: HOL blocking during fan-out

Server actor try_send is non-blocking by D12 construction. The reported HOL blocking is likely not in the server routing path. The plumber had this bug (blocking writes to port fds). lib9p avoided it with per-request threads. pane's sequential dispatch (I6) is an explicit tradeoff; the server's fan-out paths (process_disconnect watcher iteration) do O(N) non-blocking try_sends.

**Fix:** Verify which layer actually blocks. If server-side: already handled. If client-side: inherent to I6, enforced by I2 + watchdog. Phase 2: batched teardown notifications for O(1) actor work per disconnect at scale.

## Key Plan 9 mechanisms cited

- devmnt.c mountmux: separate reader process, processes slept per-request
- lib9p srv.c: per-request threads, respond() wrote directly, no central writer
- exportfs: one reader per connection, no timeout, no keepalive
- plumber: blocking sequential writes to port fds, explicit tradeoff for UI
- MAXRPC=128: tag pool bound, not flow control — structural prevention of unbounded in-flight

## Recommendations

1. Extract FrameReader to pane-session (shared between server + client) — immediate
2. Derive WRITER_CHANNEL_CAPACITY from max_outstanding_requests — Phase 1
3. Add structured overflow diagnostics — Phase 1
4. Batched teardown notification — Phase 2
5. Ctl-priority channel — Phase 2
