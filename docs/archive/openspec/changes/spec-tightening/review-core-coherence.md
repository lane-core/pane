# Core Specification Coherence Review

Reviewer: Be systems engineer consultant
Date: 2026-03-21
Corpus: foundations/spec.md, architecture/spec.md, foundations/manifesto.md, + 8 downstream specs

---

## Critical — Would cause implementation confusion

### C1. Session type implementation: `par` vs custom typestate

The architecture spec (SS7) commits to the `par` crate for session types. The technology table (SS11) says "`par` crate — Linear logic correspondence." The transport bridge section (SS7) describes bridging par's async model with calloop and discusses par's `fork_sync`, the server module, and par's futures-based internals.

But `agent-perspective.md` says the session types are now custom: "a typestate `Chan<S, Transport>` designed for pane's exact needs: transport-aware, crash-safe (Err not panic), calloop-compatible." It explicitly says "I'm not trusting a third-party library's interpretation of linear logic — I'm trusting a verified implementation purpose-built for this system."

These are contradictory. The architecture spec is the engineering document of record and it says `par`. The agent-perspective document (which reads as a later writing) says custom typestate. An implementer reading the architecture spec will build on `par`; an implementer reading the agent document will build custom. This must be resolved — either update the architecture spec to reflect the custom decision, or update the agent perspective to reflect that `par` is still the plan.

**Recommendation:** The session type build-vs-buy decision (already captured in my earlier assessment) leans toward custom. If that's the direction, the architecture spec SS7 and SS11 need to replace `par` references with the custom `Chan<S, Transport>` design, and the transport bridge section needs rewriting.

### C2. Rendering engine: smithay GLES vs Vello

The architecture spec says two things about rendering:
- Compositor composites via "smithay's GLES renderer" (SS3, SS10)
- Widgets render via "Vello (GPU-compute 2D rendering via wgpu)" (SS4 pane-ui, SS11)

The aesthetic spec says "The kit SHALL render via Vello — GPU-compute 2D rendering through wgpu."

The compositor spec says "composites all client buffers into the output framebuffer via smithay's GLES renderer."

These are not contradictory in principle (clients render with Vello into buffers, compositor composites with GLES), but the architecture spec never makes this split explicit. SS10 says "smithay's GLES renderer" for compositing but doesn't say "clients render with Vello." The open question about Vello/GLES integration (SS13) acknowledges the tension but doesn't resolve it. An implementer needs to know: is chrome rendered with GLES (by the compositor) or Vello (by the kit)? The compositor renders tag lines — those involve text, gradients, bevels. Which engine renders compositor-side chrome?

**Recommendation:** Add a sentence to architecture SS10 stating explicitly: body content renders via Vello (client-side, through the kit), chrome renders via smithay's GLES renderer (compositor-side). If chrome rendering eventually needs Vello quality (gradients, translucency), flag that as an open question.

### C3. `pane-dbus` appears in architecture but has no downstream spec

The architecture spec lists `pane-dbus` in the build sequence (Phase 7, item 16: "D-Bus bridge — notifications, PipeWire portals, NetworkManager") and in the technology table (SS11: "zbus crate — pane-dbus translates at the boundary"). The licensing spec lists it as AGPL. The foundations spec (SS3) names bridge trust as a named tension requiring trust models for each bridge.

There is no `pane-dbus` downstream spec. This is the boundary where pane's typed world meets the untyped D-Bus world — exactly the seam the foundations spec identifies as the site of day-to-day engineering friction. Without a spec, the trust model the foundations demands goes unaddressed.

**Recommendation:** Create `openspec/specs/pane-dbus/spec.md`. It should define: which D-Bus interfaces are bridged, the trust model for each, how D-Bus signals map to pane protocol events, and how type safety degrades at the bridge.

### C4. `pane-watchdog` appears in architecture but has no downstream spec

The architecture spec describes pane-watchdog in detail (SS3) and places it in Phase 6 of the build sequence. The licensing spec lists it as AGPL. But there is no downstream spec with requirements and scenarios.

Given that the watchdog's simplicity is load-bearing ("the less it does, the harder it is to kill"), a spec is needed precisely to prevent scope creep.

**Recommendation:** Create `openspec/specs/pane-watchdog/spec.md` with requirements codifying the Erlang heart pattern, heartbeat protocol, escalation triggers, and the explicit boundary of what it does NOT do.

### C5. `pane-roster` appears in architecture but has no downstream spec

