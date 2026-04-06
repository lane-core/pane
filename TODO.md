# TODO

Small items, flaky tests, cleanup tasks. Not architectural — those go in PLAN.md.

## Flaky tests

- [ ] `unix_stream_rapid_connect_disconnect` — failed once under full stress suite run, passed on retry. Timing-sensitive (real sockets, 100 iterations). Investigate: is the server's accept thread racing with the client's connect? May need a short sleep or retry in the test, or the server may have a real cleanup race under rapid reconnection.

## Cleanup

- [x] ~~Comprehensive warning audit~~ — completed session 3. Zero clippy warnings. Dead code removed (ServiceHandle::new, alloc_conn_id). Scaffolding suppressed with `#[allow]` + rationale (Dispatch::cancel, Pane fields, PaneBuilder.pane). Complex types factored into aliases. ProtocolServer::new suppressed (spawns thread, no Default).

## Deferred

- [ ] **Notification-triggers-request** — a notification handler (`Handles<P>::receive`) currently cannot send requests (no DispatchCtx access). Deferred to Phase 2 via self-messaging or Messenger carrying dispatch context. Roundtable confirmed this doesn't create a ratchet.
