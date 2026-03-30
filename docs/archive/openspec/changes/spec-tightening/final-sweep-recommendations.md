# Final Sweep: Spec Corpus Readiness Assessment

A design sweep of the full spec corpus before implementation shifts from
foundations to Phase 3. Not a consistency audit. The question: can an
engineer build from this?

---

## Prioritized Recommendations

### 1. ADD: pane-app kit API sketch (Critical, architecture spec)

**The gap.** Phase 3 builds pane-app. The architecture spec describes what
the kit does (looper, handler chain, routing, connection management) but
not what its API looks like. An implementer hits questions immediately:

- What does creating a looper look like? `PaneApp::new()` that spawns
  threads, or a builder?
- How does a handler declare what messages it handles? Trait impl?
  Registration? Pattern match?
- How does the handler chain work? BHandler had `SetNextHandler()` -- is
  pane's equivalent explicit chaining, or trait-based dispatch?
- How does a client create a pane? What's the minimal "hello world"?
- What does `MessageReceived()` look like in Rust? A trait method? A
  closure? An enum match?

The architecture spec says "analogous to the Application Kit" but the Be
API was concrete -- `BApplication`, `BWindow`, `BHandler::MessageReceived()`.
Pane's equivalent needs at least a conceptual API sketch: the 10-line
"hello pane" that shows the programming model. Without this, Phase 3 is
an API design project masquerading as an implementation task.

**Action.** Add a subsection to architecture spec 4 (Kit Decomposition)
under pane-app with a conceptual API sketch. Not final types -- a shape.
Show the looper creation, handler registration, message dispatch, and
pane creation patterns. The Schillings standard: "common things are easy
to implement and the programming model is CLEAR."


### 2. ADD: Routing rule format (Critical, architecture spec or pane-app)

**The gap.** Routing is "built into the kit" and rules are "one file per
rule" in well-known directories. But nowhere does the spec corpus define
what a routing rule looks like. This is a Phase 3 deliverable. An
implementer needs to know:

- What's the file format? A DSL? TOML? An executable?
- What fields does a rule have? (Pattern, action, target, quality, priority?)
- How is the match language structured? Glob on content type? Regex on tag
  text? Both?
- How does transformation work? ("Extract filename" is mentioned but
  never defined.)
- What's the evaluation order? First match? All matches with disambiguation?

The BeOS equivalent was the file type system -- MIME types as attributes,
`BRoster::Launch()` dispatching by type. It was concrete. The Translation
Kit had `BTranslatorRoster` with a defined add-on interface. Pane's
routing is more ambitious but less defined.

**Action.** Define the routing rule file format. Even a strawman that can
evolve is better than nothing. The implementer needs to know the shape of
the data before building the evaluator.


### 3. TIGHTEN: pane-notify event delivery contract (High, pane-notify spec)

**The gap.** pane-notify is Phase 3's first deliverable and its spec is
the most concrete of the component specs. But there's an ambiguity in
event delivery that will bite during implementation: the spec says events
are delivered "as messages to the consumer's looper or channel" -- but
pane-app (which defines the looper) is built on top of pane-notify. The
bootstrap is circular:

- pane-notify delivers events to loopers
- Loopers are defined in pane-app
- pane-app depends on pane-notify for config reactivity and plugin watching

The compositor exception (calloop event source) is specified. What about
pane-store, pane-roster, and other servers that use loopers but aren't
pane-app clients? Do they use the same looper abstraction? Does pane-notify
have its own minimal event delivery that pane-app wraps?

**Action.** Clarify the layering. pane-notify provides raw events via
channels (its own primitive). pane-app wraps this into looper-integrated
delivery. Servers that aren't pane-app clients use channels directly.
State this explicitly.


### 4. CONSOLIDATE: Three-tier access model (Moderate, architecture + pane-comp + pane-fs)

**The redundancy.** The three-tier access table appears identically in:
- Architecture spec section 3 (pane-fs description)
- pane-comp spec section 7
- pane-fs spec (requirement: filesystem tier)

Three copies, identical latency numbers, identical use-case descriptions.
If these numbers change (io_uring could shift the FUSE tier significantly),
three places need updating. This is a maintenance drift magnet.

