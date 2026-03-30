# Architecture Spec Review — Be Systems Engineer

A thorough review of `openspec/specs/architecture/spec.md` against the foundations spec, verified against the Haiku source (`~/src/haiku`) and the Be Newsletter archive (`~/src/haiku-website`).

The architecture spec captures many of the right ideas. But as the concrete realization of the foundations spec, it has significant gaps, stale artifacts, and places where it drifts from the principles it claims to implement. This review is organized to guide a wholesale rewrite.

---

## 1. Section-by-Section Assessment

### 1.1 Vision

**Alignment:** Mostly strong, but drifts in framing. The Vision section says pane is "analogous to what Apple did with Mac OS X over Unix in the 2000s, but grounded in BeOS's design convictions rather than NeXT's." The foundations spec (section 1) says pane is "closer to what NeXTSTEP was to Mach/BSD" and explicitly names NeXTSTEP's integration depth as the aspiration. These two framings are in tension. Apple's Mac OS X was a pragmatic compromise (Carbon/Cocoa, the Finder debacle, years of transition). NeXTSTEP was the opinionated thing — the one where the developer experience *was* the platform. The foundations spec wants the NeXTSTEP relationship to the base system. The architecture spec invokes the Mac OS X comparison, which is precisely the compromised version of that relationship.

**BeOS fidelity:** The Gassee quote about rendering power vs. expressive power is used correctly. The characterization of BMessage/BLooper is accurate. The one inaccuracy: "No global coordinator was needed because the protocol was the coordination" — this is true of app-level coordination, but the app_server itself *was* a coordinator for all rendering. The compositor fills the same role. This is fine, but the framing should acknowledge that "no global coordinator" means "no coordinator for message routing," not "no central services at all."

**Recommendation:** Rewrite the Vision to use the NeXTSTEP framing consistently. Drop the Mac OS X comparison. Keep the Gassee rendering/expressive power distinction. Add the foundations spec's "democratic orientation" language, which is entirely absent from the architecture spec's Vision.

### 1.2 The Pane Primitive

**Alignment:** Good but incomplete. The four views (visual, protocol, filesystem, semantic) are faithful to foundations section 2. The tag line / body / protocol connection decomposition is concrete and useful.

**What's missing:** The foundations spec says "Panes compose. Two panes viewed together form a compound structure — tensored up to a notion of abstraction whose concrete presentation depends on observational context." The architecture spec says nothing about pane composition. This is a significant gap. How do panes compose structurally? What's the monoidal product concretely? This matters for layout, for scripting (addressing "the third pane in the left split"), and for the "pane as universal object" commitment.

**What's missing:** The optics commitment from foundations section 4 is entirely absent from the Pane Primitive section. The foundations spec says the relationship between internal state and each external view "is governed by optics — composable, bidirectional access paths." The architecture spec mentions views but never mentions optics. The Pane Primitive section should state how optic-governed views work for this object.

**What's missing:** Legacy application wrapping is described but the trust/seam model from foundations section 2 isn't addressed. The foundations spec says "for host-level tools, the file is primary and the pane is a view; for pane-native participants, the pane is primary and the file is a projection. The mutable specs must establish conventions for which direction governs in each context." The architecture spec doesn't resolve this.

**Recommendation:** Add pane composition semantics. State how optics govern the relationship between internal state and each projection. Resolve the file-vs-pane primary direction question for native and legacy contexts.

### 1.3 Target Platform

**Alignment:** Fine. Concrete and appropriate.

