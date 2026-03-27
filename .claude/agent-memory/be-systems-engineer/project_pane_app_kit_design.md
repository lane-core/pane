---
name: pane-app kit API design decisions
description: Key design decisions in pane-app kit spec (2026-03-26) — App not a looper, flat enum vs handler chain, filesystem scripting, routing rules as TOML files
type: project
---

pane-app kit spec written (2026-03-26). Key departures from BeOS and reasons:

1. **App is NOT a looper.** Unlike BApplication (which inherited BLooper), pane's App just owns the connection and factory. BApplication's looper was confusing (which messages go to app vs window?). Per-pane loopers handle everything.

2. **Flat enum replaces handler chaining.** BeOS used chain-of-responsibility (SetNextHandler). Pane uses exhaustive Rust enum matching, which is strictly stronger -- the compiler forces completeness. Handler chaining was primarily needed for scripting delegation (ResolveSpecifier), which pane handles via the filesystem.

3. **Filesystem IS the scripting protocol.** BeOS scripting required BMessages and ResolveSpecifier. Pane uses /srv/pane/ FUSE mount -- any tool, any language. Strictly more powerful, zero kit dependencies for consumers.

4. **Routing rules as TOML files** in well-known directories, watched by pane-notify. Subsumes MIME type association, command dispatch, and content transformation. Quality-based disambiguation (Translation Kit pattern).

5. **Connection management is kit-internal.** Session types, handshake, three-phase protocol, reconnection -- all hidden. Developer sees App::connect() and PaneEvent::Disconnected/Reconnected.

**Why:** the Schillings test. "Common things are easy to implement and the programming model is CLEAR." The hello-pane example is 14 lines.

**How to apply:** when implementing pane-app, the public API surface is what matters. Session types are an implementation detail, never re-exported. pane-session is a private dependency.
