## Why

The inter-server protocol uses `PaneMessage<ServerVerb>` with attrs carrying the payload. The architecture spec says type safety is recovered via typed view/builder patterns, but no views or builders exist yet. Without them, the first server-to-server interaction will fall back to raw `msg.attr("key")` access — exactly the stringly-typed BMessage problem the pattern exists to prevent. This needs to exist before any server pair starts talking.

## What Changes

- Define a `TypedView` trait for parsing `PaneMessage<ServerVerb>` into validated typed structs
- Define a `TypedBuilder` pattern for constructing `PaneMessage<ServerVerb>` with compile-time required-field enforcement
- Implement the pattern for pane-route's messages: `RouteCommand` (route a text fragment), `RouteQuery` (query available handlers for content)
- Implement the pattern for pane-roster's messages: `RosterRegister` (infrastructure server registers), `RosterQuery` (query running apps), `RosterServiceRegister` (app registers an operation)
- Add error types for view parse failures (missing field, wrong type)

## Specs Affected

### New
- `inter-server-views`: Typed view/builder pattern, ViewError types, trait definitions

### Modified
- `pane-protocol`: Inter-server protocol gains concrete typed views and builders

## Impact

- New module `pane-proto::server::views` with trait + error types
- New module `pane-proto::server::route` with RouteCommand, RouteQuery views/builders
- New module `pane-proto::server::roster` with RosterRegister, RosterQuery, RosterServiceRegister views/builders
- Pattern established for all future server interactions
- Tests: round-trip build → serialize → deserialize → parse for each view
