# Heritage Annotations in Rust Code

Every module and significant type/trait in pane should document its design heritage from BeOS/Haiku and Plan 9 where applicable.

## Format

**Module-level:** `//! Design heritage:` block in the module doc, naming both systems if both are relevant.

**Type/trait-level:** Inline comment on the doc comment, using `///` with the system name:
```rust
/// Plan 9: analogous to qid.path (stable, machine-comparable)
/// BeOS: team_id was kernel-assigned but self-reported
```

**Method-level:** Short inline `// Plan 9:` or `// BeOS:` comment when a specific method mirrors a specific API.

## When to add

- Every new module: heritage block in module doc
- Every new public type: at least one heritage note if there's a precedent
- Every new public method: only if it directly mirrors a Be/Plan 9 API
- If neither system has a precedent: say so explicitly ("No direct Be or Plan 9 precedent — this is new ground for pane's distributed model")

## What to reference

- Be/Haiku: cite the specific type, method, or mechanism (e.g., "BMessenger::SendMessage", "team_id", "ServerProtocol.h AS_CREATE_APP")
- Plan 9: cite the specific mechanism, man page, or file (e.g., "factotum(4)", "9P Tattach", "/srv", "exportfs(4)")
- Don't just say "inspired by BeOS" — name the specific thing

## Why
Lane asked for this. Heritage annotations serve three purposes:
1. Design rationale: why this shape and not another
2. Searchability: grep for "Plan 9:" to find all Plan 9 adaptations
3. Divergence tracking: where pane differs, the annotation explains why
