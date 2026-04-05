# Distributed Pane

The host machine is just a server. Its privilege as the user's interface is contingent — it has a display, a keyboard, low-latency I/O — but these are performance characteristics, not architectural distinctions. The local compositor is a server your eyes happen to be connected to. A headless instance in the cloud is the same thing without the display. A remote pane is the same as a local pane with higher latency.

This is the same principle as "the pane as universal object" (foundations §2) applied to the system's topology. Just as a pane is one object with many views — visual, protocol, filesystem, semantic — a pane system is one system with many hosts. The views are projections of the same state through different transports, at different latencies. The data does not change because it crosses a network boundary. The grading that restricts each observer's view (foundations §2) applies to network topology the same way it applies to permissions: a remote observer sees a coherent quotient of the system through the same protocol, just over a different transport.

This principle is the synthesis of two design traditions. BeOS proved that when everything communicates the same way (BMessage, BLooper, typed messaging), integration is natural — but app_server was architecturally special, the one component that couldn't be treated as just another server. Plan 9 proved that when nothing is special because it's local (9P, per-process namespaces, `import`, `cpu`), distribution is natural — but it never built the desktop experience that would have made the idea sing. Pane brings both: the messaging discipline that makes a desktop feel alive, and the location independence that makes distribution invisible.

---

## 1. The Foundational Deployment Model

Headless pane is not a secondary deployment target. It is the foundation that the full desktop extends.

A headless pane instance speaks the complete protocol — session-typed handshake, active-phase messaging, identity forwarding, capability negotiation — without rendering. It manages panes, routes messages, participates in service discovery, hosts the filesystem namespace. The only thing it does not do is put pixels on a screen. Everything else is identical.

This means the full Pane Linux desktop is a strict superset of a headless deployment: a headless instance plus a compositor, plus a display, plus fonts, plus a session manager. Remove those layers and you still have a functioning pane system — just one that interacts through the protocol and filesystem tiers rather than the visual tier.

### The adoption funnel

The strategic consequence: users adopt pane without reinstalling their operating system.

```
nix flake on any unix-like
  → headless pane: server, kits, protocol, filesystem interface
  → configuration accumulates in nix expressions
  → add compositor, then desktop, as readiness grows
  → the flake IS the seed of a full Pane Linux configuration
```

On Darwin, the user gets headless pane via a nix-darwin module — pane-headless supervised by launchd, the kit crates for building pane-native applications, and the pane-fs namespace for scripting. On NixOS, the same via a NixOS module with systemd supervision. On Pane Linux, the same services run natively under s6-rc. The user's `pane.services.*` configuration is the same in all three cases — only the init backend changes.

Settings transfer because they were always nix expressions. When a user graduates from headless-on-Darwin to full Pane Linux, their configuration carries over without migration.

---

## 2. Network Transparency

### The Transport trait

Pane's session-typed channels are parameterized over transport: `Chan<S, T>` where `T: Transport`. The `Transport` trait requires two methods:

```rust
fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError>;
fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError>;
```

Everything above this — the session-typed handshake, the active-phase protocol, message serialization, protocol correctness — is transport-agnostic. Adding a `TcpTransport` (or TLS-wrapped variant) makes the entire protocol stack network-transparent with zero changes to the protocol logic.

This is the same property that made 9P powerful: the protocol was independent of its transport. Plan 9 ran 9P over pipes, TCP, TLS, and IL (their custom reliable datagram protocol). The protocol didn't care. Pane's Transport trait provides this property with an additional guarantee Plan 9 did not have: compile-time verification that both sides of the conversation agree on the protocol structure.

### What travels over the network

The pane protocol is already message-oriented and serialized (postcard, length-prefixed frames with per-service wire discriminants). Every `ClientToServer` and `ServerToClient` message is a self-contained value — no pointers, no shared memory references, no file descriptors. This means every protocol message can cross a network boundary without transformation. The handshake, the active phase, and the teardown work identically over unix sockets and TCP.

What changes for remote connections:

- **Latency.** Local unix sockets: ~1.5-3μs. LAN TCP: ~100-500μs. WAN TCP: ~5-50ms. The protocol's async-by-default design (fire-and-forget operations batched, sync only when a response is needed) mitigates this, but interactive operations (input dispatch, completion requests) will feel the latency.

- **Identity.** Local unix sockets carry implicit identity via `SO_PEERCRED` (kernel-verified uid, pid). TCP connections carry transport-derived identity via TLS client certificates. In both cases, `PeerAuth` is derived from the transport — not declared in the handshake. The identity model is the same — unix users — but the verification mechanism changes.

