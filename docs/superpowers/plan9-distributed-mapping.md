# Plan 9 Distributed Model: Mapping to Pane

A grounded technical assessment of how Plan 9's distributed computing mechanisms map to pane's architecture. Written from the perspective of someone who shipped code into plan9port and worked on the 9P protocol stack. The goal is precision about what applies, what partially applies, and where naive translation would lose the point.

## Primary Sources

- Pike, Presotto, Dorward, Flandrena, Thompson, Trickey, Winterbottom, "Plan 9 from Bell Labs" (Computing Systems, 1995)
- Pike, Presotto, Thompson, Trickey, "The Use of Name Spaces in Plan 9" (OSDI, 1992)
- Pike, "Plumbing and Other Utilities" (USENIX, 2000)
- Cox, Grosse, Pike, Presotto, Ritchie, "Security in Plan 9" (USENIX Security, 2002)
- Presotto, Winterbottom, "The Organization of Networks in Plan 9" (USENIX, 1993)
- 9P2000 specification: intro(5), RFC draft (ericvh.github.io/9p-rfc/rfc9p2000.html)
- Man pages: cpu(1), exportfs(4), import(1), factotum(4), plumber(4), srv(4), bind(1), mount(1)
- Mirtchovski, Simmonds, Minnich, "Persistent 9P Sessions for Plan 9" (IWP9, 2006)
- u-root/cpu: Go reimplementation of Plan 9 cpu semantics over SSH (github.com/u-root/cpu)

---

## Area 1: 9P and pane-proto

### How Plan 9 did it

9P achieves network transparency with 13 message pairs. The entire protocol fits in a few pages:

```
Tversion/Rversion   — negotiate version and max message size
Tauth/Rauth         — authenticate (optional)
Tattach/Rattach     — establish root fid
Twalk/Rwalk         — navigate hierarchy (up to 16 elements per walk)
Topen/Ropen         — open fid for I/O
Tcreate/Rcreate     — create file and open it
Tread/Rread         — read bytes from offset
Twrite/Rwrite       — write bytes at offset
Tstat/Rstat         — get metadata
Twstat/Rwstat       — set metadata
Tclunk/Rclunk       — release fid
Tremove/Rremove     — delete file
Tflush/Rflush       — cancel pending request
```

Message format: 4-byte size, 1-byte type, 2-byte tag, then type-specific fields. Integers little-endian, text UTF-8 with 2-byte count prefix. Tags multiplex concurrent requests on a single connection.

The critical design properties:

**Fids are client-chosen.** A fid is a 32-bit handle the client picks to name a position in the server's file tree. The server never assigns fids. This means the client controls its own handle table, which simplifies multiplexing (each sub-client picks from a disjoint fid range) and eliminates a round-trip for handle allocation.

**Qids are server-assigned.** A qid (13 bytes: 1-byte type, 4-byte version, 8-byte path) uniquely identifies a file on the server. Two files are the same iff their qids match. The version field increments on modification, giving clients a cheap staleness check without a separate cache-coherence protocol.

**No server-initiated messages.** 9P is strictly request-response. The server never pushes data. Clients that want events must block on a read (the "event file blocks until ready" pattern). This is simultaneously 9P's greatest strength and its most obvious limitation. Strength: the protocol is trivially implementable, every message has a response, error handling is local to each request. Limitation: real-time event delivery requires either polling or a blocking read per event stream, which consumes a thread or coroutine per subscription.

**Twalk multi-element walks.** A single Twalk can traverse up to 16 path elements, returning a qid for each. This amortizes round-trips for deep paths. If the walk partially succeeds (element 5 of 10 fails), the fid is unaffected and the response indicates how far it got. This partial-success semantic is unusual and worth noting: most protocols would fail atomically.

**Tflush for cancellation.** A client can cancel any pending request by sending Tflush with the request's tag. The server must respond with Rflush (never Rerror). The client cannot reuse the tag until Rflush arrives. This gives explicit cancellation semantics without connection teardown.

**What makes it work at scale:** The protocol is stateless enough that any 9P server can be implemented in an afternoon. exportfs, the relay that makes distribution work, is about 2000 lines of C. The plumber is about 2000 lines. Rio's 9P server portion is similarly modest. The small protocol surface means every service speaks the same language, and the implementation cost is low enough that nobody is tempted to bypass it.

### Mapping to pane

Pane's protocol is structurally different from 9P and should remain so. Here is why, and what 9P still teaches.

**Pane-proto is richer and that is correct.** Pane has a session-typed handshake (`ClientHello` -> `ServerHello` -> `ClientCaps` -> `Branch<Accepted, Rejected>`), then a bidirectional active phase with typed enums (`ClientToComp`, `CompToClient`). 9P has `Tversion/Rversion` then free-form requests. Pane's handshake provides compile-time enforcement that both sides agree on capabilities before entering the active phase. 9P's version negotiation is a single message pair with a version string comparison. The session-typed approach is strictly better for a protocol with a fixed, known set of participants (compositor and native client), because the type system catches protocol violations at compile time rather than at runtime.

