# Linux Subsystem Research — Audio/Media, Init Systems, Kernel Interfaces, D-Bus

Research for pane spec-tightening. The Linux subsystems that pane builds on top of, examined from the perspective of a desktop environment that applies a unified OS design philosophy over the Linux base.

Sources: kernel documentation, man pages, project documentation, Arch Wiki, freedesktop.org specifications, skarnet.org (s6), smarden.org (runit), PipeWire docs, Haiku API reference (BeOS Media Kit).

---

## 1. Audio/Media Stack

### 1.1 ALSA — The Kernel Interface

ALSA (Advanced Linux Sound Architecture) is the kernel subsystem that provides sound card device drivers. It is not an audio server — it is the hardware abstraction layer between userspace and audio hardware.

**What ALSA provides:**

- PCM (Pulse Code Modulation) devices for digital audio playback and capture
- A control interface for mixer settings (volume, mute, routing)
- A sequencer interface for MIDI
- A timer interface for synchronization
- A userspace library (alsa-lib) that abstracts hardware differences behind a plugin architecture

**Architecture:** Each sound card can have multiple PCM devices, each with playback and capture streams, each with one or more substreams. The kernel driver exposes these as device files. alsa-lib's plugin system (dmix, softvol, dsnoop) provides software mixing, volume control, and other features that the hardware may lack.

**Why ALSA alone is insufficient for desktop use:**

1. **No mixing without dmix.** Most integrated sound cards (Intel HD Audio, the common case) lack hardware mixing. Without a sound server, only one application can use the audio device at a time. ALSA's dmix plugin provides software mixing but is limited — it doesn't support per-application volume control, format conversion is basic, and latency management is crude.

2. **No routing.** ALSA provides a static mapping from applications to hardware devices. Moving audio output from speakers to headphones (or Bluetooth) requires applications to reconfigure themselves. There is no central routing authority.

3. **No network audio.** ALSA is strictly local.

4. **No video.** ALSA handles audio only.

**What pane needs to know:** ALSA is the floor. Every audio path on Linux ultimately touches ALSA (or a DMA-BUF for hardware-decoded media). PipeWire sits on top of ALSA and provides everything ALSA lacks. Pane should never interact with ALSA directly — PipeWire is the interface.

### 1.2 PulseAudio — The Desktop Audio Server (Legacy)

PulseAudio is a sound server that runs as a per-user daemon, accepting audio from applications and routing it to output devices via ALSA. It was the standard desktop audio server from roughly 2008 to 2022.

**Architecture:** Client-server model over Unix sockets. Applications link against libpulse and connect to the PulseAudio daemon. The daemon maintains:

- **Sources** (capture devices) and **sinks** (playback devices)
- Per-application volume control and routing
- Automatic device switching (plug in headphones, audio moves)
- Network transparency (stream audio to/from remote machines)
- Format conversion (sample rate, channel count, bit depth)
- A module/plugin system for extending functionality

**What PulseAudio solved:** Multiple applications sharing one sound card. Per-application volume. Automatic device switching. Network audio. These are all things a desktop user expects. ALSA alone provides none of them.

**What PulseAudio lacks:**

1. **No professional audio support.** PulseAudio's latency model (timer-based scheduling) is designed for desktop use, not real-time audio. It cannot provide the sub-millisecond latencies that professional audio (JACK) requires.
2. **No video.** Audio only.
3. **No graph-based routing.** PulseAudio's routing model is source-to-sink, not an arbitrary node graph. Complex routing (send one app's audio through an equalizer, then to speakers, while also recording a mix of two apps) requires workarounds.

**What pane needs to know:** PulseAudio is the system that PipeWire replaces. PipeWire provides a PulseAudio-compatible server and ABI-compatible client libraries, so applications written for PulseAudio work without modification on PipeWire. Pane does not need to think about PulseAudio directly, but needs to understand that many applications will speak the PulseAudio protocol and PipeWire's compatibility layer handles this.

### 1.3 JACK — Professional Audio

JACK (JACK Audio Connection Kit) is a sound server designed for professional audio work: real-time, low-latency audio and MIDI routing between applications.

**Architecture:** A graph-based model:

- **Clients** register with the JACK server and create **ports** (audio or MIDI, input or output)
- **Connections** link output ports to input ports, forming an arbitrary directed graph
- The server processes the graph synchronously — all clients in the graph are called in topological order once per audio period
- Fixed buffer sizes and sample rates ensure deterministic latency
- Real-time scheduling (SCHED_FIFO) keeps the audio thread from being preempted

**What JACK provides that PulseAudio doesn't:** Arbitrary graph-based routing. Sub-millisecond latency. Synchronous processing guarantees. MIDI alongside audio. This is the infrastructure that makes digital audio workstations, synthesizers, and effects processors work together.

**What JACK lacks for desktop use:** JACK is exclusive — when JACK owns the sound card, PulseAudio cannot use it (and vice versa, without bridging). JACK doesn't do automatic device switching, per-application volume, or any of the desktop niceties. It's pro-audio infrastructure, not a desktop audio server.

**What pane needs to know:** JACK's graph-based model is the right model for media processing. PipeWire adopts this model and unifies it with desktop audio. PipeWire provides JACK-compatible client libraries (pw-jack), so JACK applications work on PipeWire without modification.

### 1.4 PipeWire — The Unification

PipeWire is the modern multimedia framework that replaces both PulseAudio (desktop audio) and JACK (professional audio) while adding video handling. It is now the standard on major distributions.

**Core architecture:** A graph-based processing framework.

- **Nodes** are processing elements that consume and/or produce buffers. A node can be a hardware device, an application stream, a filter, or a virtual device. Nodes live either in the PipeWire daemon or in client processes.
- **Ports** are directional endpoints on nodes — input or output. A node with only output ports is a source; only input ports, a sink; both, a filter.
- **Links** connect output ports to input ports, forming the processing graph.
- **Devices** are abstractions over hardware (ALSA cards, V4L2 cameras, Bluetooth adapters) that produce nodes.

**The daemon vs. the session manager:** PipeWire deliberately separates mechanism from policy.

The **PipeWire daemon** (pipewire) provides: the graph execution engine, buffer management, format negotiation, the protocol for clients to create/destroy/link nodes, and the low-level media transport. It does NOT decide which nodes to connect, which devices to open, or what routing policy to apply.

The **session manager** (WirePlumber) provides: all policy decisions. It monitors for new devices and creates nodes for them. It watches for new application streams and links them to appropriate devices. It handles default sink/source selection, device configuration, access control, and routing policy. WirePlumber is implemented as a modular Lua-scripted framework built on GObject.

This separation is architecturally significant: it means a desktop environment can influence PipeWire's behavior through the session manager layer without touching the media transport. Custom routing policy, device prioritization, and access control are all session manager concerns.

**Port configuration modes:**

- **DSP mode:** One port per audio channel, 32-bit floating-point. This is the native processing format — all mixing and effects happen in float32.
- **Passthrough mode:** A single multichannel port with the negotiated format (could be compressed audio, raw PCM at any sample rate, etc.).

**Video handling:** PipeWire handles video alongside audio through the same node/port/link model. The primary use cases:

1. **Screen capture:** The Wayland compositor streams screen content as PipeWire video nodes. Applications (OBS, browsers for WebRTC) connect to these nodes. Access is mediated through xdg-desktop-portal — the compositor provides a D-Bus interface (org.freedesktop.portal.ScreenCast), the portal asks the user for permission, and on approval a PipeWire stream is established. Video data flows as either memory-mapped pixel buffers or DMA-BUF file descriptors (zero-copy from GPU memory).

