---
type: policy
status: current
supersedes: [pane/non_exhaustive_extensions]
created: 2026-03-31
last_updated: 2026-04-10
importance: normal
keywords: [non_exhaustive, planned_extensions, audit_obligation, PeerAuth, AuthSource, Address]
agents: [pane-architect]
---

# Non-Exhaustive Types and Planned Extensions

Types marked `#[non_exhaustive]` with their planned future extensions. When adding extensions, audit all downstream match arms (wildcard arms exist but may need real handling).

## PeerAuth (struct) — `crates/pane-proto/src/peer_auth.rs`

Planned fields: none currently identified. Marked non-exhaustive defensively — transport metadata may grow (e.g., connection timestamp, transport type tag).

## AuthSource (enum) — `crates/pane-proto/src/peer_auth.rs`

Planned variants:

- `Anonymous` — read-only namespace access without authentication (session-type consultant proposed this)
- `Token { ... }` — capability/token-based auth (Be agent mentioned this)
- `Delegated { ... }` — factotum-style delegated authentication (Plan 9 agent)

## Address (struct) — `crates/pane-proto/src/address.rs`

Planned fields:

- Direct connection info (socket addr, port) for direct pane-to-pane communication (Lane's decision: not server-mediated only)
- Connection hint / routing preference

## Audit obligation

When adding a variant or field to any of these types:

1. Grep for all match/destructure sites across the workspace
2. Update Display impls
3. Update serialization tests (roundtrip the new variant)
4. Update docs that show the type's structure
