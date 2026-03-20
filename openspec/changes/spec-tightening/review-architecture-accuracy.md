# Architecture Spec: Accuracy and Consistency Review

Reviewed: `openspec/specs/architecture/spec.md` against `openspec/specs/foundations/spec.md`
Date: 2026-03-20

---

## Critical

Issues that would lead to wrong implementation decisions.

### C1. Monadic error composition absent from architecture

The foundations spec (section 6) names "monadic error composition" as one of three theoretical commitments, and section 4 calls the scripting protocol the "single convergence point" of "session types, optics, monadic error composition." The architecture spec mentions "Monadic error handling" exactly once (section 5, line 331) as a clause in a sentence about the scripting protocol. It never explains what monadic error composition means architecturally, how errors compose through the kit pipeline, or how this differs from conventional Result types. An implementer reading only the architecture spec would treat errors as standard Rust Result/Option and miss the monadic composition requirement entirely.

The foundations spec's section 6 is titled "Monadic Error Composition" and establishes that "failures are values, not exceptions" and "error handling composes through the same typed channels that handle the happy path." The architecture spec's section 7 (Protocol Design) has a "Crash handling" subsection that discusses catch_unwind but never connects it to the monadic composition principle. The crash handling design (catch_unwind boundary, session-terminated event) is described in mechanical terms without grounding it in the foundations commitment.

### C2. Foundations lists "notification system" as core infrastructure; architecture has no such component

Foundations section 10 says: "The core system layer -- compositor, roster, attribute store, filesystem interface, notification system, watchdog, init abstraction -- has an intimate relationship with the host OS."

The architecture spec has no "notification system" server. pane-notify is a kit library (fanotify/inotify abstraction), not a notification system in the user-facing sense (desktop notifications like freedesktop.org notifications). The architecture spec mentions D-Bus bridge handling "notifications" in the build sequence (Phase 5, item 14: "pane-dbus -- D-Bus bridge (notifications, PipeWire portals, NetworkManager)") but this is a bridge to freedesktop notifications, not a core system component.

The foundations spec's notification example in section 2 ("a notification is a pane") establishes that notifications are panes with filesystem projections, queryable attributes, and routing rules. How this actually works architecturally -- who creates the notification pane, how external notification sources are translated, how retention policies are expressed -- is unspecified.

Either the foundations spec's enumeration of core components is stale (listing something that was absorbed into other mechanisms), or the architecture spec is missing a component. Either way, an implementer can't build the notification story from what's in the architecture spec.

### C3. Foundations says "init abstraction"; architecture says s6 concretely

Foundations section 10 lists "init abstraction" as a core system layer component, implying the design should abstract over init system choice. The architecture spec section 9 commits concretely to s6, s6-rc, and s6-linux-init with detailed boot sequences, service definition formats, and s6-fdholder integration. This is the right engineering call (the architecture draft explicitly eliminated init abstraction as unnecessary), but the foundations spec hasn't been updated to match. An implementer reading both would be confused about whether the init choice is supposed to be abstracted or concrete.

### C4. Tag line rendering contradiction between sections 2 and 4

Section 2 (The Pane Primitive) says: "The tag line is always compositor-rendered (the compositor owns the chrome)."

Section 4 (Kit Decomposition), under pane-ui: "Text rendering, tag line management, styling primitives, layout, widget rendering."

If the tag line is always compositor-rendered, why does pane-ui (a client-side kit) manage tag lines? There's a plausible reading (pane-ui manages tag line content that the client sends to the compositor for rendering), but the division of responsibility is ambiguous. When section 2 says "tag line content travels through the pane protocol: the client sends tag content, the compositor renders it," that's clear. But "tag line management" as a pane-ui responsibility muddies it. An implementer needs to know: does pane-ui prepare tag line content (composing the text, determining what commands appear), or does it render the tag line pixels?

---

## Important

Ambiguities that would cause confusion during implementation.

### I1. "Interface Kit" used ambiguously for both BeOS and pane

The architecture spec uses "Interface Kit" to mean three different things:

1. BeOS's Interface Kit (the historical reference): "This is how BeOS worked... every application using the Interface Kit" (section 10, line 593)
2. Pane's equivalent kit (pane-ui): "Pane's Interface Kit has layout management (taffy -- flexbox/grid) from the beginning" (section 4, line 237)
3. An alias clarified in parentheses: "the Interface Kit (pane-ui) provides shared rendering infrastructure" (section 10, line 595)

