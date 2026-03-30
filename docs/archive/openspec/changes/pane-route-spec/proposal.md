## Why

pane-route is the content router — the Plan 9 plumber adapted for pane. The architecture spec describes it at a high level (named ports, ordered rule set, service-aware multi-match) but there are no behavioral contracts for the rule format, matching algorithm, port model, or the interaction with pane-roster's service registry. Without these, implementing pane-route requires making design decisions ad hoc.

## What Changes

- Define the routing rule data model (what a rule looks like, how it's stored)
- Define the matching algorithm (how rules are evaluated against messages)
- Define the port model (how applications listen, how messages are delivered)
- Define the service registry interaction (how registered services extend matching)
- Define the multi-match chooser behavior (transient scratchpad pane)
- Define the filesystem-based rule configuration (`~/.config/pane/route/rules/`)

## Specs Affected

### New
- `pane-route`: Routing rules, matching algorithm, port model, service integration, multi-match

### Modified
- None

## Impact

- New spec at openspec/specs/pane-route/spec.md
- No code changes — spec only
- RouteCommand typed view already exists in pane-proto; this spec defines the server that receives it
