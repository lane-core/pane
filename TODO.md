# TODO

Small items, flaky tests, cleanup tasks. Not architectural — those go in PLAN.md.

## Flaky tests

- [ ] `unix_stream_rapid_connect_disconnect` — failed once under full stress suite run, passed on retry. Timing-sensitive (real sockets, 100 iterations). Investigate: is the server's accept thread racing with the client's connect? May need a short sleep or retry in the test, or the server may have a real cleanup race under rapid reconnection.

## Cleanup

- [ ] `pane-app` dead code warnings — `Dispatch::cancel`, `Pane` fields (`id`, `tag`), `PaneBuilder.pane` field, `ServiceHandle::new()`. `Dispatch::insert/fire_reply/fire_failed` are now live (Tasks 3-6 complete). `cancel` will become live when Cancel wiring lands.
- [ ] `pane-session` dead code warnings — `ServerState.next_conn_id` / `alloc_conn_id`. Scaffolding for multi-connection (Phase 2).
- [ ] Comprehensive warning audit across all crates (`cargo clippy --workspace`). Categorize each warning as: scaffolding (will resolve with upcoming work), genuine dead code (remove), or suppressible.

## Deferred

- [ ] **Notification-triggers-request** — a notification handler (`Handles<P>::receive`) currently cannot send requests (no DispatchCtx access). Deferred to Phase 2 via self-messaging or Messenger carrying dispatch context. Roundtable confirmed this doesn't create a ratchet.
