# pane-fs Optic Taxonomy

Per-entry optic classification for /pane/ namespace (2026-04-03).

## Taxonomy

- `/pane/<id>/tag` — **Lens(PaneState, String).** GetPut/PutGet/PutPut all hold within a snapshot.
- `/pane/<id>/body` — **Lens(PaneState, Content).** Same laws as tag.
- `/pane/<id>/attrs/<name>` — **Getter(S, AttrValue).** Read-only. Laws vacuous. Value is in FUSE permission derivation.
- `/pane/<id>/ctl` — **NOT an optic.** Write-only command channel. Effectful, non-idempotent. Session types govern this.
- `/pane/<id>/event` — **NOT a Fold.** Blocking JSONL stream. Temporal, non-deterministic, non-repeatable. Stream vocabulary.

## Path composition

- `/pane/3/tag` = Iso . AffineTraversal . Lens = AffineTraversal (partial read-write)
- `/pane/3/attrs/cursor` = Iso . AffineTraversal . Getter . AffineFold = AffineFold (partial read-only)

## Snapshot model

Optic laws hold within a single snapshot (immutable). Across snapshot boundary, PutGet can be violated temporally (stale read after write). This is a caching/consistency concern, not an optic-law violation. The fix is protocol-level (write barriers).

## Where formalism earns its keep

Testable laws for tag/body, composition algebra for path traversal, FUSE interface derivation from optic type. NOT for ctl, event, or temporal consistency.

**How to apply:** Derive FUSE permissions from optic types. When adding new entries, classify before implementing. Don't try to make ctl or event fit the optic hierarchy.