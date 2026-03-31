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
| `factotum` — per-user auth agent | (not yet implemented) | Plan 9's auth model (per-user factotum, no root) informs pane's security design but implementation is deferred. |

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

## What pane does NOT take from Plan 9

| Concept | Why not |
|---------|---------|
| Everything-is-a-file for internal APIs | pane uses typed Rust APIs internally. The filesystem interface (pane-fs) is an external projection, not the internal communication mechanism. Typed channels are safer than byte-stream file operations for inter-component communication. |
| Kernel-mediated namespaces | pane runs in userspace. FUSE provides the namespace abstraction without kernel modifications. |
| rio-style window management | pane's compositor model descends from Be's app_server, not from rio's draw-device model. The per-pane threading model is Be; the namespace and distribution model is Plan 9. |
