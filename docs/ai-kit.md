# AI Kit

How AI agents participate in the pane ecosystem. Not a framework
— a mapping of agent needs onto unix multi-user infrastructure
and pane's protocol architecture.

The thesis: unix was designed for multiple inhabitants sharing a
system, collaborating, communicating, and governing shared
resources through composable protocols. The tools it built for
human coordination — accounts, permissions, `.plan` files,
`finger`, `who`, `mail`, `mesg`, `cron` — are exactly the tools
agents need. pane does not build an "AI framework." It recovers
what unix already solved and extends it with typed protocols,
namespace transparency, and headless-first architecture.

---

## 1. An Agent Is a Unix User

An agent has a unix user account. Not metaphorically — literally.

| Unix concept | Agent use |
|---|---|
| User account | Identity. `PeerAuth::Kernel { uid, pid }` identifies the agent to every pane service. |
| Home directory | Persistent state. Memories, configuration, project files, mail spool. |
| File permissions | Isolation. The agent can only access what unix permissions allow. |
| Process accounting | Auditing. Everything the agent did is logged by the kernel. |
| Nix user profile | Reproducibility. The agent's tools are declaratively specified, atomic, rollbackable. |
| `cron` / s6 timers | Scheduling. Periodic tasks (builds, monitoring, research) run on schedule. |

No special AI infrastructure required. The operating system
already knows how to authenticate, isolate, resource-account,
and communicate between multiple inhabitants. Agents use the
same mechanisms.

### Identity

An agent's identity is its uid. Local connections authenticate
via `SO_PEERCRED` (`PeerAuth::Kernel`). Remote connections
authenticate via TLS client certificate (`PeerAuth::Certificate`),
mapped to a local unix account by the server. The architecture
spec's transport-derived identity model (§Connection Model)
applies identically to agents and humans.

`who` shows which agents are logged in. `finger agent.builder`
shows the agent's `.plan` — what it does, what it's working on,
what it's allowed to access. Standard unix commands, standard
output, standard composability.

### Headless panes

An agent's running processes are headless panes — `Handler`
implementations connected to a pane server, participating in
the protocol, visible in the namespace at `/pane/<n>/`. An
agent pane has no `DisplayHandler` (no visual surface), but it
has full protocol participation: `Handles<P>` for service
protocols, `request_received` for ad-hoc inter-pane requests,
`pane_exited` for monitoring, `send_request` for typed
request/reply.

The same binary, the same protocol, the same Handler code that
runs headless can opt into display by adding `DisplayHandler`.
An agent that normally runs headless can present a visual
interface when a user opens a session with it.

---

## 2. The `.plan` File

Every agent has a `~/.plan` file — a declarative specification
of what it can see, do, and access. The name comes from Plan 9's
`finger` convention: `finger user` displays the user's `.plan`.
pane recovers the convention and gives it teeth.

### What `.plan` governs

| `.plan` declaration | Enforcement mechanism |
|---|---|
| Filesystem paths the agent can access | Landlock (kernel-enforced) |
| Operations it can perform | Landlock rules (read, write, execute, create) |
| Network destinations it can reach | Network namespaces |
| Which panes it can observe | pane-fs view filtering |
| What models it can use | Routing rules (local vs remote model) |
| What data it can send externally | Routing rules (data classification) |

The mapping from `.plan` declarations to kernel enforcement is
direct. What the `.plan` says is what the kernel enforces. There
is no gap between specification and enforcement — the `.plan` IS
the security policy, expressed as a file you can read, edit, and
version-control.

### Format

The `.plan` is a structured text file. The first section is
human-readable description (what `finger` displays). Subsequent
sections declare capabilities:

```
Plan: Development assistant for pane.
      Run test suites on commit.
      Monitor build output for patterns.
      Mail results to lane.

[access]
read = ~/src/pane, /pane/by-sig/com.pane.*
write = ~/mail, ~/memories, ~/tmp
execute = cargo, just, nix

[network]
allow = none

[models]
default = local
```