2. **Camera access:** V4L2 camera devices are exposed as PipeWire video source nodes, again mediated through the portal for permission.

This means pane-comp (the compositor) needs to implement the xdg-desktop-portal ScreenCast interface to enable screen sharing. The actual video transport is PipeWire's job — the compositor just needs to feed frame data into a PipeWire node.

**Compatibility layers:**

- **PulseAudio:** pipewire-pulse provides a drop-in PulseAudio server. Applications using libpulse connect to PipeWire transparently.
- **JACK:** pw-jack sets LD_LIBRARY_PATH so JACK applications load PipeWire's reimplementation of JACK client libraries.
- **ALSA:** An ALSA plugin routes ALSA-only applications through PipeWire.

**Configuration:** Hierarchical — package defaults in /usr/share/pipewire/, system overrides in /etc/pipewire/, user overrides in ~/.config/pipewire/. Package updates don't clobber user customizations.

**Network audio:** PipeWire supports RTP, AES67, Apple AirPlay, JACK network (netjack2), and Roc streaming protocols. Distributed audio systems are first-class.

### 1.5 BeOS Media Kit vs. PipeWire — The Comparison

BeOS's Media Kit and PipeWire share the same fundamental architecture: a graph of processing nodes connected by typed ports, with a central roster/manager that brokers connections. The comparison is illuminating for pane's "media kit" design.

**BeOS Media Kit architecture:**

- **BMediaNode** is the base class for all processing elements. Four primary subclasses: BBufferProducer (outputs buffers), BBufferConsumer (receives buffers), BTimeSource (provides timing), BControllable (exposes parameters).
- Nodes can multiply-inherit — a sound card is simultaneously a BBufferConsumer (playback), BTimeSource (hardware clock), and BControllable (volume/mute).
- **BMediaRoster** is the application interface: node discovery, connection establishment, playback control, time source coordination. Applications never interact with BMediaNode directly — they go through the roster.
- **Buffers** (BBuffer) carry media data between nodes via shared memory. Each buffer has a performance timestamp, media-type-specific metadata, and size information.
- **Format negotiation** uses media_format structures with wildcards — "I don't care about sample rate" lets the system pick the optimal format.
- **Latency tracking:** Four latency types (algorithmic, processing, scheduling, downstream). Consumers report latency upstream; producers adjust timestamps accordingly. Late buffers trigger LateNoticeReceived() callbacks.
- **Three time concepts:** Media time (position in content), real time (system clock), performance time (scheduled output time from a BTimeSource). Synchronization between audio and video, even across different applications, is achieved through shared time sources.
- **Run modes:** B_OFFLINE (process as fast as possible), B_RECORDING (never drop buffers), B_INCREASE_LATENCY / B_DROP_DATA (trade-offs for live playback).

**Structural parallels with PipeWire:**

| Concept | BeOS Media Kit | PipeWire |
|---------|---------------|----------|
| Processing element | BMediaNode | Node |
| Data endpoint | media_source / media_destination | Port |
| Connection | BMediaRoster::Connect() | Link |
| Hardware abstraction | BMediaAddOn (loaded by media_addon_server) | Device → Node (via ALSA monitor) |
| Central broker | BMediaRoster | Session manager (WirePlumber) |
| Buffer transport | Shared memory (BBuffer) | Shared memory / DMA-BUF |
| Format negotiation | media_format with wildcards | Format negotiation with preferred/allowed lists |
| Latency management | Explicit upstream/downstream reporting | Quantum-based scheduling with latency compensation |
| Time sources | BTimeSource (per-node) | Clock (per-node) |
| Parameter control | BControllable + BParameterWeb | Properties + metadata |

**Where they diverge:**

1. **Scope.** The Media Kit was audio/video only. PipeWire also handles screen capture, cameras, and MIDI, and bridges PulseAudio/JACK/ALSA ecosystems.

2. **Policy separation.** In BeOS, BMediaRoster handled both mechanism (connecting nodes) and policy (default routing). PipeWire separates these — the daemon is mechanism, the session manager is policy. This is a cleaner architecture.