The architecture spec describes pane-roster in detail (SS3): service directory, application lifecycle, session save/restore, service registry with quality-based selection. The plugin-discovery spec depends on it (`.app` directories detected by pane-roster). But there is no downstream spec.

Roster is the component that makes the application ecology work. Launch semantics (single/exclusive/multiple), service registry queries, process tracking via pidfd, session save/restore — these all need requirements and scenarios.

**Recommendation:** Create `openspec/specs/pane-roster/spec.md`.

### C6. `pane-store` appears in architecture but has no downstream spec

The architecture spec describes pane-store in detail (SS3): xattr-based attribute indexing, fanotify change detection, query interface, live queries. The plugin-discovery spec depends on it (plugin metadata queryable via pane-store). The filesystem-config spec depends on it (config keys queryable across the system). But there is no downstream spec.

**Recommendation:** Create `openspec/specs/pane-store/spec.md`.

---

## Important — Should be fixed before building

### I1. Foundation principle SS4 (Optics) has no concrete architecture realization

The foundations spec devotes an entire section (SS4) to optics — lens laws, composition, and the convergence of session types + optics = scripting protocol. The architecture spec discusses optics in the scripting protocol section (SS5) and lists dynamic optic composition as an open question (SS13).

But no downstream spec addresses optics. There is no optic type defined in any spec. The scripting protocol section (SS5) is aspirational — it describes the ResolveSpecifier pattern and says "pane recovers it" but never specifies the concrete optic types, the trait an optic-exposing handler must implement, or the wire format for optic-addressed property access.

The foundations spec calls this "the hardest design problem in translating this concept." The architecture spec acknowledges it in open questions. Neither resolves it. No downstream spec attempts to.

This is correctly an open question. But the risk is that the scripting protocol — identified by both specs as one of BeOS's most important features — becomes permanently deferred because no spec ever forces the design to become concrete.

**Recommendation:** Create `openspec/specs/scripting-protocol/spec.md` even if parts are marked TBD. At minimum, define the trait a scriptable handler must implement, the wire representation of a specifier chain, and the GetSupportedSuites equivalent. The open question about dynamic vs static composition can remain open, but the protocol shape should be specified.

### I2. Terminology: "routing" overloaded across three different meanings

The term "routing" appears in three distinct senses:

1. **Content routing** (pane-app kit): tag line text is activated, the kit evaluates routing rules, content is dispatched to a handler. This is the "plumbing" concept from Plan 9/acme.
2. **AI model routing** (pane-ai, architecture SS4): routing rules dispatch inference requests to local vs remote models. "The routing rule IS the privacy policy."
3. **Message routing** (eliminated router): the old pane-route server, now gone. But "routing" in the pane-app kit section (SS4) still reads like message routing in places.

The plugin-discovery spec uses "routing rules" for content routing (sense 1). The architecture spec uses "routing rules" for both content routing and AI model routing interchangeably. The foundations spec doesn't disambiguate.

An implementer building the routing rule evaluator in the pane-app kit needs to know: are content routing rules and model routing rules the same mechanism? The architecture spec implies yes ("the same routing infrastructure that dispatches content to handlers dispatches inference requests to models") but the domains are different enough that a single rule format may not serve both.

**Recommendation:** Distinguish explicitly. Either (a) confirm they are the same rule evaluator operating on the same rule format (and explain what a unified rule looks like), or (b) define content routing and model routing as separate mechanisms that share a common pattern but have different rule schemas.

### I3. Terminology: "pane-ui" vs "Interface Kit" vs "pane-text"

The architecture spec calls the rendering kit "pane-ui" and compares it to "the Interface Kit." The kit hierarchy (SS4) shows pane-text below pane-ui, providing text buffer data structures and structural regular expressions. pane-ui provides "text rendering, styling primitives, layout, widget rendering."

But the aesthetic spec says "the kit SHALL render via Vello" without specifying which kit. The compositor spec says "the Interface Kit (pane-ui) renders text, widgets, and graphics." The architecture SS10 says "the Interface Kit (pane-ui) provides shared rendering infrastructure."

The question: is text rendering a pane-ui concern or a pane-text concern? The hierarchy shows pane-text below pane-ui, which implies pane-text provides data structures and pane-ui provides rendering. But pane-text is described as "text buffer data structures and structural regular expressions" — this sounds like it includes editing operations, not just storage.

**Recommendation:** Make explicit: pane-text owns text content (buffers, editing operations, structural regexps). pane-ui owns text rendering (glyph atlas, GPU-accelerated text display). The split should be clear enough that an implementer knows which crate to put a function in.

