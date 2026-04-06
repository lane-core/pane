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

## What to reference — with source citations

Every heritage annotation must include a **source citation** — the specific file:line or man page section where the claim can be verified. Don't just name the concept; cite the proof.

**Be/Haiku citations** (Haiku source at `~/src/haiku/`):
- Cite header or source file with path relative to Haiku root and line number
- Examples: `src/kits/app/Looper.cpp:1162`, `headers/private/app/ServerProtocol.h:32-373`, `headers/os/app/Messenger.h:92-94`
- For private APIs, use `headers/private/` paths
- For public APIs, use `headers/os/` paths

**Plan 9 citations** (vendored at `reference/plan9/`):
- Man pages: `reference/plan9/man/SECTION/NAME:LINE` (e.g., `reference/plan9/man/5/0intro:91-96`)
- Source: `reference/plan9/src/sys/src/9/port/devmnt.c:803`
- Papers: `reference/plan9/papers/names.ms:243-246`

**Format in code:**
```rust
//! Design heritage: BeOS BLooper::task_looper()
//! (src/kits/app/Looper.cpp:1162) blocked via MessageFromPort()
```

- Don't just say "inspired by BeOS" — name the specific mechanism AND cite the source
- If a citation can't be found, soften the claim ("similar to" vs "from") and flag for follow-up
- The Be and Plan 9 citation audit agents can verify citations — run them after adding annotations

## Why
Lane asked for this. Heritage annotations serve three purposes:
1. Design rationale: why this shape and not another
2. Searchability: grep for "Plan 9:" to find all Plan 9 adaptations
3. Divergence tracking: where pane differs, the annotation explains why
