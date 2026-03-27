# Final Consistency Audit — Pane Spec Corpus

Audited 2026-03-26. All 13 specs + 2 pending integration materials.

---

## A. Pending Insertions

Spec language drafted in `maty-integration-plan.md` and `review-pane-session-ergonomics.md` but NOT yet applied to any spec.

### A1. Maty event actor validation paragraph (1a)
- **Target:** `architecture/spec.md` S7, after line ~456 ("calloop-compatible…")
- **Text:** "Event actor validation" paragraph (Fowler/Hu Maty proof, calloop = Maty handler, inter-session deadlock freedom). ~2 paragraphs.
- **Status:** Ready as-is. No conflicts with current S7 text. The existing S7 already mentions crash-safe and calloop-compatible properties; this adds the formal validation.

### A2. Binary session types sufficiency paragraph (1a)
- **Target:** Same location as A1, contiguous.
- **Text:** "Why binary session types suffice despite multiparty interactions" — compositor mediates N dynamic clients, global types for documentation only.
- **Status:** Ready. Slight adjustment needed: the existing architecture S7 says "Branching uses standard Rust enums" (line ~452) which describes the Select/Branch mechanism, but the Maty paragraph references a different pattern (active-phase typed enums). These are distinct mechanisms at different phases. The insertion should explicitly note this distinction to avoid confusion.

### A3. Active phase decomposition paragraph (1a)
- **Target:** Same location, contiguous.
- **Text:** Structured phases (handshake, teardown) use binary session types; active phase uses typed message enums with exhaustive match.
- **Status:** Ready, but needs one adjustment: the current architecture S7 line ~452 says "Branching uses standard Rust enums. Enum variants contain session continuations." This describes Select/Branch, not the active-phase pattern. The insertion text correctly distinguishes these, but the pre-existing line 452 could mislead readers into thinking there is only one enum pattern. Add a clarifying note at the existing text or restructure S7 to separate the two uses of enums.

### A4. Protocol phasing subsection (1b)
- **Target:** `architecture/spec.md` S7, new subsection after "Async by default" (~line 477), before "Crash handling" (~line 479).
- **Text:** Three-phase protocol (handshake/active/teardown), wire encoding, type examples. ~40 lines.
- **Status:** Ready. The handshake type example uses `Send<Ready, End>` as the accepted-branch terminal, but the ergonomics review (review-pane-session-ergonomics.md S5c) proposes renaming `close_and_take` to `finish`. The insertion should use `finish` terminology to match the recommendation, since both documents are being integrated simultaneously.

### A5. Foundations S3 enrichment sentence (1c)
- **Target:** `foundations/spec.md` S3, end of paragraph beginning "Session types formalize exactly this discipline" (after line ~55 "...the theoretical framework that lets the compiler verify it.").
- **Text:** One sentence about Fowler/Hu Maty validating the event-loop actor pattern.
- **Status:** Ready as-is. No conflicts.

### A6. Sources addendum (1d)
- **Target:** `architecture/spec.md` Sources section, after line ~836.
- **Text:** Fowler & Hu OOPSLA citation.
- **Status:** Ready. Note: `foundations/spec.md` Sources already cites "Fowler et al. (POPL 2019)" for a different paper. The two are distinct (POPL 2019 = Exceptional Asynchronous Session Types; OOPSLA = Maty/Speak Now). Both should be present in architecture Sources.

### A7. Ergonomics recommendations (review-pane-session-ergonomics.md)
- **Target:** Not spec text — these are implementation recommendations for pane-session crate.
- **Items:** `request()` combinator, `offer!`/`choice!` macros, `finish` rename, `into_active_phase` helper, `unix_pair`/`accept_session` constructors, protocol type doc convention, `TrackedChan`.
- **Status:** These do NOT need spec insertion. They are implementation guidance for the Phase 3 task list. However, the architecture spec S7's "Protocol phasing" subsection (A4 above) should reference the naming convention (`finish` not `close_and_take`) since it describes the API surface.