### I4. Build sequence: Phase 3 includes agent infrastructure before the compositor exists

Phase 3 item 6: "Minimal agent infrastructure — agent user accounts, `.plan` file convention, message passing." Phase 4 item 6 (numbering collision — see M4): "pane-comp skeleton — first pixels on screen."

The development methodology says agents inhabit the system from the earliest phases, and the agent-perspective document describes agents using the compositor. But Phase 3 agents have no compositor to connect to. The architecture spec says agents communicate through "the same session-typed protocol" as everything else, but at Phase 3 the compositor doesn't exist yet.

This isn't necessarily wrong — agents could participate via protocol and filesystem without visual output. But the spec doesn't say this. It says "agents inhabit the system under development" without clarifying what "inhabit" means before the compositor exists.

**Recommendation:** Clarify in Phase 3 that agent infrastructure at this stage is headless — agents communicate via protocol and filesystem, exercise the transport bridge and pane-app kit, but do not have visual panes until Phase 4.

### I5. `pane-shell` is referenced in build sequence but has no spec or kit definition

Phase 4 item 7: "pane-shell — PTY bridge client, first usable terminal. The milestone that makes pane a daily driver." pane-text provides "editing primitives that pane-shell and editor panes compose with."

But pane-shell is not in the kit hierarchy, not in the server list, not in the licensing spec, and has no downstream spec. Is it a kit, a server, an application? It's called a "client" in the build sequence. It appears to be the first pane-native application — the terminal equivalent.

**Recommendation:** Determine whether pane-shell is a kit (shared terminal infrastructure) or an application (the stock terminal). If it's the first pane-native application, say so explicitly. Consider whether it needs a spec or whether its requirements emerge from the pane-app and pane-text kit specs.

### I6. Foundation named tension "bridge trust" not addressed by any downstream spec

Foundations SS3 names it: "Bridges translate at the boundary, but the bridge is where type safety ends." Foundations SS8 reinforces it: "The implementation specs must define a trust model for each bridge." The architecture spec (SS7 crash handling, SS3 pane-dbus mention) acknowledges D-Bus as a bridge but doesn't define the trust model. No downstream spec addresses bridge trust at all.

This overlaps with C3 (missing pane-dbus spec) but is broader — the trust model applies to any bridge: D-Bus, PipeWire, XWayland, legacy Wayland clients.

**Recommendation:** Each bridge-related spec (pane-dbus, pane-media, the XWayland/legacy Wayland section) should include a trust model subsection defining: what the bridge trusts from the foreign side, what it validates, what it cannot validate, and what the failure mode is.

### I7. Foundation named tension ".plan governance" partially addressed

Foundations SS10 names it: "The governance tension: who authors and audits it?" The architecture spec (SS4 pane-ai) says "The governance question is resolved by the same mechanisms as any other configuration: filesystem permissions, version control, audit trails." Architecture SS13 lists agent governance as an open question with specific sub-questions.

This is addressed but not resolved. The foundations spec raises it as a tension; the architecture spec punts it to the open questions. No downstream spec addresses it.

**Recommendation:** Acceptable to leave open for now, but when `openspec/specs/pane-ai/spec.md` is written, it must address governance concretely — not just "filesystem permissions" but specific defaults, escalation paths, and audit mechanisms.

### I8. Compositor spec references only winit backend

The compositor spec's requirements start with "Compositor boots with winit backend." The architecture spec mentions "smithay's DRM backend" for multi-monitor. But the compositor downstream spec has no requirements for the DRM/KMS backend — the production rendering path.

This is fine for early development (winit is the development backend), but the spec should acknowledge that winit is the development target and DRM/KMS is the production target.

**Recommendation:** Add a note or future requirement to the compositor spec acknowledging the DRM/KMS backend as the production path.

### I9. Three-tier access model duplicated verbatim

The three-tier access table (filesystem/protocol/in-process with latency figures) appears identically in:
- Architecture spec SS3 (pane-fs section)
- Compositor spec SS7
- pane-fs spec

This isn't a contradiction — it's the same table. But duplication creates a maintenance risk: if latency targets change, all three must be updated.

**Recommendation:** The architecture spec should be the canonical source. The downstream specs should reference it rather than duplicating.

---

## Minor — Editorial

### M1. Build sequence item numbering collision

Phase 3 and Phase 4 both have an item numbered 6. Phase 3: "Minimal agent infrastructure" is item 6. Phase 4: "pane-comp skeleton" is also item 6. The numbering should be continuous (items 1-23) or restart per phase. Currently it's inconsistent — some phases continue numbering, some restart.