Most of the time the meaning is recoverable from context, but several instances are genuinely ambiguous. Lines 619 and 643 ("The Interface Kit renders at the scaled resolution", "Shared across all pane-native clients via the Interface Kit") could mean either. The kit hierarchy diagram in section 4 uses only the crate name (pane-ui), which is the correct canonical reference. Pick one name and use it consistently. When referring to BeOS's Interface Kit in a historical comparison, say "BeOS's Interface Kit."

### I2. Glyph atlas sharing claim needs architectural clarification

Section 4 (pane-ui) says: "rasterized glyphs are cached and shared across pane-native processes via shared memory, so the per-process cost is a lookup rather than re-rasterization."

Section 10 says: "Glyph atlas, color palette, control styles, layout primitives."

Technology table says: "Instanced rendering. Shared across all pane-native clients via the Interface Kit."

This is described as if it's straightforward, but it raises significant design questions. Who owns the shared memory segment? Who decides when to evict glyphs? How do multiple processes coordinate writes to the atlas without corruption? Is this a shared read-only atlas that some central process (compositor?) populates, or is it a concurrent data structure with multi-writer access? The claim "shared across pane-native processes via shared memory" implies a specific architecture that isn't specified.

This matters because it's one of the few places where pane needs shared mutable state across process boundaries, which cuts against the message-passing-only model the rest of the architecture promotes.

### I3. Route dispatch for legacy panes is contradictory

Section 3 (pane-comp) says: "For native panes, a route action sends a TagRoute event to the pane client; the pane-app kit evaluates routing rules and dispatches. For legacy panes, the compositor handles route dispatch through its own kit integration."

This means the compositor itself links against pane-app kit for routing. But section 3 also says pane-comp "Does not contain: routing logic." These are in direct contradiction. Either the compositor contains routing logic (for legacy panes), or it doesn't. The distinction "routing logic" vs "kit integration that happens to include routing" is too subtle to be useful.

### I4. Optics are structural in foundations but unresolved in architecture

Foundations section 4 makes optics a core theoretical commitment: "The relationship between internal state and each external view is governed by optics -- composable, bidirectional access paths." It describes composition: "the projection from internal state to protocol messages, composed with the projection from protocol to filesystem representation, gives the composite projection from internal state to filesystem."

The architecture spec mentions optics in two places: section 2 (pane primitive, briefly) and section 5 (scripting protocol, substantively). The scripting protocol section proposes "dynamic optic composition at the protocol level" but acknowledges this is the "hardest design problem" and flags it as an open question. Fair enough.

But there's a larger gap: optics as the mechanism governing the relationship between the four views of a pane (visual, protocol, filesystem, semantic) is asserted in section 2 but never architecturally realized. How does pane-ui's rendering relate to the optic from internal state to visual view? How does pane-fs's FUSE translation relate to the optic from internal state to filesystem view? These are described as independent subsystems, not as optic compositions. The architectural design as written would work fine without optics for anything except the scripting protocol -- which means the foundations spec's claim that optics are the "vertical structure" of the whole system is not yet reflected in the architecture.

### I5. Accessibility architecture is structurally absent

Foundations section 2 lists "a semantic object to accessibility infrastructure" as one of the four views of a pane. Architecture section 2 lists "Semantic: roles, values, and actions for accessibility infrastructure" as one of the four views. Section 4 (pane-ui) mentions that "the accessibility tree is a byproduct of the widget model."

But how? The architecture never describes:
- What accessibility protocol pane implements (AT-SPI2? Something custom?)
- How the semantic view of a pane is projected
- Where accessibility infrastructure lives (compositor? kit? separate server?)
- How the semantic view stays consistent with the other views per the optics commitment

The statement "the accessibility tree is a byproduct of the widget model" is the extent of the architectural specification for accessibility. This is insufficient -- an implementer cannot derive an accessibility architecture from this sentence.

### I6. Scratchpad/floating element concept mentioned but unarchitected

Section 10 (The Aesthetic) mentions "Floating elements (scratchpads, popups) are translucent to show context." The layout model in section 2 describes only a tree of containers with splits and leaves. Floating elements (popups, context menus, dropdowns, tooltips, scratchpads) need a different model -- they sit above the layout tree, have their own positioning rules, and interact with focus differently. The architecture doesn't describe how the compositor handles floating panes vs. tiled panes, yet this affects the compositor's core data structures.