- **Failure modes.** Local connections fail cleanly (process exits, fd closes). Network connections fail ambiguously (timeout, partition, half-open). The session-typed crash boundary (`SessionError::Disconnected`) handles both, but remote connections need reconnection semantics that local connections do not.

### The four-tier access model

The architecture's three-tier access model extends to four tiers for distributed pane:

| Tier              | Mechanism        | Latency   | Use case                                      |
| ----------------- | ---------------- | --------- | --------------------------------------------- |
| In-process        | Kit API          | Sub-μs    | Application logic within a pane-native client |
| Protocol (local)  | Unix sockets     | ~1.5-3μs  | Kit-to-server, rendering, input dispatch      |
| Filesystem        | FUSE at `/pane/` | ~15-30μs  | Scripts, inspection, configuration            |
| Protocol (remote) | TCP/TLS          | ~0.5-50ms | Cross-instance communication                  |

The tiers are not architectural boundaries — they are latency characteristics of the same protocol. A message sent via `Messenger::send_message()` reaches its destination whether that destination is a local looper, a local compositor, or a remote headless instance. The kit chooses the tier; the developer doesn't.

---

## 3. The Unified Namespace

### pane-fs as query system

pane-fs is the unification of two ideas that arrived from different directions.

BFS (the Be File System) stored typed attributes on files — `MAIL:from`, `MAIL:subject`, `MAIL:status` — and indexed them. Live queries (`MAIL:status == New`) returned dynamic result sets that updated as files changed. The query engine was the database; the filesystem was the interface. Email, contacts, music libraries — different attributes, same mechanism.

Plan 9's synthetic filesystems presented computed views as file trees. `/proc/` wasn't a snapshot of process state; it was a live projection. `/dev/cons` wasn't a device node; it was a connection to whichever program was serving your terminal. The filesystem wasn't storage — it was a protocol interface for any tool that could read files.

pane-fs is both. Every directory in the pane-fs hierarchy is a _view_ — a computed projection of the underlying state through a filter predicate:

- `/pane/` — all panes (local and remote), numbered sequentially
- `/pane/by-uuid/<uuid>/` — stable global identity (symlinks to `/pane/<n>/`)
- `/pane/by-sig/com.pane.agent/` — panes with signature `com.pane.agent`
- `/pane/by-type/shell/` — panes of type `shell`
- `/pane/self/` — calling pane's own directory
- `/pane/local/` — panes on this instance
- `/pane/remote/` — panes on remote instances
- `/pane/remote/<host>/` — panes on a specific remote host

