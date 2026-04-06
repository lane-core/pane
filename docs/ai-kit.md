# AI Kit

An agent is a unix user. It has a uid, a home directory, a
shell, a login session. Its running processes are headless
panes — Handler implementations connected to a pane server,
participating in the protocol. The operating system
authenticates, isolates, resource-accounts, and schedules
agents using the same mechanisms it uses for human users.

No new protocol, no new service, no new runtime concept.
Agent needs map onto unix multi-user infrastructure and
pane's existing protocol architecture.

---

## 1. An Agent Is a Unix User

An agent has a unix user account. Not a metaphorical one —
a real uid in `/etc/passwd`. Local connections authenticate
via `SO_PEERCRED` (`AuthSource::Kernel { pid }`). Remote
connections authenticate via TLS client certificate
(`AuthSource::Certificate { subject, issuer }`), mapped to a
local unix account by the server. The architecture spec's
transport-derived identity model applies identically to
agents and humans.

The home directory holds persistent state: memories,
configuration, project files, mail spool. File permissions
isolate agents from each other. Process accounting logs
what the agent did. The agent's tools are declared in a Nix
user profile — atomic, rollbackable. Periodic tasks run via
`cron` or s6 timers.

`who` shows which agents are logged in. `finger ada` shows
the agent's `.plan`.

### Headless panes

An agent's running processes are headless panes. A headless
pane implements `Handler`, connects to a pane server, and
participates in the protocol. It has no `Handles<Display>`.
It has `Handles<P>` for service protocols, `request_received`
for ad-hoc inter-pane requests, `pane_exited` for monitoring,
`send_request` for typed request/reply.

The agent's s6 service harness allocates a login session
(utmp entry, PTY) before exec'ing the agent binary. The
agent is both a Handler speaking the pane protocol and a unix
user with a terminal it can drive programmatically — spawning
subprocesses, running shell commands. The PTY is
infrastructure the harness provides; the Handler doesn't
manage it. The agent is addressable via `write` and `talk`
alongside its protocol participation.

The same Handler code that runs headless can opt into display
by implementing `Handles<Display>`. An agent that normally
runs headless presents a visual interface when a user opens a
session with it.

Agent panes are enumerable through the per-signature pane-fs
index: `ls /pane/by-sig/com.pane.ai.agent.<name>/`. Without
pane-fs, agent panes are not externally discoverable.
(pane-fs specified in architecture.md §Namespace;
FUSE implementation pending.)

### Crash safety

When an agent's pane panics, the standard pane exit machinery
applies:

1. **Drop compensation fires.** Obligation handles held by
   the crashing agent (ReplyPort, ClipboardWriteLock) are
   dropped, sending failure terminals to peers. Panes with
   pending requests receive `on_failed` via their Dispatch
   entries.
2. **Server broadcasts `PaneExited`.** All panes on the same
   Connection receive `pane_exited(pane, reason)` where
   reason is `ExitReason::Failed`. The restarted agent gets
   a new pane Id — monitoring by Id loses track; monitoring
   via the `by-sig` index (when pane-fs is available) is
   resilient to restarts.
3. **pane-fs updates.** The crashed pane's directory
   (`/pane/<n>/`) is removed from the namespace. (Requires
   pane-fs.)
4. **Presence.** `who` shows the agent as logged out only if
   the s6 service exits. A pane crash doesn't end the unix
   session — the harness may restart the agent.

Persistent state — `.plan`, `.access`, memories, mail spool —
is on-disk, not in-process. The agent restarts into its
persistent context. The s6 service decides restart policy.

---

## 2. `.plan` and `.access`

Two files in the agent's home directory with distinct purposes
and distinct ownership.

`~/.plan` is self-description. The agent writes it. `finger`
displays it. Free-form text.

`~/.access` is governance. The agent's owner writes it. The
s6 harness compiles it into kernel enforcement (Landlock,
network namespaces) at launch time. The agent cannot modify
it.

The names come from Plan 9's `finger` convention (`.plan` for
people to read) extended with a structured companion
(`.access` for machines to enforce).

### `.plan` — self-description

`~/.plan` is human-readable, displayed by `finger`. What the
agent does, what it's working on, how to reach it. No
machine-parsed structure. The agent updates `.plan` as it
works.

