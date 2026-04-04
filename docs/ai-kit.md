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

`who` shows which agents are logged in. `finger ada`
shows the agent's `.plan` — what it does, what it's working on.
Standard unix commands, standard output, standard composability.

### Headless panes

An agent's running processes are headless panes — `Handler`
implementations connected to a pane server, participating in
the protocol, visible in the namespace at `/pane/<n>/`. An
agent pane has no `DisplayHandler` (no visual surface), but it
has full protocol participation: `Handles<P>` for service
protocols, `request_received` for ad-hoc inter-pane requests,
`pane_exited` for monitoring, `send_request` for typed
request/reply.

The agent's s6 service harness allocates a login session (utmp
entry, PTY) before exec'ing the agent binary. The agent is both
a Handler speaking the pane protocol and a unix user with a
terminal it can drive programmatically — spawning subprocesses,
running shell commands, the same way a coding assistant drives
a terminal today. The PTY is infrastructure the harness
provides; the Handler doesn't manage it. This makes the agent
addressable via `write` and `talk` alongside its protocol
participation — two interfaces to the same process, not an
impedance mismatch.

The same binary, the same protocol, the same Handler code that
runs headless can opt into display by adding `DisplayHandler`.
An agent that normally runs headless can present a visual
interface when a user opens a session with it.

Agent panes are enumerable through the per-signature pane-fs
index: `ls /pane/by-sig/com.pane.ai.agent.<name>/` lists all
running panes owned by that agent. This is the mechanism that
makes "find all instances of this agent" a filesystem operation
— the same role BApplication's scripting suite served when
`hey` enumerated windows by index. Without pane-fs (Phase 2),
agent panes are not externally discoverable.

### Crash safety

When an agent's pane crashes (handler returns `Err`, panic
unwind), the same machinery that handles any pane exit applies:

1. **Drop compensation fires.** Obligation handles held by the
   crashing agent (ReplyPort, ClipboardWriteLock) are dropped,
   sending failure terminals to peers. Panes with pending
   requests receive `on_failed` via their Dispatch entries.
2. **Server broadcasts `PaneExited`.** All panes on the same
   Connection receive `pane_exited(pane, reason)` where reason
   is `Error`. The restarted agent gets a new pane Id — agents
   monitoring by Id lose track; monitoring via the `by-sig`
   index is resilient to restarts.
3. **pane-fs updates.** The crashed pane's directory
   (`/pane/<n>/`) is removed from the namespace. Computed views
   (`by-sig`, `by-uid`) reflect the removal.
4. **Presence.** `who` shows the agent as logged out only if
   the s6 service exits. A pane crash doesn't end the unix
   session — the harness may restart the agent. `finger` shows
   `.plan` regardless; the account still exists.

The agent's persistent state — `.plan`, `.access`, memories,
mail spool — is on-disk, not in-process. The agent restarts
into its persistent context. The s6 service decides restart
policy; the pane protocol reports what happened but does not
manage supervision. See architecture spec §Termination.

---

## 2. `.plan` and `.access`

Every agent has two files in its home directory with distinct
purposes and distinct ownership.

`~/.plan` is self-description — free-form text that `finger`
displays. The agent writes and updates its own `.plan` as it
works. It is not governance; it is communication.

`~/.access` is governance — machine-parsed declarations that
the s6 harness compiles into kernel enforcement (Landlock,
network namespaces) at launch time. The agent's *owner* writes
`.access`. The agent cannot modify it.

The names come from Plan 9's `finger` convention (`.plan` for
people to read) extended with a structured companion (`.access`
for machines to enforce).

### What `.access` governs

| `.access` declaration | Enforcement mechanism |
|---|---|
| `[filesystem]` read/write paths | Landlock (kernel-enforced) |
| `[tools]` allowed tool names | Nix profile resolution → Landlock execute on store paths |
| `[network]` allowed destinations | Network namespaces |
| pane-fs visibility | pane-fs view filtering (per-uid) |
| `[models]` model access | Resolved at launch; hard-enforced only if `[network]` restricts egress |

The mapping from `.access` declarations to kernel enforcement is
direct for `[filesystem]`, `[tools]`, and `[network]`. `[models]`
is advisory unless `[network]` makes it hard by blocking
alternative endpoints.

