# BeAPI → pane Translation Rules

Apply in order. Given a Be concept, these rules determine the pane equivalent.

## Rule 0: Vertical Integration
If it exists because Be controlled kernel/graphics server directly, solve the underlying problem with pane's tools (Wayland, Linux VFS, s6).

## Rule 1: Namespace
Drop B prefix. Crate path replaces it. BMessage → pane_app::Message.

## Rule 2: Configure-and-Attach → Method on Host
If Be had a standalone type you construct then pass to an owner, make it a method on the owner. Exception: types with significant runtime behavior survive as traits.

## Rule 3: Dynamic → Static (protocol) + Filesystem (scripting)
Protocol messages → typed enum variants. Scripting → filesystem at /pane/<id>/attrs/.

## Rule 4: Virtual Overrides → Trait with Defaults
Split into per-variant Handler methods. Compiler catches missing variants.

## Rule 5: Lifecycle — Consumed by run()
Pane consumed by run(). Communication during run → Messenger (Clone + Send).

## Rule 6: Error Handling
status_t / InitCheck() → Result<T, E>, fail at construction.

## Rule 7: Observers
Two levels of failure observation:
- **Actor-level:** monitor() + PaneExited for death watching (implemented). This is the "who died" signal.
- **Conversation-level (future, per C3):** When inter-pane request-response exists, pending requests should resolve to failure when the peer exits — the "which interaction failed" signal. Layer on top of PaneExited, not a replacement.
- **Property changes:** filesystem at /pane/<id>/attrs/ via pane-notify.

See serena memory `pane/session_type_design_principles` principle C3 for rationale.

## Rule 8: System Services
Separate crates/services, not globals.

## Naming
CamelCase → snake_case. Message variants match handler methods: Message::CloseRequested ↔ Handler::close_requested().
