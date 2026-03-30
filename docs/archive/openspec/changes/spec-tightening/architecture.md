## Context

Over many design conversations the spec evolved through several framings: monadic composition → sequent calculus / Value-Compute polarity → session types. Each was a refinement, not a mistake — but the specs still contain language from every phase. The roster went from hybrid supervisor to directory. pane-route went from text matcher to communication infrastructure. pane-shell went from VT parser spec to semantic interface spec. These changes happened in conversation but weren't fully propagated through the spec corpus.

The README makes strong claims about BeOS that read as "we are replicating this" when they should read as "we are influenced by this." The architecture spec's vision section still references sequent calculus. Multiple kit descriptions reference Value/Compute polarity which was superseded by session types. The Compositional Layers section references "duploid's three-fourths associativity rule" which is no longer the framing.

## Goals / Non-Goals

**Goals:**
- Internal consistency across all specs
- Clear separation between influences (what inspired us) and commitments (what we're building)
- Session types as the single protocol framing
- All pending spec rewrites synced to main
- Every claim in the README verifiable against the architecture spec

**Non-Goals:**
- New features or design decisions
- Code changes
- Resolving open questions (those stay open)

## Decisions

### 1. One pass through the architecture spec

Read and rewrite as a single coherent document. Don't patch — write fresh where sections are stale. The vision, pillars, server descriptions, kit descriptions, protocol section, and technology section should all use consistent language.

### 2. Influences stated as influences

"Influenced by BMessage" not "modeled on BMessage." "Draws from Plan 9's plumber" not "inspired by Plan 9's plumber but actually a communication infrastructure." State what we learned from each reference, then state what we're doing — which may differ.

### 3. Session types as the protocol foundation

All references to Value/Compute polarity, sequent calculus, CBPV, duploids removed. Session types (via `par`) are the framing. The theory (Caires-Pfenning, linear logic) is noted once as background, not woven through every description.

### 4. Sync all pending rewrites

The pane-shell, pane-route, and pane-roster specs in the changes directories are newer than what's in main specs. Sync them.

## Risks / Trade-offs

**[Scope creep]** → The temptation to redesign while editing. Mitigation: if something needs redesigning, note it as an open question, don't fix it in this pass.

## Open Questions

None — this is a consistency pass, not a design pass.
