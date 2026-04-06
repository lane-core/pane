# Plan 9 Divergences

Each entry: the Plan 9 concept, how pane adapts it, and why.
Default policy: when pane draws on Plan 9, cite the specific concept and document divergences.

## Event Loop Model

| Plan 9 | pane | Rationale |
|--------|------|-----------| 
| Per-process event loop via `alt` in libthread (threadalt over channels) | Per-pane calloop `EventLoop` with multi-source dispatch | Same philosophy: single-threaded sequential dispatch, concurrency via multiple loopers. calloop replaces libthread's `alt` with fd-readiness-based source multiplexing. |
| `alarm(2)` — single per-process pending alarm, replaced on each call | Multiple concurrent timers per looper via calloop `Timer` sources | Plan 9 deliberately kept one alarm; pane needs multiple (pulse, delayed messages, periodic). calloop's TimerWheel handles deadline scheduling. |

## Distributed Architecture

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `import`/`export` — mount remote fileservers into local namespace | `App::connect_remote()` — TCP connection to remote pane server | Same principle: remote server is just another server with higher latency. Local machine has no architectural privilege. |
| Per-process namespaces (`rfork(RFNAMEG)`) | Per-app connection topology; pane-fs synthetic namespace | Plan 9 namespaces were kernel-mediated; pane uses userspace FUSE. The namespace concept is preserved: each app sees its own view of the pane hierarchy. |
| 9P protocol — stateful file protocol with attach, walk, open, read, write, clunk | pane-session session-typed channels | 9P's statefulness maps to session types. The `clunk` concept (cleanup on abandon) is preserved in `PaneCreateFuture`'s Drop impl. |
| `/srv` — service registry as filesystem | pane-roster — service discovery across instances | Plan 9 posted services to /srv as file descriptors. pane-roster abstracts over init systems (s6/launchd/systemd) but the concept is the same: named services discoverable through a namespace. |
| `factotum` — per-user auth agent with `confirm`/`needkey` consent protocol | TLS + `.plan` + Landlock; no separate agent daemon | Same principle (separate auth from services) via Rust's Transport trait. factotum's `confirm` pattern (interactive consent for key use) and `needkey` (prompt for missing credentials) are worth adopting as interactive `.plan` overrides. factotum's `disabled=by.factotum` auto-quarantine of failed keys maps to temporary PeerIdentity blacklisting. |

## Filesystem / Namespace

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| Synthetic filesystems (`#` devices, `/proc`, `/net`) | pane-fs — FUSE synthetic filesystem exposing pane hierarchy | Same pattern: computed views presented as files. pane-fs makes BFS-style queries accessible as Plan 9-style synthetic paths. |
| `/proc/N/ctl` — write commands to control processes | (planned) pane-fs `ctl` files for pane control | Plan 9's ctl pattern: imperative operations via writes to a control file. Deferred until pane-fs is implemented. |
| Union directories — overlay multiple directories | pane-fs unified namespace — local + remote interleaved under `/pane/` | Plan 9 union mounts let multiple servers contribute to one path. pane-fs interleaves local and remote panes in a single hierarchy. |

## Session / Protocol

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `clunk` — release a fid (file handle) when done | `PaneCreateFuture` Drop impl sends `RequestClose` | Clunk-on-abandon: if you drop the handle without consuming it, the resource is cleaned up. Directly from 9P. |
| Stateful fid walks (`walk` + `open` + `read/write` + `clunk`) | Session-typed `Chan<S, Transport>` with typestate transitions | 9P's stateful protocol is the spiritual ancestor. Session types make the state machine compiler-checked rather than runtime-checked. |
| `Tversion`/`Rversion` — protocol negotiation | `ClientHandshake`/`ServerHandshake` session types | Same purpose: version negotiation and capability exchange before active phase. |

## Identity / Security

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| No superuser; per-user `factotum` auth agent | `PeerIdentity` in handshake; `.plan` sandbox descriptors | Plan 9's "no root" philosophy. pane's identity model forwards user identity across connections without privilege escalation. |
| Host-owner distinction (owner of CPU server vs. user) | Host as contingent server — local hardware has no architectural privilege | Directly from Plan 9: the machine you're sitting at is just a terminal; the CPU server does the work. pane generalizes: any server is just a server. |

## Timers (Phase 2 — planned)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `alarm(2)` per-process, no cross-process timer facility | Timers are looper-local; no cross-pane timer registry | Plan 9 didn't make timers a namespace resource. Timers are scheduling primitives, not shared state. pane follows this: each looper owns its timers. |
| Alarm delivery via note (signal-like) | Timer events enter the normal message batch via `state.batch` | Plan 9 notes preempted; pane timer events are regular messages processed sequentially. Less surprising, same eventual effect. |

## Clipboard

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `/dev/snarf` — per-rio-instance shared file, read returns contents, write sets them, no locking | `Clipboard` kit type with `ClipboardWriteLock` typestate (lock/commit/revert) | Plan 9's snarf was last-writer-wins with no concurrency control. pane adds transactional semantics. Snarf was per-session (all windows shared one); pane supports named clipboards with sensitivity/TTL/locality metadata. |
| Recursive rio uses parent's snarf buffer | Clipboard federation with `Locality` enum (Local/Remote/Federated) | When rio ran recursively, inner instances delegated to the parent's snarf. pane generalizes: remote instances can participate in clipboard federation, subject to policy. |