**9P's request-response discipline vs pane's bidirectional active phase.** 9P never has the server send unsolicited messages. Pane's active phase has the compositor sending `Focus`, `Blur`, `Key`, `Mouse`, `Close`, `CompletionRequest` to the client at any time. This is the right choice for a windowing protocol: input events are inherently server-initiated (the compositor sees the keyboard, not the client). But it means pane cannot use 9P's simple tag-based request multiplexing. Instead, pane uses PaneId-based demultiplexing, which is the correct adaptation.

**What pane should learn from 9P:**

1. **Client-chosen identifiers.** 9P's client-chosen fids avoid a round-trip for handle allocation. Pane currently has the compositor assign `PaneId` in `PaneCreated`. Consider allowing the client to propose a `PaneId` in `CreatePane` that the compositor either accepts or rejects. This eliminates the correlation problem between `CreatePane` and `PaneCreated` (the `pending_creates` VecDeque in app.rs) and matches 9P's philosophy that the client owns its handle space. The compositor validates uniqueness, just as a 9P server validates that a walked-to fid doesn't collide.

2. **Explicit cancellation.** 9P's Tflush is a first-class protocol operation. Pane has no cancellation mechanism. When a client sends `RequestResize` and then changes its mind, there is no way to retract the request. For the active phase's typed enums, a `Cancel { token: u64 }` variant (mirroring 9P's tag-based flush) would be useful, particularly for long-running operations like completions.

3. **Version negotiation discipline.** 9P's `Tversion` resets all state on the connection. If version negotiation fails, the connection is clean. Pane's handshake already has `Rejected` as a terminal state, which is correct. But consider: if the protocol evolves, pane needs a migration story. 9P's answer is "the version string determines the entire protocol; old and new cannot coexist on one connection." Pane's `ClientHello.version` field serves this purpose. Make the invariant explicit: a version mismatch means `Rejected`, period, no fallback negotiation within the handshake.

4. **The filesystem as the universal fallback.** 9P teaches that the file interface is the one that lasts. Typed protocols are great for native clients; the filesystem is for everything else. Pane already has this with the three-tier model (filesystem / protocol / in-process). The lesson is: never let the typed protocol become the only way to do something. Every operation accessible through `ClientToComp`/`CompToClient` should eventually have a filesystem equivalent via pane-fs. The filesystem is the universal FFI.

### Warnings about naive translation

**Do not implement 9P as pane's wire protocol.** 9P's message set is designed for file servers. Pane's compositor is not a file server in the 9P sense — it manages surfaces, input focus, layout trees, and frame timing. Projecting all compositor operations into open/read/write/close would create the same kind of impedance mismatch that made `/dev/bitblt` in 8-1/2 awkward for rich graphics. The file projection (pane-fs) is the right place for the file abstraction; the native protocol should speak the compositor's language.

**Do not mistake protocol simplicity for simplicity of the system built on it.** 9P is 13 messages, but the conventions layered on top (clone/ctl/data for connections, numbered directories for resources, event-file-blocks-until-ready for async) are substantial. The "one protocol" claim has a significant layer of unspecified convention. Pane's typed enums make these conventions explicit and compiler-checked, which is an improvement, not a compromise.

---

## Area 2: Per-Process Namespaces and Graded Equivalence

### How Plan 9 did it

Every Plan 9 process has its own namespace — a private mapping from pathnames to resources. Two system calls compose namespaces:

**`bind(new, old, flags)`** — takes a portion of the existing namespace visible at `new` and makes it also visible at `old`. Both paths are in the same namespace.

**`mount(fd, old, flags)`** — takes a 9P file server on `fd` and attaches its tree at `old`.

Both accept flags: `MREPL` (replace), `MBEFORE` (union, new searched first), `MAFTER` (union, new searched last). MBEFORE and MAFTER create union directories.

**`rfork(flags)`** controls namespace inheritance: shared (changes propagate bidirectionally) or copied (changes are independent).

The kernel resolves path lookups by consulting the process's mount table — a linked list of `(old_path, mounted_channel, flags)` entries. When a walk crosses a mount point, the kernel switches to the mounted channel and continues the walk there. Union directories are resolved by trying each mounted entry in order until one succeeds.

This is the mechanism that makes rio work: rio mounts itself onto `/dev` in each child's namespace, providing per-window `/dev/cons`, `/dev/mouse`, etc. The child process never knows it is talking to rio rather than the kernel. This is also how `cpu` works: the remote shell's namespace mounts the terminal's exported namespace at `/mnt/term`, then binds specific files from there into standard locations.

**What namespaces actually provide:**

1. Per-process capability sets — what you can see IS what you can access
2. Transparent service substitution — replace any subtree without affecting anyone else
3. Interposition — insert a relay/monitor/filter by mounting in front of the real service
4. Composition without coordination — each process assembles its own view

**The honest limitation:** Plan 9's designers acknowledged that "for a process to function sensibly the local name spaces must adhere to global conventions" — processes must agree on what `/dev` and `/bin` mean. Convention replaces enforcement. No kernel mechanism forces namespace layout. This works in a system with a small, disciplined user base. It would not survive a large ecosystem with adversarial or careless actors.

### Mapping to pane

Pane cannot replicate Plan 9's per-process namespaces. Linux mount namespaces require `CAP_SYS_ADMIN` or `CLONE_NEWUSER` (user namespaces), they are heavyweight (kernel data structure per namespace), they are not designed for per-window manipulation, and they do not support the kind of lightweight bind/mount composition that Plan 9 makes trivial. This is not a limitation to work around; it is a fundamental constraint to accept.

**Graded equivalence is the right replacement.** The architecture spec describes each observer seeing "a coherent quotient of the system based on their permissions." This is not the same as per-process namespaces, but it solves the same problem at the right level.

In Plan 9, a process's namespace IS its permission set — if you cannot see a file, you cannot access it. In pane, a connection's identity (unix uid, `.plan` governance, TLS cert) determines what it can see through pane-fs and what operations it can perform through the protocol. The mechanism differs (namespace composition vs identity-based access control) but the principle is the same: the view IS the capability.

**Concrete recommendations:**

1. **Per-connection views in pane-fs.** When pane-fs serves a FUSE request, it knows the requesting process's uid from the FUSE context. Different users should see different content under `/pane/`. An agent user with a restrictive `.plan` should see only the panes it is authorized to observe. This is Plan 9's per-process namespace idea implemented via access control rather than mount composition. The effect is the same: different consumers see different views of the same hierarchy.

2. **Bind-like composition for pane-fs.** Consider supporting a `.pane-namespace` or equivalent configuration that lets a user declare: "under `/pane/`, also show me remote host X's panes at `/pane/remote/X/`." This is the `mount` operation translated to pane's context. The composition happens in pane-fs's synthetic tree construction, not in the kernel. The cost is that it is per-user rather than per-process, but for a desktop environment, per-user granularity is sufficient.

3. **The `.plan` file as namespace specification.** In Plan 9, a process's namespace is constructed by its profile script (`/usr/$user/lib/profile`). In pane, an agent's `.plan` file declares what it can see and do. Make the analogy explicit: the `.plan` IS the namespace specification. When pane-fs constructs a view for an agent, it consults the agent's `.plan` to determine what is visible. No additional access-control layer needed — the `.plan` determines the view, and the view determines the capability.

4. **Do not attempt union directories.** Plan 9's union directories (MBEFORE/MAFTER) are powerful but create ambiguity: when a file exists in multiple mounted trees, which one do you get? The answer depends on mount order, which is non-obvious and a source of bugs. Pane's filesystem should have one canonical location for each piece of state. Remote panes go under `/pane/remote/<host>/`, not unioned into the local `/pane/` tree.

### Warnings about naive translation

**Per-process namespaces are a kernel feature.** You cannot faithfully replicate them in userspace. FUSE gives you per-request uid, not per-process namespace state. Two processes with the same uid will see the same pane-fs content even if they have different "views" of the system. If pane needs per-process differentiation finer than per-uid, it would need to use Linux user namespaces or a per-process FUSE mount, both of which are heavy. Accept per-uid granularity for pane-fs and use the typed protocol for finer-grained access control where needed.

**Convention over enforcement is fragile.** Plan 9 relied on convention to keep namespaces coherent. This worked at Bell Labs. It will not work for pane's agent ecosystem, where agents may be poorly written or adversarial. The `.plan` enforcement (Landlock, capability restrictions) is the right answer — it makes the namespace not just a view but a sandbox. This is stronger than what Plan 9 provided.

---

## Area 3: import/exportfs and Remote pane-fs Mounting

### How Plan 9 did it

Distribution in Plan 9 is built from two pieces:

**exportfs(4)** is a user-level relay that serves an arbitrary portion of a namespace over 9P. When a remote client sends a Twalk, exportfs performs the walk locally and returns the result. When the client sends Tread, exportfs reads locally and returns the data. It is a namespace-to-9P translator — about 2000 lines of C. Key options: `-r root` (serve a subtree), `-s` (serve entire namespace), `-R` (read-only), `-P patternfile` (restrict via regex).

**import(1)** connects to a remote exportfs and mounts the result into the local namespace: `import lab.pc /proc /n/labproc`. Now `/n/labproc/42/status` shows remote process 42's status.

The combination is powerful because exportfs can serve ANY namespace, not just on-disk files. If your namespace includes rio's window files, the plumber, and your local filesystem, exportfs can serve all of that to a remote machine. This is what makes `cpu` work: the terminal exports its entire namespace (including display devices) to the remote CPU server.

**Failure modes that Plan 9 struggled with:**

1. **Hung mounts.** If the remote server dies, any process with a fid pointing at the mounted tree blocks on its next I/O operation. The kernel waits indefinitely for the 9P response. In early Plan 9, the only recovery was to reboot. The `recover` program (Mirtchovski et al., IWP9 2006) addressed this by proxying 9P sessions, tracking fid state, and re-establishing connections on failure — but it was never part of the standard distribution.

2. **No cache coherence protocol.** 9P has no invalidation mechanism. If you read a file and someone else modifies it, your cached copy is stale. The qid version field lets you detect staleness on the next stat, but there is no push notification. For a filesystem, this is tolerable (you re-read). For a UI state interface, it would be painful — you would miss focus changes, resize events, input.

3. **Latency sensitivity.** Every file operation over a remote mount is a network round-trip. On a LAN this is 100-200us. Over the internet, 50-200ms. Plan 9 was designed for LANs. Remote mounts over WAN connections are usable for batch operations (file copy, process inspection) but not for interactive work (editing, window management).

### Mapping to pane

Pane needs remote state access for the headless deployment model. The question is what `/pane/remote/<hostname>/` should look like and how it should work.

**Concrete recommendations:**

1. **Remote pane-fs as a protocol bridge, not a 9P mount.** When a user accesses `/pane/remote/lab-server/`, pane-fs should not attempt to mount a remote FUSE filesystem over the network. Instead, pane-fs should establish a pane protocol connection (TcpTransport + TLS) to the remote headless compositor and translate FUSE operations into protocol messages. This is the same architecture pane-fs already uses for local state (FUSE -> protocol), just with a remote transport. The translation is the same; only the transport changes.

2. **Lazy connection establishment.** Do not connect to remote hosts at pane-fs mount time. Connect on first access to `/pane/remote/<hostname>/`. Cache the connection. Reconnect on failure with exponential backoff. This avoids the hung-mount problem that plagued Plan 9: if the remote host is unreachable, the `open()` or `read()` call returns an error (ECONNREFUSED, ETIMEDOUT) rather than blocking indefinitely.

3. **Event forwarding for remote panes.** 9P has no push mechanism, so remote event delivery requires blocking reads. Pane's protocol already has server-initiated events (`CompToClient::Focus`, `Resize`, etc.). For remote access, the pane-fs event file (`/pane/remote/host/42/event`) should use the same mechanism it uses locally: establish a protocol subscription and translate events into JSONL lines for the reading process. The event is not polled; it is pushed through the protocol and buffered in pane-fs for the reader.

4. **Read-only by default, with explicit opt-in for control.** Remote pane state should be readable without special authorization (subject to `.plan` permissions). Writing to remote `ctl` files requires explicit authorization — this is the patternfile (`-P`) mechanism from exportfs translated to pane's trust model. The `.plan` file specifies which remote operations are permitted.

5. **Namespace discovery at `/pane/remote/`.** Listing `/pane/remote/` should show configured remote hosts (from user or system configuration), not attempt network discovery. This is deliberate: Plan 9's `import` required you to name the host explicitly. Auto-discovery (mDNS, etc.) is a separate concern that should not block filesystem responsiveness.

6. **Connection metadata at `/pane/remote/<host>/status`.** Expose connection state (connected/disconnected/connecting), latency, last-seen timestamp. This is the network transparency that Plan 9 did not provide — Plan 9 mounts were either working or hung, with no intermediate state visible to the user.

### Warnings about naive translation

**Do not implement a general-purpose network filesystem.** The temptation is to build "pane's 9P" — a general file-serving protocol for remote access. Resist this. Pane-fs is a synthetic filesystem that translates to protocol messages. The protocol is the real interface; the filesystem is a projection. Making the filesystem the network-primary interface inverts the architecture and reintroduces all of 9P's limitations (no push, latency per operation, no structured queries).

**Latency is not transparent.** Plan 9 pretended remote files were local and paid for it with hung processes and poor WAN performance. Pane should make latency visible. A remote pane's `attrs/latency` should expose the current round-trip time. Tools that access remote state should know they are accessing remote state, even if the path looks the same. Transparency of mechanism, not of performance.

---

## Area 4: cpu and Remote Pane Execution

### How Plan 9 did it

The `cpu` command is the most important composition in Plan 9's distributed model. The flow:

1. User types `cpu -h labserver` on their terminal
2. The cpu client authenticates to the remote CPU server via factotum
3. cpu starts `exportfs` locally, serving the terminal's namespace — including `/dev/cons` (keyboard/screen), `/dev/mouse`, `/dev/draw` (graphics), and local files
4. The remote CPU server starts `rc` (the shell) in a new namespace
5. The remote namespace mounts the exported terminal namespace at `/mnt/term`
6. Standard binds: `bind /mnt/term/dev/cons /dev/cons`, etc.
7. Architecture-specific `/bin` is rebound for the remote CPU architecture
8. The remote shell now reads/writes the local terminal's window through 9P

The key insight: **computation moves to the CPU; I/O stays with the user.** The remote shell runs on fast hardware but types and draws on the local screen. From the remote process's perspective, `/dev/cons` is a local file — it happens to be served by the terminal over the network.

This is not SSH. SSH gives you a remote shell with remote devices. `cpu` gives you a remote CPU with YOUR devices. The namespace reconstruction is what makes this possible: the remote profile script rebuilds the namespace to point `/dev` at the terminal's exported devices while pointing `/bin` at the remote architecture's binaries.

**What `cpu` got wrong:**

1. **Namespace reconstruction is fragile.** The remote profile script must correctly bind everything the user expects from `/mnt/term`. If the profile has errors, the remote session is broken in subtle ways. There is no verification that the reconstructed namespace is complete.

2. **Everything goes over 9P.** Screen drawing, keyboard input, mouse events — all file operations over the network. On a LAN this works (Plan 9 was designed for Lucent's internal network). Over a WAN, the latency of every keystroke being a 9P round-trip makes interactive use painful.

3. **No session persistence.** If the network drops, the cpu session dies and all remote processes lose their namespace mounts. The `recover` proxy addressed this partially but was never standard.

**The u-root/cpu Go reimplementation** (github.com/u-root/cpu) is instructive. It uses SSH for transport and authentication (no custom protocol), runs a local 9P server to export the local filesystem, and mounts it on the remote side. The remote command runs with access to local files via 9P. This demonstrates that the `cpu` concept is separable from Plan 9's kernel: the idea is "export local state, execute remotely, I/O stays local."

### Mapping to pane

"Run my pane app on a remote headless instance" should feel like launching a local app that happens to use remote compute. The pane equivalent of `cpu` is:

1. The user has pane running locally (full compositor with display)
2. The user targets a remote headless pane instance for execution
3. The local compositor exports its display protocol to the remote app
4. The remote app connects to the local compositor via TcpTransport + TLS
5. Input and rendering stay local; computation runs remotely

**Concrete recommendations:**

1. **Reverse connection model, not forward mount.** Plan 9's `cpu` exports the terminal's namespace to the remote machine. Pane should do the inverse: the remote app connects BACK to the local compositor. The local compositor listens on a TLS port (or the connection is tunneled via SSH). The remote app is started with `PANE_COMPOSITOR=tcp://local-machine:port` and connects normally. From the remote app's perspective, it is a normal pane client — it sends `ClientHello`, gets `ServerHello`, enters the active phase. The transport is TCP+TLS instead of Unix socket; the protocol is identical.

    This is simpler than Plan 9's approach because pane's protocol is already transport-agnostic. The `Transport` trait abstracts over unix sockets, memory channels, and TCP. Adding TcpTransport is an implementation of the trait, not a new mechanism.

2. **Identity forwarding in the handshake.** When a remote app connects, the compositor needs to know who it is. Extend `ClientHello` (or `ClientCaps`) with a `PeerIdentity` structure: username, uid, hostname, TLS client certificate fingerprint. The compositor uses this for access control and for display in the pane's chrome (showing that a pane is remote). This is analogous to `cpu`'s authentication step via factotum, but using TLS client certs instead of Plan 9's p9sk1 tickets.

3. **No namespace reconstruction needed.** This is where pane's model is simpler than Plan 9's. In Plan 9, the remote shell needs its entire namespace rebuilt — `/dev`, `/bin`, `/env`, everything. In pane, the remote app only needs a compositor connection. It does not need local devices mounted (pane apps do not read `/dev/cons` — they receive `Key` events through the protocol). It does not need a local filesystem (unless the app itself needs files, which is the app's concern, not pane's). The entire "namespace reconstruction" problem dissolves because the protocol is the interface, not the filesystem.

4. **File access as a separate concern.** If a remote app needs access to local files (e.g., a remote editor needs to read local documents), this is a separate protocol or mechanism — not part of the pane compositor protocol. Options: SFTP, NFS, 9P (via plan9port's 9pfuse or the u-root approach), or a future pane-fs remote export. Do not conflate "run a remote pane app" with "give a remote process access to local files." Plan 9 conflated these because everything was 9P; pane should not.

5. **Session persistence via protocol reconnection.** When the network drops between a remote app and the local compositor, the app should be able to reconnect and resume. This requires the compositor to keep pane state alive for a grace period after disconnection (the pane exists but has no active client) and the client to re-attach with a session token. This is what Plan 9's `recover` program tried to do at the 9P level. Pane can do it at the protocol level: add a `Reconnect { session_token }` variant to the handshake that bypasses full re-negotiation and re-attaches to existing pane state.

### Warnings about naive translation

**Do not export the compositor as a file server.** Plan 9's `cpu` works because the terminal IS a file server (rio serves `/dev/cons` etc. via 9P). Pane's compositor is not a file server — it is a protocol server. Attempting to export it as a 9P tree would require re-implementing the entire compositor protocol as file operations, which is both wasteful (the protocol already exists) and lossy (push events do not fit the file model). Use the protocol directly. The file interface (pane-fs) is for local inspection, not for the network transport of real-time compositor state.

**Do not assume LAN latency.** Plan 9's `cpu` was designed for building-scale networks where round-trips were sub-millisecond. Pane's remote execution must work over the internet. This means: batch compositor updates (don't send one `SetContent` per keystroke — buffer and flush), implement frame-rate adaptation for remote panes, and make latency measurement part of the protocol (a periodic `Ping`/`Pong` or timestamps in existing messages).

---

## Area 5: factotum, .plan, and the Trust Model

### How Plan 9 did it

Factotum is a per-user authentication agent that runs as a file server at `/mnt/factotum`. Applications that need to authenticate do not handle crypto themselves — they open `/mnt/factotum/rpc`, write a `start` message specifying the protocol and role, then exchange `read`/`write` messages that carry protocol bytes between factotum and the remote party. When done, they read `authinfo` to get the authenticated identity.

The critical design properties:

**Applications never see secrets.** The `rpc` file is the only interface. Applications send and receive protocol bytes, but the keys and crypto operations are inside factotum. A compromised application cannot extract passwords — it can only initiate authentication conversations, which factotum mediates.

**Protocol independence.** Factotum implements multiple auth protocols (p9sk1, ssh-rsa, apop, etc.) behind the same rpc interface. An application says `start proto=p9sk1 role=client` and factotum handles the rest. Adding a new protocol means adding code to factotum, not to every application.

**The speaks-for chain.** Factotum on the user's terminal holds the user's keys. When the user connects to a CPU server, the remote factotum delegates to the terminal's factotum via `/mnt/term/mnt/factotum`. The remote process authenticates by asking its local factotum, which forwards to the terminal's factotum over the network. Secrets never leave the terminal. Cox et al. (USENIX Security 2002): "Factotum is the only process that needs to create capabilities, so all network servers can run as untrusted users."

**Key management via ctl.** Keys are `attr=value` tuples: `proto=p9sk1 dom=example.com user=alice !password=secret`. Attributes prefixed with `!` are never displayed. The `ctl` file accepts `key`, `delkey`, `debug` commands. Reading `ctl` shows all keys with secrets redacted.

**Confirmation gates.** Keys marked with `confirm` require interactive approval via the `confirm` file. A GUI process reads confirmation requests and writes yes/no responses. This prevents silent key use — the user must explicitly approve each authentication that uses a sensitive key.

**Secstore for persistence.** Keys are volatile in factotum's memory. Secstore is a separate network service that stores encrypted key files, retrieved at login via PAK (password-authenticated key exchange). This provides single sign-on: one password retrieves all keys.

### Mapping to pane

Pane's trust model has four layers: unix identity (uid/gid), `.plan` governance, TLS client certificates, and Landlock sandboxing. The question is whether pane needs a factotum-like service or whether these layers are sufficient.

**Assessment: pane does not need a factotum equivalent, but it needs the separation principle factotum embodies.**

The reason Plan 9 needed factotum is that every service spoke 9P, and 9P's auth mechanism (Tauth/Rauth) required a conversation — an interactive multi-step protocol negotiation. Applications could not be expected to implement auth protocols. Factotum centralized this.

Pane's situation is different:
- Inter-pane communication uses the compositor as intermediary (messages flow through the protocol, not direct 9P connections between apps)
- Remote connections use TLS, which handles authentication at the transport layer (client certs, server certs, mutual TLS)
- Agent sandboxing uses Landlock + `.plan`, which are kernel-enforced, not protocol-negotiated

There is no "auth conversation" that pane applications need to conduct. The TLS handshake happens in the transport layer; the `.plan` governs access; Landlock enforces boundaries. No application code touches secrets.

**Concrete recommendations:**

1. **Keep secrets in the keyring, not in pane.** TLS certificates and keys should be managed by the system keyring (Linux kernel keyring, or a userspace equivalent like gnome-keyring/kwallet). Pane should not implement key storage. This is the factotum principle (centralize secret management) applied to the Linux ecosystem (use the platform's secret store).

2. **The `.plan` file as the authorization specification.** Factotum's key tuples specified what protocols a key could be used for (`proto=`, `dom=`, `role=`). The `.plan` file serves an analogous role: it specifies what an agent can see, do, and access. Make `.plan` evaluation a single, auditable code path. Every access check should go through the same `.plan` evaluator, just as every Plan 9 auth check went through factotum.

3. **PeerIdentity as the factotum-like interface.** When a remote client connects via TLS, the handshake extracts identity from the client certificate. This identity flows into `PeerIdentity` in the protocol handshake. The compositor and pane-fs use `PeerIdentity` for all access decisions. This is the speaks-for chain in miniature: the TLS certificate speaks for the user; the `.plan` governs what that user can do.

4. **Confirmation for sensitive operations.** Factotum's confirmation gates are worth adopting. When a remote agent requests a destructive operation (close a pane, modify system configuration), the compositor should support a confirmation flow: notify the user, wait for approval. This could be a special notification pane type, not a separate subsystem. The mechanism is: agent sends `ctl` command -> pane-fs checks `.plan` -> `.plan` says `confirm_destructive: true` -> compositor creates a confirmation notification -> user approves/denies -> operation proceeds or is rejected.

5. **No auth negotiation in the pane protocol.** Do not add Tauth/Rauth-like messages to the pane protocol. TLS does this at the transport layer. Putting auth in the application protocol would mean every transport needs its own auth adaptation (Unix sockets use SO_PEERCRED, TCP uses TLS, etc.). Instead, abstract identity extraction behind the `Transport` trait: `fn peer_identity(&self) -> Option<PeerIdentity>`. Unix transport extracts uid from SO_PEERCRED; TLS transport extracts identity from the client cert; memory transport returns a test identity.

### Warnings about naive translation

**Factotum's power came from 9P ubiquity.** Every Plan 9 service used Tauth, so factotum's mediation was universally useful. Pane does not have a single auth protocol for all services. Adding one would mean re-implementing TLS's job. Use TLS. It is battle-tested, has hardware acceleration, handles certificate rotation, and has decades of security audit behind it. Factotum handled what, at its core, was a missing transport-layer authentication feature that Plan 9 had to build itself.

**The speaks-for chain requires trust anchoring.** In Plan 9, the auth server was the trust root. In pane, the TLS CA is the trust root. For a personal system (one user, their agents), a self-signed CA managed by Nix is sufficient. For a multi-user deployment, integrate with an organizational CA. Do not invent a new trust infrastructure.

---

## Area 6: The Plumber and Pane Routing

### How Plan 9 did it

The plumber is a user-space file server (about 2000 lines of C) that routes inter-application messages based on pattern-matching rules. It serves files at `/mnt/plumb/`:

- `/mnt/plumb/send` — write a message to route it
- `/mnt/plumb/rules` — read/write the rule set
- `/mnt/plumb/edit`, `/mnt/plumb/image`, etc. — named ports; apps read from these

Messages are six-line textual headers (src, dst, wdir, type, attr, ndata) followed by data. Rules are pattern-action groups: match objects (src, dst, data, type, attr) with verbs (is, matches, isfile, isdir), then execute actions (plumb to port, plumb client command, data set, attr add).

**What makes the plumber powerful:**

1. **Content transformation, not just dispatch.** Rules rewrite messages: `data set $1` extracts a substring, `attr add addr=$2` adds metadata. The plumber normalizes paths, validates file existence (`isfile`), extracts line numbers. Receivers get clean, resolved data.

2. **File server architecture.** The plumber is a 9P server. Messages pass through regular file I/O. No IPC mechanism to learn. Works over the network identically. Pike: "no extra technology such as remote procedure call or request brokers needs to be provided."

3. **Central authority, user-configurable.** One rules file governs all routing. Users edit it in their preferred editor. No per-application configuration. Pike: "The plumber, by removing such decisions to a central authority, guarantees that all applications behave the same."

4. **Lazy application launch.** `plumb client editor` starts the editor if the `edit` port has no reader. The plumber queues the message until the port opens. Applications start on demand, triggered by content, not by explicit launch.

5. **The click attribute.** When a user B3-clicks in acme, the message includes a `click` attribute with the cursor offset. The plumber finds the "longest leftmost match touching the click position" — extracting the relevant text from surrounding context. This is content extraction, not just content routing.

### Mapping to pane

Pane has already made the key architectural decision: routing is a kit-level concern, not a separate server. The architecture spec explains why: a central router is a single point of failure, and the complexity of making it resilient exists only because the single point of failure was created in the first place. Instead, the pane-app kit loads routing rules from the filesystem, evaluates them locally, and dispatches directly.

This diverges from the plumber's architecture (central file server) but preserves its important properties. Here is what to keep and what to skip.

**What to keep:**

1. **User-editable rules as the routing authority.** The plumber's rules file is a text file the user edits. Pane's routing rules should be files in a well-known directory (e.g., `~/.config/pane/routing/` or `/pane/config/routing/`), one rule per file or one file with rule blocks. The format should be declarative and readable. The plumber's pattern-action language is a good starting point, adapted for pane's content types (not just text — also typed attributes from the scripting protocol).

2. **Content transformation as a routing feature.** The plumber does not just dispatch; it transforms. A routing rule that matches `parse.c:42` should extract the filename and line number and deliver them as separate attributes. Pane's routing rules should support extraction and rewriting, not just pattern matching and dispatch. This is the plumber's deepest contribution: the router does work on the data flowing through it.

3. **Lazy pane creation.** The plumber's `plumb client` starts an application if the destination port has no reader. Pane should support the same pattern: a routing rule can specify a pane type and application to launch if no handler is currently registered for the matched content. Pane-roster provides the launch infrastructure; the routing rule provides the trigger.

4. **Graceful degradation.** When the plumber cannot route a message (no matching rule, no reader on the port), the write to `/mnt/plumb/send` returns an error. Applications fall back to their own behavior. Pane's routing should do the same: if no rule matches, the kit should return a "not routed" result, and the application decides what to do.

**What to adapt:**

1. **Distributed evaluation instead of central server.** The plumber is a single process. Pane distributes rule evaluation into each client process via the kit. This means rule files must be watched for changes (pane-notify) and re-evaluated on modification. The plumber re-evaluated on every message; pane should re-load rules when the files change (not on every dispatch — cache the parsed rules).

2. **Typed messages instead of text headers.** The plumber's six-line text format is universal but untyped. Pane's routing messages should be typed — using the scripting protocol's `PropertyInfo` and `AttrValue` system. A routing message has a source, destination hint, working directory, content type, typed attributes, and payload. The typing provides validation; the text-based filesystem interface provides the universal fallback.

3. **No named ports as files.** The plumber's port files (`/mnt/plumb/edit`, `/mnt/plumb/image`) are the delivery mechanism: apps read from them. In pane's model, delivery goes through the compositor protocol or direct Messenger connections. Filesystem-based delivery (write to `/pane/plumb/send`, read from `/pane/plumb/edit`) should exist as the universal fallback for scripts, but kit-native routing should use typed channels.

**What to skip:**

1. **The file server architecture for the router itself.** The plumber is a 9P server because everything in Plan 9 is a 9P server. Pane's routing is a library, not a server. This is correct. Making routing a server reintroduces the single-point-of-failure problem. Making it a library means it cannot crash independently of the application using it.

2. **The click attribute and mouse-position-based extraction.** The plumber's click attribute is specific to acme's text-based interaction model. Pane has typed tag-line commands and a structured scripting protocol. Content extraction should use the optic-based specifier chain, not mouse-position regex matching. The B3-click behavior (route the clicked text) is a compositor feature (the tag line's command activation), not a routing feature.

### Warnings about naive translation

**The plumber was a central authority because Plan 9 had no alternative.** Each application running its own rules would mean inconsistent behavior. The pane-app kit solves this differently: rules are shared files, evaluation is distributed, but the behavior is consistent because every client reads the same rules. Do not add a plumber server "for consistency" — file-based rule sharing achieves the same consistency without the single point of failure.

**Rule conflicts are harder to debug when evaluation is distributed.** When the plumber is one process, you can trace exactly which rule matched. When every client evaluates rules independently, a mismatch between two clients (different rule file versions due to a race with pane-notify) can cause inconsistent routing. Mitigate this by giving rules a version number (hash of the rules directory) and logging it with each routing decision. Pane-fs can expose the current rule version per-process at `/pane/<id>/attrs/routing_version`.

---

## Cross-Cutting Themes

### The discipline of one interface

Plan 9's deepest lesson is not "everything is a file." It is: **the number of distinct interfaces is the enemy of composition.** Every additional protocol, API, or IPC mechanism is a boundary across which composition stops.

Pane has three interfaces: the typed protocol, the filesystem, and the in-process kit API. This is more than Plan 9's one, but it is a principled three. Each serves a different performance/safety tradeoff. The critical discipline is: do not add a fourth. Every new feature should be expressible through all three existing tiers, not through a new mechanism.

Concretely: when pane adds clipboard support, it should be a protocol extension (new `ClientToComp`/`CompToClient` variants), a filesystem projection (`/pane/<id>/clipboard`), and a kit API (`Clipboard::lock() -> ClipboardLock`). Not a separate D-Bus service. Not a custom IPC channel. Not a new socket.

### Transparency is a spectrum, not a binary

Plan 9 aimed for total transparency: remote files look exactly like local files. This goal is noble and mostly unreachable over real networks. Latency, partitions, and partial failures break the illusion.

Pane should aim for transparency of mechanism (the same protocol works locally and remotely) without transparency of performance (users and programs should know when they are operating remotely). Expose latency. Expose connection state. Make failure explicit. The three-tier model already supports this: the filesystem tier is the slow, universal, latency-visible interface; the protocol tier is the fast, typed, latency-sensitive interface.

### Failure must be part of the protocol

9P handles errors via `Rerror` — a message that replaces the expected response. Pane handles errors via `SessionError::Disconnected` and (in the future) error variants in protocol messages. The lesson from Plan 9's hung-mount problem is: **a missing response is the worst kind of error.** Pane's protocol should have timeouts at the transport layer and explicit "I don't know" responses at the protocol layer. A compositor that loses contact with a remote client should emit `PaneDisconnected` events, not silently keep the pane in the layout tree.

### Convention must be documented and enforced

Plan 9's conventions (clone/ctl/data, numbered directories, event-file-blocks) were powerful but informal. Pane's equivalent conventions (ctl file command format, attrs/ directory structure, event JSONL format) should be formally specified and validated. The typed protocol gives you compiler enforcement for native clients; the filesystem conventions need explicit documentation and test suites for script clients.

---

## Summary of Recommendations

| Area | Adopt | Adapt | Skip |
|---|---|---|---|
| **9P -> pane-proto** | Client-chosen IDs, explicit cancellation, filesystem as universal fallback | Version negotiation discipline | 9P wire format, request-response-only model |
| **Namespaces -> graded equivalence** | Per-observer views, .plan as namespace spec | Per-connection views in pane-fs (uid-based, not per-process) | Union directories, kernel namespace operations |
| **import/exportfs -> remote pane-fs** | Lazy connection, connection metadata exposure | Protocol bridge instead of 9P mount, event forwarding | General-purpose network filesystem, transparent latency |
| **cpu -> remote execution** | Reverse connection model, session persistence | Identity forwarding via TLS, no namespace reconstruction | Filesystem-based device export, forward mount model |
| **factotum -> .plan + TLS** | Separation of auth from app logic, confirmation gates | PeerIdentity as factotum-like interface, Transport::peer_identity() | Auth conversation in protocol, custom key management |
| **plumber -> pane routing** | User-editable rules, content transformation, lazy launch, graceful degradation | Distributed evaluation, typed messages, filesystem fallback | Central router server, mouse-position extraction |

## Confidence Assessment

- **Areas 1, 6 (protocol, routing):** High confidence. These are direct translations with clear tradeoffs. The typed protocol is strictly better than 9P for pane's use case; distributed routing is the right architecture.
- **Area 2 (namespaces):** High confidence in the recommendation to not attempt kernel namespaces. Medium confidence in the uid-based pane-fs view approach — this needs design work around edge cases (agents with multiple personas, shared agent groups).
- **Area 3 (remote mounting):** Medium-high confidence. The protocol bridge approach is sound but unproven at pane's scale. Latency handling needs empirical validation.
- **Area 4 (remote execution):** Medium confidence. The reverse connection model is clean but session persistence (reconnect + resume) is hard to get right. The state reconciliation after a reconnect — which pane events were missed, how to resync — is an open design problem.
- **Area 5 (authentication):** High confidence that pane should NOT build a factotum. Medium confidence in the specific TLS + .plan + Landlock layering — this is architecturally sound but the interaction between TLS client certs and .plan governance needs careful specification.