### M2. Font inconsistency between filesystem-config and aesthetic specs

The filesystem-config spec uses "Iosevka" as an example font value: `cat /etc/pane/comp/font` returns "Iosevka". The aesthetic spec declares Inter (proportional) and Monoid (monospace) as official fonts. Iosevka appears nowhere in the aesthetic spec. The filesystem-config example should use one of the declared official fonts to avoid confusion about which fonts are canonical.

### M3. Config path ambiguity: `/etc/pane/comp/` vs design tokens

The aesthetic spec says "design tokens defined in the kit" and "configurable via `/etc/pane/comp/`." The filesystem-config spec defines `/etc/pane/<server>/` as the pattern. But design tokens are kit-level (compiled into pane-ui), while config files are server-level (read by pane-comp). Are they the same values? Does changing `/etc/pane/comp/accent-color` change the kit's design token, or does the compositor need to relay it?

The compositor renders chrome using design tokens. Clients render bodies using kit design tokens. If the accent color changes via `/etc/pane/comp/accent-color`, the compositor picks it up via pane-notify. But how do clients pick it up? The kit is in-process — it needs to learn about the change somehow.

**Recommendation:** Specify the propagation path: config file changes -> pane-notify -> compositor -> protocol event to clients -> kit updates tokens. Or: clients also watch `/etc/pane/comp/` via pane-notify independently.

### M4. "Frutiger Aero" undefined

Both the architecture spec and aesthetic spec use "Frutiger Aero" as the name of the aesthetic without defining the term. The architecture spec gives examples (beveled borders, subtle gradients, warm saturated palette). The aesthetic spec gives requirements. But the term itself is a retronym from internet design discourse, not a formal design movement. A reader unfamiliar with the term will not know what it means.

**Recommendation:** Add a brief definition where the term first appears: "Frutiger Aero — a design aesthetic characterized by [core properties], named retroactively for the visual language common to early-2000s interfaces (Windows Vista/7 era, early Aqua)."

### M5. Manifesto references "wio" without context

The manifesto says "wio tried a subset of this and hit fundamental impedance mismatches." A reader unfamiliar with wio will not know what this refers to. Brief parenthetical: "(wio — a Plan 9-inspired Wayland compositor)."

### M6. pane-input placement in build sequence vs dependency

The kit hierarchy shows pane-input depending on pane-text and pane-ui. But the build sequence places pane-input in Phase 8 (item 19) while pane-text and pane-ui are implied in earlier phases (pane-shell in Phase 4 uses pane-text; widget rendering in Phase 7 item 14 uses pane-ui). This means the Input Kit's grammar engine doesn't exist during the phases when the compositor's input handling (Phase 5, item 9: "compositor-level key bindings") is being built.

Phase 5 builds "input binding — compositor-level key bindings, focus management, tag switching" without the Input Kit. This is presumably hardcoded compositor bindings, with the Input Kit added later to generalize them. That's fine, but the spec should say so.

### M7. Accessibility: semantic view mentioned but never specified

Foundations SS2 lists "semantic object to accessibility infrastructure" as one of the pane's views. Architecture SS2 lists a "Semantic" view with "roles, values, and actions for accessibility infrastructure." Architecture SS4 (pane-ui) says "the accessibility tree is a byproduct of the widget model." But no downstream spec addresses accessibility. No requirements, no scenarios, no protocol for accessibility tree export.

This is Phase 7+ territory and doesn't block early work, but it should be tracked.

### M8. Foundation tension "aesthetic customization friction" not fully resolved

Foundations SS9 names a tension: "the extension model celebrates low-friction composition (drop a file, gain a behavior), but aesthetic customization demands may in principle be higher friction." The aesthetic spec resolves this by saying "no theme engine" — customization is constrained to individual design tokens. But the foundations spec says "the implementation specs must have a rigorous standard of determining the stock aesthetic retains its identity while customization remains accessible." The aesthetic spec doesn't define what "retains its identity" means concretely — which tokens can change without breaking identity, and which cannot.

### M9. `pane-ai` kit in architecture vs build sequence discrepancy

The kit hierarchy (SS4) lists `pane-ai` at the top of the dependency tree. The build sequence lists it as Phase 8 item 20. But Phase 3 item 6 says "Minimal agent infrastructure" — agent accounts, `.plan`, message passing, Unix communication patterns. The architecture calls this "Not the full AI Kit — just enough for agents to participate as system users."