**One concern:** The init system abstraction (pane-init) is described well here but the architecture spec doesn't discuss startup ordering. Haiku's launch_daemon solved this elegantly: pre-create communication ports before starting any services, so messages queue before the target starts. (Source: Axel Dorfler's launch_daemon design — see `~/src/haiku/src/servers/launch/` and the Haiku API docs at `haiku-os.org/docs/api/launch_intro.html`.) The architecture spec should address startup ordering, especially since the router elimination means kit-level routing needs to know where servers are before they're fully alive.

**Recommendation:** Add a startup/boot sequence section that addresses service discovery during startup. Consider whether pane-roster needs to be available before other servers, and what happens when a kit tries to route to a server that hasn't registered yet.

### 1.4 Design Pillars

**1.4.1 Text as Action**

Good. Concrete and specific.

**1.4.2 Visual Consistency Through Shared Infrastructure**

**Alignment:** Faithful to foundations section 9 (aesthetic commitment). The comparison to BeOS's Interface Kit is accurate — app_server did the actual rendering, but the consistency came from everyone using the same kit. The architecture spec correctly maps this to Wayland's client-side rendering model.

**BeOS fidelity:** Accurate. In BeOS, BView::Draw() was called on the client side, but the drawing commands were buffered and sent to app_server for execution. This is confirmed in the Haiku source: `src/servers/app/ServerWindow.cpp` handles the drawing command stream, and `src/kits/interface/View.cpp` builds and sends the commands. The architecture spec's "kits provide consistency, compositor does chrome" is the right translation.

**1.4.3 Modular Composition**

**Stale artifact:** This section says "Developers write against a clean Rust API; the system manages the threading, the message passing, and the protocol safety behind that API." This is correct, but it doesn't mention session types — which the foundations spec makes central. The architecture spec treats session types as a separate topic (Design Pillar 4) rather than integrating them into the composition model from the start.

**1.4.4 Session-Typed Protocols**

**Alignment:** Mostly strong. The BMessage/BLooper comparison is accurate and well-stated.

**Stale artifact:** References the `par` crate by name. The foundations spec doesn't name specific crates. The architecture spec should describe the properties it needs from a session type library rather than committing to a specific implementation.

**What's missing:** The foundations spec (section 3) says "Asynchronous messages can be batched; synchronous interactions force a flush." The architecture spec mentions this for drawing commands but doesn't establish it as a general principle of the protocol. This was one of the most important performance lessons from BeOS — George Hoffman's newsletter article (Issue 2-36) documents how sync calls were "much slower than asynchronous calls" because they flushed the async buffer. The architecture spec's protocol section should establish batching as a first-class protocol concern, not just a drawing optimization.

**1.4.5 Semantic Interfaces**

Good. The four-level view (human, application, compositor, debugger) is well-articulated.

**1.4.6 Filesystem as Interface**

**Alignment:** Faithful to the BFS spirit, but the architecture spec undersells it. The foundations spec (section 7) describes the BeOS email composition in detail — the filesystem as a *queryable structured data layer*, not just "state and configuration as filesystem primitives." The architecture spec reduces the filesystem to configuration and plugin discovery.

**What's missing:** The attribute indexing / query model that made BeOS's email composition possible. pane-store provides this, but the Design Pillar for "Filesystem as Interface" should establish the *principle* that the filesystem is a database, not just storage. The current text focuses on config files and FUSE, missing the deeper point.

**BeOS fidelity:** BFS's three always-indexed attributes (name, size, last_modified) meant basic queries always worked. The architecture spec's pane-store section mentions indexing but doesn't establish what's always-indexed. The architecture should specify a set of "free attributes" that every pane always has — type, identity, creation time — analogous to BFS's defaults.

**1.4.7 Composable Extension**

**Alignment:** The four extension types (routing rules, translators, pane modes, protocol bridges) are well-articulated.

**BeOS fidelity:** The Translation Kit description is accurate. The quality/capability self-rating mechanism (described in `~/src/haiku/headers/os/translation/TranslationDefs.h`, the `translation_format` struct with `quality` and `capability` floats) is a useful pattern that the architecture spec mentions for routing but should formalize as a general multi-match resolution strategy.

**1.4.8 Developer Experience as Design Philosophy**

Good. The NeXTSTEP connection is correct and the two-world problem warning is important.

### 1.5 Servers

**Stale artifacts throughout:** Multiple references to `pane-route` as a server that no longer exists. The section text says "Each server runs its own threaded looper" and lists the servers, but the routing section explicitly says routing is a kit concern, not a server. The tension is resolved in the Routing subsection, but the framing is confusing — routing gets a dedicated subsection under "Servers" even though it's *not* a server. Move it.

#### pane-comp

**Alignment:** Solid. Responsibilities are clear and appropriately scoped.

**What's missing:** The foundations spec says "the compositor provides uniform chrome" (section 9). The architecture spec says pane-comp renders "borders, tag lines, split lines, focus indicators." But how does the compositor know what to put in the tag line? The tag line is "editable text that serves as title, command bar, and menu simultaneously" — it's content-bearing, not just decoration. The protocol for tag line content between client and compositor needs to be specified.

**What's missing:** Input handling is described as "in-process, not a separate server — latency-critical" which is the right call, but the foundations spec's Input Kit (section 10: "The Input Kit generalizes the keybinding power of the best text editors across every interface") isn't reflected in the architecture spec at all. Where does keybinding resolution happen? Is it compositor-side, kit-side, or both? BeOS handled this in the app_server (input_server, actually — a separate server: `src/servers/input/`). The architecture spec collapses this into the compositor without acknowledging the design tradeoff.

**BeOS fidelity:** In BeOS, input_server was a separate server that processed raw input events, applied input method transforms, and dispatched to the active app via app_server. See `~/src/haiku/src/servers/input/InputServer.h`. This separation meant input method add-ons (IME, keyboard remapping) were loaded into input_server, not app_server. The architecture spec puts input handling in the compositor, which is simpler but loses the ability to load input add-ons in an isolated process. This tradeoff should be acknowledged and the decision justified.

#### Routing

**Alignment with foundations:** The kit-level routing is faithful to foundations section 6: "Routing is a kit-level concern — no central service whose failure breaks all communication." The elimination of the central router is correct.

**Stale artifact:** The newsletter-wisdom document still references "pane-route" throughout as "the expressive power layer." The architecture spec's Routing section is internally consistent about routing being a kit concern, but the framing as a sub-section of "Servers" creates confusion.

**Recommendation:** Move routing out of the Servers section entirely. It belongs in the Kits section as a core capability of pane-app.

#### pane-watchdog

Good. Appropriately minimal. Faithful to the Erlang `heart` model.

#### pane-roster

**Alignment:** Good. The three-role decomposition (service directory, app lifecycle, service registry) is well-articulated.

**BeOS fidelity:** The description maps well to BRoster's actual behavior (confirmed in `~/src/haiku/headers/os/app/Roster.h`): application tracking, launching (with single-launch/exclusive-launch semantics), discovery (FindApp), and monitoring (StartWatching). The architecture spec adds service registry, which BRoster didn't have — BRoster's application discovery was MIME-type-based, not service-registry-based. This is a valid extension for pane's needs, but the architecture spec should note the departure.

**What's missing:** BRoster had `Launch()` with launch semantics (B_SINGLE_LAUNCH, B_EXCLUSIVE_LAUNCH, B_MULTIPLE_LAUNCH). The architecture spec says roster "facilitates launching desktop apps" but doesn't specify launch semantics. Single-instance vs. multi-instance behavior is important for the UX and should be specified.

**What's missing:** BRoster tracked recent documents, folders, and applications. This is a small thing, but it contributed to the integrated feel. Should pane-roster maintain recent history? If so, how does it compose with pane-store's attribute indexing?

#### pane-store

**Alignment:** Faithful to BFS. The "rebuilt from xattr scan on startup, like BFS" is exactly right.

**BeOS fidelity:** Accurate. BFS maintained B+ tree indices for indexed attributes, and BQuery required at least one indexed attribute per query. The architecture spec's pane-store mirrors this. One concern: the architecture spec says "live query maintenance" is "a client-side composition of index reads + change notification subscriptions." This is correct in spirit — BeOS's live queries were indeed a composition of initial query results plus kernel-delivered change notifications. But in BFS, the *kernel* evaluated whether a file change affected a live query and sent the notification. In pane's architecture, who evaluates whether a pane-store change event matches a client's query? If it's the client, that's more work on the client side than BFS required. If it's pane-store, it's doing live query maintenance after all. This needs to be clarified.

#### pane-fs

Good. The Plan 9 inspiration is well-articulated.

### 1.6 Shared Infrastructure

**pane-notify:** Well-specified. The fanotify/inotify split is the right choice for Linux.

**Filesystem-Based Configuration:** Good. The one-file-per-key model with xattr metadata is clean.

**Filesystem-Based Plugin Discovery:** Good but incomplete. The architecture spec lists three plugin directories (translators, input, route rules). The foundations spec's kit ecosystem implies more. Where do pane modes live? Where do protocol bridges live? Where do aesthetic customizations (the filesystem-as-config properties from section 9) live?

### 1.7 How Composition Works

**Alignment:** This section is one of the architecture spec's strongest. The email case study is well-told and accurate.

**Stale artifact:** The routing description says "the kit evaluates routing rules locally." This is consistent with the router elimination. But the paragraph about attribute indexing says "A client that subscribes to change notifications and maintains a query result set has a live query — without pane-store implementing 'live queries' as a feature." As noted above, this needs more precision about who evaluates query membership on change events.

### 1.8 Kits

**Alignment:** The introductory text is strong. "Kits are not wrappers over protocols — they ARE the programming model" is exactly the foundations spec's position.

**The critical gap:** The foundations spec (section 10) names specific kits: "AI Kit, Media Kit, Input Kit." The architecture spec lists a different set: pane-proto, pane-app, pane-ui, pane-text, pane-store-client, pane-notify, pane-ai. There is no Media Kit, no Input Kit. There is a pane-text that BeOS never had as a separate kit (text handling was part of the Interface Kit). The kit decomposition needs careful review.

Detailed kit assessment below (section 6).

### 1.9 Composition Model

**Alignment:** The four composition levels (session types, Rust idioms, reactive signals, threaded loopers) are a useful taxonomy. But this section reads more like an implementation guide than an architecture spec. The foundations spec's composition model is philosophical — infrastructure-first, emergent behavior. The architecture spec's composition model is about Rust APIs.

**Recommendation:** Lead with the architectural composition principles (how servers compose, how kits compose, how extensions compose) and follow with the implementation mechanisms.

### 1.10 Pane Protocol

**Alignment:** The session types section is solid. The async/sync distinction in the protocol is one of the most important lessons from BeOS and is well-articulated here.

**What's missing:** The foundations spec (section 4) commits to "session types + optics = the scripting protocol." The Pane Protocol section says nothing about scripting. This is the single biggest gap in the architecture spec.

### 1.11 Resilience

**Alignment:** Faithful to foundations section 6 (monadic error composition). The three resilience guarantees (session boundaries, server crashes, failure isolation) are correct.

**What's missing:** The foundations spec says "Recovery is structured" and lists four specific mechanisms: roster tracking, compositor handling client disappearance, kit-level routing (no central failure point), and watchdog. The architecture spec's resilience section is more general. It should be specific about the recovery protocol at each failure point.

**What's missing:** The "affine/linear gap" is flagged as an open design question. This is a real issue. In Rust, dropping a channel endpoint is safe (affine) but doesn't fulfill the linear protocol obligation. The architecture spec should at least outline the strategies: catch_unwind at session boundaries, structured error types in the protocol, graceful degradation when a peer disappears. The foundations spec calls this "monadic error composition" — errors as values that compose through typed channels. The architecture spec should describe how this works concretely.

### 1.12 Client Classes

Good. The two-world problem is honestly stated and the mitigation strategies are reasonable.

### 1.13 pane-shell

Good. The pane-shell-lib as a composable library is the right approach. The comparison to emacs modes is apt.

### 1.14 Layout

Fine. Tree-based tiling with tag-based visibility is a reasonable choice.

### 1.15 Aesthetic

**Alignment:** Faithful to foundations section 9. The Frutiger Aero reference is specific and useful. "One opinionated look" is the right call.

**What's missing:** The foundations spec says "The extension model celebrates low-friction composition (drop a file, gain a behavior), but aesthetic customization demands higher friction (rebuild through the kit). The mutable specs must find where the stock aesthetic retains its identity while customization remains accessible." The architecture spec says "Individual properties configurable via filesystem-as-config (accent color, font size) but not wholesale theme replacement." This is a start but doesn't resolve the tension. What exactly is configurable? Just accent color and font size, or all the visual properties (gradient intensity, corner radius, translucency level)? The architecture spec should specify the aesthetic configuration surface.

### 1.16 Accessibility

Appropriately modest. Cell grid accessibility as a research problem is honest.

### 1.17 Technology

Good. Specific and justifiable choices.

**Concern:** The `par` crate is listed but the foundations spec doesn't commit to a specific crate. The architecture spec should describe the required properties of the session type library (deadlock-free by construction, compatible with per-thread blocking, composable with Rust's ownership model) and note par as the current candidate.