These are all projections over the same indexed state. The top-level uses short numeric IDs (like Plan 9's `/proc/<pid>`). Remote panes receive local numbers when mounted — namespace transparency. `by-uuid` provides cross-machine stable reference. `local` and `remote` are discovery views, not architectural boundaries.

### Why unified, not segmented

The alternative — remote panes live under `/pane/remote/<host>/` and local panes under `/pane/` with no overlap — creates a two-namespace problem. Scripts must know whether a pane is local or remote to construct the right path. Tools that list panes must query two locations. The architectural boundary leaks into every consumer.

Plan 9 taught this lesson: the power of `import` was that after mounting a remote fileserver, local tools worked on remote resources _without modification_. The network boundary was in the namespace composition, not in the applications. pane-fs should provide the same property: a script that reads `/pane/3/body` gets the content regardless of where the pane lives.

The potential concerns with unified namespaces — collision, latency, ambiguity — resolve cleanly:

**Collision.** Pane Ids are UUIDs — globally unique by construction. The filesystem uses short numeric local IDs (`/pane/1/`, `/pane/2/`); the UUID lives in `/pane/by-uuid/<uuid>/` as a stable cross-machine reference. Two panes on different instances never collide at the UUID level; local numbers are per-namespace.

**Listing latency.** pane-fs does not query remote servers on every `readdir`. It reads from pane-store's local index, which maintains a cached view of remote pane metadata updated asynchronously via change notifications over the protocol. This is the BFS live query model: the result set is maintained, not recomputed. A remote host going down means its entries go stale and get marked — not that `ls /pane/` hangs.

**Write routing.** Reading remote pane state goes through the cached index. Writing (to `ctl`, `tag`, `attrs/`) routes over TLS to the owning instance. The UUID (looked up from the local number via `by-uuid`) tells pane-fs which instance to contact. This is transparent to the writer.

**Ambiguity.** There is none. Each pane has a globally unique ID. Each path resolves to exactly one pane. The computed views are filters, not union mounts — there is no mount-order dependency, no MBEFORE/MAFTER ambiguity. Plan 9's union directory problems don't apply because pane-fs doesn't do unions. It does computed projections over an indexed store.

### Routing rules and transport awareness

Pane routing rules — kit-level content-to-handler dispatch — operate over the unified namespace. A routing rule that says "open text files with the editor" works regardless of whether the editor pane is local or remote.

For cases where transport matters (latency-sensitive operations, bandwidth constraints, privacy), routing rules can filter on topology metadata:

```
# Route to local handler when available, remote as fallback
match type:text/plain → handler:com.pane.editor topology:local
match type:text/plain → handler:com.pane.editor
```

The specific syntax and semantics of transport-aware routing are to be determined, but the principle is: topology is metadata that routing rules can match on, not an architectural fork.

---

## 4. Identity and Trust

### Unix identity over the network

The trust model is unix identity, extended to the network. There is no pane-specific authentication scheme. The existing unix primitives — users, groups, file permissions, Landlock, namespaces — provide the enforcement. Pane's job is to carry the identity faithfully across the network transport and let the existing infrastructure enforce it.

For local connections (unix domain sockets), identity is implicit: `SO_PEERCRED` provides the kernel-verified uid of the connecting process. No declaration needed, no spoofing possible.

For remote connections (TCP/TLS), identity is derived from the TLS client certificate: `PeerAuth::Certificate { subject, issuer }`. The Hello message carries no identity — authentication is transport-level (see architecture spec §Connection Model). The server maps the certificate subject to a local unix user for enforcement purposes. A remote agent whose certificate maps to `agent.reviewer` gets the filesystem permissions, Landlock constraints, and `.plan` governance of the local `agent.reviewer` account. If no local account maps, the connection is rejected.

### The `.plan` file

An agent's `.plan` is a declarative specification of what it can see, do, and access. It lives in the agent's home directory — human-readable, editable, version-controllable. The `.plan` governs behavior regardless of whether the agent is local or remote:

- What panes it can observe (via pane-fs view filtering)
- What operations it can perform (via Landlock enforcement)
- What network destinations it can reach (via network namespaces)
- What models it can use and what data it can send externally (via routing rules)

For remote agents, the hosting instance verifies the `.plan` before granting access. The `.plan` is the single source of truth for agent permissions — not a separate ACL, not a permission dialog, not a hidden configuration file.

### Plan 9's factotum and why pane doesn't need one

Plan 9 separated authentication from services via factotum — a per-user authentication agent that held keys and mediated between clients and servers. This was necessary because every 9P service needed an authentication conversation, and embedding auth in each service was untenable.

Pane's architecture resolves this differently. TLS handles transport-layer authentication (the certificate is the key). `.plan` handles authorization (what you're allowed to do). Landlock handles enforcement (kernel-level constraints). No individual service needs to implement authentication — the transport layer and the operating system handle it.

The `Transport` trait provides identity uniformly via `PeerAuth`:

- `UnixTransport`: `PeerAuth::Kernel { uid, pid }` from `SO_PEERCRED`
- `TlsTransport`: `PeerAuth::Certificate { subject, issuer }` from client certificate
- `MemoryTransport`: test-configured `PeerAuth`

Each transport derives identity its own way. The protocol layer sees `PeerAuth` regardless of how it was obtained. This is factotum's principle — separate auth from application logic — achieved through Rust's trait system rather than a separate daemon.

The core/full server decomposition and the Nix flake architecture
are documented in `docs/pane-linux.md`.

---

## 5. Emergent Patterns

Network-transparent pane enables compositions that no individual feature was designed to provide.

**Distributed agent ecosystems.** An AI agent runs as a unix user on a headless pane instance in the cloud. It has its own home directory, its own `.plan`, its own memories (files with typed attributes indexed by pane-store). From the local desktop, it appears as another pane in the unified namespace — its output is readable at `/pane/8/body` (where 8 is the local number assigned when the remote pane was mounted), its state is queryable via pane-store, its behavior is governed by a `.plan` you can edit. The agent doesn't know or care whether you're on the same machine. The pane protocol handles the transport.