This split is clear in the architecture text but the kit hierarchy doesn't reflect it. The hierarchy shows a single `pane-ai` crate. The build sequence implies two stages: a minimal agent infrastructure (Phase 3) and the full AI Kit (Phase 8). These may be the same crate at two maturity levels, or they may be separate crates. Unclear.

### M10. Foundations SS8 says "one communication model" but architecture has two

Foundations SS8 says "one communication model (session-typed protocols with kit-level routing)." But the architecture describes two communication models: the pane session protocol (for native clients) and the Wayland protocol (for legacy clients). The architecture acknowledges this as "the two-world problem" (SS13) and describes progressive integration. But the foundations spec's claim of "one communication model" is aspirational, not descriptive of the actual architecture, which necessarily has two.

---

## Named Tension Scorecard

| Tension (from Foundations) | Architecture addresses it? | Downstream spec addresses it? |
|---|---|---|
| pane vs fd (SS2) | Yes — filesystem projection as bridge, "which direction governs" noted | pane-fs spec defines the filesystem tier but doesn't address governance direction |
| optics vs crash (SS4) | Yes — crash boundary, temporary lens law violations | No downstream spec |
| bridge trust (SS3, SS8) | Partially — D-Bus mentioned, trust model deferred | No downstream spec (C3) |
| aesthetic customization (SS9) | Yes — "no theme engine," constrained tokens | Aesthetic spec defines tokens, doesn't define identity boundary (M8) |
| .plan governance (SS10) | Partially — open question in SS13 | No downstream spec (I7) |
| typed/untyped boundary (SS3) | Yes — bridge concept, "weathering the elements" | No downstream spec |

---

## Build Sequence Dependency Check

| Phase | Implicit dependency not captured | Severity |
|---|---|---|
| Phase 3 (agent infra) | Depends on Phase 2 transport bridge, but agents need filesystem infrastructure (pane-notify) which is also Phase 3. Ordering within Phase 3 matters: pane-notify must precede agent infra. | Low — likely obvious to implementer |
| Phase 4 (compositor) | pane-comp needs pane-notify for config watching (filesystem-config spec says "all servers SHALL watch their config directories via pane-notify"). pane-notify is Phase 3, so ordering is correct. | None |
| Phase 5 (input binding) | Input binding at Phase 5 is compositor-level hardcoded bindings. The Input Kit (Phase 8) generalizes them. This two-stage approach is not stated. | Low (M6) |
| Phase 6 (routing) | Routing in pane-app kit depends on pane-roster for multi-match queries. Both are Phase 6. Ordering within Phase 6 matters: roster should precede or be concurrent with routing. | Low |
| Phase 7 (pane-fs) | pane-fs at Phase 7 means the filesystem tier of the three-tier model doesn't exist until Phase 7. Phases 3-6 can only use the protocol tier. Any spec that assumes filesystem availability (e.g., agent `.plan` as files) works regardless because `.plan` files are on the real filesystem, not `/srv/pane/`. | None |
| Phase 8 (legacy Wayland) | XWayland and legacy Wayland support at Phase 8 means no Firefox, no Chromium, no Electron apps until Phase 8. This is very late for a "daily driver" milestone claimed at end of Phase 5. | Important — the daily driver claim at Phase 5 is only true if the user's entire workflow is pane-native (i.e., pane-shell only). |
| Phase 8 (pane-ai) | Full AI Kit at Phase 8 depends on pane-input (also Phase 8) for grammar integration? Or is it independent? | Low |

---

## Summary

**The core story is consistent.** Foundations -> Architecture -> Manifesto tell a coherent story. The philosophy is clear, the architecture realizes it faithfully, the manifesto provides appropriate context. The terminology is mostly consistent and the design philosophy carries through.

**The main gaps are missing downstream specs** (C3-C6) for components the architecture describes in detail but which have no requirements/scenarios documents. This is the most actionable finding.

**The most dangerous inconsistency is C1** (par vs custom session types). An implementer needs to know which to build on. The architecture spec must be updated to reflect whichever decision is current.

**The most important structural gap is I1** (optics/scripting protocol never becoming concrete). This is correctly identified as hard, but without a spec forcing it toward concreteness, it risks permanent deferral — and the scripting protocol is identified as one of BeOS's most important features.

**The build sequence is mostly sound** but the "daily driver" claim at Phase 5 is misleading without legacy Wayland support (Phase 8). Either move legacy Wayland earlier or qualify the daily driver claim.