### 1.18 Build Sequence

Reasonable. The phase ordering makes sense.

---

## 2. What's Missing: Commitments from the Foundations Spec

### 2.1 The Scripting Protocol (CRITICAL)

The foundations spec (section 4) makes the most ambitious commitment in the entire document: "Session types + optics = the scripting protocol." It describes the BeOS scripting protocol in detail — `ResolveSpecifier()`, `GetSupportedSuites()`, the `hey` command-line tool — and commits to recovering it with stronger guarantees.

The architecture spec says *nothing* about the scripting protocol. This is the single largest gap.

What BeOS actually did, verified against the Haiku source:

1. **Every BHandler implements `ResolveSpecifier()`** (`~/src/haiku/headers/os/app/Handler.h`, line 58). This method takes a scripting message with a stack of specifiers and resolves one level: it either returns `this` (meaning "I handle this property") or returns another handler (meaning "this handler is closer to the target").

2. **The resolution loop** is in `BLooper::resolve_specifier()` (`~/src/haiku/src/kits/app/Looper.cpp`, line 1428). It iterates: call `ResolveSpecifier()` on the current target, get a new target, repeat until the target stops changing or the specifier stack is exhausted. This is the "each handler peeling off one specifier" pattern the foundations spec describes.

3. **The specifier types** are defined in `BMessage.h` (line 42-49): `B_DIRECT_SPECIFIER`, `B_INDEX_SPECIFIER`, `B_NAME_SPECIFIER`, `B_ID_SPECIFIER`, `B_REVERSE_INDEX_SPECIFIER`, `B_RANGE_SPECIFIER`. These are the *optics* of the system — different ways to address into a handler's state: by identity, by index, by name, by range.