```
Development assistant for pane.
Run test suites on commit.
Monitor build output for patterns.
Mail results to lane.

Currently: running staleness pass on docs/ai-kit.md against
architecture.md. Three errors found so far.
```

The static part (role, responsibilities) stays. The live part
(current work) updates as work progresses. `finger ada` shows
both.

### `.access` — governance

`~/.access` is machine-parsed. The s6-rc service harness
compiles it to Landlock rules at launch time.

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

**`[filesystem]`** compiles to Landlock rules (path +
read/write permission).

**`[tools]`** lists tool names resolved against the agent's
Nix user profile at launch time. Each name produces concrete
store paths that get Landlock execute permission. If a name
doesn't resolve, the agent refuses to start.

**`[network]`** maps to network namespace configuration.

**`[models]`** declares which model the agent uses, resolved
at launch time. If `[network] allow = none`, the declaration
is hard-enforced by the network sandbox. Runtime data routing
(classifying requests and directing them to different models
based on content) is not yet specified.

### What `.access` governs

| Declaration | Enforcement |
|---|---|
| `[filesystem]` read/write paths | Landlock (kernel) |
| `[tools]` allowed tool names | Nix profile → Landlock execute on store paths |
| `[network]` allowed destinations | Network namespaces |
| pane-fs visibility | pane-fs view filtering, per-uid (requires pane-fs) |
| `[models]` model access | Hard only if `[network]` restricts egress |

### Trust boundary

Landlock is voluntary — a process applies rules to itself.
The trust boundary is the s6-rc service harness, not the
agent binary. The service's `run` script applies Landlock
rules (compiled from `.access`) before exec'ing the agent.
The agent never touches Landlock itself. Landlock is
no-new-privileges compatible — a process cannot undo rules
applied by its parent.

### Requesting new tools

When an agent needs a tool not in its `[tools]` list:

1. The agent sends `mail` to its owner requesting the tool.
2. The mail surfaces as an interactive notification pane.
   The owner approves, denies, or responds with
   clarification. The owner can edit `.access` directly for
   broader changes.
3. On approval, the owner updates `~agent/.access` and the
   agent's Nix profile. The s6 service restarts. The harness
   re-reads `.access`, resolves against the updated profile,
   applies fresh Landlock rules.

Landlock is no-new-privileges. Tool additions require a
service restart.

### Cross-user enrichment

An agent that writes to another user's pane requires explicit
permission. The target user's `.access` or global policy
grants `enrich` permission to the agent's uid. See
`docs/legacy-wrapping.md` §3 for the enrichment protocol.

### Remote agents

A remote agent connecting over TLS is mapped to a local unix
account. The local account's `.access` governs what the
remote agent can do. `finger` shows the local `.plan`.

The mapping from TLS certificate subject to local uid is not
yet specified. See `docs/pane-linux.md` for the open question
and `docs/distributed-pane.md` §4 for the identity model.

### Agent groups

Unix groups provide shared permissions across agent teams.

```
# /etc/group
agent:x:1099:ada,bob,guide
builders:x:1100:ada,bob
```

The `agent` group includes all agent users — shared baseline
permissions (read access to system docs, write access to
shared mail directories) are set once on the group. The
`builders` group grants CI tools and build directories to
any agent that needs them.

Group membership is managed by the system administrator or
provisioned via Nix. `.access` `[filesystem]` paths interact
with groups through standard unix semantics: if a directory
is group-readable and the agent is in the group, Landlock
permits the read.

---

## 3. Communication

Two domains.

**Agent ↔ agent (and agent ↔ system):** pane-fs and the
protocol. Agents read and write pane state through the
namespace (`/pane/<n>/body`, `/pane/<n>/attrs/`,
`/pane/<n>/ctl`), use `Handles<P>` with `DeclareInterest` for
typed protocol interaction, and `mail` for async messaging.
`.access` governs which agents communicate with which.
(pane-fs specified in architecture.md §Namespace;
FUSE implementation pending.)

**Human ↔ agent:** unix terminal commands. `write ada` or
`talk bob` — text on the agent's terminal. `mail` for async.
`mesg` for availability. These work because agents are real
unix users with real TTYs (§1).

pane-fs and the typed protocol are the primary agent
interface. `mail` bridges both domains. `write`/`talk` are
for humans at terminals.

