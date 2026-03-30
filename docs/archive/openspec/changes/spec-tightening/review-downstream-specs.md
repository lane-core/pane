# Downstream Spec Review — 2026-03-21

Reviewed against:
- `/openspec/specs/architecture/spec.md` (the architecture spec)
- `/openspec/specs/foundations/spec.md` (the foundations spec)

---

## 1. aesthetic/spec.md — Clean with one note

**Verdict: clean.**

Accurately reflects arch §10 (rendering model, aesthetic). Correct on kit-mediated consistency, client-side rendering, compositor-rendered chrome. The rendering split matches. Design tokens as a concept are consistent with the arch spec's "individual properties configurable" language.

**One note:** Spec names Inter and Monoid as official fonts. The arch spec doesn't name fonts — it says "proportional sans-serif" and "monospace." This is appropriate detail for a component spec to add, not a contradiction, but font choices should be flagged as design decisions not yet committed in the arch spec.

---

## 2. filesystem-config/spec.md — Clean

**Verdict: clean.**

Accurately reflects arch §9 (mutable config on immutable base, Nix defaults with overrides, `user.pane.modified` xattr tracking, activation script reconciliation). The one-file-per-key model and pane-notify reactivity are consistent with the architecture's filesystem-as-interface commitment and the kit hierarchy (pane-notify is a kit, not a server). btrfs xattr usage is correctly stated.

---

## 3. licensing/spec.md — Clean

**Verdict: clean.**