4. **The commands** are in `AppDefs.h` (line 97-102): `B_GET_PROPERTY`, `B_SET_PROPERTY`, `B_CREATE_PROPERTY`, `B_DELETE_PROPERTY`, `B_COUNT_PROPERTIES`, `B_EXECUTE_PROPERTY`. These form a CRUD-like protocol that every handler supports.

5. **`property_info`** (`~/src/haiku/headers/os/app/PropertyInfo.h`) is the metadata type: each handler declares what properties it supports, what commands apply to each property, and what specifier types are valid. This is the discoverability mechanism — `GetSupportedSuites()` returns this.

6. **The `hey` tool** (`~/src/haiku/src/bin/hey.cpp`) is the CLI that exercises this protocol. It parses commands like `hey Tracker get Frame of Window 0` into a scripting BMessage with specifier stack, sends it to the target via BMessenger, and prints the reply.

The foundations spec's insight: the specifier types (B_INDEX_SPECIFIER, B_NAME_SPECIFIER, etc.) are *optics in disguise*. An index specifier is a lens into a collection by position. A name specifier is a lens into a collection by key. A range specifier is a traversal. The `ResolveSpecifier` chain is optic composition at runtime.

The architecture spec needs a section that:

1. **Defines the scripting protocol** — what messages a pane must handle, what properties it must expose, how specifier resolution works.
2. **Maps specifiers to optics** — formally connects the BeOS specifier types to the optics framework the foundations spec commits to.
3. **Defines discoverability** — the equivalent of `GetSupportedSuites()`, how a script or agent discovers what a pane exposes.
4. **Addresses the static/dynamic tension** — the foundations spec acknowledges this: "Optics are typically static. How optic-addressed access composes across handler boundaries in a running system — where the structure is only known at runtime — is the hardest design problem." The architecture spec must at least frame the approach.
5. **Specifies the CLI tool** — pane needs a `hey` equivalent. This is how agents and scripts interact with running applications. It should be specified at the architecture level because it exercises the fundamental protocol.

### 2.2 Optics (SIGNIFICANT)

The foundations spec dedicates an entire section (4) to optics as the mechanism for multi-view consistency. The architecture spec mentions views (visual, protocol, filesystem, semantic) but never connects them to optics. The lens laws (GetPut, PutGet) are absent. The composition of optics (internal state -> protocol -> filesystem = composite optic) is absent.

The architecture spec should:

1. State that each pane's four views are optic projections of internal state.
2. Establish lens laws as correctness criteria for view implementations.
3. Describe how optic composition gives the filesystem view "for free" once the internal-state-to-protocol and protocol-to-filesystem optics are defined.

### 2.3 Monadic Error Composition (MODERATE)

The foundations spec (section 6) says "Failures are values, not exceptions. Error handling composes through the same typed channels that handle the happy path." The architecture spec's Resilience section describes failure isolation but doesn't describe the *composition* model for errors. How do errors propagate through the session type protocol? What does "monadic" mean concretely in this context? The architecture spec should show the error propagation path: component crash -> session terminated event -> handler receives typed error -> recovery strategy applied compositionally.

