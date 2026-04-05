# BeAPI → pane Translation Rules

Apply in order. Given a Be concept, these rules determine the pane equivalent.
Verified against `docs/architecture.md` as of 2026-04-05.

## Rule 0: Vertical Integration
If it exists because Be controlled kernel/graphics server directly,
solve the underlying problem with pane's tools (Wayland, Linux VFS, s6).

## Rule 1: Namespace
Drop B prefix. Crate path replaces it. BMessage → pane_app::Message.

## Rule 2: Configure-and-Attach → Method on Host
If Be had a standalone type you construct then pass to an owner,
make it a method on the owner. Exception: types with significant
runtime behavior survive as traits.

## Rule 3: Dynamic → Static (protocol) + Filesystem (scripting)
Protocol messages → typed enum variants. Scripting → filesystem
at /pane/<id>/attrs/.

## Rule 4: Virtual Overrides → Trait with Defaults
Split into per-variant Handler methods. Compiler catches missing
variants via Handles<P> exhaustive match.

## Rule 5: Lifecycle — Consumed by run()
Pane consumed by run(). Communication during run → Messenger
(Clone + Send). PaneBuilder consumed by run_with — setup is
a distinct phase from the event loop.

## Rule 6: Error Handling
status_t / InitCheck() → Result<T, E>, fail at construction.
Three error channels (Protocol, Control, Crash) replace flat
status_t. See architecture.md "Error channels" table.

## Rule 7: Observers
- **Death watching:** Handler::pane_exited(pane, reason). How to
  register interest is deferred.
- **Conversation-level failure:** Dispatch entries resolve to
  on_failed when peer drops ReplyPort.
- **Property changes:** pane-fs at /pane/<id>/attrs/ (deferred).

## Rule 8: System Services
Separate crates/services, not globals. The compositor IS the
registry (no separate registrar process).

## Naming

**Full guide:** `docs/naming-conventions.md`

CamelCase → snake_case. Message variants match handler methods:
Message::CloseRequested ↔ Handler::close_requested().

Method patterns (adapted from Be, convergent with Rust):
- **Getters:** bare name — `name()`, `id()`
- **Setters:** `set_` prefix — `set_content()`, `set_pulse_rate()`
- **Predicates:** `is_` prefix — `is_locked()`, `is_hidden()`
- **Mutating ops:** verb + object — `add_handler()`, `remove_handler()`
- **Notification hooks:** past-participle — `activated()`, `resized()`, `close_requested()`
- **Commands:** imperative — `quit()`, `show()`, `hide()`
- **Builders:** bare names per Rust convention — `Tag::new("Title").command(cmd(...))`