The kit/server license split (MIT for kits, AGPL for servers) correctly maps to the architecture's boundary: kits are in-process libraries, servers are separate processes. The crate lists match the arch spec's kit hierarchy (§4) and server decomposition (§3). pane-notify correctly listed as MIT (it's a kit in the hierarchy diagram).

---

## 4. pane-compositor/spec.md — Three issues

This is mostly good. The three-tier threading model, RwLock, calloop scoping, and rendering split all faithfully reproduce the architecture. But:

**Issue 1: Missing tag-based visibility.** The layout tree section (§5) mentions "tag-based visibility (dwm-style bitmask)" in passing but the requirements section has no requirement covering it. Tag-based visibility is a core layout feature from arch §3 (pane-comp responsibilities: "recursive tiling with tag-based visibility"). It needs a requirement with scenarios.

**Issue 2: Missing heartbeat and crash handling.** Arch §7 commits to heartbeat between compositor and pane-watchdog (2s interval, 3 misses = 6s detection) and crash handling (catch_unwind, session-terminated events, cleanup of dead client panes). The compositor spec has no requirements for either. These are compositor responsibilities — it must implement the heartbeat protocol and crash boundaries for client sessions.

**Issue 3: Missing frame timing.** Arch §3 lists "frame timing: coordinates frame callbacks across all clients, submits composited output to DRM/KMS" as a compositor responsibility. The narrative sections mention presentation but there is no requirement covering frame callback coordination, presentation-time protocol support, or the frame-pacing relationship between compositor and clients. This is important for the async-by-default / batch-and-flush model.

**Minor:** §7 (three-tier access model) duplicates the arch spec's table verbatim. The comp spec should reference the architecture's model, not repeat it — this creates a maintenance burden where two copies can drift.

---

## 5. pane-fs/spec.md — Two issues, one serious

**Serious issue: Plumber filesystem interface.** The spec has a full requirement for plumber ports at `/srv/pane/plumb/` — "Writing to `send` SHALL route a plumb message. Reading from a named port SHALL stream matched messages as JSONL."

The architecture spec has no plumber. The word "plumb" does not appear anywhere in the architecture spec. The arch spec eliminated the central router (pane-route) and moved routing to the pane-app kit (§3 "Why no router server"). There is no plumber server, no plumb message concept, and no `/srv/pane/plumb/` path. This is a stale reference — either from an earlier architecture draft or from Plan 9's plumber concept that didn't survive into the current architecture.

The routing model is: pane-app kit loads rules from `/etc/pane/route/rules/` and `~/.config/pane/route/rules/`, evaluates locally, dispatches directly. This is a kit-level concern with no filesystem interface for message injection. If the intent is to expose routing to shell scripts via the filesystem, that's a legitimate design question — but it's not what the architecture currently specifies, and the pane-fs spec shouldn't invent it unilaterally.

**Recommendation:** Remove the plumber requirement entirely. If filesystem-accessible routing is desired, raise it as a design question for the architecture spec.

**Issue 2: Configuration filesystem interface at /srv/pane/config/.** The spec exposes server config under `/srv/pane/config/` mirroring `/etc/pane/`. The architecture spec does not mention this. Config lives at `/etc/pane/` (arch §9), and pane-fs exposes pane state at `/srv/pane/` (arch §3). These are different concerns. Mirroring `/etc/pane/` into `/srv/pane/config/` creates a second path to the same data with unclear semantics — which is authoritative? Writing to `/srv/pane/config/comp/font` vs. writing to `/etc/pane/comp/font` should not be two different mechanisms for the same operation.

**Recommendation:** Drop `/srv/pane/config/` from pane-fs. Config is already accessible at `/etc/pane/` on the real filesystem. There is no need to proxy it through FUSE.

**Acceptable extension:** The `event` file (JSONL event stream per pane) is not in the arch spec's filesystem tree but the arch spec explicitly says "the specific tree structure evolves with implementation." This is a reasonable addition that aligns with Plan 9's model and enables `tail -f` monitoring. Fine.

---

## 6. pane-notify/spec.md — Clean

**Verdict: clean.**

Accurately reflects arch §4 kit hierarchy (pane-notify at the foundation layer), §3 calloop scoping (compositor is the exception), §3 pane-store's fanotify usage (FAN_MARK_FILESYSTEM for mount-wide xattr detection). Correctly specifies the fanotify/inotify split by scope. CAP_SYS_ADMIN requirement is an important operational detail correctly documented here.

The FAN_ATTRIB disambiguation note (consumer must diff against cached values) is a valuable implementation detail that the arch spec doesn't cover at this level — appropriate for a component spec.

---

## 7. plugin-discovery/spec.md — One issue

**Issue: Routing rule paths.** The spec lists `~/.config/pane/route/rules/` as a well-known plugin directory. The architecture spec (§4, pane-app) says rules load from `/etc/pane/route/rules/` and `~/.config/pane/route/rules/`. The plugin-discovery spec correctly has both (system-wide under `/etc/pane/` and user under `~/.config/pane/`). **This is consistent.**

However, the spec says "pane-roster SHALL detect it, register the application's metadata, and make it launchable" for `.app` directories. The architecture spec says pane-roster handles application lifecycle (launch semantics, monitoring, session save/restore) and service registry, but plugin directory scanning is described as a kit-level concern: "The pane-app kit loads routing rules from the filesystem" and "watches rule directories via pane-notify for live discovery." The question is whether `.app` directory discovery is a roster responsibility (server-side) or a kit responsibility (client-side, like routing rules and translators).

Given that pane-roster is the application directory and needs to know about installed apps for launch semantics (B_SINGLE_LAUNCH etc.), having the roster scan `.app` directories is defensible. But this is a design decision the plugin-discovery spec is making that the architecture spec doesn't explicitly address.

**Minor: `.app` directory contents.** The spec describes `.app` directories as containing "binary/wrapper, integration metadata, pane-specific hooks, routing rules, `.plan`-governed agent companions." This comes from arch §13 (the two-world problem / open questions section), not from a committed part of the architecture. Flagging because the `.app` directory structure is still in the open questions zone.

---

## Summary

| Spec | Verdict | Action needed |
|---|---|---|
| aesthetic | Clean | None |
| filesystem-config | Clean | None |
| licensing | Clean | None |
| pane-compositor | 3 issues | Add requirements for tag-based visibility, heartbeat/crash handling, frame timing |
| pane-fs | 1 serious + 1 moderate | Remove plumber requirement, remove /srv/pane/config/ |
| pane-notify | Clean | None |
| plugin-discovery | 1 minor | Clarify .app discovery ownership (roster vs. kit) |

The biggest problem is the plumber in pane-fs — it references a concept that does not exist in the architecture. The compositor spec is structurally sound but missing requirements for three things the architecture commits to. Everything else is either clean or has minor ambiguities that are within the normal latitude of component specs adding appropriate detail.