### I7. pane-store "free attributes" claim is slightly misleading

Section 3 (pane-store) says: "Certain attributes are always available and always indexed: pane type, creation time, modification time, MIME type. This mirrors BFS's three always-indexed attributes (name, size, last_modified)."

The word "mirrors" is misleading. Pane's four free attributes (pane type, creation time, modification time, MIME type) are a different set from BFS's three (name, size, last_modified). The principle is the same (some attributes are always available), but the specific attributes differ, and the count differs. "Mirrors" suggests closer correspondence than exists. Also: name and size -- two of BFS's three -- aren't in pane's free set. MIME type and pane type -- two of pane's four -- weren't in BFS's free set (MIME type was an attribute you could create an index for, but it wasn't a default index like name/size/last_modified).

### I8. Session save/restore is described in two places with different owners

Section 3 (pane-roster): "Session save/restore: serializes running app state, restores on login."

Section 8 (Composition): "The compositor serializes layout. The roster serializes the running app list. Each app serializes its own state. On restart, each component restores its part."

The first implies pane-roster owns session save/restore. The second distributes it across compositor, roster, and each app. These are compatible readings (roster coordinates, each component does its part), but the single-line description under pane-roster's responsibilities overstates its role. An implementer reading only section 3 would build session save/restore entirely in pane-roster.

### I9. Two-world problem section mixes architectural mechanism with UX vision

Section 13 (Open Questions), "The two-world problem," starts by correctly identifying the tension between native and legacy Wayland clients. It then introduces the `.app` directory concept, Nix flake-based installation, progressive integration, and "the framework models user actions as an internal representation with metadata." This last clause appears from nowhere -- it's the first and only mention of user actions being modeled as an internal representation with metadata. It's a significant architectural claim buried in an open question with no grounding in the rest of the document. What is this internal representation? Where is it defined? How does it relate to the pane protocol? This reads like an idea that was explored and partially written up but not integrated into the architecture.

---

## Minor

Terminology inconsistencies, missing references, editorial issues.

### M1. Gassée quote is accurate but applied out of original context

The architecture spec (section 3) attributes to Gassée: "people developing the system now have to contend with two programming models." This is from Be Newsletter #1-4, "Heterogeneous Processing." In context, Gassée was discussing the problems of heterogeneous DSP+CPU architectures -- having a DSP with its own operating system alongside the main CPU created two programming models. The architecture spec applies it to a central message router creating two programming models (direct messaging + router-mediated messaging). The principle transfers well, but an informed reader would notice the original context was about hardware heterogeneity, not software routing architecture. Consider either noting the recontextualization or paraphrasing instead of quoting.

### M2. Raynaud-Richard thread cost numbers are slightly imprecise

The architecture spec says: "~20KB per thread, ~70KB per window with both client and server threads."

The actual newsletter data (Be Newsletter #4-46): ~20KB per thread, ~56KB for a window with both threads created, ~70KB for a window that has been Show()n (which adds rendering-readiness overhead beyond just having both threads). The ~70KB figure includes more than "both client and server threads" -- it includes the window being shown. This is a minor imprecision that doesn't affect pane's design, but it slightly mischaracterizes what the 70KB buys you.

### M3. "Support Kit" analogy for pane-proto is slightly off

Section 4 says: "pane-proto is the foundation -- pure types and serialization, analogous to the Support Kit."

BeOS's Support Kit contained BString, BList, BFlattenable, BArchivable, BLocker, BAutolock, and other utility classes. It was a utility grab-bag, not a protocol/type definition kit. pane-proto is described as "wire types, session type definitions, serde derivations, validation" -- this is more like BeOS's app_server protocol headers than the Support Kit. The analogy is close enough for orientation but would confuse someone who knows the BeOS Support Kit well.

### M4. "pane-shell" described as "PTY bridge client" without architectural detail

Build Sequence Phase 2 says: "pane-shell -- PTY bridge client, first usable terminal."

This is the only mention of pane-shell's architecture. It's listed as a milestone ("makes pane a daily driver") but never described in the kit or server decomposition sections. How does pane-shell interact with the compositor? Does it use pane-ui for rendering, or something different? What makes it a "bridge" -- is it bridging a PTY's byte stream into the pane protocol? Is it a pane-native client? These questions have obvious answers (yes, it uses pane-ui; yes, it's a native client that wraps a PTY), but they should be stated.

### M5. pane-text appears in kit hierarchy but is barely described

pane-text gets one paragraph (section 4, lines 241-245) describing structural regular expressions. Its relationship to pane-ui is unclear from the hierarchy diagram -- it's at the same level as pane-ui but below pane-input. Is pane-text a dependency of pane-ui? The hierarchy shows pane-text and pane-ui at the same tier. If a developer is building a text-editing pane, do they use pane-text directly, or through pane-ui? The boundary between "text buffer data structures" (pane-text) and "text rendering" (pane-ui) needs clarification.

### M6. Missing cross-reference: foundations section 2 seam between file and pane

Foundations section 2 says: "The pane as universal object coexists with Linux's own universal abstraction: the file descriptor. This creates a seam: for host-level tools, the file is primary and the pane is a view; for pane-native participants, the pane is primary and the file is a projection. The mutable specs must establish conventions for which direction governs in each context."

The architecture spec never addresses this seam directly. pane-fs provides the filesystem projection, but nowhere does the architecture establish conventions for "which direction governs" -- when a conflict exists between file state and pane state, which is authoritative? This is a foundations mandate ("the mutable specs must establish") that the architecture doesn't fulfill.

### M7. Missing cross-reference: foundations section 8 trust model for bridges

Foundations section 8 says: "Bridges are where type safety meets its limits: the foreign side is unverified. The mutable specs must define the trust model for bridges."

The architecture spec describes pane-dbus as a bridge (D-Bus bridge, section 11 technology table) but never defines a trust model for bridges. What does the system assume about data coming through the D-Bus bridge? Is it validated? What happens when it violates protocol expectations? This is another foundations mandate left unaddressed.

### M8. Missing cross-reference: foundations section 9 aesthetic/extension tension

Foundations section 9 says: "The mutable specs must find where the stock aesthetic retains its identity while customization remains accessible."

The architecture spec section 10 (The Aesthetic) says "No theme engine" and describes "One opinionated look" with individual properties configurable. This partially addresses the foundations mandate but doesn't engage with the specific tension the foundations spec raises: "the extension model celebrates low-friction composition (drop a file, gain a behavior), but aesthetic customization demands higher friction (rebuild through the kit)." The architecture doesn't explain where that boundary falls.

### M9. Build sequence phase ordering may not match dependency reality

Phase 2 has pane-app (item 4) before pane-shell (item 5), which is correct (shell depends on app kit). But Phase 4 has routing (item 8) before pane-roster (item 9). Since routing in the pane-app kit queries pane-roster's service registry for multi-match scenarios, routing without roster is limited to single-match cases. This isn't wrong (the build sequence can ship partial functionality per phase), but it should be noted that routing in Phase 4.8 is limited until pane-roster lands in Phase 4.9.

### M10. elogind dependency appears without introduction

The service definition example (section 9) lists `elogind` as a dependency of pane-comp but never introduces elogind, explains why the compositor depends on it, or discusses its role. For a document that explicitly discusses s6 as the init system and libudev-zero as a systemd replacement, the appearance of elogind (a systemd component extracted by Gentoo/Void) deserves a sentence explaining what it provides (seat management, session tracking) and why it's needed.

### M11. "Schillings" vs "Schillings'" possessive inconsistency

Line 180: `Schillings: "common things are easy..."` (no possessive, as attribution)
Line 380: `Schillings' benaphore` (possessive)

Both are correct grammatically, but the first use without a title or introduction reads oddly. The name appears for the first time at line 180 with no context -- who is Schillings? An implementer who doesn't know BeOS history wouldn't know this is Benoit Schillings, Be engineer. The Raynaud-Richard reference at line 394 at least has a first name.

### M12. Foundations section 10 mentions "agents" and ".plan" but architecture doesn't cross-reference

The foundations spec says: "An agent's behavioral specification is a human-readable, editable, version-controllable artifact in its home directory." The architecture spec (pane-ai section) says: "An agent's behavior... is declared in `.plan`."

Foundations uses "behavioral specification" (generic); architecture uses "`.plan` file" (concrete). The foundations spec never mentions `.plan` -- it says "home directory" as the location. The architecture's choice of `.plan` is well-motivated (the Unix `.plan` file tradition), but it's an architectural decision that should acknowledge it's concretizing a foundations principle.

### M13. Foundations section 4 open question about static vs dynamic optics is partially resolved

Foundations section 4 flags: "How optic-addressed access composes across handler boundaries in a running system -- where the structure is only known at runtime -- is the hardest design problem in translating this concept, and the mutable specs must solve it."

Architecture section 5 proposes an answer: "dynamic optic composition at the protocol level... each handler resolves one step and forwards." But section 13 (Open Questions) also flags this as unresolved: "How do optic-addressed property accesses compose across handler boundaries at runtime?" The architecture simultaneously proposes an approach and lists the problem as open. This is acceptable (the approach is proposed but unvalidated), but the relationship between the proposal (section 5) and the open question (section 13) should be made explicit.

### M14. The guide agent (foundations section 1) is absent from architecture

Foundations section 1 describes a "resident guide agent" that teaches pane using pane, learns the user's patterns, and uses the same tools the user will eventually use directly. This is a concrete user-facing experience concept. The architecture spec's pane-ai section describes agent infrastructure (Unix users, `.plan` files, communication through Unix primitives) but never mentions the guide agent specifically. The guide agent is the most user-facing application of the agent infrastructure and would be a natural place to demonstrate how the infrastructure works in practice.

### M15. Architecture spec says "the compiler" enforces session type adherence, but Phase 1 transport strategy is a hand-written state machine

Section 1: "What session types add to the Be model: compile-time enforcement of the protocol discipline."
Section 7 (Phase 1): "The actual socket transport is a hand-written state machine that mirrors the session type."

A hand-written state machine that "mirrors" the session type is exactly the kind of thing that can drift from the session type without the compiler catching it. The architecture acknowledges this: "the seam between typed protocol and untyped transport is where bugs live." But the confident claim in section 1 about compile-time enforcement is overstated for Phase 1, where the socket transport is not verified by the session type system. The mitigating claim -- "if you change it, the transport code stops compiling because it references the same enums and structs" -- is about structural consistency (using the same types), not protocol consistency (following the same state machine).

### M16. The "par" crate claims vs current reality

Architecture spec section 7: "The `par` crate implements session types in Rust via the Caires-Pfenning/Wadler correspondence..."

pane-proto's Cargo.toml does not list `par` as a dependency. It lists `optics = "0.3"`. The architecture spec correctly notes "session type migration in progress" in the build sequence, but the rest of the document (sections 1, 6, 7, 13) describes par integration as if it's the current approach rather than a planned migration. An implementer reading section 7 would expect to find par in the dependency tree.

This isn't wrong (the spec is aspirational/directional), but the present-tense framing ("Par operates on in-memory futures::channel::oneshot pairs") should be future-tense or marked as planned.

---

## Stale Artifact Check

### No stale artifacts found from the following previously-identified concerns:

- **Value/Compute polarity**: not present anywhere in architecture spec. Clean.
- **ProtocolState runtime state machine**: not present. Clean.
- **pane-route as a server**: properly eliminated and explained in section 3. The "Why no router server" subsection is clear and well-argued. Clean.
- **pane-init as abstraction layer**: not present in architecture. The architecture commits to s6 concretely. Clean. (But see C3 for the inconsistency with foundations.)
- **Centralized rendering**: mentioned only as a negative ("not by centralizing rendering"). Clean.

### One partial stale artifact:

The routing rules path `~/.config/pane/route/rules/` (section 4, line 216) contains the word "route" which is the old pane-route server name. This is fine as a filesystem path (routing rules should live in a "route" directory), but it's worth noting that this is the only surviving use of "route" as a noun in the architecture. All other references use "routing" (the activity) rather than "route" (the server). The path is reasonable and should probably stay as-is.

---

## Summary

The architecture spec is substantially well-written and internally consistent. The major gaps are between the architecture and foundations specs: three foundations mandates are unaddressed (notification system, init abstraction mismatch, file/pane seam governance), one theoretical commitment is architecturally absent (monadic error composition), and one is underspecified (optics as multi-view mechanism). Within the architecture spec, the main internal inconsistency is the routing logic contradiction for legacy panes (section 3 says the compositor both does and doesn't contain routing logic).

The document reads as written by someone who understands both the Be heritage and the target platform well. The technical claims are almost all accurate; the few imprecisions (Gassee quote context, Raynaud-Richard numbers, Support Kit analogy) are minor and don't affect design decisions. The biggest risk is an implementer reading the architecture spec without the foundations spec and missing the optics/monadic-error commitments entirely, since the architecture doesn't fully realize them.
