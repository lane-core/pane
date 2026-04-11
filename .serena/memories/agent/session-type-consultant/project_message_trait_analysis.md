---
name: Message-as-trait EAct analysis (2026-04-03)
description: Message trait proposal for EAct modeling — conditionally sound, 5 conditions, naming collision flagged, CLONE_SAFE via #[obligation] annotation
type: project
---

Analyzed proposal: Message as a trait (not enum) for unified EAct dispatch modeling.

**Verdict:** Conditionally sound.

**Key findings:**
1. EAct message = queue entry (p, q, ℓ(V)) — session affiliation, direction, label, payload. Label IS enum variant; no reification needed.
2. service_id() belongs on Protocol, NOT Message — session identity is on the queue process, not individual entries (EAct Fig. 5 §3.1)
3. Two traits (ValueMessage/ObligationMessage) is WRONG — mixed enums like ClipboardMessage must stay unified per TH-Handler (one handler per session endpoint, one branching type). Use const CLONE_SAFE: bool instead.
4. Handler-as-Handles<Lifecycle> strengthens EAct correspondence — unifies E-React dispatch path, eliminates the architectural deviation of two dispatch mechanisms
5. CLONE_SAFE must use explicit #[obligation] annotation on variants — macro expansion cannot reliably detect !Clone bounds
6. Naming collision: `Message` the trait vs `Message` the lifecycle value enum. Resolution: rename lifecycle enum to `LifecycleMessage`, free `Message` for the trait.

**Five conditions for soundness (C1-C5):**
- C1: service_id on Protocol not Message
- C2: CLONE_SAFE from #[obligation] annotations not trait inference
- C3: Handler-as-Handles<Lifecycle> preserves default method ergonomics
- C4: Message supertrait includes Serialize + DeserializeOwned + Send + 'static
- C5: Mixed-obligation enums stay unified (no split)

**EAct references:**
- T-Send Fig. 3: ℓ in selection precondition, V : A_j
- TV-Handler Fig. 3: handler branches match session type labels exactly
- TH-Handler Fig. 8 (line 2732): V : Handler(S_in, A), handler value typed by input session type + actor state
- E-React Fig. 4 (line 1986): single dispatch mechanism (ℓ(x) → M) ∈ H̄
- Progress Theorem 3.10 (line 2973): compliant protocols + well-typed → can reduce or canonical form
- Global Progress Corollary 3.14 (line 3139): handler termination → eventual progress

**How to apply:** Use these conditions as implementation constraints for Phase 1 Protocol trait + Message derive work in PLAN.md. The naming collision must be resolved before Protocol/Message types are defined.