## Plumbing / Routing

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `plumber(4)` — central language-driven file server for inter-app messages | Kit-level routing rules evaluated locally in pane-app | Plan 9's plumber was a single process, a potential bottleneck and SPOF. pane evaluates routing rules in-process (kit library). Same pattern-action language concept, no intermediary. |
| Plumb message format: text header + data, port-based dispatch | Typed `Message` enum with pattern matching, session-type channels | Plan 9 messages were text with `name=value` attributes. pane uses Rust enums with compile-time exhaustiveness checking. |

## Observer / Property Watching (UPDATED from primary sources)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| No subscription/notification in 9P — blocking reads on state files (wctl, wait, mouse) | Dual path: blocking-read pane-fs files (scripting) + push `start_watching` (protocol) | Plan 9's blocking-read model requires one thread per observable per observer. Works for scripting, doesn't scale for reactive UI. pane provides both. |
| rio wctl: read blocks until window changes size, location, or state | `/pane/<id>/event` file blocks until pane state changes | Direct adaptation of wctl pattern for external tool integration. |
| plumber multicast: "A copy of each message is sent to each client that has the corresponding port open" | Kit-level routing: all interested receivers get dispatched copies | plumber(4) already did multicast. pane routing should preserve this. |
| plumber `click` attribute: cursor context refines regex match | (not yet designed — should be adopted) | The click attribute pattern lets the plumber narrow a text selection to the semantically relevant portion. pane routing rules should support content-refinement. See plumb(6) `click` description. |
| plumber BUGS: "file name space is fixed" | Kit-level routing evaluates in app's own namespace | Confirmed from plumber(4) BUGS section. The plumber couldn't route messages involving files in newly mounted services because it used its own fixed namespace. pane avoids this by design. |

## Connection Resilience (v1 REMOVED — redesign pending)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `aan(8)` filter — always-available network, buffers messages during temporary disconnection, retransmits unacknowledged data after reconnection | (planned) ReconnectingTransport wrapping TCP with reconnection + message buffering + replay | v1 prototype had a ReconnectingTransport impl; removed during redesign. Design: configurable timeout (default 60s vs aan's 1 day). **Divergence:** aan was a symmetric filter applied to both client and server via `import -p`/`exportfs`; pane's version will be client-side only initially. aan used a custom sequence-number protocol; pane will replay buffered messages at the framing layer, simpler but loses server-side messages sent during disconnection. |

## Export / Namespace Filtering (UPDATED from primary sources)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `exportfs -P patternfile` — regex `+`/`-` patterns on path names | `.plan` file governs what a remote observer can see | exportfs(4) confirms: "For a file to be exported, all lines with prefix `+` must match and all those with prefix `-` must not match." pane's `.plan` should use structured predicates (signature, type, sensitivity) rather than path regexes. |

## Diagnostic / Debugging Patterns (v1 REMOVED — redesign pending)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `iostats` — transparent 9P proxy that monitors file operations | (planned) ProxyTransport wrapping any Transport impl | v1 prototype had a ProxyTransport; removed during redesign. Design: wrap any Transport, log all send/recv with timestamps and hex preview. The names paper pattern applied to pane's Transport trait. |
| `exportfs -d` — log all 9P traffic to a debug file | (planned) `--protocol-trace <file>` flag on headless binary | Design: log handshake + active-phase messages to file. Divergence: pane logs at both transport and application layers; Plan 9 logged only at the 9P layer. |

## Terminal / Window Architecture (NEW — from primary sources)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| rio `consctl` mode changes revert on close (lease pattern) | (not yet designed for pane-shell) | From rio(4): "Closing the file makes the window revert to default state (raw off, hold off)." The fd acts as a lease — holding it open holds the mode. pane-shell should use a similar pattern for raw-mode control: open a handle, mode persists while handle is held, reverts on drop. Rust's RAII makes this natural. |
| rio mounts per-window `/dev/cons` via namespace, shadowing kernel cons | pane-shell communicates via standard pane protocol channels | Plan 9 used per-process namespaces to multiplex /dev/cons per window. pane doesn't have kernel namespace support, but the session-typed protocol achieves the same multiplexing — each pane has its own protocol channel. |
| rio `text` file — read-only full window contents | pane-fs `/pane/<id>/body` — semantic content of pane | rio's text file was the complete scrollback buffer. pane-fs body should be the semantic content (e.g., for a shell: command output history). |
| rio `wdir` — writable, app updates on chdir, used for plumb messages | pane-fs `/pane/<id>/attrs/cwd` — writable, app updates, used for routing | Direct adaptation. The shell maintains this file so that routing rules know the working directory context. |
| rio `wsys` directory — per-window subdirectories | pane-fs `/pane/` hierarchy | rio(4) served a `wsys` directory with one subdirectory per window, each containing cons, consctl, label, mouse, etc. pane-fs serves the same structure under `/pane/<id>/`. |

## What pane does NOT take from Plan 9