**Remote development environments.** A developer connects their local pane desktop to a headless instance running on a build server. The build server's panes (compiler output, test results, log streams) appear in the local unified namespace alongside local panes, with locally-assigned numbers. Routing rules direct "open file" actions to the remote editor. The filesystem at `/pane/12/attrs/cwd` shows the remote working directory. The developer's muscle memory works — same keystrokes, same commands, same routing — the computation just happens elsewhere.

**Multi-machine workflows.** A music production setup: local machine runs the UI panes, a rack server runs DSP processing panes, a NAS hosts the project files. All three are pane instances. The unified namespace shows every pane. pane-store indexes attributes across all instances. A routing rule says "when I activate an audio file, open it in the DSP pane on the rack server." The user sees one workspace. The infrastructure sees three machines.

**Development velocity.** pane-headless eliminates the compositor as a development bottleneck. Before headless, every subsystem that needed to run was gated on pane-comp being functional enough to connect to — a graphical compositor running in a VM. With pane-headless, subsystem development parallelizes: pane-roster, pane-store, pane-fs, the scripting protocol, the AI kit, and routing all develop and test against the headless server. The compositor becomes the last mile — chrome rendering, input dispatch, layout visualization, Wayland legacy support. Everything else can be built, tested, and even deployed before the compositor is feature-complete.

**The guide agent.** The onboarding agent described in the introduction doc — the guide who teaches pane by using pane — can run on a remote instance with more compute than the user's machine. The guide's panes appear locally. The user learns the system from a fellow inhabitant who happens to live in the cloud.

---

## 6. Comparison

### Plan 9

Plan 9's distributed computing model is pane's primary design reference for network transparency. The mapping:

| Plan 9                 | Pane                                     | Adaptation                                                                 |
| ---------------------- | ---------------------------------------- | -------------------------------------------------------------------------- |
| 9P                     | pane-proto over Transport                | Richer protocol (session types, typed enums); same transport independence  |
| Per-process namespaces | Graded equivalence via `.plan` + pane-fs | Per-user views (not per-process); pane-fs serves different content per uid |
| `import`               | pane-fs remote namespace queries         | Computed views over pane-store index, not 9P file mounting                 |
| `cpu`                  | `App::connect_remote()`                  | Reverse connection — remote app connects back to local compositor          |
| factotum               | `.plan` + TLS + Landlock                 | Same principle (separate auth from services); different mechanism          |
| plumber                | pane-app kit-level routing               | Distributed evaluation, typed messages, filesystem fallback                |
| /srv                   | `/srv/pane/` + pane-roster               | Service directory + active federation                                      |
| Synthetic filesystems  | pane-fs computed directories             | BFS query semantics as filesystem paths                                    |

Plan 9's discipline — a minimal set of powerful abstractions, taken seriously across the whole system — is what pane recovers. The specific mechanisms differ because pane sits on Linux, not a custom kernel. But the design question is always the same: does this abstraction earn its keep?

### BeOS

BeOS's contribution to pane's distributed model is the messaging discipline and the API aesthetics that make the system usable. The protocol that travels over the network is descended from BMessage. The kit API that developers use to build pane-native applications is descended from the Application Kit. The attribute indexing that powers the unified namespace is descended from BFS.

BeOS's limitation was that app_server was architecturally special — the one component that couldn't be treated as just another server. Pane corrects this. The compositor is a server. Remove it and you still have a functioning system. This is the insight that makes headless deployment possible and distribution natural.

### sixos

sixos provides the distribution substrate. Where NixOS couples to systemd, sixos replaces it with s6 — a process supervision system whose philosophy (small tools, explicit dependencies, race-free startup) aligns with pane's own design values. The relationship is: nixpkgs provides packages, sixos provides the system builder, pane provides the personality.

---

## Sources

- Pike, Presotto, et al., "Plan 9 from Bell Labs" (Computing Systems, 1995)
- Pike, "The Use of Name Spaces in Plan 9" (OSDI, 1992)
- Presotto, Winterbottom, "The Organization of Networks in Plan 9" (USENIX, 1993)
- Cox, Grosse, Pike, Presotto, Ritchie, "Security in Plan 9" (USENIX Security, 2002)
- Pike, "Plumbing and Other Utilities" (USENIX, 2000)
- Mirtchovski, Simmonds, Minnich, "Persistent 9P Sessions for Plan 9" (IWP9, 2006)
- amjoseph, "sixos — a nix os without systemd" (38c3, 2025; codeberg.org/amjoseph/sixos)
- u-root/cpu: Plan 9 cpu semantics over SSH (github.com/u-root/cpu)
