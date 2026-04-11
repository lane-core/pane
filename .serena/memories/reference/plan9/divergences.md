---
type: reference
status: current
supersedes: [pane/plan9_divergences]
sources: [pane/plan9_divergences]
created: 2026-03-29
last_updated: 2026-04-10
importance: high
keywords: [plan9, divergences, 9P, namespace, cpu, factotum, plumber, snarf, rio, exportfs, addressing, fid]
related: [reference/plan9/_hub, reference/plan9/decisions, reference/plan9/distribution_model, reference/plan9/man_pages_insights]
agents: [plan9-systems-engineer, pane-architect, session-type-consultant]
---

# Plan 9 Divergences

Each entry: the Plan 9 concept, how pane adapts it, and why.
Default policy: when pane draws on Plan 9, cite the specific
concept and document divergences.

## Event Loop Model

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| Per-process event loop via `alt` in libthread (threadalt over channels) | Per-pane calloop `EventLoop` with multi-source dispatch | Same philosophy: single-threaded sequential dispatch, concurrency via multiple loopers. calloop replaces libthread's `alt` with fd-readiness-based source multiplexing. |
| `alarm(2)` — single per-process pending alarm, replaced on each call | Multiple concurrent timers per looper via calloop `Timer` sources | Plan 9 deliberately kept one alarm; pane needs multiple (pulse, delayed messages, periodic). calloop's TimerWheel handles deadline scheduling. |

## Distributed Architecture

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `import` / `export` — mount remote fileservers into local namespace | `App::connect_remote()` — TCP connection to remote pane server | Same principle: remote server is just another server with higher latency. Local machine has no architectural privilege. |
| Per-process namespaces (`rfork(RFNAMEG)`) | Per-app connection topology; pane-fs synthetic namespace | Plan 9 namespaces were kernel-mediated; pane uses userspace FUSE. The namespace concept is preserved: each app sees its own view of the pane hierarchy. |
| 9P protocol — stateful file protocol with attach, walk, open, read, write, clunk | pane-session session-typed channels | 9P's statefulness maps to session types. The `clunk` concept (cleanup on abandon) is preserved in `PaneCreateFuture`'s Drop impl. |
| `/srv` — service registry as filesystem | pane-roster — service discovery across instances | Plan 9 posted services to /srv as file descriptors. pane-roster abstracts over init systems (s6 / launchd / systemd) but the concept is the same: named services discoverable through a namespace. |
| `factotum` — per-user auth agent with `confirm` / `needkey` consent protocol | TLS + `.plan` + Landlock; no separate agent daemon | Same principle (separate auth from services) via Rust's Transport trait. factotum's `confirm` pattern (interactive consent for key use) and `needkey` (prompt for missing credentials) are worth adopting as interactive `.plan` overrides. |
| factotum auth *conversation* over afid channel (protocol-agile, runtime-switchable) | Transport trait resolves identity before handshake; identity is a transport property, not negotiated in Hello | factotum gained protocol agility via conversation; pane gains it via trait impls. factotum could switch auth at runtime; pane's is fixed per transport type. |
| `authinfo` result of auth — user identity string, no pid | `PeerAuth::Kernel { uid, pid }` — includes process identity | Plan 9 auth was user-granularity (all your processes are equally you). pane needs per-pane ownership (`pane_owned_by()`), requiring pid. |
| auth(2) explicit conversation even on local connections | SO_PEERCRED implicit kernel assertion on unix sockets | Plan 9's uniform conversation model was conceptually clean but the security benefit on local connections was theoretical. pane achieves uniformity through the Transport trait producing PeerAuth uniformly. |

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
| Stateful fid walks (`walk` + `open` + `read` / `write` + `clunk`) | Session-typed `Chan<S, Transport>` with typestate transitions | 9P's stateful protocol is the spiritual ancestor. Session types make the state machine compiler-checked rather than runtime-checked. |
| `Tversion` / `Rversion` — protocol negotiation | `ClientHandshake` / `ServerHandshake` session types | Same purpose: version negotiation and capability exchange before active phase. |

## Identity / Security

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| No superuser; per-user `factotum` auth agent | `PeerIdentity` in handshake; `.plan` sandbox descriptors | Plan 9's "no root" philosophy. pane's identity model forwards user identity across connections without privilege escalation. |
| Host-owner distinction (owner of CPU server vs. user) | Host as contingent server — local hardware has no architectural privilege | Directly from Plan 9: the machine you're sitting at is just a terminal; the CPU server does the work. pane generalizes: any server is just a server. |

## Timers (Phase 2 — planned)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `alarm(2)` per-process, no cross-process timer facility | Timers are looper-local; no cross-pane timer registry | Plan 9 didn't make timers a namespace resource. Timers are scheduling primitives, not shared state. pane follows this. |
| Alarm delivery via note (signal-like) | Timer events enter the normal message batch via `state.batch` | Plan 9 notes preempted; pane timer events are regular messages processed sequentially. Less surprising, same eventual effect. |