The `[access]` section maps to Landlock rules. The `[network]`
section maps to network namespace configuration. The `[models]`
section maps to routing rules for model invocation.

### Cross-user enrichment

An agent that needs to write to another user's pane (e.g., the
guide agent demonstrating features by modifying another pane's
attributes) requires explicit permission. The target user's
`.plan` or global policy must grant `enrich` permission to the
agent's uid. See `docs/legacy-wrapping.md` §3 (enrichment
protocol) for the mechanism — it applies identically to
cross-user agent access.

### Remote agents

A remote agent connecting over TLS is mapped to a local unix
account based on its certificate subject. The local account's
`.plan` governs what the remote agent can do — same enforcement,
same audit trail, same `finger` output. See
`docs/distributed-pane.md` §4 for the full identity and trust
model.

---

## 3. Communication

Each unix multi-user communication primitive maps to a concrete
pane mechanism. No custom notification framework — the channels
are unix channels, extended with pane's typed protocol and
namespace.

### `write(1)` — direct message to a terminal

Unix `write` copies lines from your terminal to another user's
terminal. The mechanism: it opens the target user's tty device
(`/dev/pts/N`) and writes to it. `mesg n` revokes write
permission on the tty (chmod), blocking `write` at the
filesystem level.

**pane equivalent:** an agent writes to another pane's `ctl`
file. `echo "notification: build succeeded" > /pane/3/ctl`
delivers a line to pane 3's handler as a `CommandExecuted`
event. The target pane's filter chain decides whether to
display it, queue it, or consume it. If the user has set
availability to "no interruptions" (see `mesg` below), the
notification filter queues to mail instead.

The mechanism is the same as unix `write`: write bytes to a
file that represents another user's interface. The permission
model is the same: the target's filesystem permissions (and
`.plan` Landlock rules) control who can write to its `ctl`.

### `talk(1)` — interactive session

Unix `talk` splits the terminal into two halves — simultaneous
bidirectional typing between two users. The mechanism: a
`talkd` daemon on each machine negotiates the connection; once
established, both terminals show both sides in real time.

**pane equivalent:** the user opens an interactive session pane
that connects to the agent's headless pane via an application-
defined `SessionProtocol`:

```rust
struct SessionProtocol;
impl Protocol for SessionProtocol {
    const SERVICE_ID: ServiceId = service_id!("com.pane.session");
    type Message = SessionMessage;
}

#[derive(Serialize, Deserialize)]
enum SessionMessage {
    Line(String),
    Typing { partial: String },
    End,
}
```

Both sides implement `Handles<SessionProtocol>`. The session
pane appears in the namespace — `cat /pane/9/body` shows the
conversation transcript. When either side sends `End`, the
session closes. The conversation is observable and scriptable
through pane-fs, unlike unix `talk` which left no trace.

### `mail(1)` — asynchronous messages

Unix `mail` delivers messages to a user's mail spool
(`/var/mail/<user>`). The message is a file. The recipient
reads it on their schedule. `mail -s "subject" user` sends;
`mail` reads.

**pane equivalent:** an agent writes a file to the target
user's mail directory (`~/mail/` or a shared mail spool). The
file carries pane-store attributes:

```
~/mail/build-result-2026-04-02
  user.pane.type = mail
  user.pane.from = agent.builder
  user.pane.subject = Build failed — session type mismatch
  user.pane.status = unread
  user.pane.component = pane-roster
  user.pane.commit = a3f2c91
```

pane-store indexes these attributes. "Show me all unread mail
from agent.builder where component is pane-roster" is a standard
pane-store query — the same query that indexes music, documents,
and every other file with typed attributes. The agent's mail IS
the filesystem. `cat ~/mail/build-result-2026-04-02` reads it.
`echo read > ~/mail/build-result-2026-04-02.status` marks it
read.

This is the BeOS email proof: Tracker (the file manager) became
the mail client because email was just files with queryable
attributes. pane recovers this — no mail application needed.
The file manager, pane-store queries, and `cat` compose into a
mail system because the infrastructure is right.

### `wall(1)` — broadcast to all users