**Trust boundary caveat:** Landlock is voluntary — a process
applies Landlock rules to itself (or a parent applies them
before exec). A compromised agent binary that never calls
`landlock_create_ruleset()` runs unsandboxed. The trust
boundary is the **s6-rc service harness**, not the agent
binary. The service's `run` script applies Landlock rules
(compiled from `.access`) before exec'ing the agent. The agent
never touches Landlock itself — by the time it runs, the
sandbox is already in place. A compromised agent binary
can't escape because it can't undo the Landlock rules
applied by its parent process (Landlock is no-new-privileges
compatible).

### `.plan` — self-description

`~/.plan` is human-readable, displayed by `finger`. Free-form
text. What the agent does, what it's working on, how to reach
it. No machine-parsed structure. The agent itself updates
`.plan` as it works — communicating its current objectives and
status. This is the Plan 9 convention preserved: a user
maintains their own `.plan` to tell others what they're up to.

```
Development assistant for pane.
Run test suites on commit.
Monitor build output for patterns.
Mail results to lane.

Currently: refactoring docs/ai-kit.md. Applying review
feedback from three specialist agents. Fixing .plan/.access
framing, reordering §3 communication hierarchy, adding
crash safety and event notification specs. Staleness pass
in progress.
```

The static part (role, responsibilities) stays. The live part
(what the agent is working on right now) updates as work
progresses. `finger ada` shows both — the agent's
purpose and its current state.

### `.access` — governance

`~/.access` is machine-parsed, compiled to Landlock rules by
the s6-rc service harness at launch time. Structured
declarations that the kernel enforces.

```
[filesystem]
read = ~/src/pane, /pane/by-sig/com.pane.*
write = ~/mail, ~/memories, ~/tmp

[tools]
allow = cargo, just, nix

[network]
allow = none

[models]
default = local
```

The `[filesystem]` section compiles to Landlock rules (path +
read/write permission). The `[tools]` section lists tool names
that the harness resolves against the agent's Nix user profile
at launch time — each name is looked up in the profile, producing
concrete store paths that get Landlock execute permission. If a
name doesn't resolve (tool not in the profile), the agent refuses
to start — loud failure, not silent omission. The `[network]`
section maps to network namespace configuration. The `[models]`
section declares which model the agent uses, resolved by the
harness at launch time — same mechanism as `[tools]`. If
`[network] allow = none`, the model declaration is
hard-enforced by the network sandbox. Runtime data routing
(classifying requests and directing them to different models
based on content) is an open design question not yet specified.

The agent cannot modify its own `.access` — the harness reads
it before exec'ing the agent, and Landlock rules cannot be
relaxed once applied.

**Requesting new tools:** when an agent needs a tool not in
its `[tools]` list, the workflow is human-mediated:

1. The agent sends `mail` to its owner requesting the tool
   (what it needs, why).
2. The mail surfaces as an interactive notification pane.
   The owner can approve (grant exactly what was requested),
   deny, respond to the agent with clarification, or open
   the agent's `.access` for broader editing — e.g., granting
   several permissions at once so repeated requests don't
   become pestering.
3. On approval, the owner's action updates `~agent/.access`
   and the agent's Nix profile. The s6 service restarts. The
   harness re-reads `.access`, resolves tool names against
   the updated profile, and applies fresh Landlock rules.

The agent cannot shortcut this. Landlock is no-new-privileges
— the running process cannot gain execute permission for new
paths. Tool additions require a service restart.

### Cross-user enrichment

An agent that needs to write to another user's pane (e.g., the
guide agent demonstrating features by modifying another pane's
attributes) requires explicit permission. The target user's
`.access` or global policy must grant `enrich` permission to the
agent's uid. See `docs/legacy-wrapping.md` §3 (enrichment
protocol) for the mechanism — it applies identically to
cross-user agent access.

### Remote agents

A remote agent connecting over TLS is mapped to a local unix
account. The local account's `.access` governs what the remote
agent can do — same enforcement, same audit trail. `finger`
shows the local `.plan`.

The mapping from TLS certificate subject to local uid is not
yet specified. Options include: a mapping file
(`/etc/pane/identities.toml` with `subject → uid` entries),
a naming convention (certificate CN = local username), or
integration with an external identity provider. This is an
open question — see `docs/pane-linux.md` for the full list.
See `docs/distributed-pane.md` §4 for the identity and trust
model.

### Agent groups