## Clipboard

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `/dev/snarf` — per-rio-instance shared file, read returns contents, write sets them, no locking | `Clipboard` kit type with `ClipboardWriteLock` typestate (lock / commit / revert) | Plan 9's snarf was last-writer-wins with no concurrency control. pane adds transactional semantics. |
| Recursive rio uses parent's snarf buffer | Clipboard federation with `Locality` enum (Local / Remote / Federated) | When rio ran recursively, inner instances delegated to the parent's snarf. pane generalizes: remote instances can participate in clipboard federation, subject to policy. |

## Plumbing / Routing

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `plumber(4)` — central language-driven file server for inter-app messages | Kit-level routing rules evaluated locally in pane-app | Plan 9's plumber was a single process, a potential bottleneck and SPOF. pane evaluates routing rules in-process. |
| Plumb message format: text header + data, port-based dispatch | Typed `Message` enum with pattern matching, session-type channels | Plan 9 messages were text with `name=value` attributes. pane uses Rust enums with compile-time exhaustiveness checking. |

## Observer / Property Watching

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| No subscription / notification in 9P — blocking reads on state files (wctl, wait, mouse) | Dual path: blocking-read pane-fs files (scripting) + push `start_watching` (protocol) | Plan 9's blocking-read model requires one thread per observable per observer. Works for scripting, doesn't scale for reactive UI. pane provides both. |
| rio wctl: read blocks until window changes size, location, or state | `/pane/<id>/event` file blocks until pane state changes | Direct adaptation of wctl pattern for external tool integration. |
| plumber multicast: "A copy of each message is sent to each client that has the corresponding port open" | Kit-level routing: all interested receivers get dispatched copies | plumber(4) already did multicast. pane routing should preserve this. |
| plumber `click` attribute: cursor context refines regex match | (not yet designed — should be adopted) | The click attribute pattern lets the plumber narrow a text selection to the semantically relevant portion. pane routing rules should support content-refinement. |
| plumber BUGS: "file name space is fixed" | Kit-level routing evaluates in app's own namespace | Confirmed from `plumber(4)` BUGS. pane avoids this by design. |

## Death Notification / Process Monitoring

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `wait(2)` / `Waitmsg` — parent-child only, blocks until child exits | `ControlMessage::WatchPane` / `PaneExited` — any-to-any, server-mediated push | Plan 9 wait was parent-child; `/proc/<pid>/wait` allowed third-party but required one blocking thread per watched process. pane uses server-mediated push (BeOS BRoster::StartWatching model) via control channel. |
| `/proc/<pid>/wait` — blocking read, third-party, one thread per watched process | (future) pane-fs `/pane/<id>/wait` — blocking read file as filesystem-tier projection | Plan 9's blocking-read model is preserved as a pane-fs projection for scripting. The protocol tier (WatchPane) is primary. |
| No exit reason in /proc wait beyond text string | `ExitReason` enum (Graceful / Disconnected / Failed / InfraError / Unreachable) | Plan 9's Waitmsg.msg was a freeform text string. pane uses a typed enum. |
| No process death broadcast — must know the pid and block | `WatchPane { watch_id, target: Address }` — explicit subscription, one-shot delivery | Plan 9 had no "tell me when anything dies." pane follows the same principle: explicit per-target watches, no global broadcast. |

## Connection Resilience (v1 REMOVED — redesign pending)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `aan(8)` filter — always-available network, buffers messages during temporary disconnection | (planned) ReconnectingTransport wrapping TCP with reconnection + message buffering + replay | v1 prototype had a ReconnectingTransport impl; removed during redesign. **Divergence:** aan was a symmetric filter applied to both client and server via `import -p` / `exportfs`; pane's version will be client-side only initially. |

## Export / Namespace Filtering

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `exportfs -P patternfile` — regex `+` / `-` patterns on path names | `.plan` file governs what a remote observer can see | exportfs(4) confirms the regex pattern model. pane's `.plan` should use structured predicates (signature, type, sensitivity) rather than path regexes. |

## Diagnostic / Debugging Patterns (v1 REMOVED — redesign pending)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| `iostats` — transparent 9P proxy that monitors file operations | (planned) ProxyTransport wrapping any Transport impl | v1 prototype had a ProxyTransport; removed during redesign. |
| `exportfs -d` — log all 9P traffic to a debug file | (planned) `--protocol-trace <file>` flag on headless binary | Design: log handshake + active-phase messages to file. |

