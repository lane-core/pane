# Observer Pattern: Filesystem Attributes, Not Messaging

Decision made 2026-03-30 after Be engineer analysis of BHandler::StartWatching/SendNotices.

## The decision

Observable state uses filesystem attributes at `/pane/{id}/attr/{property}` + pane-notify watches. No messaging-layer observer (no StartWatching/SendNotices equivalent on Handler).

## Why

Be's StartWatching/SendNotices was added in R5 (last release), incompletely adopted (commented out in BMenuItem), had confusing dual API directionality, and required manual dead-observer cleanup.

Filesystem attributes are strictly better for observable state:
- **Initial-value problem solved** — read the attr, then start watching (no subscribe-then-query race)
- **Crash recovery** — last-written state persists
- **Scriptability** — `pane-notify watch /pane/42/attr/progress` works without app cooperation
- **C1 alignment** — each observation is a separate filesystem watch, not a shared observer list
- **Zero new infrastructure** — pane-notify and filesystem attributes already exist

## Where filesystem doesn't work

- High-frequency frame data (cursor position at 60fps) → compositor shared memory, not observation
- Transient events ("user clicked") → point-to-point Messenger::send_message, already handled
- Cross-process state where filesystem isn't mounted → doesn't apply, /pane/ is required for pane to function

## Future API surface

When pane-fs is implemented, add to Messenger:
```rust
fn set_property(&self, name: &str, value: &[u8]) -> Result<()>
```
This is the SendNotices equivalent: write an attribute, pane-notify watchers are automatically notified.