### 2.4 The Democratic Orientation (MODERATE)

The foundations spec (section 1) makes the "democratic orientation" a core principle: "the best user experiences are not generated by imposing a particular view of what computing should be, but by providing a powerful and flexible foundation." The architecture spec's Vision section doesn't mention this. The "one opinionated look" in the Aesthetic section partially contradicts it (though the foundations spec addresses this tension explicitly). The architecture spec should articulate how the system provides both a compelling stock experience and the infrastructure for users to build alternatives.

### 2.5 The NeXTSTEP Integration Depth (MODERATE)

The foundations spec says pane aspires to be "one thing, not a desktop running on a distro." The architecture spec doesn't address the distribution aspects at all: package management, system updates, the relationship between pane's config model and the host distro's package manager, how pane's filesystem conventions compose with the host filesystem. This may belong in a separate distribution spec, but the architecture spec should at least acknowledge the aspiration and identify the integration points.

### 2.6 The Guide Agent (MINOR)

The foundations spec describes a resident guide agent that teaches pane using pane (section 1, elaborated in extended-discussion.md). The architecture spec's AI section describes agents in general but doesn't specify the guide agent. This is probably fine for the architecture level — the guide agent is an application, not infrastructure. But the architecture spec should ensure the agent infrastructure is sufficient to support it.

---

## 3. What's Stale

### 3.1 pane-route References

The newsletter-wisdom document still refers to "pane-route" as a server. The architecture spec has eliminated it but the text is internally inconsistent — the Routing subsection is under "Servers" even though it explicitly says routing is a kit concern. The Composition Model section refers to routing as a server capability in some phrasings.

Recommendations: Remove all references to pane-route as a server. Move routing into the Kits section as a pane-app capability. Update the composition examples to show kit-level routing.

### 3.2 The `par` Crate Commitment

The architecture spec names `par` as the session type implementation. The foundations spec is implementation-agnostic. The architecture spec should describe required properties, not specific crates. (The Technology section can list candidates.)

### 3.3 The Synthesis Section in research-beos.md

The research document's synthesis section (1.8) still references "pane-route" throughout and describes it as a server: "pane-comp, pane-route, pane-roster, and pane-shell are separate processes communicating via session-typed protocols." This is stale. The research document should be updated alongside the architecture spec.

---

## 4. BeOS Fidelity Issues

### 4.1 The Input Server Omission

BeOS had a separate input_server (`~/src/haiku/src/servers/input/InputServer.h`). This was architecturally significant: input processing, keyboard mapping, input method add-ons, and input device management all happened in a separate process from app_server. The architecture spec folds all input handling into pane-comp. This is a valid simplification for the initial implementation, but it means:

- Input method add-ons (IME for CJK input) must be loaded into the compositor process
- Keyboard remapping plugins share the compositor's address space
- A buggy input add-on can crash the compositor

The architecture spec should at least acknowledge this tradeoff. If the intent is to add a separate input server later, say so. If the intent is to keep input in the compositor permanently, justify why the simpler architecture is worth the reduced isolation.

### 4.2 The BMessenger Model

The architecture spec correctly identifies BMessenger as the addressing mechanism — it carried messages "directly between applications via kernel ports." What it doesn't capture is how BMessenger enabled the scripting protocol. A BMessenger identifies a target by (team_id, port_id, handler_token) — see `~/src/haiku/headers/os/app/Messenger.h`, private members at line 92. This triple addresses a specific handler in a specific looper in a specific team. The scripting protocol depends on this: `hey Tracker get Frame of Window 0` sends a scripting message to Tracker's BApplication, which resolves specifiers down to the target handler.

Pane's addressing model for the scripting protocol needs to be specified. How does a script address "the text content of the third pane in the left split of workspace 2"? What's the pane equivalent of (team_id, port_id, handler_token)?

### 4.3 The BMessageFilter Gap

The architecture spec mentions BMessageFilter as one of the "four small, composable primitives" of BeOS messaging (section on Kits). But there is no pane equivalent specified. BMessageFilter was architecturally important — it enabled cross-cutting concerns (logging, access control, input preprocessing) without subclassing. The newsletter (Issue 2-36, William Adams) specifically praised it: "sub-classing a nice small object that is unlikely to change is probably easier than sub-classing a highly active object like BWindow or BApplication."

The architecture spec should specify the pane equivalent. In a Rust context, this maps naturally to middleware — functions that intercept and optionally transform messages before they reach the handler. The session type framework should accommodate this.

### 4.4 The Observer Pattern

BHandler supported an observer pattern: `StartWatching()`, `StopWatching()`, `SendNotices()` (see `~/src/haiku/headers/os/app/Handler.h`, lines 63-81). This was how components subscribed to state changes without tight coupling. The architecture spec mentions "reactive signals" for observable state but doesn't connect them to the messaging model. In BeOS, the observer pattern was built into BHandler — it used the same messaging infrastructure as everything else. Pane should ensure reactive signals compose with the session-typed protocol rather than being a separate mechanism.

---

## 5. The Server Decomposition

With the router eliminated, the servers are: pane-comp, pane-roster, pane-store, pane-fs, pane-watchdog. Plus pane-notify as a shared library (not a server).

### Assessment

This is a reasonable minimal set. The questions:

