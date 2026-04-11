---
type: reference
status: current
sources: [agent/be-systems-engineer/reference_haiku_rs]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [haiku_rs, rust_bindings, FFI, BApplication, BMessage, BMessenger, BLooper, BHandler]
related: [reference/haiku/_hub, reference/haiku/book]
agents: [be-systems-engineer, pane-architect]
---

# haiku-rs — Rust bindings to Haiku

`haiku-rs` (crate `haiku` v0.3.0, MIT, by Niels Sascha Reedijk) is
**FFI bindings to the running Haiku system, not an abstract
reimplementation**. Links libc, calls kernel ports directly.
Depends on `lazy_static` for the global Roster.

GitHub: <https://github.com/nielx/haiku-rs>

## Coverage

- **Application Kit:** Application, Looper, Handler, Message,
  Messenger, Notification, Roster
- **Kernel Kit:** Port, Team
- **Storage Kit:** file attributes, MIME
- **Support Kit:** errors, Flattenable

## Key design choices

- `Message` is `repr(C)` binary-compatible with Haiku's 1FMH
  format (dynamic named fields, hash table, type codes)
- `Flattenable` trait = BFlattenable (`type_code`,
  `is_fixed_size`, `flatten`, `unflatten`) — modern Rust would
  use serde
- `Application<A: ApplicationHooks + Send>` — generic over state type
- `Messenger` wraps kernel Port + token, three send modes (sync /
  async-reply / fire-forget)
- `Looper<H: Handler>` spawns thread, HashMap of handlers by token
- `Handler` trait has single method: `message_received` (faithful
  to BHandler)
- ROSTER as `lazy_static` global — **pane avoids globals**
  (correct divergence)
- No filter, no coalescing, no deadlock prevention, no reply
  discipline

## Comparison with pane

Full analysis at `docs/superpowers/haiku-rs-analysis.md`.

The short version: haiku-rs is a faithful Rust wrapper around
the live Haiku system. pane is a Rust reimagining of the BeOS
design ideas in a non-Haiku environment. Different goals.