---

## B. Stale References

### B1. `par` crate references — architecture/spec.md

The architecture spec was updated to declare a custom session type implementation but retains par-era language in several places:

| Line | Text | Issue |
|------|------|-------|
| ~460-468 | "The transport bridge" subsection | Entire subsection describes par as the target, par's async model, par's runtime. The custom `Chan<S, Transport>` has replaced par. This subsection contradicts the paragraph at ~449-457 which correctly describes the custom implementation. |
| ~463 | "Only par's runtime gives you the second" | False — the custom Chan gives ordering too via typestate. |
| ~465 | "Par session types driven over unix sockets" | par is no longer a dependency or target. |
| ~467 | "define types in par, use them for in-memory testing" | par is not used. |
| ~469 | "like dialectic's Transmitter/Receiver backend trait" | Aspirational reference is fine as historical context but reads as current design intent when the custom impl already exists. |
| ~710 | "proptest + in-memory par channels" | Testing uses the custom memory transport, not par. |
| ~773-774 | "calloop + par integration" open question heading and first sentence | The heading says "par" but the body correctly describes the custom fd-based approach. The heading is stale. |
| ~822 | Sources: "faiface/par crate — session types for Rust (design reference, not a dependency)" | This is correctly qualified. Keep. |

**Action:** Rewrite the "transport bridge" subsection (~460-471) to describe the current custom implementation and the three-phase protocol pattern (drawing from A4 above). Replace the open question heading at ~773. Fix the testing row at ~710.

### B2. `par` in agent-perspective.md — line 61
- **Text:** "The session types used par."
- **Status:** Correct as historical narration. The sentence explicitly frames this as "the first version" that has since changed. No action needed.

### B3. `cell grid` references — workflow/spec.md, pane-fs/spec.md

| File | Line | Text | Issue |
|------|------|------|-------|
| workflow/spec.md | 101 | "implementing the cell grid renderer" / "CellRegion doesn't validate width > 0" | The cell grid was an earlier rendering concept for terminal content. The architecture spec now describes body rendering through the Interface Kit (pane-ui), not a cell grid abstraction. `CellRegion` does not appear in the codebase or any other spec. This scenario example is stale. |
| pane-fs/spec.md | 45 | "The tree does not expose rendering internals (cell grids, glyph data, buffer state)" | Parenthetical mention as a negative example. Technically fine — it says what NOT to expose. But "cell grids" implies they exist somewhere. Replace with more generic wording. |
| pane-fs/spec.md | 53 | "not a cell grid, not rendering state" | Same: a negative example that names a concept that doesn't exist in the current architecture. |

### B4. `bcachefs` mention — architecture/spec.md line 19
- **Text:** "bcachefs when it matures"
- **Status:** The architecture spec explicitly commits to btrfs (lines 182, 786). The dependency philosophy paragraph names bcachefs as a future possibility alongside "FUSE-over-io_uring" and "PipeWire over PulseAudio" (which are current commitments). The juxtaposition misleads: bcachefs is speculative while the others are decided. Rephrase to make the speculative nature explicit, or remove it to avoid contradicting the btrfs commitment.

### B5. `femtovg` mention — architecture/spec.md line 704
- **Text:** "The forward-looking choice over femtovg (OpenGL)."
- **Status:** femtovg was an earlier candidate. The parenthetical "(OpenGL)" is the rationale for rejecting it. This is correctly framed as a design decision with rationale. No action needed — it explains why Vello was chosen.

### B6. `ext4` mentions — multiple files
- All ext4 references are correctly framed as "ext4 is insufficient, btrfs chosen instead." No stale references. No action needed.

### B7. `fuser` mention — agent-perspective.md line 61
- **Text:** "The FUSE layer used fuser."
- **Status:** Historical narration (part of "when I wrote the first version"). Correct. No action needed.

