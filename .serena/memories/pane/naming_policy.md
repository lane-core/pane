# BeAPI Naming Policy (CRITICAL)

Pane's API defaults to faithful BeOS identifier conventions minus the "B" prefix. Deviations require explicit justification and must be recorded in the divergences tracker.

- Default: use the Be name, snake_case per Rust convention
- Case convention: Rust idiomatic (snake_case functions, CamelCase types, SCREAMING_SNAKE constants)
- Every deviation needs: Be name, pane name, rationale
- Before naming anything, ask "what did Be call this?"