## Terminal / Window Architecture

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| rio `consctl` mode changes revert on close (lease pattern) | (not yet designed for pane-shell) | From rio(4): "Closing the file makes the window revert to default state." The fd acts as a lease — pane-shell should use a similar pattern. Rust's RAII makes this natural. |
| rio mounts per-window `/dev/cons` via namespace, shadowing kernel cons | pane-shell communicates via standard pane protocol channels | Plan 9 used per-process namespaces to multiplex /dev/cons per window. pane doesn't have kernel namespace support, but the session-typed protocol achieves the same multiplexing. |
| rio `text` file — read-only full window contents | pane-fs `/pane/<id>/body` — semantic content of pane | rio's text file was the complete scrollback buffer. pane-fs body should be the semantic content. |
| rio `wdir` — writable, app updates on chdir, used for plumb messages | pane-fs `/pane/<id>/attrs/cwd` — writable, app updates, used for routing | Direct adaptation. |
| rio `wsys` directory — per-window subdirectories | pane-fs `/pane/` hierarchy | rio(4) served a `wsys` directory with one subdirectory per window. pane-fs serves the same structure under `/pane/<id>/`. |

## Inter-Pane Addressing (Address + ServiceHandle.send_request)

| Plan 9 | pane | Rationale |
|--------|------|-----------|
| fid obtained by walk / open — name resolved once, then direct binding | `Address` — lightweight copyable address (pane_id + server_id). ServiceHandle<P> is the typed binding for protocol-scoped communication. | Same fid semantics: resolution is separate from communication. Address is sendable in messages ("here's how to reach me"). ServiceHandle is the live binding. |
| Kernel routes file ops, but direct client-to-client possible via shared namespaces | Direct pane-to-pane communication supported; server is one routing path, not the only one | Lane chose direct over server-mediated-only. Authorization for direct connections uses PeerAuth + .plan. |
| plumber(4) — separate file server for content-based routing, independent of direct file ops | Plumber-style routing kept separate from ServiceHandle.send_request — future routing service, not a messaging method | Plan 9's plumber was powerful because it was a separate service with its own rules. Conflating direct addressing and content-based routing in one API loses both clarity and the ability to evolve routing rules independently. |
| `import` made remote files transparent | Address to remote pane is API-identical to local (`Address::remote(id, server)`); on_failed may fire from partition | Plan 9 got transparency right but experienced users still knew /n/kremvax might hang. pane: same API, honest about timing. |

## What pane does NOT take from Plan 9

### Per-process namespaces → per-uid filtering + Landlock + optional 9P

Plan 9's `rfork(RFNAMEG)` gave each process its own kernel
mount table — per-process, not per-user. Inferno replicated
this in userspace. pane cannot achieve true per-process
namespaces on Linux without kernel support (mount namespaces
require CAP_SYS_ADMIN or user namespaces).

**Adopted model (three tiers):**

- **Protocol tier:** Per-connection ConnectionNamespace —
  visibility predicate per connection. Non-visible panes
  return "not found" (Inferno semantics).
- **Filesystem tier:** Per-uid FUSE filtering + Landlock
  enforcement. Coarser than per-connection but sufficient for
  scripting.
- **Composition tier (future):** 9P interface alongside FUSE
  for programmatic per-connection namespace composition.

**What's lost vs. Plan 9:** Client-side composition (bind /
mount in your own namespace), structural isolation without
identity (two same-uid processes seeing different namespaces),
recursive compositor nesting, remote namespace reconstruction
(`cpu` pattern). Acceptable losses.

### Recursive symmetry (8½/rio nesting)

8½ could run inside itself because the window manager was a
file server speaking the same interface as the kernel's `/dev`.
pane permits connecting a pane app to a different pane server,
but the FUSE namespace doesn't recurse without Linux mount
namespaces.

### import as kernel operation

Plan 9's import was a kernel mount of a remote file tree at an
arbitrary path. pane has one FUSE mount at `/pane/`; remote
panes appear there via the unified namespace. More structured,
less error-prone than arbitrary mounts, but not as flexible.

## Architectural checkpoint findings (2026-04-05)

### Critical execution gap: FUSE mount

The pane-fs design is sound but almost entirely unimplemented
at the FUSE level. AttrReader, AttrSet, PaneEntry exist; the
FUSE bridge does not. Every Plan 9 promise (scriptability,
observability, composition) depends on pane-fs being a real
mounted filesystem.

### Missing: blocking-read observer file

Plan 9 used blocking reads on state files (rio wctl, proc wait)
as the observer pattern at the filesystem tier. pane-fs has no
equivalent planned — needs an `event` or `wait` file per pane
that blocks until state changes.

### Dual-interface obligation

The kit API and filesystem must project the same state through
the same optics. MonadicLens enforces this by construction
(same fn pointer for read path and write path). Must be tested
explicitly.

### Staleness indicator for remote panes

Cached remote pane metadata can go stale when the remote host
disconnects. Need a visible staleness indicator: `/pane/<n>/attrs/connected`
returning connected / stale / unreachable.

### Bridge thread architecture

The bridge module spawns a thread per connection for par /
transport bridging. Works for handshake; for the active phase,
need either a non-blocking FrameCodec for calloop integration
or accept the extra thread + copy per message.
