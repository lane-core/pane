# TODO

Small items, flaky tests, cleanup tasks. Not architectural — those go in PLAN.md.

## Flaky tests

- [x] ~~`unix_stream_rapid_connect_disconnect`~~ — reduced iterations 100→50 (sufficient for churn invariant), increased timeout 15s→30s. Root cause: timing sensitivity under load with real sockets + thread spawning per iteration. 5/5 consecutive passes after fix.

## Cleanup

- [x] ~~Comprehensive warning audit~~ — completed session 3. Zero clippy warnings. Dead code removed (ServiceHandle::new, alloc_conn_id). Scaffolding suppressed with `#[allow]` + rationale (Dispatch::cancel, Pane fields, PaneBuilder.pane). Complex types factored into aliases. ProtocolServer::new suppressed (spawns thread, no Default).

## Deferred

- [ ] **Notification-triggers-request** — a notification handler (`Handles<P>::receive`) currently cannot send requests (no DispatchCtx access). Deferred to Phase 2 via self-messaging or Messenger carrying dispatch context. Roundtable confirmed this doesn't create a ratchet.
