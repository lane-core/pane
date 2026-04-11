---
type: decision
status: current
supersedes: [pane/messenger_addressing_decisions]
sources: [pane/messenger_addressing_decisions]
created: 2026-04-05
last_updated: 2026-04-11
importance: high
keywords: [messenger, address, service_handle, send_request, direct_pane_to_pane, protocol_scoped]
related: [decision/server_actor_model, reference/papers/eact, reference/plan9/divergences]
agents: [pane-architect, session-type-consultant, plan9-systems-engineer]
---

# Messenger addressing design decisions (2026-04-05)

Decided during four-agent consultation on inter-pane messaging.

## Key decisions

### 1. Name: `Address` (not PaneAddress)

In `pane_app` namespace, so `pane_app::Address` /
`pane::Address`. No redundancy.

### 2. Direct pane-to-pane communication

Diverges from all four agents' assumption of
server-as-intermediary. Panes can establish direct channels,
not just route through the server. Address must encode enough
to support direct connection, not just "pane ID on server X."

**Why:** Lane decided. Server-mediated adds latency and makes
the server a bottleneck / SPOF for inter-pane messaging.
Direct is more faithful to Plan 9's model (mount a remote
filesystem directly).

### 3. Protocol-scoped send_request

`send_request` lives on `ServiceHandle<P>`, NOT on `Messenger`.
Messages are typed by protocol — you can only send `P::Message`
through a `ServiceHandle<P>`. No untyped cross-pane messaging.

`Messenger` retains self-targeted operations: `set_content`,
`post_app_message`, `set_pulse_rate`, `address()`.

**Why:** Compile-time protocol agreement. The session-type
consultant flagged that untyped `send_request` has runtime
deserialization failures. Protocol-scoping eliminates this
within a compilation unit.

### 4. Implement types + stubs now

Like `PeerAuth` — define the types, write tests, but routing
is stubbed. May change radically when connection layer is
implemented. Connection layer is bumped up in priority because
of this.

## Type structure

- **`Address`** — lightweight, copyable, serializable pane address
- **`Messenger`** — handler's live capability handle (self
  operations + address extraction)
- **`ServiceHandle<P>`** — typed service binding, owns
  `send_request`

## Architecture spec impact

The architecture doc currently shows `send_request` on
`Messenger`. This moves to `ServiceHandle<P>`. The
`target: &Messenger` parameter becomes implicit (`ServiceHandle`
is already bound to a target).