3. **In-process vs. out-of-process.** Media Kit nodes could run in-process (the application's address space) or out-of-process (in the media_addon_server). PipeWire nodes similarly run either in the daemon or in client processes, with the PipeWire protocol handling cross-process buffer sharing.

4. **Security model.** BeOS had no media access control. PipeWire uses a portal-based permission model — applications request screen/camera access through xdg-desktop-portal, and the compositor/session manager grants or denies.

### 1.6 What a "Pane Media Kit" Abstraction Looks Like

Pane draws from BeOS's Media Kit philosophy: media as first-class, building for the hardest case (real-time). PipeWire is the natural substrate because it already implements the Media Kit's graph-based model at the system level.

A pane media kit would be a **thin Rust crate** (pane-media) that:

1. **Wraps PipeWire's client API** with pane's session-typed protocol conventions. Creating a media node, connecting ports, and negotiating formats would speak pane's typed message protocol internally while mapping to PipeWire operations.

2. **Exposes the node graph as pane-visible state.** Media nodes and their connections would be inspectable via pane-fs (the FUSE interface), queryable via pane-store (indexed attributes on media nodes), and routable via pane-route (content-type-aware dispatch of media-related actions).

3. **Provides parameter control as pane configuration.** BControllable's BParameterWeb (a tree of sliders, checkboxes, and selectors for controlling a media node) maps to pane's filesystem-based configuration pattern — each parameter is a file, metadata in xattrs, changes watched by pane-notify.

4. **Integrates screen capture with pane-comp.** The compositor implements the xdg-desktop-portal ScreenCast D-Bus interface. When a screen share is approved, pane-comp feeds frame data into a PipeWire video source node. The actual encoding and transport is PipeWire's job.

5. **Delegates policy to PipeWire's session manager.** Pane does not need its own media routing policy — WirePlumber handles this. What pane provides is the UI surface for _exposing_ routing decisions to the user: a pane that shows the node graph, lets users drag connections, adjust parameters. The actual routing is PipeWire's; the visibility and control is pane's.

The key insight from BeOS: the Media Kit succeeded because it was a **system service**, not an application library. Any application could publish itself as a media node and immediately participate in the system's media graph. PipeWire already provides this. Pane's job is to make it visible, controllable, and integrated with pane's interaction model (tag lines, routing, filesystem exposure).

---

## 2. Init Systems

Pane defines contractual guarantees it needs from the init system and pane-init maps these to the concrete init system. The three candidates are systemd, s6, and runit.

### 2.1 systemd

systemd is PID 1 on most Linux distributions. It is a service manager, init system, and a sprawling collection of system management tools.

**Service files:** Declarative INI-format unit files. A service unit specifies: the binary to run (ExecStart), how to restart it (Restart=on-failure), dependencies (After=, Requires=, Wants=), resource limits (via cgroups integration), environment variables, user/group, and capabilities. Unit files live in /etc/systemd/system/ (admin), /usr/lib/systemd/system/ (package), and ~/.config/systemd/user/ (user session).

**Socket activation:** systemd can listen on sockets on behalf of services and pass the listening socket as a file descriptor when the service starts. The mechanism: systemd creates the socket, sets environment variables LISTEN_FDS and LISTEN_PID, and passes file descriptors 3+ to the spawned process. The service calls sd_listen_fds() to discover its sockets. Benefits:

- Services start on-demand (first connection triggers launch)
- Parallel boot (all sockets created immediately, services start when ready)
- Zero-downtime restarts (systemd holds the socket, new instance picks it up)
- Privilege separation (systemd binds privileged ports as root, service runs unprivileged)

This is directly analogous to Haiku's launch_daemon pre-creating ports before starting services. The insight is the same: create communication endpoints first, let services start in any order, messages queue until the service is ready.

**Dependency ordering:** Units declare ordering (After=/Before=) and requirement (Requires=/Wants=/BindsTo=) relationships. systemd builds a transaction graph, checks for cycles, and activates units in dependency order. The dependency graph is stored in hashmaps with O(1) lookup.

**Cgroups integration:** Every service gets its own cgroup. This provides: resource accounting (CPU, memory, I/O per service), resource limits (MemoryMax=, CPUQuota=), and reliable process tracking (all descendants of a service are in its cgroup, so killing a service kills all its children — no orphan processes).

**journald:** Structured logging. Services write to stdout/stderr; journald captures it with metadata (unit name, PID, timestamp, boot ID, priority). Logs are binary and queryable (journalctl -u pane-comp.service --since "5 minutes ago"). Log data is indexed.

**Readiness notification:** Services signal readiness by calling sd_notify(0, "READY=1") or writing to $NOTIFY_SOCKET. systemd marks the unit as active only after this notification. This enables dependent services to wait for actual readiness, not just process start.

**What systemd provides that pane-init needs:**

| Pane contract | systemd mechanism |
|--------------|-------------------|
| Restart on crash | Restart=on-failure |
| Readiness notification | sd_notify / Type=notify |
| Dependency ordering | After= / Requires= |
| Process isolation | cgroups (resource limits, process tracking) |
| Logging | journald (structured, queryable) |
| On-demand activation | Socket activation |

**What systemd costs:** It is an enormous dependency. It is opinionated about how the system is organized. It absorbs functionality that might otherwise be independent (DNS resolution, network configuration, login management, device management). For a project that values modularity and clear interfaces, systemd's scope is a tension — pane uses it, but does not want to depend on systemd-specific features that don't have equivalents in other init systems.

### 2.2 s6

s6 (skarnet's small supervision suite) is a process supervision system built on Unix philosophy principles: small programs, clear interfaces, composable tools.

**Supervision tree:** Two levels of supervision:

- **s6-svscan** is "a supervisor for the supervisors." It watches a scan directory containing service directories, and for each one, spawns an s6-supervise instance. s6-svscan can itself run as PID 1 (via s6-linux-init) or be supervised by another init.
- **s6-supervise** is the direct supervisor of a single daemon. It spawns the daemon, monitors it, restarts it on crash. Because the daemon is s6-supervise's direct child, the PID is always known — no .pid file race conditions.

**Service definition:** A service directory contains:

- `./run` — an executable that execs the daemon in the foreground (never backgrounds)
- `./finish` — optional, runs on daemon exit (cleanup)
- `./log/run` — optional, a dedicated logging process (the daemon's stdout is piped to the logger's stdin)
- `./notification-fd` — a file containing the file descriptor number the daemon will use for readiness notification

**Readiness notification protocol:** The daemon writes a newline character to a file descriptor of its choice. s6-supervise picks up this notification and broadcasts it to waiting processes. This is simpler than systemd's sd_notify — it requires no library, just a write() call. Three consumer programs:

- **s6-svwait** — waits for one or more services to reach a state (up, down, ready)
- **s6-svlisten1** — waits for a single service, with race avoidance (subscribes before checking, so no window for missed events)
- **s6-svlisten** — waits for multiple services simultaneously

**Notification broadcasting:** Uses the fifodir mechanism (s6-ftrig-* family) — filesystem-based publish-subscribe between unrelated processes. A fifodir is a directory containing named pipes, one per subscriber. The publisher writes to all pipes; each subscriber reads from its own.

**Control interface:** s6-svc sends control commands to a running s6-supervise (start, stop, restart, send signal). s6-svscanctl controls the entire supervision tree. All control is through files in the service directory — no daemon-specific protocol, no special IPC.

**Bernstein chaining:** Service run scripts compose behavior through execline-style chaining: each program adjusts one aspect of the process environment (drop privileges, set environment, change directory, set resource limits) then execs the next program. The final exec is the daemon itself. This is composable and transparent — the full startup sequence is visible in the run script.

**s6-fdholder:** A program that holds open file descriptors across process restarts. When a daemon restarts, it can retrieve its listening sockets from the fdholder rather than re-creating them. This provides the same zero-downtime restart capability as systemd's socket activation, but via an independent, composable mechanism.

### 2.3 s6-rc — Dependency Management

s6-rc is a separate tool that provides dependency management on top of s6's supervision. The key architectural decision: **supervision and dependency management are separate concerns.**

- s6 handles: keeping daemons alive, restarting them on crash, managing their lifecycle
- s6-rc handles: starting services in the right order at boot, bringing sets of services up/down atomically

**Two service types:**

- **Longruns** — daemons supervised by s6. s6-rc delegates their lifecycle to the supervision tree.
- **Oneshots** — scripts that run once (mount filesystems, create directories, initialize hardware). s6-rc runs them directly.

This dual model solves the "udevd problem" — where a daemon must start before one-time initialization tasks can proceed.

**Offline compilation:** s6-rc compiles service definitions into a binary database at build time using s6-rc-compile. Dependency analysis, cycle detection, and topological sorting happen offline — not at boot. This eliminates the computational overhead of dependency resolution from the boot path. No other service manager does this.

**Readiness-aware dependency ordering:** When service A depends on service B, s6-rc waits for B's readiness notification (not just process start) before starting A. This is correct dependency ordering — "B is ready to serve" not just "B's process exists."

### 2.4 runit

runit is a simpler supervision system with a three-stage init model.

**Three stages:**

- **Stage 1** (/etc/runit/1): One-time system initialization. Runs to completion, has full control of /dev/console for emergency shells.
- **Stage 2** (/etc/runit/2): Runs runsvdir, which supervises all services. Should not return until shutdown. If it crashes, runit restarts it.
- **Stage 3** (/etc/runit/3): Shutdown tasks. Runs when the system is halting or rebooting.

**Service supervision:** runsvdir scans a directory of service directories. For each, it spawns runsv (analogous to s6-supervise), which manages the daemon and an optional logging process. The service directory structure:

- `./run` — the daemon startup script (must run in foreground)
- `./finish` — optional, runs on daemon exit
- `./check` — optional, health check script
- `./log/run` — dedicated logging process
- `./conf` — environment variables

**Control:** sv sends commands to runsv (start, stop, restart, status). Status is communicated through files in the supervise/ subdirectory.

**What runit lacks compared to s6:**

- No readiness notification protocol. runit knows when a process is running but not when it's ready to serve. Dependency ordering must rely on timing or polling.
- No dependency management tool equivalent to s6-rc.
- No fifodir-based notification broadcasting.
- No fdholder for file descriptor persistence.

**What runit provides:** Simplicity. The entire system is straightforward to understand. For services that don't need readiness-aware ordering, runit works well.

### 2.5 Comparison: Contractual Guarantees

| Contract | systemd | s6 + s6-rc | runit |
|----------|---------|-----------|-------|
| **Restart on crash** | Restart=on-failure | s6-supervise (automatic) | runsv (automatic) |
| **Readiness notification** | sd_notify (READY=1) | write newline to fd | Not supported |
| **Dependency ordering** | After= / Requires= | s6-rc compiled database | Not supported (ad hoc) |
| **Logging** | journald (structured) | s6-log (per-service, rotated) | svlogd (per-service, rotated) |
| **Process isolation** | cgroups | Not built-in (use cgroups externally) | Not built-in |
| **Socket holding** | Socket activation (built-in) | s6-fdholder (composable) | Not supported |
| **On-demand activation** | Socket units | s6-ipcserverd + fdholder | Not supported |
| **Boot parallelism** | Dependency graph + socket activation | s6-rc compiled deps + readiness | runsvdir starts all simultaneously |

### 2.6 The pane-init Mapping

pane-init defines contracts and maps them to the concrete init system. The contracts pane needs:

1. **Supervised restart.** When a pane server (pane-comp, pane-route, pane-roster, pane-store) crashes, the init system restarts it. The restarted server re-registers with pane-roster. This is universally supported.

2. **Readiness notification.** A pane server signals when it's ready to accept connections — not just when its process starts, but when initialization is complete and the server is listening. systemd and s6 support this natively. For runit, pane-init would need to implement its own readiness protocol (write to a pipe, poll a health endpoint).

3. **Dependency ordering.** pane-comp should start before pane-shell. pane-route should start before applications that route. The ordering is shallow — pane has ~5-8 infrastructure servers with simple dependency relationships. systemd and s6-rc handle this natively. For runit, pane-init would use the readiness protocol to block dependent services.

4. **Logging.** Each pane server's stdout/stderr should be captured and available for debugging. All three systems provide per-service logging.

5. **Process tracking.** When pane-init needs to know "is pane-store alive?", it should get a definitive answer. systemd's cgroups provide this. s6-supervise always knows its child's PID. runit's runsv likewise.

**The mapping for each init system:**

**systemd:** Each pane server is a systemd service unit. Type=notify for readiness. After= for ordering. Restart=on-failure. WantedBy=pane.target for grouping. Socket activation for pane-comp's Wayland socket. This is the most feature-complete mapping.

**s6 + s6-rc:** Each pane server is a longrun with a run script, a notification-fd file, and s6-rc dependency declarations. s6-fdholder can hold the Wayland listening socket across compositor restarts. The readiness protocol is a single write() — trivial to implement.

**runit:** Each pane server is a service directory with a run script. pane-init adds a readiness layer: each server writes to a notification pipe when ready; pane-init's startup sequence checks these pipes before starting dependent servers. This is the most work but still tractable because pane's dependency graph is small.

The key insight from Haiku's launch_daemon: pre-create communication endpoints. For all three init systems, pane-init should create the Unix sockets that pane servers listen on before starting the servers. This way, clients can connect (and have their messages queued) even before the server is fully initialized. systemd's socket activation does exactly this. For s6, s6-fdholder achieves it. For runit, pane-init creates the sockets itself and passes them via environment variables or file descriptors.

---

## 3. Kernel Interfaces

### 3.1 fanotify — Filesystem-Wide Event Notification

fanotify provides filesystem event monitoring at three scopes: per-inode, per-mount, and per-filesystem.

**Architecture:** An application creates a fanotify notification group (fanotify_init), adds marks to the group (fanotify_mark) specifying what to watch and what events to receive, then reads events from the group's file descriptor.

**Mark scopes:**

- **FAN_MARK_INODE:** Watch a specific file or directory (by inode). Events fire for that specific object.
- **FAN_MARK_MOUNT:** Watch an entire mount point. Events fire for any object on that mount.
- **FAN_MARK_FILESYSTEM:** Watch an entire filesystem across all its mount instances. One mark covers everything.

**Event types:** FAN_ACCESS, FAN_MODIFY, FAN_OPEN, FAN_CLOSE_WRITE, FAN_CLOSE_NOWRITE, FAN_CREATE, FAN_DELETE, FAN_MOVE (FAN_MOVED_FROM | FAN_MOVED_TO), FAN_ATTRIB (metadata changes including xattr modifications), FAN_DELETE_SELF, FAN_MOVE_SELF, FAN_FS_ERROR.

**Permission events:** FAN_ACCESS_PERM, FAN_OPEN_PERM, FAN_OPEN_EXEC_PERM. These block the requesting process until the fanotify listener responds with FAN_ALLOW or FAN_DENY. This enables content scanning (antivirus), hierarchical storage management, and access control policy enforcement.

**FID (File Identifier) reporting:** Instead of providing file descriptors for each event (which is expensive and doesn't scale), FAN_REPORT_FID mode provides file handles (struct file_handle) and directory + name information (FAN_REPORT_DFID_NAME). This is essential for mount-wide and filesystem-wide watches where events can come from millions of files.

**Capability requirement:** fanotify requires CAP_SYS_ADMIN. Unprivileged users can only mark inodes. FAN_MARK_MOUNT and FAN_MARK_FILESYSTEM are privileged operations. This means pane-store (which uses FAN_MARK_FILESYSTEM for bulk xattr tracking) must run with elevated capabilities or as a privileged service.

**Dual mask system:** Each mark has a mark mask (what generates events) and an ignore mask (what suppresses events). This enables sophisticated filtering — e.g., cache invalidation: watch for FAN_MODIFY, and when the cache is invalidated, add FAN_MODIFY to the ignore mask until the next FAN_CLOSE_WRITE, avoiding redundant events during large writes.

**What pane uses fanotify for:** pane-store uses FAN_MARK_FILESYSTEM to watch for xattr changes (FAN_ATTRIB) across the entire filesystem. When any file's extended attributes change, pane-store receives the event and updates its in-memory index. One mark covers the entire filesystem — no recursive directory walking, no per-directory watch limits.

### 3.2 inotify — Per-Directory Watches

inotify provides per-file and per-directory event notification without privilege requirements.

**Architecture:** An application creates an inotify instance (inotify_init), adds watches (inotify_add_watch) on specific files or directories, and reads events from the instance's file descriptor. Each watch consumes a kernel-side data structure (~1,080 bytes on 64-bit systems).

**Limitations:**

1. **No recursive watching.** Watching a directory does not watch its subdirectories. Recursive watching requires adding a watch on every subdirectory, which is expensive for deep hierarchies.
2. **Watch limit.** max_user_watches defaults to 8,192 (kernel >= 5.11 auto-adjusts up to 1,048,576 based on available memory). Each watch costs kernel memory.
3. **No filesystem-wide scope.** You watch specific paths, not "everything on this filesystem."
4. **Race conditions with directory creation.** Between creating a directory and adding a watch on it, events can be missed. Robust recursive watching is surprisingly hard.
5. **Events carry filenames, not file handles.** If a file is renamed between the event and processing, the name is stale.

**What inotify is good for:** Watching specific directories — config directories, plugin directories, well-known paths. This is exactly what pane needs for targeted watches on /etc/pane/route/rules/, ~/.config/pane/translators/, and other well-known directories.

**How pane-notify combines both:** fanotify for mount-wide/filesystem-wide coverage (pane-store's bulk attribute tracking). inotify for targeted directory watches (config directories, plugin directories). The consumer requests a watch by scope; pane-notify selects the appropriate kernel interface.

### 3.3 xattrs — Extended Attributes

Extended attributes are name-value pairs attached to files, stored by the filesystem alongside the file's data and standard metadata.

**Namespaces:** Linux xattrs are partitioned into four namespaces:

- **user.*** — No restrictions on naming or content. Any process that can read/write the file can read/write its user xattrs. This is the namespace pane uses (user.pane.*).
- **trusted.*** — Only accessible to processes with CAP_SYS_ADMIN.
- **security.*** — Used by SELinux, AppArmor, and other LSMs.
- **system.*** — Used by the kernel (e.g., system.posix_acl_access for POSIX ACLs).

**Filesystem support and limitations:**

| Filesystem | xattr support | Size limit | Scalability |
|-----------|---------------|------------|-------------|
| **ext4** | Yes | Total names + values must fit in one filesystem block (1KB-4KB depending on block size) | Poor for heavy xattr use |
| **btrfs** | Yes | No practical limit | Excellent — scalable storage algorithms |
| **XFS** | Yes | No practical limit | Excellent |
| **bcachefs** | Yes | No practical limit | Excellent |
| **tmpfs** | Yes | Limited by memory | N/A (RAM-backed) |

The ext4 limitation is significant: if pane stores multiple attributes per file (user.pane.type, user.pane.description, user.pane.icon, etc.), the total must fit in a single block. With 4KB blocks this is usually adequate but can be tight. btrfs, XFS, and bcachefs have no such limitation.

**Performance:** xattr reads are fast — they're stored inline with the inode (for small values) or in a single additional block. There is no indexing on Linux — unlike BFS, which maintained B+ tree indices over attribute values. pane-store provides this indexing in userspace, reading xattrs and maintaining an in-memory index.

**What pane uses xattrs for:** File metadata in the user.pane.* namespace: content type (user.pane.type), plugin metadata (user.pane.plugin.type, user.pane.plugin.handles), config metadata (user.pane.description, user.pane.range). This is pane's equivalent of BFS attributes — but without filesystem-level indexing (pane-store provides that) and without the query language (pane-store provides that too).

**The gap between BFS and Linux xattrs:** BFS attributes were typed (B_STRING_TYPE, B_INT32_TYPE, etc.) and the filesystem understood the types. Linux xattrs are opaque byte blobs — the filesystem doesn't know or care about the type. pane-store must encode type information in the attribute name or value (e.g., a type prefix, or a companion user.pane.type xattr). BFS had filesystem-level indexing and query execution. pane-store provides these in userspace, which means queries are slower (no kernel-level B+ tree) but more flexible (pane-store can index any attribute without requiring a filesystem-level mkindex).

### 3.4 memfd — Anonymous File Descriptors

memfd_create() creates a file descriptor backed by anonymous memory — no filesystem path, no backing file on disk.

**What it provides:**

- A file descriptor that can be passed between processes via Unix socket fd-passing (sendmsg with SCM_RIGHTS)
- Memory that can be mmap'd by both sender and receiver for zero-copy shared memory
- Optional sealing (F_SEAL_SHRINK, F_SEAL_GROW, F_SEAL_WRITE, F_SEAL_SEAL) that prevents modification after sharing — the receiver can trust the content won't change
- No filesystem access needed — works in sandboxed environments

**How Wayland uses it:** The wl_shm (Wayland shared memory) protocol uses memfd_create for buffer sharing:

1. Client calls memfd_create() to get an anonymous file descriptor
2. Client mmap's it and writes pixel data
3. Client passes the fd to the compositor over the Wayland socket
4. Compositor mmap's the same memory and reads the pixels
5. No copy — both processes see the same memory

This is the fallback buffer sharing mechanism. The preferred mechanism for GPU-rendered content is DMA-BUF (see below), which shares GPU memory directly without going through system RAM.

**What pane uses memfd for:** Buffer sharing between pane-native clients and pane-comp for cell grid content and widget rendering. Also used for any shared memory needs between pane servers (e.g., shared state that needs zero-copy access). The sealing mechanism is particularly useful — a client that seals a buffer after writing guarantees the compositor sees exactly what was written.

### 3.5 pidfd — Process File Descriptors

pidfd provides a file-descriptor-based handle to a process, solving the PID reuse race condition that has plagued Unix process management since the beginning.

**The problem:** PIDs are recycled. If process A wants to send a signal to process B (PID 42), but B exits and PID 42 is reassigned to process C between A checking B's existence and sending the signal, A kills the wrong process. This is not theoretical — it's a real class of bugs in process managers.

**The solution:** pidfd_open(pid) returns a file descriptor that refers to a specific process. The kernel maintains the reference. If the process exits, the pidfd still refers to the now-zombie process — it doesn't suddenly point to a different process.

**What pidfd provides:**

- **pidfd_open(pid):** Create a pidfd for an existing process.
- **pidfd_send_signal(pidfd, sig, ...):** Send a signal to the process referred to by the pidfd. Race-free.
- **waitid(P_PIDFD, pidfd, ...):** Wait for the specific process to change state.
- **poll/epoll on pidfd:** The pidfd becomes readable (EPOLLIN) when the process exits. This integrates process lifecycle monitoring into the event loop.

**What pane uses pidfd for:** pane-roster tracks running applications and infrastructure servers. Using pidfds instead of PIDs means pane-roster can monitor process lifecycle through epoll (the same event loop mechanism used for everything else) and send signals without race conditions. When pane-roster needs to determine "is this the same pane-store I registered, or a replacement?", the pidfd provides a definitive answer.

### 3.6 seccomp — Syscall Filtering

seccomp-BPF (Secure Computing with Berkeley Packet Filter) allows a process to install a filter that inspects every syscall it makes and decides whether to allow it.

**Architecture:** The process calls prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER) or seccomp(SECCOMP_SET_MODE_FILTER, ...) with a BPF program. The BPF program examines the syscall number and (with limitations) the arguments, and returns a verdict:

- **SECCOMP_RET_ALLOW:** Permit the syscall.
- **SECCOMP_RET_ERRNO:** Block the syscall and return an error.
- **SECCOMP_RET_TRAP:** Deliver SIGSYS to the process (for custom handling).
- **SECCOMP_RET_KILL:** Kill the process immediately.
- **SECCOMP_RET_LOG:** Allow but log.
- **SECCOMP_RET_TRACE:** Notify a ptrace tracer.

Verdicts are checked in priority order: KILL > TRAP > ERRNO > TRACE > LOG > ALLOW.

**Important limitation:** seccomp-BPF filters are inherited by child processes and cannot be removed once installed. They can only be made more restrictive. This is by design — a sandboxed process cannot unsandbox itself.

**What pane uses seccomp for:** Sandboxing pane servers and pane-native applications. A pane server that only needs to read/write files, communicate over Unix sockets, and manage memory can have its syscall surface reduced dramatically. If pane-store is compromised, seccomp prevents it from executing arbitrary binaries, opening network sockets, or performing privileged operations. The filter is installed after initialization (when the server has opened all the file descriptors it needs) and restricts the runtime to the minimum syscall set.

**seccomp is not a complete sandbox.** It filters syscalls but doesn't restrict filesystem access, network endpoints, or IPC. For a complete sandbox, combine seccomp with namespaces and filesystem restrictions.

### 3.7 Mount Namespaces and User Namespaces

**Mount namespaces** give a process its own view of the filesystem mount table. Changes to mounts inside the namespace don't affect other namespaces.

**User namespaces** provide UID/GID mapping — a process can appear to be root inside its namespace while being an unprivileged user outside. This enables unprivileged creation of other namespace types (mount, network, PID, etc.).

**What they provide together:**

- A process can create a user namespace (unprivileged)
- Inside that user namespace, it can create a mount namespace
- Inside the mount namespace, it can mount tmpfs, bind-mount specific directories, and create an isolated filesystem view
- The process appears to be root inside its namespace but has no special privileges on the host

This is how Flatpak, bubblewrap, and other desktop sandboxing tools work. bubblewrap creates a completely empty mount namespace with a tmpfs root, then selectively bind-mounts only the directories the application needs.

**Overhead:** Namespaces are lightweight compared to virtual machines — all processes share the same kernel. The overhead is primarily in the kernel's namespace bookkeeping (mount propagation, UID mapping lookups). For desktop use, the overhead is negligible.

**Comparison to Plan 9 namespaces:** Plan 9's per-process namespaces were the _primary composition mechanism_ — rio mounted itself onto /dev in each window's namespace, the plumber was mounted at /mnt/plumb, remote resources were mounted anywhere. The namespace was the capability set, the configuration, and the environment.

Linux namespaces are "a poor man's Plan 9 namespaces" (Yotam Kolko). Specific limitations:

1. **Heavyweight creation.** Plan 9's rfork(RFNAMEG) gives a process a copy of its parent's namespace trivially. Linux's clone(CLONE_NEWNS) requires either root or a user namespace chain. The overhead is higher, the API is more complex.
2. **No per-process mount without privilege.** Mounting a FUSE filesystem or bind-mounting a directory requires either root or a user namespace. Plan 9's mount() was unprivileged by default.
3. **No union directories.** Plan 9's bind with MBEFORE/MAFTER creates union directories — a directory that shows files from multiple sources. Linux has overlayfs but it's a full filesystem, not a per-directory bind.
4. **No single protocol behind the namespace.** Plan 9's namespace was composed of 9P servers. Any resource could be mounted because every resource spoke 9P. Linux mounts are filesystem-specific — you can't mount a Unix socket at a path without FUSE.

**What pane uses namespaces for:** Sandboxing legacy applications and potentially pane servers. A legacy Wayland application wrapped in a pane can run in a restricted mount namespace that only exposes the files it needs. pane-fs (the FUSE filesystem) can be mounted in the application's namespace without affecting the host. User namespaces enable this without root.

The deep aspiration — using namespaces as Plan 9 uses them, as the primary composition mechanism — is not practical on Linux. The privilege requirements, the heavyweight creation, and the lack of a universal mount protocol prevent it. Pane achieves the _effect_ of per-pane namespaces (isolated state, private communication channels) through typed protocols and per-pane server-side state, not through kernel namespace manipulation.

### 3.8 io_uring — Completion-Based Async I/O

io_uring is a Linux kernel interface for high-performance asynchronous I/O, added in kernel 5.1.

**Architecture:** Two ring buffers in shared memory between kernel and userspace:

- **Submission Queue (SQ):** Userspace writes Submission Queue Entries (SQEs) describing I/O operations to the tail. The kernel reads from the head.
- **Completion Queue (CQ):** The kernel writes Completion Queue Events (CQEs) to the tail when operations finish. Userspace reads from the head.

Both queues are in memory shared via mmap — no copying between kernel and userspace. A single io_uring_enter() syscall can submit multiple operations and wait for completions, amortizing syscall overhead.

**How it differs from epoll:**

| Aspect | epoll | io_uring |
|--------|-------|----------|
| **Model** | Readiness notification ("this fd is readable") | Completion notification ("this read finished, here's the data") |
| **Syscalls per operation** | epoll_wait + read/write (2 syscalls) | io_uring_enter (1 syscall, or 0 with SQ polling) |
| **Batching** | One event per syscall | Multiple SQEs per io_uring_enter |
| **Operations** | File descriptor readiness only | Read, write, accept, connect, sendmsg, recvmsg, openat, statx, splice, and ~60 more |
| **Zero-copy** | No (still need read/write syscalls) | Shared memory queues eliminate copies |
| **Polling mode** | No | Kernel can poll SQ continuously, eliminating io_uring_enter entirely |

**Completion delivery modes:**

- **Default:** Completions delivered when the process enters the kernel (on next syscall).
- **COOP_TASKRUN:** Completions delivered during any syscall, not just io_uring_enter.
- **DEFER_TASKRUN:** Completions delivered only when explicitly polled. Most predictable for event loops.

**Relevance to pane:**

For pane-comp (the compositor), the primary event loop uses **calloop** which is built on **epoll**. The compositor needs to poll: the Wayland socket (client connections), DRM (display) file descriptors, input device file descriptors (libinput), and timer file descriptors. epoll handles this well — these are all readiness-notification use cases where the compositor needs to know "something is ready to read" and then dispatches accordingly.

io_uring would be relevant if pane had high-throughput file I/O (bulk file reads for pane-store indexing, large buffer operations). For the compositor's event loop, epoll is the right tool — calloop already wraps it well. For pane-store's initial filesystem scan (reading xattrs from thousands of files at startup), io_uring's batched read operations could provide significant speedup over sequential syscalls. This is an optimization opportunity, not an architectural requirement.

### 3.9 epoll — Event Notification

epoll is the standard Linux event notification mechanism for monitoring multiple file descriptors.

**Architecture:** An epoll instance is a kernel object that maintains a set of file descriptors and their interest masks. Userspace calls epoll_wait() to block until one or more file descriptors are ready for the requested operation (read, write, error).

**What calloop uses:** calloop (the event loop library used by smithay and pane-comp) is built on epoll. It provides a Rust abstraction over epoll with:

- Event sources (file descriptors, timers, signals, channels)
- A dispatch loop that calls registered callbacks when sources are ready
- Integration with Wayland's server event loop

The compositor's main loop: epoll_wait blocks until any source is ready. On wakeup, calloop dispatches to the appropriate handler: Wayland client events, DRM page flip completions, libinput events, timer expirations, or internal channel messages from per-client session threads.

**What pane needs to know:** epoll is the foundation of the compositor's event loop. It's well-understood, well-tested, and perfectly adequate for the compositor's needs. Other pane servers (pane-route, pane-roster, pane-store) use the threaded looper model (std::thread + channels) rather than epoll, following BeOS's BLooper pattern. The compositor is the only component where epoll (via calloop) is the primary event mechanism, because it must integrate with smithay's Wayland event loop.

---

## 4. The D-Bus Question

### 4.1 What D-Bus Provides

D-Bus (Desktop Bus) is the standard IPC mechanism for Linux desktops. It is a message-oriented middleware with a central daemon that routes messages between applications.

**Architecture:**

- A **message bus daemon** (dbus-daemon or dbus-broker) accepts connections from applications and routes messages between them
- **Two bus instances:** a system bus (machine-global, for system services) and a session bus (per-user-login, for desktop applications)
- **Four message types:** method calls (requests), method returns (responses), errors (failures), and signals (broadcasts)
- An **object model:** applications expose objects at hierarchical paths (/org/freedesktop/NetworkManager), objects implement interfaces (org.freedesktop.NetworkManager), interfaces define methods and signals
- **Bus names:** unique connection names (:1.42) and well-known names (org.freedesktop.NetworkManager) for service discovery
- **Activation:** the bus daemon can start services on demand when their well-known name is requested

**What the Linux desktop depends on D-Bus for:**

- **Desktop notifications:** org.freedesktop.Notifications — applications send notification requests, the desktop environment displays them
- **Network management:** org.freedesktop.NetworkManager — connection status, Wi-Fi scanning, VPN control
- **Power management:** org.freedesktop.UPower — battery status, suspend/hibernate, lid close actions
- **Screen capture:** org.freedesktop.portal.ScreenCast — the mechanism by which PipeWire screen sharing is brokered
- **Media player control:** org.mpris.MediaPlayer2 — play/pause/next from desktop controls
- **Bluetooth:** org.bluez — device pairing, audio routing
- **Systemd control:** org.freedesktop.systemd1 — service management from desktop UI
- **Polkit authorization:** org.freedesktop.PolicyKit1 — privilege elevation dialogs
- **Secret storage:** org.freedesktop.secrets — password/keyring management
- **File manager integration:** org.freedesktop.FileManager1 — "show in file manager" actions
- **Clipboard (Flatpak):** portal-based clipboard access for sandboxed apps
- **Accessibility:** org.a11y.Bus — screen readers and assistive technology
- **Input methods:** org.freedesktop.portal.InputMethod — for Flatpak apps

This list is not exhaustive. D-Bus is the nervous system of the Linux desktop. Virtually every system service that desktop applications interact with speaks D-Bus.

### 4.2 Why Pane Needs to Bridge D-Bus

Pane cannot ignore D-Bus. The ecosystem depends on it. Firefox sends notifications via D-Bus. PipeWire's screen capture is brokered via D-Bus portals. NetworkManager reports connection status via D-Bus. Bluetooth audio routing goes through D-Bus. If pane doesn't bridge D-Bus, these services are invisible to pane's users.

The two-world problem applies here too: pane-native components speak pane's typed protocol; the Linux ecosystem speaks D-Bus. Without a bridge, pane is isolated from the system it runs on.

### 4.3 What pane-dbus Would Need to Translate

pane-dbus is a protocol bridge — a daemon that translates between D-Bus messages and pane's native message model. It needs to handle:

1. **Signal subscription and forwarding.** When NetworkManager emits a "connection changed" signal, pane-dbus receives it, translates it into a pane-native message, and forwards it to pane-route (where routing rules can match it and dispatch to the appropriate pane — e.g., a status widget showing network state).

2. **Method call proxying.** When a pane-native component needs to call a D-Bus method (e.g., telling UPower to suspend), pane-dbus translates the pane-native request into a D-Bus method call and returns the result.

3. **Portal implementation.** pane-comp needs to implement xdg-desktop-portal interfaces (ScreenCast, Screenshot, FileChooser, etc.) on the D-Bus session bus. These are the portals that Flatpak apps and PipeWire use to request access to system resources. The compositor is the authority — it decides whether to grant screen sharing, which output to share, etc.

4. **Notification reception.** org.freedesktop.Notifications is one of the most important interfaces. Applications send notification requests via D-Bus. pane-dbus receives them and translates into pane-native notification events, which pane-route can match and dispatch to a notification pane.

5. **Service activation.** Some D-Bus services expect to be activated on demand (the bus daemon starts them when their name is requested). pane-dbus needs to participate in this activation model or ensure the services are already running.

**The translation model:** D-Bus messages have a well-defined structure (object path, interface, method/signal name, typed arguments). pane-dbus maps these to pane's message model:

- D-Bus object path + interface + signal name → pane-route content pattern
- D-Bus method call → pane session-typed request/response
- D-Bus signal arguments → pane message attributes
- D-Bus bus names → pane-roster service identifiers

The bridge is bidirectional: pane components can receive D-Bus signals (notifications, status changes) and invoke D-Bus methods (suspend, network control). The Linux ecosystem can invoke pane's portal implementations via D-Bus.

### 4.4 Alternatives and Complements

**Varlink** is a JSON-based IPC protocol being adopted by systemd as a D-Bus alternative for certain use cases.

- Uses Unix sockets or TCP (no special bus daemon)
- Messages are JSON objects terminated by NUL bytes
- Interface definition language with typed methods and errors
- Sequential request processing (no multiplexing — simpler state)
- Service discovery via a resolver at /run/org.varlink.resolver
- No signals/broadcasts — method calls only (with streaming support)

Varlink's advantages over D-Bus: simpler protocol (JSON over sockets, no bus daemon), works during early boot and late shutdown (no daemon dependency), streaming support, and native JSON makes it easy to bridge to web/REST APIs.

Varlink's limitations: no broadcast/signal mechanism (D-Bus signals are widely used), limited adoption outside systemd, no equivalent to D-Bus's object model (paths, interfaces). Varlink does not replace D-Bus for the desktop — it replaces D-Bus for system service communication (user/group lookups, machine management, etc.).

**What this means for pane:** Varlink is not a D-Bus replacement for pane's purposes. The desktop ecosystem speaks D-Bus and will continue to do so. However, for pane's own internal IPC between pane servers, Varlink's design is instructive: JSON over Unix sockets, simple sequential semantics, interface definitions. Pane's own protocol (session-typed messages over Unix sockets, postcard serialization) is already in this spirit — simpler and more principled than D-Bus, but interoperating with D-Bus at the boundary via pane-dbus.

**Android Binder** is sometimes mentioned as a D-Bus alternative. It's a kernel-level IPC mechanism designed for Android. It requires kernel support (not available on standard Linux distributions without patching), is designed for Android's security model, and has no ecosystem adoption on desktop Linux. Not relevant for pane.

### 4.5 The D-Bus Bridge Architecture

pane-dbus should be structured as:

1. **A single daemon** that connects to both the D-Bus session bus and the D-Bus system bus.
2. **D-Bus → pane translation:** Subscribes to D-Bus signals matching configurable patterns. Translates received signals into pane-route messages. The routing rules determine where they go.
3. **pane → D-Bus translation:** Accepts pane-native requests to invoke D-Bus methods. Performs the call and returns the result through pane's protocol.
4. **Portal hosting:** Implements xdg-desktop-portal D-Bus interfaces on behalf of pane-comp. Acts as the portal backend that Flatpak and PipeWire talk to.
5. **Filesystem exposure:** Exposes D-Bus service state at /srv/pane/dbus/ — bus names, object trees, interface introspection. This makes D-Bus discoverable through pane's filesystem interface.

The bridge is a plugin (Design Pillar 7 — Composable Extension). It lives in a well-known directory. If the user doesn't need D-Bus (e.g., a minimal system without Bluetooth, NetworkManager, or Flatpak), the bridge doesn't start. The system degrades gracefully.

---

## 5. Synthesis: How These Subsystems Inform Pane's Design

### The audio/media stack validates BeOS's bet

BeOS bet that building for the hardest case (real-time media) would produce architecture that was better for every case. PipeWire proves this bet twenty years later: by building a media framework that handles professional audio (JACK's domain) and desktop audio (PulseAudio's domain) and video in a single graph-based architecture, PipeWire eliminated the fragmentation that plagued Linux audio for two decades. The graph-based model that BeOS's Media Kit pioneered — producers, consumers, buffers, format negotiation, latency tracking — is now the standard on Linux.

Pane's job is not to replicate PipeWire. It's to make PipeWire's capabilities visible and controllable through pane's interaction model: media nodes as panes with tag lines, routing as visible graph connections, parameters as filesystem-exposed configuration. The "pane media kit" is a thin typed Rust wrapper over PipeWire's client API, not a reimplementation.

### Init system abstraction works because pane's needs are simple

Pane needs five contracts from the init system: restart on crash, readiness notification, dependency ordering, logging, and process tracking. All three candidate init systems (systemd, s6+s6-rc, runit) can provide these, though with different degrees of native support. The key insight from Haiku's launch_daemon and systemd's socket activation is the same: pre-create communication endpoints so services can start in any order and messages queue until the service is ready.

pane-init's job is small: map five contracts to three backends. The mapping is well-defined. The contracts are the right abstraction level — they express what pane needs without encoding how the init system provides it.

### The kernel interfaces are pane's actual platform

Linux is not a monolithic platform — it's a collection of independent kernel interfaces. Pane's relationship to Linux is through these specific interfaces:

- **fanotify + inotify** (via pane-notify): filesystem change detection, the reactive foundation for pane-store's attribute indexing and the filesystem-based configuration model
- **xattrs**: file metadata storage, pane's equivalent of BFS attributes (with userspace indexing compensating for the lack of kernel-level query support)
- **memfd**: zero-copy buffer sharing between clients and compositor
- **pidfd**: race-free process management for pane-roster
- **seccomp**: syscall filtering for server and application sandboxing
- **namespaces**: application isolation (with the understanding that Linux namespaces are not Plan 9 namespaces — they're heavyweight and privilege-gated, not a primary composition mechanism)
- **epoll** (via calloop): the compositor's event loop foundation
- **io_uring**: potential optimization for pane-store's bulk filesystem operations

These interfaces are stable, well-documented, and Linux-specific. Pane is explicitly a Linux desktop environment, and these are the kernel capabilities it leverages. The spec already lists most of these; this research confirms they're the right choices and clarifies their capabilities and limitations.

### D-Bus is the boundary, not the architecture

Pane's internal architecture is typed protocols over Unix sockets — simpler, more principled, and more composable than D-Bus. But pane's external boundary is D-Bus, because that's what the Linux desktop ecosystem speaks. The pane-dbus bridge is not a compromise — it's the correct architectural response to living on a platform you don't fully control. The bridge translates at the boundary; pane's internal protocol remains clean.

The analogy to BeOS is instructive: BeOS had its own IPC (BMessage over ports) but also implemented POSIX for compatibility. The native protocol was the real architecture; POSIX was the bridge to existing software. Pane's session-typed protocol is the real architecture; D-Bus is the bridge to the existing Linux ecosystem.

---

## Sources

### PipeWire
- [PipeWire Overview](https://docs.pipewire.org/page_overview.html) — core architecture
- [PipeWire Objects Design](https://docs.pipewire.org/page_objects_design.html) — node/port/link/endpoint model
- [PipeWire Session Manager](https://docs.pipewire.org/page_session_manager.html) — policy separation
- [PipeWire — Arch Wiki](https://wiki.archlinux.org/title/PipeWire) — practical architecture and configuration
- [Niri Screencasting](https://deepwiki.com/YaLTeR/niri/4.3-screencasting-and-screenshots) — compositor portal integration

### BeOS Media Kit
- [The Be Book — Media Kit Overview](https://www.haiku-os.org/legacy-docs/bebook/TheMediaKit_Overview_Introduction.html) — complete architecture
- [Inside the BeOS Media Kit (birdhouse.org)](https://birdhouse.org/beos/byte/22-media_kit/) — design philosophy
- [BeOS Media Kit GMPI Review](http://plugin.org.uk/GMPI/beos-review.html) — plugin API analysis

### Audio Stack
- [ALSA — Arch Wiki](https://wiki.archlinux.org/title/Advanced_Linux_Sound_Architecture) — kernel audio interface
- [ALSA — Wikipedia](https://en.wikipedia.org/wiki/Advanced_Linux_Sound_Architecture) — architecture overview
- [PulseAudio — Arch Wiki](https://wiki.archlinux.org/title/PulseAudio) — desktop audio server
- [PulseAudio — Wikipedia](https://en.wikipedia.org/wiki/PulseAudio) — architecture and design
- [JACK — Arch Wiki](https://wiki.archlinux.org/title/JACK_Audio_Connection_Kit) — professional audio
- [JACK — Wikipedia](https://en.wikipedia.org/wiki/JACK_Audio_Connection_Kit) — architecture

### Init Systems
- [systemd.socket(5)](https://www.freedesktop.org/software/systemd/man/latest/systemd.socket.html) — socket activation
- [systemd/systemd DeepWiki](https://deepwiki.com/systemd/systemd) — architecture overview
- [s6 Overview](https://skarnet.org/software/s6/overview.html) — supervision architecture
- [s6-rc: Why?](https://skarnet.org/software/s6-rc/why.html) — dependency management rationale
- [skarnet/s6 DeepWiki](https://deepwiki.com/skarnet/s6) — implementation details
- [runit](https://smarden.org/runit/) — three-stage init
- [runit — Wikipedia](https://en.wikipedia.org/wiki/Runit) — architecture overview

### Kernel Interfaces
- [fanotify(7)](https://man7.org/linux/man-pages/man7/fanotify.7.html) — filesystem notification
- [fanotify_mark(2)](https://man7.org/linux/man-pages/man2/fanotify_mark.2.html) — mark types
- [inotify(7)](https://man7.org/linux/man-pages/man7/inotify.7.html) — per-directory watches
- [Inotify Limits](https://watchexec.github.io/docs/inotify-limits.html) — watch limitations
- [xattr(7)](https://man7.org/linux/man-pages/man7/xattr.7.html) — extended attributes
- [memfd_create(2)](https://man7.org/linux/man-pages/man2/memfd_create.2.html) — anonymous shared memory
- [Wayland Shared Memory Buffers](https://wayland-book.com/surfaces/shared-memory.html) — memfd in Wayland
- [pidfd_open(2)](https://man7.org/linux/man-pages/man2/pidfd_open.2.html) — process file descriptors
- [Completing the pidfd API (LWN)](https://lwn.net/Articles/794707/) — design rationale
- [Seccomp BPF — Kernel Docs](https://docs.kernel.org/userspace-api/seccomp_filter.html) — syscall filtering
- [Linux Namespaces — Wikipedia](https://en.wikipedia.org/wiki/Linux_namespaces) — namespace types
- [Linux Namespaces Are a Poor Man's Plan 9 Namespaces](https://yotam.net/posts/linux-namespaces-are-a-poor-mans-plan9-namespaces/) — comparison
- [What is io_uring?](https://unixism.net/loti/what_is_io_uring.html) — architecture
- [io_uring — Wikipedia](https://en.wikipedia.org/wiki/Io_uring) — overview

### D-Bus and Alternatives
- [D-Bus Tutorial](https://dbus.freedesktop.org/doc/dbus-tutorial.html) — architecture and object model
- [D-Bus Specification](https://dbus.freedesktop.org/doc/dbus-specification.html) — protocol details
- [D-Bus — Wikipedia](https://en.wikipedia.org/wiki/D-Bus) — overview and history
- [Varlink](https://varlink.org/) — protocol specification
- [Varlink: a protocol for IPC (LWN)](https://lwn.net/Articles/742675/) — design rationale
- [systemd Varlink and D-Bus (Phoronix)](https://www.phoronix.com/news/Systemd-Varlink-D-Bus-Future) — migration trajectory
- [D-Bus and Varlink IPC in systemd](https://deepwiki.com/systemd/systemd/2.5-d-bus-and-varlink-ipc) — comparison