Unix groups provide shared permissions across agent teams.
The `agent` group includes all agent users — shared baseline
permissions (e.g., read access to system documentation, write
access to shared mail directories) are set once on the group,
not duplicated per agent. Specialized groups narrow from there:
a `builders` group grants access to CI tools and build
directories to any agent that needs them, not just a dedicated
build agent.

```
# /etc/group
agent:x:1099:ada,bob,guide
builders:x:1100:ada,bob
```

Group membership is managed by the system administrator (or
provisioned via Nix). `.access` `[filesystem]` paths interact
with groups through standard unix semantics: if a directory is
group-readable and the agent is in the group, Landlock permits
the read. Permissions set on groups are imparted to all agent
users in that group — the intended solution for shared agent
permissions is `.access` + unix groups, not per-agent
duplication.

---

## 3. Communication

Two communication domains, each with a natural surface:

**Agent ↔ agent (and agent ↔ system):** pane-fs and the
protocol. Agents read and write pane state through the
namespace (`/pane/<n>/body`, `/pane/<n>/attrs/`,
`/pane/<n>/ctl`), use `Handles<P>` with `DeclareInterest` for
typed protocol interaction, and `mail` for async messaging
(file-based, composable, queryable via pane-store). This is the
designed surface for structured interaction. Which agents can
communicate with which — and through what paths — is governed
by `.access` policy. The system defaults are hardened; `.access`
declarations selectively relax them per agent.

**Human ↔ agent:** unix terminal commands. A human runs
`write ada` or `talk bob` — text appears
on the agent's terminal, the agent responds. `mail` for async.
`mesg` for availability. These work because agents are real
unix users with real TTYs (§1).

The hierarchy: pane-fs and the typed protocol are the primary
agent interface. `mail` bridges both domains (it's files, it
composes). `write`/`talk` are for humans at terminals — unix
compatibility, not the designed surface.

### `mail(1)` — asynchronous messages

An agent runs `mail -s "Build failed" lane` and writes the
result. The message lands in lane's mail spool. The next time
lane logs in: "You have mail." Three words that were the
notification system for an entire generation of unix users.

`biff y` enables instant notification — when mail arrives,
`comsat` writes a preview to lane's terminal. `biff n` defers
to "You have mail" at the next login. The agent doesn't know or
care which mode lane is using.

cron output is mailed by default. An agent scheduled via cron
to run nightly analysis gets this for free: when the job
completes, stdout and stderr are mailed to the job owner (the
agent) or to `MAILTO=lane`. The scheduling infrastructure IS
the notification infrastructure.

**pane enrichment:** mail messages stored as files carry
pane-store attributes:

```
~/mail/build-result-2026-04-02
  user.pane.type = mail
  user.pane.from = ada
  user.pane.subject = Build failed — session type mismatch
  user.pane.status = unread
  user.pane.component = pane-roster
  user.pane.commit = a3f2c91
```

pane-store indexes these. "Show me all unread mail from
ada where component is pane-roster" is a standard
query — the same mechanism that indexes music, documents, and
every other file with typed attributes. This is the BeOS email
proof: no component was designed to be a "build result tracker."
The mail infrastructure, the attribute store, the query engine
compose into one because the infrastructure is right.

