# Isomorphic Packaging Principle — BeOS Engineering Vet

## 1. Did BeOS achieve this, violate it, or never attempt it?

BeOS achieved it for the things it cared about and never attempted it for the things it added later.

**Window title: three views, genuine coherence.** SetTitle() in Window.cpp (line 2047 in Haiku) updates fTitle locally, then sends AS_SET_WINDOW_TITLE to the app_server. The app_server takes the all-windows lock (`_MessageNeedsAllWindowsLocked` returns true for this message code — line 4526), updates the server-side Window object, and redraws the tab. The scripting protocol (property_info at line 157) exposes "Title" as a GET/SET property. A B_SET_PROPERTY for "Title" calls the same SetTitle() method (line 892). So: programmatic API, scripting protocol, and on-screen tab all go through the same path. Set the title from `hey`, it appears on screen. Set it from code, `hey` reads it back. The views are genuinely isomorphic.

**Window frame: coherent but synchronous.** MoveTo() is a synchronous call — `FlushWithReply` (line 2406). The client-side fFrame only updates after the server confirms. Frame() returns fFrame. The scripting protocol's "Frame" property (case 3, line 846) calls Frame() and MoveTo()/ResizeTo(). All views agree because the implementation serializes through the app_server. But this was achieved at a performance cost the Be engineers explicitly warned about (Hoffman, Issue 2-36: sync calls flush the cache and incur round-trip latency).

**BFS attributes and Tracker: coherent through node monitoring.** When an attribute changes on a file, BFS emits a B_ATTR_CHANGED node monitor notification. Tracker's PoseView::AttributeChanged() (line 5864) handles this: looks up the affected pose, reopens the model, updates the icon cache via `IconCache::sIconCache->IconChanged()`, invalidates the display. BQuery with live updates emits B_QUERY_UPDATE when an indexed attribute changes. This was real coherence — change a MIME type attribute from the command line, Tracker picks it up and updates the icon. Change it from Tracker's info window, the query picks it up. The filesystem and the GUI agreed.

**The mechanism:** Node monitoring was the coherence bus. It was a kernel-level pub/sub system that notified interested parties when nodes, entries, attributes, or stat data changed. This is the infrastructure that made the views agree — without it, each view would have stale caches. The principle was there, the implementation was there, but the coherence was reactive (notification-driven) not synchronous. There was always a window where views disagreed.

## 2. Where BeOS violated it, and what broke

**Stack & Tile: invisible to scripting.** Confirmed in the Haiku source — `src/servers/app/stackandtile/` has zero scripting support. No `property_info`, no `ResolveSpecifier`, no `GetSupportedSuites`. The grouping relationship is entirely server-side state managed through DesktopListener hooks. You can see two windows stacked on screen, but `hey` cannot discover, query, or modify the relationship. This is exactly the kind of bug the spec's principle calls out: "a relationship visible in one view and absent from another."

Stack & Tile was added to Haiku, not original BeOS, but it illustrates the failure mode perfectly. It was implemented as a server-side feature with visual manifestation but no protocol projection. The relationship existed in one view (screen) and was absent from another (scripting). Nobody could automate window tiling. Nobody could script layout management. The visual feature existed in an unreachable silo.

**Tracker icon cache vs. actual file type: lag and retry.** Look at the retry loop in AttributeChanged (line 5904): it tries to open the node up to 100 times with 10ms sleeps between attempts, because the node might be "busy" — in the middle of a mimeset operation. During those retries, the on-screen icon is stale relative to the filesystem. The comment at line 5747 is telling: "we may have missed some other attr changed notifications." The cache could get permanently stale if notifications were lost. The coherence was best-effort, not guaranteed.

**Node monitor limits.** Tracker had a hard limit on how many node monitors it could hold. `NeedMoreNodeMonitors()` (line 1726) bumps the limit by 512 at a time via `setrlimit(RLIMIT_NOVMON)`, but it can fail. If you opened a directory with thousands of files, Tracker might silently stop watching some of them. Those files would become invisible to the coherence mechanism — attribute changes would not update the display. One view (filesystem) would change; another (Tracker) would not notice.

**Workspaces and window state: an approximation.** The scripting protocol exposed Workspaces as a uint32 bitmask. But the actual workspace behavior was richer — virtual desktops with per-workspace window positions, per-workspace focus state. The bitmask told you which workspaces a window was in, but not where it was positioned on each one, not whether it was focused on workspace 3 while minimized on workspace 5. The scripting view was a lossy projection of the actual state, and nobody documented the loss.

**Replicants: serialized views with identity problems.** A Replicant was a BView archived to a BMessage and instantiated in another application's window. The archived view had state from the moment of archival. If the source application updated, the replicant's state diverged. There was no live coherence mechanism between a replicant and its source — the "view" was a snapshot, not a projection. The BDragger/BShelf system didn't maintain the isomorphism the spec demands.

## 3. Is the formalism load-bearing or decorative?

Mixed. The principle itself is load-bearing. The category theory is decorative for most implementers and useful for exactly one audience.

**What's load-bearing:** "Views are isomorphic packagings of the same data" and "a relationship visible in one view and absent from another is a bug." These statements change what gets built. An implementer reading this knows that adding Stack & Tile without a scripting projection is shipping a bug, not shipping a feature with a gap. That's a real constraint that prevents real failures.