### `mail(1)` — asynchronous messages

```sh
mail -s "Build failed" lane
```

The message lands in lane's mail spool. `biff y` enables
instant notification via `comsat`. `biff n` defers to
"You have mail" at next login. The agent doesn't know or
care which mode lane is using.

cron output is mailed by default. An agent scheduled via cron
to run nightly analysis gets notification for free: stdout
and stderr are mailed to `MAILTO=lane`.

**pane enrichment.** Mail messages stored as files carry
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

pane-store indexes these. "Show me all unread mail from ada
where component is pane-roster" is a pane-store query — the
same mechanism that indexes any file with typed attributes.
(pane-store is not yet specified; see PLAN.md Phase 2.)

`user.pane.type`, `user.pane.from`, and `user.pane.status`
are system-defined attributes. Application-specific attributes
(`user.pane.component`, `user.pane.commit`) are agent-defined
extensions. Agents introduce new `user.pane.*` attributes
freely; they become queryable when pane-store indexes them
(same mechanism as BeOS's `mkindex`). The schema is open.

### Unix terminal commands

The following commands work because agents have login sessions
with real TTYs (§1). Agent-to-agent communication should use
pane-fs and the protocol instead.

### `write(1)` — direct terminal message

`write` opens the target user's tty device (`/dev/pts/N`)
and writes text to it. `mesg n` on the target's terminal
revokes group-write permission on the tty device; the
`write` open() fails. The agent decides: queue as mail,
escalate, or wait.

**pane enrichment.** In addition to tty writes, agents
interact with panes through the namespace. `/pane/3/ctl`
accepts line commands (write-only) — `echo "save" >
/pane/3/ctl` invokes the save command. `/pane/3/attrs/theme`
reads or writes a property — `echo "dark" >
/pane/3/attrs/theme` sets the theme. `ctl` is imperative
(do this); `attrs/` is declarative (set this to that).
`ls /pane/3/commands/` discovers what `ctl` accepts.
(pane-fs specified; FUSE pending.)

### `talk(1)` — split-screen real-time session

`talkd` negotiates the connection. The screen splits: the
user types in the top half, the agent responds in the
bottom half. Character-by-character — the user sees the
agent streaming tokens. Ctrl-D ends the session.

**pane enrichment.** A talk session can be attached to a
shared pane — both participants see the same editor buffer
alongside the conversation. `cat /pane/9/body` shows the
transcript. talk provides the session; pane-fs provides the
enrichment. (pane-fs specified; FUSE pending.)

### `wall(1)` — broadcast

`wall` broadcasts to every logged-in user — human and agent.
`wall` from root overrides `mesg n`. Peer communication can
be refused; root broadcast cannot.

### `mesg(1)` — availability control

`mesg n` revokes group-write on the tty device. `write` and
`talk` connections fail. An agent that wants to notify a busy
human has its `write` fail and falls back to mail.

**pane enrichment.** In addition to tty-level `mesg`, a
user's pane session can expose `/pane/self/attrs/available`.
Agents interacting through pane-fs check this attribute.
`.access` provides the hard enforcement: if the agent's
`.access` doesn't grant write access, Landlock blocks the
write regardless of availability. (pane-fs specified; FUSE pending.)

### `vacation(1)` — auto-delegation

An agent that's busy sets up a `vacation`-style auto-reply.
Incoming mail gets a response explaining the agent is
occupied, optionally forwarding to a backup. `vacation`
tracks replies (one per sender per interval) and respects
mailing list etiquette.

---

## 4. Memory

An agent's memories are files in its home directory. Each
file carries typed attributes.

```
~/memories/
  debugging-session-2026-04-01.md
  architecture-insight-protocol-split.md
  user-preference-commit-style.md
```

```
user.pane.type = memory
user.pane.kind = debugging
user.pane.importance = high
user.pane.created = 2026-04-01T14:30:00Z
user.pane.tags = protocol, session-types
```

pane-store indexes these. "Show me all memories tagged
'protocol' from the last week" is a standard query.
(Requires pane-store.)

### Memory vs pane state

Agent memories (`~/memories/`) are persistent knowledge that
survives across sessions — preferences, project context,
domain knowledge. Agent pane state (`/pane/<n>/attrs/`,
`/pane/<n>/body`) is live operational state. Both are files.
Both are queryable. They differ in lifecycle, not mechanism.

---

## 5. Agents Build Things

An agent modifies the system by producing artifacts on the
same surfaces human developers use. It writes files.

A routing rule: write a rule file to
`~/.config/pane/route/rules/`. A configuration change: write
to `/etc/pane/` or `~/.config/pane/`. A new tool: write a
script, add it to the Nix profile. A bridge extension: add
commands or properties to a `.app` bundle's `bridge/`.
A shared configuration: copy files to a shared directory or
git repo.

Every artifact is a file — inspectable, versionable,
shareable, reversible. A user's collection of agent-built
customizations accumulates as personal configuration.

---

## 6. Presence and Discovery

### `who(1)` / `w(1)` / `users(1)`

`who` reads utmp and shows every logged-in user:

```
lane           pts/0   2026-04-02 09:00
ada            pts/1   2026-04-02 09:01
bob            pts/2   2026-04-02 09:01
```

`w` adds activity: idle time, current process, CPU.

```
USER           TTY     FROM     LOGIN@  IDLE  WHAT
lane           pts/0   :0       09:00   0.00s vim architecture.md
ada            pts/1   :0       09:01   0.00s cargo test
bob            pts/2   :0       09:01   3:22  (idle)
```

Agent users are members of the `agent` group (§2). `who`
output combined with group membership identifies which
logged-in users are agents.

### `finger(1)`

`finger ada` displays the `.plan`:

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

Currently: running staleness pass on docs/ai-kit.md.
Three errors found so far.
```

`.plan` shows role and current work. `.project` shows a
one-line summary. The agent updates `.project` as it works.
`.access` is not displayed by `finger` — it's for the harness
and auditors.

For remote agents: `finger ada@headless.internal` queries the
remote machine's finger daemon (RFC 1288).

### Structured state via pane-fs

`finger` shows identity and self-description. pane-fs adds
live structured state:

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

Running panes are discoverable via the per-signature index.
`body` shows current output. `attrs/` shows typed state.
(pane-fs specified; FUSE pending.)

**Capability discovery.** `ls /pane/5/attrs/` lists exposed
properties. `ls /pane/5/commands/` lists accepted commands.
`cat /pane/5/commands/build` shows command metadata. These
directories reflect `supported_properties()` and the command
vocabulary declared by the handler. An automation agent uses
these listings to adapt to the panes it encounters.

### Composability

```sh
# All agents' plans (agent group members who are logged in)
for user in $(getent group agent | cut -d: -f4 | tr , '\n'); do
  who | grep -q "^$user " && finger $user
done

# All of ada's panes and their status (requires pane-fs)
for pane in $(ls /pane/by-sig/com.pane.ai.agent.ada/); do
  echo "pane $pane: $(cat /pane/$pane/attrs/status)"
done

# Unread mail from any agent
find ~/mail -newer ~/.last-read -exec cat {} \;
```

Unix layer: `who`, `finger`, `w`. Pane layer: `ls`, `cat` on
pane-fs. Query: pane-store. Monitoring: pane-notify or
`/pane/<n>/event`. (pane-store, pane-notify not yet
specified.)

### Event notification

An agent that reacts to changes in another pane reads
`/pane/<n>/event` — a blocking read that yields one line per
event:

```
$ cat /pane/5/event
attrs/status building
attrs/status testing
body 1024
exited graceful
```

Each line names the changed resource and its new value. The
read blocks until the next event. One event per line,
blocking read — the Plan 9 pattern (rio's `wctl`, acme's
event file). The blocking read must run on a dedicated thread,
not in a Handler callback (I2: looper must not block).

For bulk monitoring, pane-notify provides filesystem-level
change notification on pane-fs paths — watching
`/pane/by-sig/com.pane.ai.agent.<name>/` to detect new or
exiting agent panes. `/pane/<n>/event` is per-pane;
pane-notify is per-path. (pane-notify is not yet specified.)

---

## 7. The Guide

The canonical first agent. Purpose: teach new users by
demonstrating the system. The guide uses pane *using pane*.

What makes it possible:

- **Headless pane.** The guide runs without a display,
  connects to the pane server, participates in the protocol,
  appears in the namespace.
- **Scripting via pane-fs (Phase 3 — Handles\<Routing\>).**
  The guide reads and writes other panes' properties.
  `echo "dark" > /pane/2/attrs/theme` demonstrates theming.
  `cat /pane/1/attrs/cursor` points out where the user is.
  `ls /pane/2/attrs/` discovers properties; `ls
  /pane/2/commands/` discovers commands.
- **Clipboard.** The guide copies example commands to the
  clipboard for the user to paste. Clipboard is an
  independent service Connection — no display needed.
- **Self-description via pane-fs.** `cat /pane/7/body`
  returns what the guide is saying. `cat
  /pane/7/attrs/topic` returns what it's teaching. A user
  discovers the namespace by inspecting their teacher.
- **`.access` governance.** The guide's `.access` declares
  read access to the user's panes and write access to its
  own state. Landlock enforces it.

### Development methodology

The guide inhabits the system from the earliest possible
moment — not as a feature for later, but as a development
tool. Its failures are the system's integration tests. Its
needs drive the API design. See `docs/workflow.md`.

---

## 8. Sub-Agent Delegation via VM

An agent that delegates subtasks deploys a pane linux VM and
provisions sub-agents on it. The VM is a self-contained pane
system — its own users, its own `.plan` files, its own
multi-user infrastructure. The sub-agents are unix users on
a pane system, same as the outer agent.

### How it works

1. The outer agent has access to a hypervisor (KVM on Linux,
   QEMU on macOS) — a tool in its environment, managed
   through `.access`.
2. The agent boots a pane linux VM, provisioned via nix flake.
3. Inside the VM, sub-agents are unix users with accounts,
   home directories, `.plan` files, shells — the full
   infrastructure of §1–§7.
4. Sub-agents work inside the VM using the same tools: `mail`
   for results, `finger` for status, pane-fs for state, cron
   for scheduling.
5. The outer agent retrieves results via ssh, shared
   filesystem (virtiofs/virtio-9p), pane protocol connection,
   or reading files from a mounted VM disk.
6. The VM is disposable. Destroy it when done.

### Why VMs

Landlock provides process-level sandboxing — sufficient for
trusted agent code. Sub-agent delegation often involves
code the outer agent doesn't fully trust: third-party tools,
experimental configurations, user-submitted scripts. KVM
provides hardware-enforced isolation. The VM has its own
kernel and memory space. A compromised sub-agent cannot
escape to the host.

The cost is a VM boot — seconds with microVM approaches
(firecracker-style, or QEMU with minimal firmware). The
benefit is broad sub-agent permissions without risking the
host.

### Use cases

**Task delegation.** Break a complex task into subtasks,
provision a sub-agent per subtask, collect results. The
sub-agents coordinate among themselves (they're users on
a shared system) without the outer agent micromanaging.

**Untrusted code execution.** Evaluate user-submitted code
by deploying it in a VM. If it misbehaves, destroy the VM.

**Build sandboxes.** Clean VM per build. The build runs with
its own toolchain, its own user. Results are mailed out or
written to shared filesystem. Destroyed after completion.

**Experimentation.** Test a configuration change by booting
a VM with the proposed configuration, running tests, and
reporting whether the change is safe. Disposable.

### Phase mapping

| Component | Phase |
|---|---|
| Hypervisor access via `.access` | Phase 1 (`.access` parser + Landlock) |
| Agent manages VM via hypervisor | Phase 2 (requires pane linux VM image) |
| Sub-agent provisioning in VM | Phase 2 (nix flake for VM config) |
| Convenience tooling (pane-vm CLI) | Post-Phase 2 |

---

## 9. Relationship to Architecture Spec

The AI Kit introduces no new protocol, no new service, no new
runtime concept. It is a usage pattern over existing
infrastructure.

An agent process is a headless pane (Handler). Its identity
comes from `AuthSource::Kernel` (local) or
`AuthSource::Certificate` (remote). Its panes are visible in
the pane-fs namespace at `/pane/<n>/` and discoverable via
`/pane/by-sig/`. Inter-agent communication uses `Messenger`
and `send_request`. Typed protocol participation uses
`Handles<P>`. Memory and mail indexing use pane-store
attributes. File watching uses pane-notify.

`.plan` is new to pane (from Plan 9's `finger` convention).
`.access` is new to pane (governance declarations compiled
to Landlock). Both are files in the agent's home directory,
not protocol concepts.

The one new component: an `.access` parser that translates
declarations into Landlock rules, network namespace
configuration, and pane-fs view filters. This is a
launch-time tool invoked by the s6-rc service `run` script
before exec'ing the agent — not a build-time tool and not
a runtime service.

### Partially specified dependencies

- **pane-fs** — the filesystem namespace at `/pane/`. Specified
  in architecture.md §Namespace (tree layout, snapshot model,
  ctl dispatch, computed views, json reserved filename). FUSE
  implementation pending. Agent discovery, structured state,
  and cross-pane scripting depend on it.
- **pane-store** — attribute indexing and queries. Agent
  memory queries and mail indexing depend on it. Not yet
  specified. PLAN.md Phase 2.
- **pane-notify** — filesystem-level change notification on
  pane-fs paths. Bulk monitoring depends on it. Not yet
  specified.

### Phase mapping

| Component | Phase |
|---|---|
| Agent user accounts + `.plan` files | Phase 1 (unix, no pane dependency) |
| Agent headless panes | Phase 1 (Handler, headless server) |
| Agent communication (mail, notifications) | Phase 1 (filesystem) |
| `.access` parser → Landlock | Phase 1 (standalone tool) |
| Agent pane-fs presence | Phase 2 (requires pane-fs) |
| Agent memory queries | Phase 2 (requires pane-store) |
| Guide agent cross-pane scripting | Phase 3 (requires Handles\<Routing\>) |

### Consistency model

pane-fs for local panes is sequentially consistent — reads
reflect the most recent write in dispatch order. For remote
panes, pane-fs reads from pane-store's cached index, updated
asynchronously via change notifications over the protocol.
This is eventually consistent: a remote pane's `attrs/status`
may lag behind actual state.

Writes to remote panes (`ctl`, `attrs/`) route over TLS to
the owning instance and apply synchronously on the remote
side. The local cache updates when the change notification
arrives back. An agent that writes to a remote pane and
immediately reads the same attribute may see the old value.

For coordination requiring stronger guarantees, use
`send_request` — the typed request/reply mechanism provides
a synchronous round-trip.

---

## 10. What This Is Not

- **Not an AI framework.** No `AgentRuntime`, no
  `AgentManager`, no `AgentProtocol`. Agents are users.
  Users run programs. Programs are panes. Panes speak the
  protocol.

- **Not an orchestration system.** Agents coordinate through
  mail, pane-fs, and the protocol. No central coordinator,
  no DAG engine, no workflow DSL.

- **Not a model hosting system.** pane does not run models.
  `.access` declares which model the agent uses; the harness
  resolves it at launch. The model is infrastructure the
  agent uses, not infrastructure pane provides.

- **Not cloud-dependent.** A user running entirely on local
  models gets the same agent infrastructure. The difference
  is one `.access` field.

---

## Appendix: Unix Multi-User UX Patterns

The full research report on unix multi-user infrastructure
and its mapping to the agent model was completed and its
conclusions are incorporated below.

It covers: inter-user communication (write, talk, ytalk,
wall), presence and identity (who, w, finger, rwho, ruptime),
.plan and .project files, mail as system infrastructure
(mbox, Maildir, cron→mail pipeline, biff/comsat, vacation),
the unix permission model, terminal multiplexing and shared
sessions (screen, tmux, wemux), system accounting (accton,
lastcomm, sa, last), the login sequence, batch and scheduling
(at, batch, cron, MAILTO), social protocols and etiquette,
forgotten patterns (Zephyr, comsat/biff, csh notify), and
unexpected compositions.

Primary sources: Unix man pages (V7, 4.2BSD, 4.3BSD),
RFC 742 (Name/Finger), RFC 1288 (Finger User Information
Protocol), Ritchie's "The Evolution of the Unix Time-sharing
System" (1984), Les Earnest on finger's creation (Stanford AI
Lab), MIT Project Athena documentation on Zephyr.

---

## Sources

- Plan 9: per-process namespaces, `.plan` files, `finger`,
  factotum (`docs/distributed-pane.md` §4)
- pane architecture: Handler, headless-first, Protocol,
  PeerAuth (`docs/architecture.md`)
- Distributed pane: identity model, `.access` governance
  (`docs/distributed-pane.md` §4)
- The guide agent: `docs/use-cases.md` §7
