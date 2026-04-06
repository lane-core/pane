# TODO

Small items, flaky tests, cleanup tasks. Not architectural — those go in PLAN.md.

## Flaky tests

- [ ] `unix_stream_rapid_connect_disconnect` — failed once under full stress suite run, passed on retry. Timing-sensitive (real sockets, 100 iterations). Investigate: is the server's accept thread racing with the client's connect? May need a short sleep or retry in the test, or the server may have a real cleanup race under rapid reconnection.

## Cleanup

- [ ] `pane-app` dead code warnings — `Dispatch` methods (`insert`, `fire_reply`, `fire_failed`, `cancel`), `Pane` fields (`id`, `tag`), `PaneBuilder.pane` field, `ServiceHandle::new()`, `ServerState.next_conn_id` / `alloc_conn_id`. These are scaffolding for request/reply wiring (Tasks 2-6) and will become live code. Suppress or remove after wiring is complete.
- [ ] Comprehensive warning audit across all crates (`cargo clippy --workspace`). Categorize each warning as: scaffolding (will resolve with upcoming work), genuine dead code (remove), or suppressible.