Unix `wall` writes a message to every logged-in user's terminal.
The mechanism: it iterates `/dev/pts/*` and writes to each.

**pane equivalent:** an agent writes to a broadcast pane — a
headless pane that all interested parties monitor via the filter-
based exit/event notification mechanism. Or simpler: the agent
iterates `ls /pane/by-sig/com.pane.shell/` and writes a
notification to each shell's `ctl`. Same pattern as `wall` —
iterate the terminals, write to each.

### `mesg(1)` — availability control

Unix `mesg` controls whether `write` and `talk` can reach your
terminal. `mesg n` revokes write permission on your tty device.
`mesg y` restores it. The mechanism is filesystem permissions
on `/dev/pts/N`.

**pane equivalent:** a per-user attribute file at
`~/.config/pane/mesg` (or equivalently, a pane-store-indexed
attribute on the user's session). Value: `y` or `n`.

When `mesg` is `n`, agents check this before choosing delivery
mode. Direct notifications (writing to `ctl`) are deferred —
the agent writes to `~/mail/` instead. When `mesg` is `y`,
direct notifications are permitted.

The check is a convention enforced by well-behaved agents (same
as unix — `mesg n` only blocks `write` because `write` checks
permissions; a root process can still write to the tty). The
`.plan` provides the hard enforcement: if an agent's `.plan`
doesn't grant write access to the user's panes, `mesg` is
irrelevant — Landlock blocks the write regardless.

---

## 4. Memory

An agent's memories are files in its home directory. Each memory
is a file with typed attributes — what kind of memory it is, how
important it is, when it was created, what it's about.

```
~/memories/
  debugging-session-2026-04-01.md
  architecture-insight-protocol-split.md
  user-preference-commit-style.md
```

Each file carries pane-store attributes:

```
user.pane.type = memory
user.pane.kind = debugging
user.pane.importance = high
user.pane.created = 2026-04-01T14:30:00Z
user.pane.tags = protocol, session-types
```

pane-store indexes these attributes. "Show me all memories tagged
'protocol' from the last week" is a standard pane-store query —
the same query mechanism that indexes email, music, documents, and
every other file with typed attributes. The agent's memory system
IS the filesystem. Nothing opaque, nothing proprietary.

### Memory vs pane state

Agent memories (files in `~/memories/`) are persistent knowledge
that survives across sessions — learned preferences, project
context, domain knowledge. Agent pane state (`/pane/<n>/attrs/`,
`/pane/<n>/body`) is live operational state — what the agent is
currently doing, its current output, its current status. Both
are files. Both are queryable. They differ in lifecycle, not in
mechanism.

---

## 5. Agents Build Things

An agent modifies the system by producing artifacts on the same
surface that human developers use. It doesn't call internal APIs
or modify hidden state — it writes files.

| Agent action | Mechanism |
|---|---|
| Create a routing rule | Write a rule file to `~/.config/pane/route/rules/` |
| Customize the environment | Write a config file to `/etc/pane/` or `~/.config/pane/` |
| Build a tool | Write a script, add it to the agent's Nix profile |
| Extend a bridge | Add commands/properties to a `.app` bundle's `bridge/` |
| Share a configuration | Copy files to a shared directory or git repo |

Every artifact is a file. Every file is inspectable, versionable,
shareable, reversible. Over time, a user's collection of agent-
built customizations becomes a personal configuration — the same
composability that vim/emacs plugin ecosystems provide, but with
agents as contributors alongside humans.

---

## 6. Model Routing

The choice of what data goes to which model is expressed as a
routing rule — a file, not a setting.

```
# ~/.config/pane/models/routing.toml

[default]
model = "local"          # local model for everything by default

[rules]
# Work directory contents stay local
[[rules.local]]
match = "path:~/src/*"

# General knowledge questions can go remote
[[rules.remote]]
match = "kind:general-knowledge"
provider = "api.anthropic.com"

# Credentials never leave the machine
[[rules.deny]]
match = "kind:credential"
destination = "*"
```

The routing rule IS the privacy policy. It is a file — readable,
editable, shareable, version-controllable. A user running entirely
on local models gets the same agent infrastructure as someone with
API access. The difference is one file.

---

## 7. Presence and Discovery

Each unix presence/discovery command maps to concrete pane
infrastructure. No agent dashboard — standard unix commands
compose with pane-fs.

### `who(1)` — who is logged in

Unix `who` reads `/var/run/utmp` (or equivalent) and lists
logged-in users with their terminal and login time.

**pane equivalent:** `who` works as-is for agents with unix
accounts. An agent that has an active pane connection is a
logged-in user. `who` shows:

```
lane     pts/0        2026-04-02 09:00
agent.builder  pts/1  2026-04-02 09:01
agent.reviewer pts/2  2026-04-02 09:01
```

`who | grep agent` shows all active agents. The utmp entries
are written by the agent's session manager (s6 service) when
the agent connects to its pane server.

### `finger(1)` — user information lookup

Unix `finger` displays user information: login name, real name,
home directory, shell, login time, and the contents of `~/.plan`
and `~/.project`. For remote users: `finger user@host` queries
the remote machine's finger daemon.

**pane equivalent:** `finger agent.builder` works as-is and
displays the agent's `.plan` — its purpose, current work, and
permissions. This is the primary discovery mechanism. A user
who wants to know "what is agent.builder doing and what can it
access?" runs `finger agent.builder` and reads the output.

```
$ finger agent.builder
Login: agent.builder          Name: Build Agent
Directory: /home/agent.builder Shell: /bin/pane-agent
On since Apr  2 09:01 on pts/1

Plan:
Development build agent for pane.
Run test suites on commit.
Monitor build output for patterns.
Mail results to lane.

[access]
read = ~/src/pane, /pane/by-sig/com.pane.*
write = ~/mail, ~/memories, ~/tmp
execute = cargo, just, nix

[network]
allow = none
```

For remote agents: `finger agent.builder@headless.internal`
queries the remote machine's finger daemon, displaying the
remote agent's `.plan`. Same command, same output format,
network-transparent.

### `ls(1)` + pane-fs — what is the agent doing

`finger` shows identity and governance. pane-fs shows live
operational state:

```
$ ls /pane/by-sig/com.pane.agent.builder/
5  8

$ cat /pane/5/body
building pane-proto... ok (2.3s)
building pane-session... ok (1.1s)
running tests...

$ cat /pane/5/attrs/status
building

$ cat /pane/8/body
watching ~/src/pane for changes
```

The agent's running panes are discoverable via the per-signature
index. Their `body` shows current output. Their `attrs/` show
structured state. This is live — not a cached snapshot.

### Composability

No dashboard needed. Standard tools compose:

```sh
# All active agents
who | grep agent

# All agents' plans
for agent in $(who | grep agent | awk '{print $1}'); do
  echo "=== $agent ==="
  finger $agent
done

# All build agent panes and their status
for pane in $(ls /pane/by-sig/com.pane.agent.builder/); do
  echo "pane $pane: $(cat /pane/$pane/attrs/status)"
done

# Unread mail from any agent
find ~/mail -newer ~/.last-read -exec cat {} \;
```

The discovery interface is `ls`, `cat`, `finger`, `who`. The
query interface is pane-store. The monitoring interface is
pane-notify (watch for changes) or `/pane/<n>/event` (blocking
read). All standard tools, all composable.

---

## 8. The Guide

The canonical first agent. Its purpose: teach new users by
demonstrating the system. The guide uses pane *using pane*.

### What makes the guide possible

- **Headless pane**: the guide runs without a display. It
  connects to the pane server, participates in the protocol,
  appears in the namespace.
- **Scripting via pane-fs (Phase 3 — RoutingHandler)**: the
  guide reads and writes other panes' properties to demonstrate
  features. `echo "dark" > /pane/2/attrs/theme` shows theming.
  `cat /pane/1/attrs/cursor` points out where the user is.
- **Clipboard**: the guide copies example commands to the
  clipboard for the user to paste. Clipboard is an independent
  service Connection — no display access needed.
- **Self-description via pane-fs**: `cat /pane/7/body` returns
  what the guide is currently saying. `cat /pane/7/attrs/topic`
  returns what it's teaching. A curious user discovers this and
  learns the namespace by using it to inspect their teacher.
- **`.plan` governance**: the guide's `.plan` declares read
  access to the user's panes (for demonstration) and write
  access to its own state. Landlock enforces it. The user can
  read the guide's `.plan` to understand exactly what it can do.

### Development methodology

The guide agent inhabits the system from the earliest possible
moment — not as a feature to ship later, but as a development
tool. The guide that will eventually help new users begins its
life as the agent that helps build pane. Its failures are the
system's integration tests. Its needs drive the API design. See
`docs/development-methodology.md` for the full rationale.

---

## 9. Sandboxed Agent Compute

On Linux, agents can run their own agent infrastructure inside
KVM virtual machines (QEMU on macOS). The VM is a hardware-
isolated compute environment that communicates with the agent's
userspace through pane's protocol. This is not a special feature
— it is the recursive application of the existing architecture.

### How it works

A VM running pane-headless is just another pane server. The
agent connects to it via `App::connect_service()` over TCP/TLS
— the multi-server Connection model that the architecture spec
already defines. The VM's panes appear in the agent's namespace
alongside its own panes, with locally-assigned numeric IDs.

```
agent.builder (uid 1001)
  ├─ /pane/1/  ← agent's own monitoring pane (headless, local server)
  ├─ /pane/2/  ← agent's build status pane (headless, local server)
  └─ /pane/3/  ← build sandbox shell (headless, VM server via TLS)
      └─ VM runs pane-headless, isolated by KVM
         ├─ untrusted build commands execute here
         ├─ /pane/1/ inside the VM = /pane/3/ from the agent's view
         └─ agent reads /pane/3/body for build output
```

The agent's `.plan` governs what passes between the agent and
the VM:
- Filesystem mounts passed through (virtiofs or 9pfs)
- Network access granted to the VM (network namespace)
- What the VM's pane server can DeclareInterest for

### Why VMs, not just Landlock

Landlock provides filesystem and network sandboxing at the
process level — sufficient for trusted agent code operating
within its declared permissions. But agents sometimes need to
run *untrusted* code: user-submitted scripts, experimental
builds, third-party tools. For these, process-level sandboxing
is insufficient — the untrusted code may exploit kernel
vulnerabilities that Landlock cannot prevent.

KVM provides hardware-enforced isolation. The VM has its own
kernel, its own memory space, its own device model. A
compromised process inside the VM cannot escape to the host.
The cost is a VM boot — seconds, not minutes, with
microVM approaches (firecracker-style, or QEMU with minimal
firmware).

### The `cpu` pattern

This is Plan 9's `cpu` command applied recursively. In Plan 9,
`cpu` ran computation on a remote machine while I/O stayed
local. The agent's VM is the same pattern — computation
(untrusted build, code execution, experiment) runs in the VM;
results flow back through the pane protocol to the agent's
namespace.

The agent doesn't shell out to the VM. It connects to the VM's
pane server and uses `send_request`, `Handles<P>`, and pane-fs
exactly as it would with any other server. If the VM crashes,
the agent receives `PaneExited { reason: Disconnected }` on
the affected Connection — per-Connection failure isolation.
Other Connections are unaffected.

### Use cases

**Build sandboxes.** A build agent runs untrusted build commands
in a disposable VM. The build output streams through
`/pane/3/body`. When the build completes or fails, the VM is
destroyed. Clean environment every time — stronger than Nix's
build sandbox because the isolation is hardware-level.

**Code execution.** An agent evaluates user-submitted code in a
VM. The code runs in a pane-shell inside the VM. The agent reads
the output via pane-fs, applies its judgment, reports results.
If the code tries to escape the sandbox, it hits KVM, not
Landlock.

**Sub-agent ecosystems.** An agent runs its own sub-agents inside
a VM — each sub-agent is a unix user in the VM, with its own
`.plan`, its own pane connections. The outer agent orchestrates
by connecting to the VM's pane server and observing sub-agent
panes in its namespace. This is recursive multi-user
infrastructure.

**Experimentation.** An agent tests a system configuration change
in a VM before applying it to the host. It boots a VM with the
proposed configuration, runs tests, observes results through
pane-fs, and reports whether the change is safe. The VM is
disposable — the experiment is free.

### Phase mapping

| Component | Phase |
|---|---|
| Agent connects to VM's pane-headless | Phase 2 (multi-server + TLS) |
| VM panes in agent namespace | Phase 2 (unified namespace) |
| `.plan` governs VM access | Phase 1 (.plan parser + Landlock) |
| microVM tooling (pane-vm) | Post-Phase 2 (convenience, not core) |

---

## 10. Relationship to Architecture Spec

The AI Kit introduces no new protocol, no new service, no new
runtime concept. It is a usage pattern over existing
infrastructure:

| Architecture concept | AI Kit use |
|---|---|
| Handler (headless) | Agent processes are headless panes |
| PeerAuth::Kernel | Agent identity from uid via SO_PEERCRED |
| PeerAuth::Certificate | Remote agent identity from TLS certificate |
| pane-fs namespace | Agent panes visible at `/pane/<n>/` |
| pane-fs `by-sig` view | `ls /pane/by-sig/com.pane.agent.builder/` |
| Messenger + send_request | Inter-agent and agent-user communication |
| Handles\<P\> | Typed protocol participation (e.g., SessionProtocol) |
| pane-store attributes | Memory indexing, mail indexing, query |
| pane-notify | File watching (commit monitoring, config changes) |
| Routing rules | Model selection, data classification |
| `.plan` (distributed-pane §4) | Governance → Landlock + network namespace enforcement |

The one new component the AI Kit needs: **a `.plan` parser** that
translates `.plan` declarations into Landlock rules, network
namespace configuration, and pane-fs view filters. This is a
build-time tool (part of the agent's s6 service definition), not
a runtime service.

### Phase mapping

| Component | Phase |
|---|---|
| Agent user accounts + `.plan` files | Phase 1 (unix infrastructure, no pane dependency) |
| Agent headless panes | Phase 1 (Handler, headless server) |
| Agent communication (mail, notifications) | Phase 1 (filesystem + pane-notify) |
| Agent memory (pane-store queries) | Phase 2 (requires pane-store) |
| Agent pane-fs presence | Phase 2 (requires pane-fs) |
| Guide agent cross-pane scripting | Phase 3 (requires RoutingHandler) |
| Model routing rules | Phase 2 (requires routing subsystem) |
| `.plan` parser → Landlock | Phase 1 (standalone tool) |

---

## 11. What This Is Not

- **Not an AI framework.** No `AgentRuntime`, no `AgentManager`,
  no `AgentProtocol`. Agents are users. Users run programs.
  Programs are panes. Panes speak the protocol.

- **Not an orchestration system.** Agents coordinate through
  mail, pane-fs, and the protocol — the same way human users
  coordinate. No central coordinator, no DAG engine, no
  workflow DSL.

- **Not a model hosting system.** pane does not run models. It
  routes requests to models (local or remote) via routing rules.
  The model is infrastructure the agent uses, not infrastructure
  pane provides.

- **Not cloud-dependent.** A user running entirely on local
  models gets the same agent infrastructure. The difference
  between local and cloud is one routing rule file.

---

## Sources

- Unix multi-user infrastructure: `who(1)`, `finger(1)`,
  `mesg(1)`, `.plan` convention, `mail(1)`, `cron(8)`
- Plan 9: per-process namespaces, `.plan` files, `finger`,
  factotum (§Identity and Trust in `docs/distributed-pane.md`)
- pane architecture: Handler, headless-first, pane-fs, Protocol,
  PeerAuth (`docs/architecture.md`)
- Distributed pane: identity model, `.plan` governance
  (`docs/distributed-pane.md` §4)
- The guide agent: `docs/use-cases.md` §7
- Agent vision: `docs/agents.md`, `docs/agent-perspective.md`
