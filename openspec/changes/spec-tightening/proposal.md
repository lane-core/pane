## Why

The specification corpus needs to be grounded in genuine understanding of the design references, not surface-level characterizations. Before tightening for precision, we need to deeply understand:

1. **BeOS/Haiku** — not just the API surface but why the system felt the way it did. Why pervasive multithreading produced stability rather than chaos. How BMessage's design enabled loose coupling that RPC couldn't. How the kit/server decomposition worked as an ecology. How the translation kit and replicant system created extensibility without fragility. What BFS's attribute indexing meant for the user experience. The relationship between the threading model and the scheduler's ability to keep the system responsive under load.

2. **Plan 9** — not just "everything is a file" but how per-process namespaces changed the nature of composition. How rio/acme made text actionable and what that meant for workflow. How the plumber's simplicity was its power. Why 9P as universal protocol eliminated entire categories of system complexity. The relationship between namespace manipulation and the ability to run the same program in different contexts transparently.

3. **Session types** — not just "par crate gives us Send/Recv" but what Vasconcelos's foundational work establishes about the relationship between linear types and session types. How deadlock freedom is guaranteed and what constraints it imposes. How this formalizes what BeOS engineers achieved by discipline. The Caires-Pfenning correspondence and what it means practically.

Only after this research can we write specs that are cogent about influences, clear about approach, and honest about what we're building vs what we're drawing from. The goal is that future design conversations are grounded in shared understanding, not gesture.

## What Changes

- Deep research into BeOS/Haiku, Plan 9, and session type theory
- Comprehensive review and rewrite of architecture spec with genuine understanding
- Sync all pending spec rewrites (pane-shell, pane-route, pane-roster)
- README update reflecting accurate characterization of influences
- All specs audited for stale framings and internal consistency

## Specs Affected

### Modified
- `architecture`: comprehensive rewrite grounded in research
- All specs referencing stale framings (Value/Compute, sequent calculus, old roster model)

### New
- Sync `pane-shell`, `pane-route`, `pane-roster` from pending changes to main specs

## Impact

- No code changes
- Specs become a reliable, well-understood foundation
- Design conversations become more productive because they're grounded in genuine understanding of the references
