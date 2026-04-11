# Pre-Implementation Consultation (REQUIRED)

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

## Plan 9 Engineer Consultation

For features touching distributed architecture, network transparency, state exposure, namespace design, filesystem interfaces, or cross-instance communication, ALSO consult the plan9-systems-engineer agent.

The consultation must produce:
1. **Plan 9 precedent** — how Plan 9 solved the equivalent problem (9P, namespaces, import/cpu, factotum, plumber)
2. **Mapping assessment** — what translates cleanly to pane's Linux/userspace context vs what needs adaptation
3. **Location transparency check** — does the feature work identically for local and remote panes? If not, why not, and is the asymmetry justified?
4. **Unified namespace impact** — how does this feature appear in pane-fs's computed views?

Reference: `docs/distributed-pane.md` for the distributed design spec, `docs/superpowers/plan9-distributed-mapping.md` for the Plan 9 mapping research.

Both consultations (Be and Plan 9) are required for features at the intersection — scripting protocol, routing, service registry, filesystem projection.