**Should there be an input server?** As discussed above, BeOS had one. The argument for: isolation of input add-ons, cleaner responsibility separation. The argument against: latency (another hop for every keystroke), complexity (another server to manage). My read: keep input in the compositor for now, but design the input handling as a separable subsystem so it can be extracted later if needed.

**Should there be a notification server?** The architecture spec has pane-notify as a library. In BeOS, notification was handled by the kernel (node_ref monitoring, live query updates). In pane, notification is filesystem-level (fanotify/inotify). The library approach is correct here — Linux's kernel already does the watching; pane-notify just abstracts over the kernel interfaces.

**What about a clipboard server?** BeOS had BClipboard, which communicated with the registrar (which was the same process as BRoster). Wayland has wl_data_device_manager for clipboard. The architecture spec doesn't mention clipboard at all. Where does clipboard live? In the compositor (via Wayland's data device protocol)? In a separate server? This needs to be addressed.

**What about a MIME type / file type recognition server?** The architecture spec says "file type recognition as a built-in" is NOT in pane-store, and that "type recognition is a client of pane-store that sets type attributes based on sniffing rules." But where does sniffing happen? Is it a daemon? A library? An on-demand process triggered by pane-notify events? BeOS's registrar handled MIME type management and sniffing. The architecture spec needs to specify this.

### The Interaction Model Without a Router

With kit-level routing, the interaction model is:

1. Client kits discover servers via pane-roster
2. Client kits establish direct sessions with servers
3. Content routing is evaluated locally in the client kit
4. The kit dispatches directly to the resolved handler

This means:

- Every client kit must be able to reach pane-roster (to discover servers)
- Every client kit must cache server locations (to avoid roster round-trips on every operation)
- Server location changes (crash + restart) must propagate to all clients

The architecture spec doesn't specify this propagation mechanism. When a server crashes and restarts with a new address, how do clients learn about it? Options: (a) clients query roster on connection failure and retry, (b) roster broadcasts location updates to all registered clients, (c) clients hold a session with roster that receives updates.

Option (a) is simplest and most resilient. The kit detects a broken connection, queries roster, reconnects. This is the BRoster model — `IsRunning()` and `FindApp()` are pull-based queries, not push notifications. BRoster did support push notifications via `StartWatching()`, but the common recovery path was "try to send, fail, look up again."

**Recommendation:** Specify the server discovery and recovery protocol. The kit queries roster to find servers, caches the result, and re-queries on connection failure. Roster publishes change events for clients that want proactive notification. Both paths should be available.

---

## 6. The Kit Decomposition

The architecture spec lists:
- pane-proto (foundation)
- pane-app (application lifecycle)
- pane-ui (interface)
- pane-text (text manipulation)
- pane-store-client (store access)
- pane-notify (filesystem notification)
- pane-ai (agent infrastructure)

The foundations spec names: AI Kit, Media Kit, Input Kit.

### Assessment

**pane-proto — Good.** Analogous to BeOS's Support Kit. Pure types, no runtime. This is the right foundation layer.

**pane-app — Good.** Analogous to BeOS's Application Kit. Looper abstraction, lifecycle, server connections. But it needs to explicitly include:
- Routing (currently in a separate Servers subsection)
- The scripting protocol (ResolveSpecifier equivalent)
- The observer pattern (StartWatching/SendNotices equivalent)
- Message filtering (BMessageFilter equivalent)

In BeOS, all of these were in the Application Kit. They're part of the programming model, not add-ons.

**pane-ui — Underdeveloped.** The description says "Cell grid writing helpers. Tag line management. Styling primitives (colors, attributes). Coordinate systems and scrolling." This is anemic compared to BeOS's Interface Kit, which was BWindow, BView, BControl, BLayout, BMenu, BTextView, BScrollView, BListView, BStringView, BBitmap, and dozens more. The architecture spec's pane-ui doesn't mention widgets, layout, or the rendering pipeline. The Widget Rendering section later mentions femtovg and taffy, but these aren't connected to pane-ui.

The Interface Kit was the largest kit in BeOS, and for good reason — it's where developers spend most of their time. The architecture spec's pane-ui description suggests it's a thin utility layer rather than a complete UI programming model. This contradicts the foundations spec's commitment that kits "ARE the programming model."

**Recommendation:** pane-ui should be described as the complete UI programming model: widget hierarchy, layout, rendering pipeline, event handling, the connection to the compositor's chrome rendering, and the cell grid model. The description should be proportional to its importance.