### B8. `pane-route` in AGENT.md line 42
- **Text:** "Architecture spec SS Servers: pane-route, pane-roster, pane-store, pane-fs"
- **Status:** AGENT.md is outside the spec corpus but is operationally important. pane-route has been eliminated. This list is stale.

### B9. Stale `pane-route-spec` change directory
- **Path:** `openspec/changes/pane-route-spec/`
- **Status:** Contains a full proposal, architecture, and tasks for a server that no longer exists. Should be archived.

---

## C. Missing Cross-References

### C1. Maty / event actor model — absent from all specs
- The Fowler/Hu Maty validation is in `maty-integration-plan.md` but has not been applied to any spec. The architecture spec's S7 and foundations S3 need the insertions described in Section A above.
- The architecture spec Sources section (line ~836) cites POPL 2019 Fowler but not OOPSLA Fowler/Hu (Maty). These are different papers.

### C2. Three-phase protocol — absent from architecture spec
- The handshake/active/teardown decomposition is fully designed in `maty-integration-plan.md` but the architecture spec S7 does not mention phasing. The active-phase typed-enum pattern is a significant departure from the "everything is session-typed" framing of the current S7 text. This needs the insertion from A4.

### C3. pane-comp spec does not reference pane-notify
- `pane-compositor/spec.md` does not mention pane-notify. But the architecture spec (S3 pane-comp) and the filesystem-config spec both describe the compositor receiving config change events via pane-notify/calloop. The compositor spec should reference pane-notify for config reactivity.