**What's decorative:** "The views are functors preserving the monoidal structure; the packaging transformations between them are natural isomorphisms." A Be engineer would understand the informal principle instantly and find the categorical language alienating. We thought about systems in terms of servers, messages, ports, threads, and invariants — not functors and natural transformations. The notation doesn't tell me anything the English doesn't already say.

**Where the formalism earns its keep:** If someone is actually designing the optics composition system — the thing that composes protocol-to-filesystem projections — the categorical framing tells them that the composition must preserve structure, not just data. That's a real constraint on the implementation of the optics layer. But that's one engineer, maybe two. For the 20 engineers building components on top, the English version is what they'll internalize.

The spec already does this right: "The principle requires no formalism to apply." Keep that sentence. It's the escape valve that makes the formalism available without requiring it.

## 4. Does this principle change what an implementer does differently?

Yes, in exactly two ways.

**It requires completeness at feature time, not as a follow-up.** Under "views reflect state consistently," an implementer adding a new feature could reasonably ship the visual presentation first and add the scripting/filesystem projections later (or never — see Stack & Tile). Under "views are isomorphic packagings," the feature is incomplete until all projections exist. The spec makes the filing of "add scripting support later" a bug report, not a ticket.

Concrete scenario: an implementer adds a tab grouping feature. Under the weaker statement, they build the visual grouping, the drag interaction, the animations. Under the stronger statement, they must also implement the filesystem projection (the group appears as a directory relationship under `/srv/pane/`) and the protocol projection (the group is queryable and modifiable through the session). If they don't, the spec says they shipped a bug. That's the constraint that prevents the Stack & Tile failure mode.

**It requires projection design at protocol design time.** When designing a new piece of state, the implementer must think through how it projects into each view upfront: what does this look like on screen, what does it look like in the filesystem, what does the protocol message look like, what does the accessibility tree node look like. The isomorphism requirement means these projections must round-trip. You can't have state that exists in the protocol but has no filesystem representation (or you must document the loss explicitly).

Concrete scenario: a pane has a "urgency" state (it wants attention). Under the weaker statement, this might manifest as a visual flash and nothing else. Under the stronger statement, urgency must appear as a filesystem attribute (queryable — "show me all urgent panes"), as a protocol-accessible property, and in the accessibility tree. The requirement to project into all views forces the implementer to think about whether urgency is transient or persistent, which is a design decision that would otherwise get deferred.

## 5. Interaction coherence: BeOS experience

**Setting window title through scripting: worked.** `hey AppName set Title of Window 0 to "New Title"` called B_SET_PROPERTY, which routed through BWindow::ResolveSpecifier to the Title handler, which called SetTitle(), which sent AS_SET_WINDOW_TITLE to the app_server, which took the all-windows lock and updated the tab. Visible on screen. No observable lag to a human — the message path was microseconds.

**Setting BFS attributes from the command line: worked, with observable lag.** You could `addattr -t string BEOS:TYPE "text/plain" somefile` and Tracker would eventually notice via node monitor. But "eventually" was the issue. The notification was asynchronous. There was a message queue between the kernel's node monitor emission and Tracker's BLooper processing it. Under load, with many files changing, the lag was perceptible. The Tracker busy-retry loop (100 attempts, 10ms each — up to a full second of spinning) shows they knew this was a problem.

**Ordering guarantees: none explicit.** If you changed a window's title and its frame in rapid succession through scripting, the messages went through the window's BLooper in order (single-threaded dispatch). But if one change went through the scripting protocol and another through the direct API in a different thread, there was no ordering guarantee. The all-windows lock in the app_server serialized conflicting changes, but the client-side state could see them in different orders depending on thread scheduling.

**Atomicity: per-property, not cross-property.** You could not atomically set a window's title and frame. Each was a separate message. There was no transaction mechanism. A script querying a window between the two messages would see an inconsistent state. This was never a practical problem because the messages were processed quickly, but the formal guarantee wasn't there.

**What pane needs to get right that BeOS got wrong:**

1. **Make the notification mechanism reliable.** BeOS's node monitors could be exhausted (Tracker hit limits on large directories). Pane's filesystem projection must not silently stop updating. If the coherence bus has capacity limits, they must be documented and the failure must be detectable — not silent staleness.

2. **Make propagation latency bounded and documented.** "Eventually consistent" is fine if "eventually" has a bound. BeOS's node monitor notifications had no latency guarantee. Pane should specify whether view coherence is synchronous (all views agree before the operation returns — like MoveTo) or asynchronous (views agree within a bounded window — like attribute changes), and the choice should be per-operation, documented, and motivated.

3. **Provide cross-property atomicity where it matters.** If two properties are semantically coupled (position and size during a resize, title and icon during a type change), there should be a way to change them atomically. BFS had this for individual attribute writes (they were atomic) but not for multi-attribute updates. Pane's protocol should support grouped operations where the views see the change set, not individual changes.

4. **Make lossy projections explicit.** Some views are inherently lossy — the screen can show spatial relationships that the filesystem can't capture (z-order? overlap? animation state?). The spec already gestures at this ("up to semantic equality"), but the implementation specs need to enumerate what is lost in each projection and why. BeOS's Workspaces bitmask was a lossy projection that nobody documented as lossy.

5. **Don't allow new features without projection coverage.** This is the organizational/process implication. Stack & Tile happened because the server team added a feature without coordinating with the scripting infrastructure. Pane's development process needs to treat missing projections as blocking bugs, not enhancement requests. The spec's principle gives the justification; the development culture has to enforce it.
