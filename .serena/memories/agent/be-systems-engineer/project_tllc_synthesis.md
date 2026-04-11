---
name: TLL+C dependent session type synthesis for pane
description: Synthesis of Fu/Xi/Das TLL+C paper (2026-03-30) — protocol/channel separation, ghost state discipline, relational verification as sequential-spec principle, linearity-as-ownership mappings
type: project
---

Analyzed Fu/Xi/Das "Dependent Session Types for Verified Concurrent Programming" (TLL+C, PACMPL 2026) against pane's architecture.

**Core paper insight:** Protocols are abstract (shared, copyable, describe conversation), channel types provide dual directional interpretations (linear, owned, send/recv), ghost state exists for verification only (erased before runtime). This three-way separation is the paper's structural contribution.

**What maps productively to pane:**
1. Protocol/channel separation formalizes what pane already does informally — Message enum is protocol, Messenger/ReplyPort are channel types. Sharpens C2: typestate handles are channel-type interpretations of a shared protocol definition.
2. Ghost state discipline — wherever pane uses runtime token-matching (reply tokens, completion tokens, timer IDs), ask if ownership can replace the match. ReplyPort already does this; pattern should generalize to clipboard, DnD, observer sub-protocols.
3. Relational verification as sequential-spec principle — each looper's behavior is describable as a sequential trace. Multi-session (C1) must preserve this: looper = sequential interleaving, system = concurrent composition. This is Be's original model, TLL+C gives it formal grounding.
4. Linearity determines cloneability — non-Clone handles for sub-protocols with ordering constraints (ClipboardLock, DragSession), Clone for idempotent operations (clipboard read). Let protocol structure drive ownership model.

**What doesn't map:**
- Dependent types over protocol contents (queue-tracks-its-contents verification irrelevant to UI protocols)
- Concurrency monad (Rust ownership already provides what C monad gives ML-family languages)
- Recursive protocols with type-level computation (pane protocols are event streams with episodes, not recursive)

**New principle beyond C1-C6:** Ghost state identification — for each runtime correlation datum (tokens, IDs), systematically ask "could ownership replace this?" When yes, you get protocols that are harder to misuse. Design discipline, not language feature.

**How to apply:** When designing new sub-protocols, follow three-step discipline: (1) define abstract protocol independently of direction, (2) derive typestate handles from it, (3) identify ghost state and ask if ownership can replace token-matching.
