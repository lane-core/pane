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

Agents communicate through the same graduated model unix
developed for human coordination. No custom notification
framework, no message bus, no pub/sub system. The channels
are unix channels, extended with pane's typed protocol.

### Brief notifications

An agent posts a one-liner to the user's notification stream.
The user glances at it, continues working. No conversation
opened, no context switch.

Implementation: the agent writes to a notification pane (a
headless pane the user's session watches). The compositor
renders it as transient chrome. pane-fs makes it scriptable:
`cat /pane/12/body` shows the notification text.

### Focused sessions

Real-time, bidirectional, ephemeral. The user opens an
interactive session with an agent — pair programming, design
discussion, debugging. When done, the session closes.

Implementation: the user's terminal pane and the agent's
headless pane exchange messages via `send_request` / `Handles<P>`
with an application-defined `SessionProtocol`. The interaction
is typed and session-managed. The session pane appears in the
namespace; the conversation is observable and scriptable.

### Asynchronous messages (mail)

The agent leaves a message in the user's mail spool — a file
with typed attributes (`type`, `status`, `subject`, `date`,
`component`, `commit`). The user reads it on their schedule.

Implementation: a file in `~/mail/` with pane-store-indexed
attributes. `cat ~/mail/build-result-2026-04-02` reads the
message. Queries like "show me all build failures this week
where component is pane-roster" are pane-store attribute queries
over the mail directory. This is the BeOS email proof: no
component was designed to be a "build result tracker." The mail
infrastructure, the attribute store, the query engine compose
into one because the infrastructure is right.

### Availability control

`mesg n` — don't interrupt me. Agents queue everything as
asynchronous messages instead of notifications. `mesg y` —
open the channel. Standard unix, standard semantics.

Implementation: a per-user flag file or attribute. Agents check
it before choosing notification (synchronous) vs mail
(asynchronous) delivery. The flag is a file — composable,
scriptable, queryable.

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

Standard unix commands provide agent presence and discovery.

| Command | What it shows |
|---|---|
| `who` | Which agents are logged in (have active pane connections) |
| `finger agent.builder` | The agent's `.plan` — purpose, current work, permissions |
| `ls /pane/by-sig/com.pane.agent.builder/` | The agent's running panes |
| `cat /pane/5/body` | The agent's current output |
| `cat /pane/5/attrs/status` | The agent's current status |
| `ls ~/agent.builder/mail/` | The agent's outbox (messages it sent) |

No dashboard needed. The presence information is files, the
discovery is directory listings, the status is pane attributes.
Standard tools compose: `who | grep agent` shows all active
agents. `finger -l agent.*` shows all agents' plans.

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

## 9. Relationship to Architecture Spec

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

## 10. What This Is Not

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
