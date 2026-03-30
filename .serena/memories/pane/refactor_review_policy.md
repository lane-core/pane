# Post-Refactor Review Policy (CRITICAL)

After any substantial refactor (mass rename, API restructure, architectural change):

1. Code review — audit changed code for correctness, idiom, consistency
2. Stale documentation review (parallel) — audit ALL comments, specs, docs, memories for old names/patterns
3. If code review fixes are themselves substantial → run another stale doc review after implementing them
4. Repeat until a review pass produces no substantial changes

"Substantial" = renames public identifiers, removes/adds public types, changes method signatures, restructures modules.