**pane-text — Questionable as a separate kit.** BeOS didn't have a separate text kit. BTextView was part of the Interface Kit. Structural regular expressions (sam-style) are a specific pane innovation, but the question is whether they warrant a separate kit or are a capability within pane-ui. The argument for separation: text manipulation is foundational to pane's "text as action" pillar, and other kits (pane-shell, pane-app's routing) need text operations without depending on the UI kit. The argument against: the dependency graph gets more complex without clear benefit.

**My read:** Keep pane-text separate but clarify its role as the text processing substrate that *both* the UI kit and the shell use. It's not a user-facing kit in the BeOS sense — it's a shared capability.

**pane-store-client — Good.** Analogous to BeOS's Storage Kit. The name "pane-store-client" is somewhat awkward — in BeOS it was just "the Storage Kit." Consider whether this kit should be called the Storage Kit (pane-storage or pane-store-kit) and encompass not just the attribute store client but also file operations, directory operations, and queries — the full BNode/BFile/BDirectory/BQuery surface.

**pane-notify — Good.** This is infrastructure shared across kits and servers. The "internal crate" designation is correct.

**pane-ai — The description is enormous and the scope is unclear.** The architecture spec devotes more text to pane-ai than to all other kits combined. Much of this text (agent communication patterns, .plan files, write/talk/mail/mesg/wall) is UX design, not kit architecture. As an architecture-level kit description, pane-ai should specify:
- What the kit provides to developers (API surface)
- How agents connect to the system (protocol)
- The sandbox model (isolation mechanism)
- The permission model (capability declarations)

The Unix communication primitives (write, talk, mail, mesg, wall) are applications that *use* the kit infrastructure, not kit infrastructure themselves. They should be described in a separate section or document.

**What's missing: Media Kit.** The foundations spec names the Media Kit. BeOS's Media Kit was one of its most important innovations — a real-time media processing framework with producer/consumer topology, format negotiation, and automatic latency management. PipeWire is the Linux equivalent. The architecture spec mentions PipeWire in passing ("a Media Kit abstracting over PipeWire") but doesn't list a media kit in the kit decomposition. If pane is serious about media as a peer of the rest of the system (foundations section 1 references this via BeOS), a media kit should be specified.

**What's missing: Input Kit.** The foundations spec names the Input Kit: "The Input Kit generalizes the keybinding power of the best text editors across every interface." This is not in the architecture spec's kit list. Input handling is described as a compositor responsibility, but the *kit* that applications use to declare key bindings, register input handlers, and integrate with the keybinding system is unspecified.

**What's missing: Translation Kit equivalent.** The architecture spec describes translators in the extension model but doesn't list a translation kit. In BeOS, the Translation Kit (BTranslatorRoster, BTranslator) was a kit that applications used to discover and invoke translators. Pane needs the equivalent — a kit that applications use to ask "what can handle this data?" and "convert this data for me."

### Recommended Kit Decomposition

```
Layer 0 (foundation):
  pane-proto     — wire types, session type definitions, serialization

Layer 1 (substrate):
  pane-support   — threading primitives, serialization, archiving
  pane-text      — text buffer, structural regexps, editing ops
  pane-notify    — filesystem notification abstraction

Layer 2 (system):
  pane-app       — looper, handler, messenger, message filter,
                   routing, scripting protocol, observer pattern,
                   application lifecycle, server connections
  pane-storage   — attribute read/write, queries, change subscription,
                   file/directory/node operations
  pane-translate — translator discovery, format conversion, quality routing

Layer 3 (domain):
  pane-ui        — widget hierarchy, layout, rendering, cell grid,
                   tag line, styling, event handling, chrome interface
  pane-input     — keybinding declarations, input method integration,
                   gesture handling, input dispatch
  pane-media     — PipeWire abstraction, producer/consumer topology,
                   format negotiation
  pane-ai        — agent sandbox, permission model, model dispatch,
                   behavioral specification
```

This mirrors BeOS's layering: Support Kit at the bottom, Application Kit in the middle, Interface Kit and domain kits at the top. The key additions vs. the current architecture spec: pane-support (explicit foundational utilities), pane-translate (the Translation Kit equivalent), pane-input (the Input Kit), pane-media (the Media Kit).

Whether all of these are needed for the initial implementation is a separate question. The architecture spec should describe the *target* kit ecology and note which kits are deferred. BeOS shipped with all its kits from R3 onward — the completeness of the kit set was part of the developer experience.

---

## 7. The Scripting Protocol: What the Architecture Spec Should Say

This deserves a dedicated section because the foundations spec makes it a convergence point of three theoretical commitments (session types, optics, monadic errors).

### What BeOS Did

The scripting protocol was built from:

**Commands:** GET, SET, CREATE, DELETE, COUNT, EXECUTE — a fixed verb set that every handler could support.

**Properties:** Named attributes of a handler — Frame, Title, Hidden, View, etc. Each handler declared its properties via `property_info` structs.

**Specifiers:** Addressing modes — DIRECT (the handler itself), INDEX (by position), NAME (by name), ID (by identity), RANGE (a span). These were pushed onto a stack in the scripting message.

**Resolution:** The `ResolveSpecifier` chain — each handler peeled one specifier off the stack and either handled the command or forwarded to a more specific handler.

**Discovery:** `GetSupportedSuites()` returned the handler's property declarations. Any client could ask "what can you do?" and get a structured answer.

**The tool:** `hey` — a command-line tool that parsed human-readable commands into scripting messages and displayed the results.

### How Pane Should Translate This

**Commands -> Session type protocol branches.** The CRUD verbs become branches in a scripting session type:

```
ScriptSession = &{
    get:    Recv<Specifier, Send<Result<Value, ScriptError>, ScriptSession>>,
    set:    Recv<Specifier, Recv<Value, Send<Result<(), ScriptError>, ScriptSession>>>,
    create: Recv<Specifier, Recv<Value, Send<Result<(), ScriptError>, ScriptSession>>>,
    delete: Recv<Specifier, Send<Result<(), ScriptError>, ScriptSession>>,
    count:  Recv<Specifier, Send<Result<u64, ScriptError>, ScriptSession>>,
    exec:   Recv<Specifier, Send<Result<Value, ScriptError>, ScriptSession>>,
    suites: Send<PropertyInfo, ScriptSession>,
    end:    End,
}
```

The session type makes the protocol explicit. A script must choose a verb, send a specifier, and handle the result. The loop (`ScriptSession` recursion) allows multi-command sessions.

**Specifiers -> Optic types.** Each specifier type is an optic:

- DIRECT -> identity optic (the target itself)
- INDEX -> indexed lens into a collection
- NAME -> keyed lens into a named collection
- ID -> identity-based lookup
- RANGE -> traversal over a span

The specifier stack is optic *composition*: "Frame of Window 0" is `index(0) . property("Frame")` — look up Window by index, then access its Frame property.

**Resolution -> Kit-level specifier resolution.** The pane-app kit should provide a `resolve_specifier` mechanism analogous to BLooper's. Each handler declares its properties (via a Rust trait or attribute macro). The kit walks the specifier stack, calling each handler's resolver in turn. This is where the static/dynamic tension lives: the optic types are known at compile time, but the handler graph is only known at runtime.

**Discovery -> Typed property declarations.** The `GetSupportedSuites` equivalent should return typed property metadata: property name, valid commands, valid specifier types, value types. This metadata should be derivable from the handler's trait implementation (Rust procedural macros can generate it).

**The tool -> `pane-hey` or `pane` CLI.** A command-line tool that parses commands like `pane get Frame of Pane 0 of Workspace 1`, sends a scripting session message, and prints the result. This tool is the test of the protocol — if it can express useful operations naturally, the protocol is working.

### The Hard Problem

The foundations spec flags this: "How optic-addressed access composes across handler boundaries in a running system — where the structure is only known at runtime — is the hardest design problem."

In BeOS, this was solved by dynamic dispatch: each handler's `ResolveSpecifier()` was a virtual method that could do anything. The "optics" were implicit — the code that resolved an INDEX specifier was just code that looked up a child by index. There was no formal optic structure.

For pane, the options are:

1. **Dynamic optics with type-safe values.** The specifier resolution is dynamic (runtime dispatch), but the values accessed through the optics are typed (the session type carries the value type). This is the pragmatic approach — it recovers BeOS's flexibility while adding value-level type safety.

2. **Static optics with runtime composition.** Define optic types statically, compose them at runtime. This is theoretically cleaner but may be impractical — the handler graph changes as panes are created and destroyed.

3. **Hybrid.** Static optics within a handler (accessing known properties of known types), dynamic composition across handler boundaries (resolving which handler to target next).

My recommendation is option 3. It's what BeOS effectively did — each handler's properties were statically known (declared in property_info), but the cross-handler resolution was dynamic. Session types can enforce the per-handler property access; the cross-handler resolution is a runtime loop that type-checks at each step.

---

## 8. Concrete Recommendations for the Rewrite

### Structure

1. **Vision** — rewrite with NeXTSTEP framing, democratic orientation, distribution aspiration
2. **The Pane** — add composition semantics, optic-governed views, trust model for the file/pane seam
3. **Protocol Discipline** — session types, the async/batching principle, the scripting protocol
4. **Views and Optics** — dedicated section on how internal state projects to each view via optics, lens laws as correctness criteria
5. **Concurrency** — per-component threading, the discipline that produces stability
6. **Error Composition** — monadic errors, session boundaries as error boundaries, concrete recovery paths
7. **Servers** — pane-comp, pane-roster, pane-store, pane-fs, pane-watchdog, startup ordering, server discovery protocol
8. **Kits** — the full kit ecology with adequate descriptions proportional to importance
9. **The Scripting Protocol** — dedicated section: commands, properties, specifiers, resolution, discovery, the CLI tool
10. **Extension Model** — routing rules, translators, pane modes, protocol bridges, plugin directories
11. **Composition** — how servers compose, how kits compose, the email case study
12. **Aesthetic** — visual identity, configuration surface, kit-mediated consistency
13. **Client Classes** — native, legacy, the two-world problem
14. **Technology** — implementation choices and justifications
15. **Build Sequence** — phased delivery

### Key Additions

- The scripting protocol section (absent entirely)
- The optics section (absent from architecture)
- Server discovery and recovery protocol (absent)
- Input handling architecture (collapsed into compositor without justification)
- Clipboard architecture (absent)
- MIME type / file type recognition architecture (vague)
- Startup ordering (absent)
- The full kit decomposition with adequate descriptions

### Key Removals

- Routing as a subsection under Servers (move to Kits)
- Excessive pane-ai text (move UX design to a separate document, keep kit architecture)
- Specific crate commitments in architectural sections (move to Technology)

### Key Corrections

- NeXTSTEP framing (not Mac OS X) for integration depth
- Live query evaluation responsibility (clarify pane-store vs. client-side)
- Input server tradeoff acknowledgment
- BRoster departure acknowledgment (service registry is new)

---

## 9. Summary

The architecture spec gets the big things right: the client-server split with kits as the programming model, per-component threading, session-typed protocols, filesystem as interface, modular composition. These are faithful to what we built at Be and to what the foundations spec demands.

The biggest gaps are the scripting protocol (entirely absent), the optics framework (theoretical commitment not grounded in architecture), and the kit decomposition (too thin, missing important kits). The biggest stale artifacts are pane-route references and the internal inconsistency about where routing lives.

The architecture spec reads as if it was written before the foundations spec reached its current form — specifically before the "session types + optics = scripting protocol" insight and the router elimination. A wholesale rewrite should start from the foundations spec's commitments and build the architecture to serve them, rather than revising the existing architecture spec to accommodate them.

The foundations spec is strong. The architecture spec needs to be worthy of it.