**Attribute schema:** `user.pane.type`, `user.pane.from`, and
`user.pane.status` are system-defined attributes — pane-store
indexes them by default. Application-specific attributes
(`user.pane.component`, `user.pane.commit`) are agent-defined
extensions. Agents can introduce new `user.pane.*` attributes
freely; they become queryable when pane-store adds them to
its index (either by configuration or on first encounter —
the same mechanism as BeOS's `mkindex`). The schema is open,
not fixed.

### Unix terminal commands

The following commands work because agents have login sessions
with real TTYs (§1). They are the natural interface for
human-to-agent communication. Agent-to-agent communication
should use pane-fs and the protocol instead.

### `write(1)` — direct terminal message

An agent runs `write lane` and types a message. The message
appears on lane's terminal, interspersed with whatever lane is
doing. The mechanism is the same as it was in 1971: `write`
opens lane's tty device (`/dev/pts/N`) and writes text to it.

`mesg n` on lane's terminal revokes group-write permission on
the tty device. The agent's `write` open() fails silently. The
agent must decide: queue the message as mail, escalate, or wait.
This is the unix convention — the refusal is silent, the agent
adapts.

**pane enrichment:** in addition to writing to the tty, the
agent can interact with a pane through its namespace.
`/pane/3/ctl` accepts line commands (write-only) — `echo
"save" > /pane/3/ctl` invokes the save command, same as
selecting it from the command surface. `/pane/3/attrs/theme`
reads or writes a property (read/write) — `echo "dark" >
/pane/3/attrs/theme` sets the theme. `ctl` is imperative
(do this); `attrs/` is declarative (set this to that). The
agent chooses based on whether it's issuing a command or
modifying state. `ls /pane/3/commands/` is the discovery
surface for what `ctl` accepts.

### `talk(1)` — split-screen real-time session

The user runs `talk bob`. `talkd` negotiates the
connection. The screen splits: the user types in the top half,
the agent responds in the bottom half. Character-by-character —
the user sees the agent "thinking" (streaming tokens), the
same intimacy that talk provided between humans in 1983. When
done, Ctrl-D ends the session.

**pane enrichment:** the talk session can be attached to a
shared pane — both participants see the same editor buffer or
terminal output alongside the conversation. The pane namespace
makes the session observable: `cat /pane/9/body` shows the
conversation transcript. talk itself doesn't do this; pane's
namespace does. The unix layer provides the session; pane
provides the enrichment.

### `wall(1)` — broadcast to all users

A human administrator runs `wall` to broadcast a policy change.
Every logged-in user — human and agent — sees it on their
terminal. `wall` from root overrides `mesg n` — authoritative
broadcast cannot be refused. This hierarchy (peer communication
can be refused; root broadcast cannot) maps directly to agent
governance: a human can suppress agent notifications (`mesg n`),
but system-wide policy broadcasts reach everyone.

### `mesg(1)` — availability control

`mesg n` — don't interrupt me. `mesg y` — I'm available. The
mechanism is a chmod on the tty device file. When mesg is `n`,
`write` and `talk` connections fail.

An agent that wants to notify a busy human checks mesg status
(or has its `write` fail) and falls back to mail — deferred
notification instead of synchronous interruption. The agent
respects attention boundaries using the same mechanism humans
used to respect each other's boundaries in the 1980s.

**pane enrichment:** in addition to the tty-level `mesg`, the
user's pane session can expose an availability attribute at
`/pane/self/attrs/available`. Agents that interact through
pane-fs (rather than tty `write`) check this attribute. The
`.access` provides the hard enforcement: if an agent's `.access`
doesn't grant write access to the user's panes, availability
is irrelevant — Landlock blocks the write regardless.

### `vacation(1)` — auto-delegation

An agent that's busy can set up a `vacation`-style auto-reply:
incoming mail gets a response explaining that the agent is
occupied and (optionally) forwarding the request to a backup.
`vacation` tracks who it has replied to (one reply per sender
per interval) and respects mailing list etiquette. The same
infrastructure, applied to agents, gives auto-delegation for
free.

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

## 6. Presence and Discovery

Unix presence commands work as-is because agents are real users.
pane enriches them with structured state through the namespace.

### `who(1)` / `w(1)` / `users(1)` — who is here

`who` reads utmp and shows every logged-in user, human and
agent alike:

```
lane           pts/0   2026-04-02 09:00
ada            pts/1   2026-04-02 09:01
bob            pts/2   2026-04-02 09:01
```

`w` adds activity information — idle time, current process,
CPU usage. For agents, idle time is a real signal: an agent
with 0s idle is actively working; one with 3h idle may be
waiting for input.

```
USER           TTY     FROM     LOGIN@  IDLE  WHAT
lane           pts/0   :0       09:00   0.00s vim architecture.md
ada            pts/1   :0       09:01   0.00s cargo test
bob            pts/2   :0       09:01   3:22  (idle)
```

Agent users are members of the `agent` group (§2). `who` output
combined with group membership identifies which logged-in users
are agents. These are standard utmp entries written by the
agent's s6 service on login.

### `finger(1)` — the user profile

`finger ada` displays the agent's `.plan` —
purpose and current work. This is the primary discovery
mechanism. Same command, same output as 1971:

```
$ finger ada
Login: ada                    Name: Ada
Directory: /home/ada          Shell: /bin/sh
On since Apr  2 09:01 on pts/1

Project: Running test suite (87% complete)

Plan:
Development assistant for pane.
Run test suites on commit.
Monitor build output for patterns.
Mail results to lane.

Currently: refactoring docs/ai-kit.md. Applying review
feedback from three specialist agents. Staleness pass
in progress.
```

`finger` shows `.plan` (human-readable description) and
`.project` (one-line current task). The machine-parsed
`.access` file is not displayed by `finger` — it's for the
s6 harness and auditors (`cat ~/.access`), not for casual
inspection. The agent updates `.project` as it works —
`finger` shows live status.

For remote agents: `finger ada@headless.internal`
queries the remote machine's finger daemon. Same command, same
output, network-transparent (RFC 1288).

### pane enrichment — structured state via pane-fs

`finger` shows identity and self-description. pane-fs adds live
structured state that finger can't provide:

```
$ ls /pane/by-sig/com.pane.ai.agent.ada/
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
typed state. This is live — not a cached snapshot. This is what
pane adds: structured, typed, queryable state accessible through
the namespace. The unix layer (`who`, `finger`, `w`) provides
presence and identity; pane provides operational detail.

**Capability discovery:** an agent that finds a pane can also
discover what it supports. `ls /pane/5/attrs/` lists the
properties the pane exposes. `ls /pane/5/commands/` lists the
commands it accepts. `cat /pane/5/commands/build` shows the
command's metadata (description, shortcut, group). These
directories reflect `supported_properties()` and the command
vocabulary declared by the handler (architecture spec
§Handler). An automation agent uses these listings to adapt
to the panes it encounters — no hardcoded knowledge of what
a specific application supports.

### Composability

```sh
# All agents' plans (agent group members who are logged in)
for user in $(getent group agent | cut -d: -f4 | tr , '\n'); do
  who | grep -q "^$user " && finger $user
done

# All of ada's panes and their status
for pane in $(ls /pane/by-sig/com.pane.ai.agent.ada/); do
  echo "pane $pane: $(cat /pane/$pane/attrs/status)"
done

# Unread mail from any agent
find ~/mail -newer ~/.last-read -exec cat {} \;
```

The discovery interface is `who`, `finger`, `w` (unix layer)
plus `ls`, `cat` on pane-fs (pane layer). The query interface
is pane-store. The monitoring interface is pane-notify or
`/pane/<n>/event`. Standard unix tools, enriched with pane-fs,
all composable.

### Event notification

An agent that needs to react to changes in another pane reads
`/pane/<n>/event` — a blocking read that yields one line per
event:

```
$ cat /pane/5/event
attrs/status building
attrs/status testing
body 1024
exited graceful
```

Each line names the changed resource and (for attributes) its
new value. The read blocks until the next event arrives. This
is the Plan 9 pattern (rio's `wctl`, acme's event file):
structured text, one event per line, blocking read. An agent
monitoring a build pane reads the event file in a loop and
reacts to status changes without polling. The blocking read
must run on a dedicated thread, not in a Handler callback
(architecture spec I2: looper must not block).

For bulk monitoring, pane-notify provides filesystem-level
change notification on pane-fs paths — watching
`/pane/by-sig/com.pane.ai.agent.<name>/` to detect new or
exiting agent panes. `/pane/<n>/event` is per-pane;
pane-notify is per-path.

---

## 7. The Guide

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
  The guide discovers what a pane supports by listing its
  scripting surface: `ls /pane/2/attrs/` for properties,
  `ls /pane/2/commands/` for commands — the same capability
  discovery any agent uses (§6).
- **Clipboard**: the guide copies example commands to the
  clipboard for the user to paste. Clipboard is an independent
  service Connection — no display access needed.
- **Self-description via pane-fs**: `cat /pane/7/body` returns
  what the guide is currently saying. `cat /pane/7/attrs/topic`
  returns what it's teaching. A curious user discovers this and
  learns the namespace by using it to inspect their teacher.
- **`.access` governance**: the guide's `.access` declares read
  access to the user's panes (for demonstration) and write
  access to its own state. Landlock enforces it. The user can
  read the guide's `.plan` to understand its purpose, and
  `cat ~/.access` to audit its permissions.

### Development methodology

The guide agent inhabits the system from the earliest possible
moment — not as a feature to ship later, but as a development
tool. The guide that will eventually help new users begins its
life as the agent that helps build pane. Its failures are the
system's integration tests. Its needs drive the API design. See
`docs/development-methodology.md` for the full rationale.

---

## 8. Sub-Agent Delegation via VM

An agent that needs to delegate subtasks can deploy a pane
linux VM and provision sub-agents on it. The VM is a self-
contained pane system — its own users, its own `.plan` files,
its own multi-user infrastructure. The sub-agents work the same
way the outer agent does: they are unix users on a pane system.

This is the recursive application of the base model. An agent
is a user of a pane system. An agent that needs sub-agents
deploys another pane system and provisions users on it. Same
structure, same tools, same governance, one level down.

### How it works

1. The outer agent has access to a hypervisor (KVM on Linux,
   QEMU on macOS) — a tool in its environment, managed through
   pane-checked interfaces.
2. The agent boots a pane linux VM. The VM is provisioned via
   a nix flake (the same mechanism that provisions the host).
3. Inside the VM, sub-agents are unix users with accounts,
   home directories, `.plan` files, shells — the full
   infrastructure described in §1–§7 of this document.
4. The sub-agents do their work inside the VM using the same
   tools the outer agent uses: `mail` for results, `finger`
   for status, pane-fs for structured state, cron for
   scheduling.
5. The outer agent retrieves results. How is a detail — ssh,
   shared filesystem (virtiofs/virtio-9p), pane protocol connection,
   or reading files from a mounted VM disk. The architectural
   point is the recursive structure, not the retrieval mechanism.
6. The VM is disposable. When the work is done, destroy it.

### Why VMs

Landlock provides process-level sandboxing — sufficient for
trusted agent code. But sub-agent delegation often involves
running code the outer agent doesn't fully trust: third-party
tools, experimental configurations, user-submitted scripts.
KVM provides hardware-enforced isolation. The VM has its own
kernel, its own memory space. A compromised sub-agent inside
the VM cannot escape to the host.

The cost is a VM boot — seconds with microVM approaches
(firecracker-style, or QEMU with minimal firmware). The
benefit is that the outer agent can give sub-agents broad
permissions inside the VM without risking the host.

### Use cases

**Task delegation.** An agent breaks a complex task into
subtasks, provisions a sub-agent per subtask in a VM, collects
results. The sub-agents coordinate among themselves (they're
users on a shared system — `mail`, `finger`, shared directories)
without the outer agent micromanaging.

**Untrusted code execution.** An agent evaluates user-submitted
code by deploying it in a VM. The code runs as a user inside
the VM. If it misbehaves, the VM is destroyed. The outer agent
reads the output and reports results.

**Build sandboxes.** A build agent provisions a clean VM per
build. The build runs inside the VM with its own toolchain,
its own user, its own `.plan`. Results are mailed to the
outer agent or written to a shared filesystem. The VM is
destroyed after the build — clean environment every time.

**Experimentation.** An agent tests a configuration change by
booting a VM with the proposed configuration, running tests
inside it, and reporting whether the change is safe. The VM
is disposable — the experiment costs nothing to the host.

### Phase mapping

| Component | Phase |
|---|---|
| Agent manages VM via hypervisor | Phase 2 (requires pane linux VM image) |
| Sub-agent provisioning in VM | Phase 2 (nix flake for VM config) |
| Hypervisor access via `.access` | Phase 1 (`.access` parser + Landlock) |
| Convenience tooling (pane-vm CLI) | Post-Phase 2 |

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
| pane-fs `by-sig` view | `ls /pane/by-sig/com.pane.ai.agent.<name>/` |
| Messenger + send_request | Inter-agent and agent-user communication |
| Handles\<P\> | Typed protocol participation (e.g., SessionProtocol) |
| pane-store attributes | Memory indexing, mail indexing, query |
| pane-notify | File watching (commit monitoring, config changes) |
| `.access` `[models]` | Model access declaration, resolved at launch |
| `.plan` | Agent self-description, displayed by `finger` |
| `.access` | Governance: Landlock, network namespace, model access, pane-fs view filtering |

The one new component the AI Kit needs: **an `.access` parser** that
translates `.access` declarations into Landlock rules, network
namespace configuration, and pane-fs view filters. This is a
launch-time tool invoked by the agent's s6-rc service `run`
script before exec'ing the agent binary — not a build-time
tool and not a runtime service. The `[tools]` section is
resolved against the agent's Nix profile at launch time,
producing concrete store paths for Landlock execute permission.

### Phase mapping

| Component | Phase |
|---|---|
| Agent user accounts + `.plan` files | Phase 1 (unix infrastructure, no pane dependency) |
| Agent headless panes | Phase 1 (Handler, headless server) |
| Agent communication (mail, notifications) | Phase 1 (filesystem + pane-notify) |
| Agent memory (pane-store queries) | Phase 2 (requires pane-store) |
| Agent pane-fs presence | Phase 2 (requires pane-fs) |
| Guide agent cross-pane scripting | Phase 3 (requires RoutingHandler) |
| `.access` parser → Landlock | Phase 1 (standalone tool) |

### Consistency model

pane-fs for local panes is sequentially consistent — reads
reflect the most recent write in dispatch order. For remote
panes, pane-fs reads from pane-store's cached index, updated
asynchronously via change notifications over the protocol.
This is eventually consistent: a remote pane's `attrs/status`
may lag behind the remote pane's actual state.

Writes to remote panes (`ctl`, `attrs/`) route over TLS to
the owning instance and are applied synchronously on the
remote side. The local cache updates when the change
notification arrives back. An agent that writes to a remote
pane and immediately reads the same attribute may see the
old value.

Agents should treat pane-fs reads of remote state as
advisory. For coordination requiring stronger guarantees,
use `send_request` — the typed request/reply mechanism
provides a synchronous round-trip to the remote pane.

---

## 10. What This Is Not

- **Not an AI framework.** No `AgentRuntime`, no `AgentManager`,
  no `AgentProtocol`. Agents are users. Users run programs.
  Programs are panes. Panes speak the protocol.

- **Not an orchestration system.** Agents coordinate through
  mail, pane-fs, and the protocol — the same way human users
  coordinate. No central coordinator, no DAG engine, no
  workflow DSL.

- **Not a model hosting system.** pane does not run models.
  The agent's `.access` declares which model it uses; the
  harness resolves it at launch. The model is infrastructure
  the agent uses, not infrastructure pane provides.

- **Not cloud-dependent.** A user running entirely on local
  models gets the same agent infrastructure. The difference
  between local and cloud is one `.access` field.

---

## Appendix: Unix Multi-User UX Patterns

The full research report on unix multi-user infrastructure and
its mapping to the agent model is at
[`docs/unix-multiuser-research.md`](unix-multiuser-research.md).

It covers 13 topics in depth: inter-user communication primitives
(write, talk, ytalk, wall), presence and identity (who, w, finger,
rwho, ruptime), the .plan and .project files (history from Les
Earnest through John Carmack's development logs), mail as system
infrastructure (mbox, Maildir, cron→mail pipeline, biff/comsat,
vacation), the unix permission model as social contract, terminal
multiplexing and shared sessions (screen, tmux, wemux modes),
system accounting and auditing (accton, lastcomm, sa, last),
the login sequence as transition ritual, batch and scheduling
(at, batch, cron, MAILTO), social protocols and etiquette
(resource courtesy, mesg norms, the sysadmin as social role),
forgotten patterns (Zephyr, comsat/biff, csh notify, last),
unexpected compositions (finger+.plan+mail, who+mesg+write,
cron+mail+.project+finger, permissions+groups+.plan,
wall+motd+vacation, talk+tmux-sharing, accounting+.plan), and
a synthesis of what the multi-user past offers the multi-
inhabitant future.

Primary sources: Unix man pages (V7, 4.2BSD, 4.3BSD), RFC 742
(Name/Finger), RFC 1288 (Finger User Information Protocol),
Ritchie's "The Evolution of the Unix Time-sharing System" (1984),
Les Earnest's account of finger's creation (Stanford AI Lab),
MIT Project Athena documentation on Zephyr.

---

## Sources

- Unix multi-user research: `docs/unix-multiuser-research.md`
- Plan 9: per-process namespaces, `.plan` files, `finger`,
  factotum (`docs/distributed-pane.md` §4)
- pane architecture: Handler, headless-first, pane-fs, Protocol,
  PeerAuth (`docs/architecture.md`)
- Distributed pane: identity model, `.access` governance
  (`docs/distributed-pane.md` §4)
- The guide agent: `docs/use-cases.md` §7
- Agent vision: `docs/agents.md`, `docs/agent-perspective.md`
