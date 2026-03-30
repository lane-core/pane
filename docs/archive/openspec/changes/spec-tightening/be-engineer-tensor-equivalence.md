# Be Engineer Assessment: Composition Equivalence as Invariant

## Was BeOS honest about composition structure?

No. This was a real gap, and the Haiku source makes it concrete.

**Stack & Tile is the clearest case.** Haiku's `SATGroup` (in `src/servers/app/stackandtile/SATGroup.h`) maintains a rich internal model: windows grouped into `WindowArea` objects, areas sharing `Tab` constraints through a `LinearSpec` solver, spatial relationships tracked precisely. It's a real composition structure — a constraint-solving tiling model inside the app_server.

But none of it is visible through the scripting protocol. `BWindow::GetSupportedSuites()` reports `suite/vnd.Be-window` with properties: Frame, Title, Feel, Flags, Look, Hidden, Workspaces, Minimize, View, MenuBar, TabFrame. No "Group." No "Neighbors." No "TiledWith." Zero hits for `ResolveSpecifier` or `GetSupportedSuites` anywhere in the `stackandtile/` directory.

The `BWindowStack` API exists, but it's a private link-protocol API — it talks to app_server through magic identifiers, completely bypassing the scripting protocol. You can't `hey` your way to discovering that two windows are tiled together.

The app_server knows. The user can see it on screen. But the scripting protocol — the very tool built for introspection — cannot discover the relationship. Two views disagree about what relationships exist.

**Workspaces are subtler.** BWindow exposes a `Workspaces` property (a bitmask), get/settable through `hey`. But workspace membership is a flat property on individual windows — there's no way to ask "which windows share workspace 3?" through the scripting protocol. You enumerate every window of every application and filter. The app_server's Desktop tracks this directly internally. The structure exists in one view but the other gives you only raw material to reconstruct it.

## What goes wrong when equivalences aren't preserved

1. **Automation breaks on layout.** Script moves window A; doesn't know B is tiled with it. Group breaks. The script did exactly what was asked — the system never told it about the relationship.

2. **Recovery loses composition.** Stack & Tile groups existed only in volatile app_server memory. Crash recovery restored individual window positions; groups were gone. The filesystem knew nothing about them.

3. **Tools can't compose.** Every tool that needs group awareness must build its own model by scraping individual properties. Each builds a slightly different model with slightly different bugs. Exactly the failure the scripting protocol was designed to prevent.

4. **Tracker's per-directory view state.** Two windows showing the same directory in different sort orders: last one closed wins. Filesystem thinks there's one view state, runtime knows there are two.

## Yes, it's an invariant, not a feature

The principle: **if two panes are in a composition relationship in any view, that relationship must be discoverable through every view.**

Not identically represented — the layout tree says "horizontal split," the filesystem says "sibling directories under a split node," the protocol says "sub-session containing two child sessions." The representations differ. The structure they encode must not.

This is GetPut/PutGet lifted from individual state to composition structure. Violations are either intentional-and-documented or bugs.

## Constraints on the architecture

**pane-fs.** Must encode composition relationships, not just individual pane state. A split containing A and B is a directory containing `A/` and `B/` plus attributes encoding the split's own properties (orientation, ratio). The filesystem tree mirrors the layout tree's nesting.

**The protocol.** Must emit composition events, not just geometry events. "A was added to split S alongside B" — not just "A moved to (x,y)." Protocol consumers must not need to reconstruct relationships from geometry.

**The layout tree.** Must not contain relationships inexpressible in the other views. If the compositor supports a composition primitive (tabbed stacking, etc.), it must have filesystem and protocol representations before it ships. No internal-only composition modes.

**The practical test.** For every composition operation: can a script (a) discover the relationship exists, (b) query its properties, (c) modify or dissolve it — all through the standard protocol, without special-casing? This is the test BeOS would have failed for Stack & Tile. Pane must pass it.

## The deeper point

This is what distinguishes "everything is a pane" from "everything is a BHandler." BHandler made individual objects uniformly scriptable. Composition was ad hoc. Pane's commitment means relationships between panes are also pane-like: they have state, they're projected through views, the projections must be consistent. A split is not an implementation detail of the compositor — it's an object in the system, subject to the same discipline as the panes it composes.

That's harder than individual-object consistency. It means the filesystem, protocol, and compositor must agree not just on *what exists* but on *how things relate*. But it's the constraint that makes the system actually composable rather than just claiming to be.
