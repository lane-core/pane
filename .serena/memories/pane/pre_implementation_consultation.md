# Pre-Implementation Be Engineer Consultation (REQUIRED)

Before implementing any new subsystem or major feature, consult the be-systems-engineer agent with reference to the Haiku Book (`reference/haiku-book/`), the Be Newsletter archive, and Haiku source (`/Users/lane/src/haiku/`).

The consultation must produce:

1. **A reading list** — specific `.dox` files from the Haiku Book that the implementor reads before writing code. Not a summary; the primary source.
2. **Design rationale** — newsletter articles and Haiku source explaining *why* the API has its shape.
3. **A verification checklist:**
   - Did we account for every documented hook/virtual method?
   - Did we address the threading/locking considerations they call out?
   - Did we handle the pitfalls they warn about (or consciously diverge)?
   - Are there methods we chose not to implement, and do we know why?
4. **Adaptation notes** — how to translate given pane's architecture (Wayland, session types, filesystem scripting).

The reading list and checklist live with the implementation work (in the plan or as task notes), not in memory.

When dispatching agents to implement features with Be lineage, include "read `reference/haiku-book/<path>` first" in the agent prompt.

This is not optional. It grounds implementation in actual engineering experience.