### C4. pane-comp spec does not reference pane-roster
- The compositor participates in the roster (it's a registered infrastructure server per architecture S3), but `pane-compositor/spec.md` never mentions pane-roster. It should reference roster registration and the heartbeat/watchdog relationship.

### C5. pane-fs spec does not reference compositional equivalence
- `pane-fs/spec.md` describes the per-pane filesystem tree but does not reference the compositional equivalence invariant from architecture S2. The architecture spec says composition structure appears as directory nesting under `/srv/pane/` — the pane-fs spec should explicitly commit to this.

### C6. Aesthetic spec does not reference the architecture spec's rendering split
- `aesthetic/spec.md` defines chrome vs. body rendering and names "pane-ui" but does not cross-reference `architecture/spec.md` S10 or `pane-compositor/spec.md` where the same split is described in more detail. The aesthetic spec's "Kit-mediated aesthetic enforcement" section says "The rendering split:" and defines chrome vs. body — this should note that the detailed rendering model is in the architecture spec and compositor spec.

### C7. Licensing spec does not reference pane-session
- `licensing/spec.md` lists MIT crates and AGPL crates. `pane-session` (the transport/session type crate) is absent from both lists. It is a library (should be MIT per the licensing boundary rule: "A kit is a library that lives inside the client process").

### C8. Workflow spec does not reference pane-session ergonomics review
- The workflow spec's "API stability protocol" (line ~159) mentions the Be Newsletter BMessage lesson but doesn't reference the ergonomics review's naming conventions (`finish` over `close_and_take`, intent over mechanism). This is a minor omission — the workflow spec is about process, not specific API decisions.

### C9. Foundations spec does not reference compositional equivalence
- The foundations spec S2 describes pane composition and optics over composition structure (lines ~35-37, ~88-91) but does not use the term "compositional equivalence" that the architecture spec S2 (line ~52) establishes. The architecture spec's formulation ("No composition primitive exists in one view without a representation in the others") is a concrete invariant that deserves naming in the foundations.

### C10. Plugin-discovery spec does not reference pane-fs
- Plugins are filesystem-based and discoverable via xattr queries, but the plugin-discovery spec does not reference the pane-fs filesystem interface. Plugin state could be inspectable at `/srv/pane/` — or explicitly excluded. The relationship is undefined.

---

## D. Terminology Drift

### D1. "Interface Kit" vs. "pane-ui"
- Both terms are used throughout the corpus. The architecture spec uses them interchangeably: "the Interface Kit (pane-ui)" at line ~657. This is intentional — "Interface Kit" is the BeOS-lineage name, "pane-ui" is the crate name. But some specs use one without the other:
  - `aesthetic/spec.md` line 22: "shared kit infrastructure (pane-ui)" — no "Interface Kit"
  - `foundations/spec.md` line 203: "Interface Kit" — no "pane-ui"
- **Recommendation:** Establish a convention: "Interface Kit" in conceptual/philosophical text, "pane-ui" in technical/implementation text. On first use in each spec, parenthetical: "the Interface Kit (pane-ui)".

### D2. "routing rules" location
- `architecture/spec.md` line 252: `/etc/pane/route/rules/` and `~/.config/pane/route/rules/`
- `plugin-discovery/spec.md` line 17: `~/.config/pane/route/rules/`
- These are consistent. No drift.

### D3. "chrome" scope
- `architecture/spec.md` line 44: "Borders, focus indicators, split handles"
- `architecture/spec.md` line 108: "tag lines, beveled borders, split handles, focus indicators"
- `pane-compositor/spec.md` line 21: "tag lines (with editable text, cursor, selection), beveled borders, split handles, focus indicators"
- The architecture S2 definition of chrome (line 44) omits tag lines. S3 and the compositor spec include them. Line 44 should include tag lines for consistency.

### D4. "tag line" vs. "tagline"
- All specs consistently use "tag line" (two words). No drift.

### D5. "looper" definition breadth
- `architecture/spec.md` S3 line 71: "threaded looper — a thread with a message queue"
- `architecture/spec.md` S4 line 247: "BLooper in Rust"
- `pane-notify/spec.md` line 4: "looper for looper-based servers, or through a channel for channel-based consumers"
- The term "looper" is used consistently. No drift.

### D6. "session type" vs. "typestate"
- Both are used. "Session type" is the theoretical concept; "typestate" is the Rust implementation pattern. The architecture spec line ~449 says "typestate Chan<S, Transport>". The Maty plan says "typestate Chan<S, UnixSocketTransport>". These are not conflicting — typestate is the mechanism, session type is the theory. But some contexts blur them:
  - Architecture S7 line 447: "Every interaction between components is a session — a typed conversation" (session type framing)
  - Architecture S7 line 449: "a typestate Chan<S, Transport>" (implementation framing)
  - The Maty plan A3 reveals that the active phase is NOT session-typed but uses typed enums. This creates a terminological problem: the architecture currently says "every interaction is a session" but the active phase is more accurately "a typed message dispatch loop."
- **Recommendation:** After inserting the protocol phasing text (A4), update the S7 intro to say "Every interaction between components is governed by typed protocols — session types for structured phases, typed message enums for bidirectional phases."

### D7. "compositor-rendered" vs. "GLES renderer" vs. "smithay's GLES renderer"
- The compositor renders chrome. The technology is stated differently:
  - `architecture/spec.md` line 44: "smithay's GLES renderer"
  - `pane-compositor/spec.md` line 22: "smithay's GLES renderer"
  - `aesthetic/spec.md`: does not name the compositor's rendering technology
- Consistent where stated. The aesthetic spec's omission is appropriate — it defines what, not how.

### D8. "kit-level routing" vs. "kit-level concern" vs. "pane-app kit evaluates routing rules"
- All mean the same thing. The wording varies but the concept is consistent: routing is in the kit library, not a server. No action needed.

### D9. "watchdog" naming
- Always "pane-watchdog." Consistent.

### D10. "pane-session" vs. "custom session type implementation"
- The crate `pane-session` is named in the Maty integration plan and ergonomics review but never in any spec. The architecture spec calls it "custom typestate Chan<S, Transport>" and "custom session type implementation." The specs should name the crate when discussing the implementation, at least in the technology choices table (line ~696).
