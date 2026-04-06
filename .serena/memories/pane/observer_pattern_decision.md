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
- **Minimal new infrastructure** — pane-notify preserved from prototype (not yet part of redesign crate set); filesystem attributes specified in architecture.md §Namespace

## Where filesystem doesn't work

- High-frequency frame data (cursor position at 60fps) → compositor shared memory, not observation
- Transient events ("user clicked") → point-to-point Messenger::send_message, already handled
- Cross-process state where filesystem isn't mounted → doesn't apply, /pane/ is required for pane to function

## Future API surface

pane-fs namespace is specified in architecture.md §Namespace (AttrReader, AttrSet, ctl dispatch, json reserved filename). FUSE implementation pending. When implemented, attribute writes through pane-fs trigger watchers automatically — the SendNotices equivalent.
