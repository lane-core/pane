## Why

Pane's architecture depends on sequential composition of small servers and typed protocols. The kits need a coherent pattern for how domain operations compose — chaining protocol transitions, combining reactive state, piping plumber results. Without an explicit compositional design principle, each kit will reinvent ad-hoc chaining patterns. Establishing this now, before more kits are built, sets the idiom everything else follows.

## What Changes

- Add a new architecture design pillar: "Compositional Interfaces" — kits use Rust's native monadic idioms and combinator APIs as the primary composition mechanism
- Define three compositional layers that map to specific crate boundaries:
  1. **Result-like domain types** (all kits): custom enums with success/failure shape get derived combinator APIs (`map`, `and_then`, `unwrap_or`) — adopting `result-like` or equivalent when types that need it exist
  2. **Protocol combinators** (pane-app): builder API for composing protocol operation sequences as testable values
  3. **Reactive signals** (pane-app, pane-store-client): FRP signals (`map`, `combine`, `contramap`) for change notification composition and live queries — adopting `agility` or equivalent when those kits are built
- Establish the boundary: monadic composition is for sequencing domain operations, not for wrapping imperative mutation

## Capabilities

### New Capabilities
- `compositional-interfaces`: Design principle and patterns for how kit APIs compose — result-like domain types, protocol combinators, reactive signals

### Modified Capabilities
- `architecture`: New design pillar "Compositional Interfaces" added

## Impact

- Architecture spec gains a sixth design pillar
- All future kit designs are guided by the three-layer compositional pattern
- No code changes or new dependencies in this change — principle only