**Action.** Define the three-tier model once in the architecture spec.
pane-comp and pane-fs specs reference it. Currently the pane-comp and
pane-fs specs already cite the architecture spec for this; they just also
reproduce the table. Remove the reproductions, keep the references.


### 5. CLARIFY: Scripting protocol concrete interface (High, architecture spec)

**The gap.** Architecture spec section 5 describes the scripting protocol's
theory (session types + optics = scripting) and the "hard problem" (dynamic
optic composition). The open questions section flags this for prototyping.
But there's no concrete interface sketch -- no equivalent of BeOS's
`property_info` struct, no equivalent of the `hey` command syntax, no
definition of what `GetSupportedSuites()` returns in pane's typed world.

This isn't needed for Phase 3 (it's a Phase 6+ concern), but it's the
feature that the foundations spec calls "one of BeOS's most important
features." The spec should at least sketch:

- What a handler exposes (a trait? a declarative structure?)
- What a scripting query looks like over the protocol
- What the `hey` equivalent would feel like from the command line

**Action.** Add a concrete sketch to section 5, clearly marked as
provisional. Even "the query language looks like `pane get title of
pane 3`" grounds the discussion. The hard problem (dynamic composition)
stays in open questions; the surface-level interface does not.


### 6. REMOVE: Overspecified embedding details in pane-ai (Moderate, architecture spec)

**The overspecification.** The pane-ai section specifies: HNSW indexing,
1024-dimension embeddings, reciprocal rank fusion, three fusion modes,
threshold-based live queries, embedding xattr header format with model
hash and dimensionality. This is Phase 8 material specified to
implementation-level detail.

Meanwhile, pane-app (Phase 3) has no API sketch.

The memory consolidation protocol (journal -> atomic facts -> deduplication
at write time) is research-grade design for a system that doesn't have a
working compositor yet. MemX (Sun 2026) is cited for fact-level
granularity -- this is an implementation detail that should be validated
during prototyping, not locked into the spec.

**Action.** Reduce pane-ai to: agents are users, .plan governs behavior,
Landlock enforces it, communication uses Unix patterns, local models are
first-class, routing rules govern local/remote dispatch. Move embedding
details, memory consolidation, and retrieval pipeline to a design document
that pane-ai implementation will reference but isn't spec. The vector
similarity search belongs in the pane-store spec as a capability, not in
pane-ai as a dependency.

(The vector similarity work in pane-store is sound and worth keeping
where it is. The issue is pane-ai specifying the consumer side to a
degree that forecloses design choices during implementation.)


### 7. ADD: Error recovery scenarios (High, architecture spec)

**The gap.** The error composition section (architecture 7) describes the
theory well -- monadic errors, typed crash events, recovery combinators.
But Phase 3 builds the first multi-process system (pane-notify talking to
pane-app clients talking to pane-comp). An implementer needs scenarios:

- pane-comp crashes while pane-app client is mid-message. What happens?
  (The session type returns Err(Disconnected) -- then what? Does the kit
  auto-reconnect? Does the app see an error? Both?)
- pane-store is slow to start. A pane-app kit tries to query it during
  startup. Does the query block? Fail? Queue?
- A looper's handler panics. Does the whole application die? Does the
  looper catch it and continue?

The spec says "retry, fallback, degrade gracefully" as composable
strategies but doesn't show what the default strategy is for each
server interaction.

**Action.** Add 3-4 concrete error recovery scenarios to architecture
spec section 7. Phase 3 implementers need to know the default failure
mode for each inter-server connection, not just that failures are
composable values.


### 8. TIGHTEN: Phase 3 agent infrastructure scope (High, architecture spec)

**The gap.** Phase 3 item 6 says: "Minimal agent infrastructure -- agent
user accounts, .plan file convention, message passing over the pane
protocol, Unix communication patterns (write/mail/mesg). Not the full
AI Kit -- just enough for agents to participate as system users."

This is simultaneously too ambitious and too vague. "Just enough" for what
exactly?

- Does Phase 3 need actual `write`/`mail`/`mesg` implementations, or
  placeholder scripts?
- Does the .plan file need a parser, or is it a convention doc only?
- Does "message passing over the pane protocol" mean agents connect to
  pane-comp via session types? That requires pane-app to be working first.
- What's the acceptance criterion? "An agent can run cargo test and mail
  results" requires mail infrastructure that doesn't exist.

The development methodology and agent-perspective docs paint a vivid
picture of what agent habitation looks like, but the Phase 3 scope is
undefined. Risk: Phase 3 balloons into building agent infrastructure
instead of the desktop.

**Action.** Define Phase 3 agent scope as: (1) agent user accounts exist
in NixOS config, (2) .plan file format is documented but enforcement is
manual, (3) agents communicate via filesystem artifacts (not the pane
protocol yet), (4) a CI agent runs tests via a cron job or pane-notify
watcher. That's "minimal" in a way that doesn't block on pane-app
completion. Move the pane-protocol-connected agent to Phase 6 alongside
pane-roster.


### 9. CONSOLIDATE: Kit-mediated aesthetic enforcement (Low, aesthetic + architecture)

**The redundancy.** The rendering split (compositor renders chrome, client
renders body, kit provides consistency) is described in:
- Aesthetic spec (requirement: kit-mediated aesthetic enforcement)
- Architecture spec section 10 (client-side rendering)
- pane-comp spec (sections 1 and 9)

The aesthetic spec's version is the normative statement. The architecture
and pane-comp specs elaborate on implementation. Currently all three could
drift independently.

**Action.** The aesthetic spec defines what. Architecture spec section 10
and pane-comp spec describe how. Add explicit cross-references: "The
rendering split is defined in the aesthetic spec; this section describes
its implementation." Minor effort, prevents future confusion.


### 10. ADD: pane-fs event stream format (Moderate, pane-fs spec)

**The gap.** pane-fs exposes `/srv/pane/<id>/event` as a "JSONL, read-only,
blocking" stream. But what events? What fields? What's the schema?

An implementer building the pane-fs event endpoint needs to know:
- What events are emitted (tag change, focus, resize, attribute change,
  composition change?)
- What each event's JSON shape looks like
- Whether events are ordered, and what ordering guarantees exist
- Whether there's backpressure or events are dropped on slow readers

This is a Phase 7 concern (pane-fs is Phase 7) but the event stream is
part of the compositional equivalence commitment -- events visible through
the protocol must be visible through the filesystem. The schema should be
sketched.

**Action.** Add a provisional event schema to the pane-fs spec. Even
`{"type": "tag_changed", "pane": 3, "value": "new tag text"}` is enough
to ground the interface.


### 11. CLARIFY: Compositor tag line editing model (Moderate, pane-comp spec)

**The gap.** The tag line is "editable text that serves as title, command
bar, and menu simultaneously." The compositor renders it. But:

- Who handles text editing state (cursor position, selection, undo)?
  The compositor? The client via protocol messages?
- How does the client declare actionable text in the tag line? Markup?
  Separate protocol messages?
- When the user types in the tag line, do keystrokes go to the
  compositor's tag editor or to the client? Who owns the text input
  focus?
- What happens when a client wants dynamic tag content (e.g., showing
  git branch in a shell pane's tag)?

The acme comparison is apt but acme's tag was rendered by the same
process that owned the body. In pane, the tag is compositor-rendered
but client-specified. The handoff protocol matters.

**Action.** Define the tag line ownership model in the pane-comp spec:
who owns editing state, what the protocol messages look like for tag
content updates, and how input focus works between tag and body. This
is Phase 4 critical path.


### 12. TIGHTEN: Dependency on bcachefs (Low, architecture spec)

**The gap.** Architecture spec section 1 mentions "bcachefs when it
matures" alongside btrfs. Section 3 commits to btrfs exclusively.
The dependency philosophy says "bcachefs when it matures" but the
xattr requirements (~16KB per value) are validated only for btrfs.

**Action.** Remove the bcachefs reference. The spec already commits to
btrfs. Mentioning bcachefs as a future possibility creates ambiguity
about whether the xattr assumptions are portable. If bcachefs becomes
relevant, that's a future change proposal.


### 13. ADD: Accessibility architecture (Moderate, architecture spec)

**The gap.** The foundations spec mentions "semantic: roles, values, and
actions for accessibility infrastructure" as a pane view. Architecture
spec section 2 lists it as one of the four views. pane-ui mentions
"the accessibility tree is a byproduct of the widget model." But there's
no accessibility architecture:

- What protocol does pane use for accessibility? AT-SPI over D-Bus
  (the Linux standard)?
- How does compositor-rendered chrome (tag lines) participate in the
  accessibility tree?
- Is the semantic view a fifth-class citizen or does it have the same
  priority as the other three views?

This doesn't block Phase 3 but should be addressed before Phase 7
(widget rendering), because the widget model's semantic structure is
the foundation of the accessibility tree.

**Action.** Add a paragraph to architecture spec section 2 acknowledging
that accessibility maps to AT-SPI/D-Bus and will be specified when
pane-ui's widget model is built. This prevents the accessibility tree
from being an afterthought bolted on in Phase 8.


### 14. CLARIFY: Nix activation script vs. pane-notify (Low, filesystem-config spec)

**The gap.** The filesystem-config spec describes `pane-rebuild switch`
reconciliation (Nix defaults vs user overrides). Architecture spec says
servers watch `/etc/pane/<server>/` via pane-notify for live changes.
The interaction between these two mechanisms is unspecified:

- Does `pane-rebuild switch` write files, which pane-notify picks up,
  which servers react to? Or does the activation script signal servers
  directly?
- If Nix adds a new config key, does the server learn about it via
  pane-notify or does it need a restart?

**Action.** Add a sentence to filesystem-config spec: "The activation
script writes files to /etc/pane/; servers detect changes via their
existing pane-notify watches. No direct signaling is needed." This is
almost certainly the intended design; state it.

---

## Overall Assessment: Is This Ready to Build From?

**Yes, with caveats.**

The spec corpus is remarkably coherent for its scope. The foundations
spec is one of the best design philosophy documents I've read for any
OS project -- it captures *why*, not just *what*. The architecture spec
has grown to cover the full system with genuine depth. The component
specs are concrete where they need to be.

**What's strong:**
- The theoretical framework (session types + optics + monadic errors)
  is grounded in real engineering decisions, not abstract waving.
- The Phase 1-2 work (protocol foundation, transport bridge) has been
  validated by implementation. The spec-to-code pipeline works.
- The composition model section is the best part of the architecture
  spec. It shows exactly how infrastructure-first design produces
  emergent features. An implementer reading it understands the *point*.
- The newsletter wisdom document is load-bearing -- it connects spec
  decisions to engineering experience.

**What needs work before Phase 3 starts:**
- Items 1 and 2 above (pane-app API sketch, routing rule format) are
  genuine blockers. Phase 3 cannot start without knowing what the kit
  API looks like.
- Item 3 (pane-notify event delivery layering) needs clarity to avoid
  a circular dependency during implementation.
- Item 8 (agent scope) needs tightening or Phase 3 will expand into
  building agent infrastructure at the cost of the desktop.

**Three risks most likely to bite in Phase 3-5:**

1. **The pane-app kit API is the hardest design problem in the project.**
   Harder than session types, harder than the compositor. The kit is
   the programming model -- it's what developers touch. Getting it wrong
   means getting pane wrong. And it's the one piece the spec hasn't
   specified to a usable level. The newsletter wisdom is right: "common
   things are easy to implement and the programming model is CLEAR."
   The Phase 3 risk is that the kit API emerges from implementation
   expedience rather than design intention.

2. **The tag line editing model crosses the compositor/client boundary
   in ways the spec hasn't resolved.** The tag line is the most
   distinctive UI element and the most architecturally complex: it's
   compositor-rendered, client-specified, user-editable, and it doubles
   as a command bar. Every one of those properties involves a different
   protocol concern. Underspecifying this means Phase 4 becomes a
   design project.

3. **Agent infrastructure scope creep.** The development methodology and
   agent-perspective documents are compelling and detailed. The risk is
   that they create a gravity well that pulls implementation effort
   toward agent infrastructure before the desktop works. The desktop
   is the product. Agents are inhabitants. The inhabitants need a house
   before they can move in.

**The Be standard.** Could a competent Rust developer read the
architecture spec and know how to build a pane-native application?
Not yet. They'd understand the *system* -- how servers decompose, how
protocols work, how composition happens. But they wouldn't know what
code to write. The gap is the gap between understanding the philosophy
and knowing the API. The spec needs to close that gap for pane-app
before Phase 3 begins.

The spec needs a "hello pane" example -- the 10-20 lines that create
an application, register a handler, open a pane, and respond to a
message. That example is the spec's final exam. If it's clear, the
programming model is clear. If it's not, no amount of philosophy will
compensate.
