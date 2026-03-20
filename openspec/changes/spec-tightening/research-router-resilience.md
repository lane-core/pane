# Router Resilience: The Communication Backbone as System Immune System

Research for pane spec-tightening. Covers precedents and patterns for building a component that serves as both message dispatch infrastructure and the system's last line of defense against cascading failure.

Sources:

- Erlang/OTP heart module. [heart -- kernel v10.5.](https://www.erlang.org/doc/apps/kernel/heart.html)
- Erlang/OTP heart.c source. [otp/erts/etc/common/heart.c.](https://github.com/erlang/otp/blob/master/erts/etc/common/heart.c)
- Nerves Heart project. [nerves-project/nerves_heart.](https://github.com/nerves-project/nerves_heart)
- Erlang/OTP supervisor behaviour. [Supervisor Behaviour.](https://www.erlang.org/docs/25/man/supervisor.html)
- Adopting Erlang. [Supervision Trees.](https://adoptingerlang.org/docs/development/supervision_trees/)
- Fred Hebert. [Queues Don't Fix Overload.](https://ferd.ca/queues-don-t-fix-overload.html)
- Learn You Some Erlang. [Who Supervises the Supervisors?](https://learnyousomeerlang.com/supervisors)
- Poettering. [systemd for Administrators, Part XV (Watchdog).](https://0pointer.de/blog/projects/watchdog.html)
- skarnet. [s6: service startup notifications.](https://skarnet.org/software/s6/notifywhenup.html)
- skarnet. [s6: the s6-supervise program.](https://skarnet.org/software/s6/s6-supervise.html)
- Linux kernel documentation. [The Linux Watchdog driver API.](https://www.kernel.org/doc/html/v5.9/watchdog/watchdog-api.html)
- Netflix/Hystrix. [How it Works.](https://github.com/netflix/hystrix/wiki/how-it-works)
- Google SRE Book. [Addressing Cascading Failures.](https://sre.google/sre-book/addressing-cascading-failures/)
- Enterprise Integration Patterns. [Dead Letter Channel.](https://www.enterpriseintegrationpatterns.com/patterns/messaging/DeadLetterChannel.html)
- Fowler, Lindley, Morris, Decova. [Exceptional Asynchronous Session Types.](https://dl.acm.org/doi/10.1145/3290341) POPL 2019.
- Barwell, Scalas, Yoshida, Zhou. [Generalised Multiparty Session Types with Crash-Stop Failures.](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.CONCUR.2022.35) CONCUR 2022.
- Cloud Computing Patterns. [Watchdog.](https://www.cloudcomputingpatterns.org/watchdog/)
- Be Newsletter Issue 2-36, George Hoffman on app_server threading.
- Be Newsletter Issue 3-33, Pavel Cisler on thread synchronization.

---

## 1. Erlang/OTP Supervision: The Canonical Model for "Let It Crash"

### Supervision trees: who restarts what, and in what order

Erlang's supervision tree is a hierarchy of processes divided into workers (do computation) and supervisors (monitor workers and restart them). The key insight is that restart strategies encode dependency relationships:

- **one_for_one**: children are independent. If one dies, only it is restarted. This is the right model when each child represents an independent session or client.
- **rest_for_one**: linear dependency. If one dies, it and all children started after it are restarted. This models initialization-order dependencies -- if B depends on A, and A crashes, B must also be restarted because its assumptions about A's state are now invalid.
- **one_for_all**: strong interdependency. If any child dies, all are restarted. This models tightly coupled subsystems that share implicit invariants.

Each supervisor enforces a restart intensity: maximum N restarts in T seconds. If exceeded, the supervisor itself terminates and the failure escalates to its parent supervisor. This prevents restart storms -- a process that crashes on initialization every time will not spin forever consuming resources.

Children have restart types:

- **permanent**: always restarted, regardless of exit reason.
- **transient**: restarted only on abnormal exit (not on normal shutdown).
- **temporary**: never restarted, even if siblings' deaths cause termination.

The empirical justification, cited in Adopting Erlang: "131/132 of errors encountered in production tended to be heisenbugs" -- transient, non-deterministic failures that restart effectively addresses. Restarting works because it restores known-good state without requiring the specific bug to be found and fixed in real time. The philosophy is not "ignore errors" but "contain errors to the smallest possible blast radius and restore known-good state."

### Heart: the external watchdog that monitors the VM itself

The most architecturally interesting piece of Erlang's resilience story for pane's purposes is `heart` -- an external C program that monitors the BEAM VM from outside.

The architecture is a two-process design:

1. An Erlang process inside the VM sends periodic heartbeats through a port (stdin/stdout pipe) to the external program.
2. The external C program (`heart.c`) runs a select loop with 5-second intervals. If no heartbeat arrives within `HEART_BEAT_TIMEOUT` seconds (default 60, configurable 10--65535), it considers the VM dead.

When the VM dies or becomes unresponsive:

1. `heart` optionally kills the existing Erlang process (configurable via `HEART_KILL_SIGNAL`, default SIGKILL; suppressible via `HEART_NO_KILL`).
2. `heart` executes the command stored in `HEART_COMMAND` environment variable -- typically a restart command.
3. The VM restarts from scratch.

The message protocol is minimal: `Length(2 bytes), Operation(1 byte)`, with optional body up to 2048 bytes. Message types: `HEART_ACK` (startup confirmation), `HEART_BEAT` (periodic keepalive), `SET_CMD`/`CLEAR_CMD`/`GET_CMD` (manage restart command), `SHUT_DOWN` (graceful termination), `PREPARING_CRASH` (core dump in progress).

Two additional mechanisms make heart more than a simple timer:

**Scheduler responsiveness checking.** When `check_schedulers` is enabled, before each heartbeat is sent to the external program, heart signals each scheduler in the VM to check its responsiveness. If any scheduler is unresponsive (stuck in a long computation, deadlocked, etc.), the heartbeat is suppressed. The external heart program never receives the beat, and eventually triggers restart. This catches a failure mode that simple process-level liveness cannot: the VM is technically alive but functionally useless because its schedulers are stuck.

**Validation callbacks.** A custom `{Module, Function}` callback can be registered that executes before every heartbeat. The callback must return `ok` for the heartbeat to proceed. Any exception is treated as validation failure. This allows application-specific health checks -- for example, verifying that the system can still reach its database, or that its message queues aren't backed up beyond a threshold. The system is alive, the schedulers are responsive, but the application considers itself unhealthy.

### The Nerves evolution: defense in depth with hardware watchdog

The Nerves project (Erlang for embedded systems) replaced stock heart with `nerves_heart`, which adds hardware watchdog integration. The architecture becomes three layers:

1. **Application code** sends heartbeats to the Erlang heart process.
2. **nerves_heart** (C program) pets the hardware watchdog timer (WDT) only when it receives heartbeats from Erlang.
3. **Hardware WDT** resets the system if not petted within its timeout.

If Erlang hangs, nerves_heart stops petting the WDT, which resets the board. If nerves_heart itself crashes, the WDT also resets the board. The hardware provides the final safety net that no software can subvert.

Key design choice: nerves_heart directly calls `sync(2)` and `reboot(2)` system calls rather than spawning `reboot(8)` or `poweroff(8)` commands. This eliminates process creation as a failure vector -- if the system is in such bad shape that the VM is unresponsive, spawning a child process to run `/sbin/reboot` might also fail. Direct syscalls are the most reliable path.

### How this informs pane

The heart model is the strongest precedent for what pane's router resilience needs. The core insight is the **external watchdog principle**: the thing being monitored cannot reliably monitor itself. Erlang's answer is a separate C program with its own process, its own failure domain, communicating through the simplest possible channel (a pipe with a trivial protocol).

For pane, the router IS the communication backbone. If it becomes unresponsive, no messages flow. But the router cannot monitor its own responsiveness through the infrastructure it provides -- that's circular. The heart model suggests a dedicated, minimal watchdog that monitors the router through a separate channel (a direct pipe, not the routing protocol), and that can take emergency action (restart the router, flush the journal to disk, notify the compositor directly) if the router stops responding.

The scheduler responsiveness check is equally instructive. A router that is "alive" but unable to dispatch messages in a timely fashion is functionally dead. The health check should verify not just that the router process is running, but that it can actually match and dispatch a test message within acceptable latency.

---

## 2. Watchdog Patterns: Hardware, Kernel, Supervisor

### The three-tier watchdog cascade

Lennart Poettering's systemd watchdog design articulates a cascading supervision model that generalizes the heart pattern:

1. **Hardware watchdog** supervises systemd (PID 1). If PID 1 hangs, the hardware resets the machine.
2. **systemd** supervises individual services. Each service configured with `WatchdogSec=` must periodically call `sd_notify("WATCHDOG=1")`. If a service stops notifying, systemd kills it with SIGABRT and restarts it.
3. **Each service** supervises its own internal health (application-specific checks before sending the watchdog notification).

The design rationale: integrating watchdog support into PID 1 rather than external daemons is necessary because "this is all about the supervisor chain we are building here." External watchdog daemons would require complex IPC and be "drastically more complex, error-prone and resource intensive" than PID 1's straightforward ioctl implementation.

The `/dev/watchdog` interface is minimal by necessity. A userspace daemon opens `/dev/watchdog` and keeps writing to it often enough to prevent a kernel reset -- at least once per minute by default. Each write delays the reboot time. If the daemon stops writing (because it crashed, because the system is hung), the hardware timer expires and resets the machine. The `WDIOF_SETTIMEOUT` flag allows runtime modification of the timeout. `CONFIG_WATCHDOG_NOWAYOUT` means once the watchdog is started, there is no way to disable it -- if the monitoring daemon crashes, the system reboots. No exceptions.

### s6: readiness notification as the complement to watchdog

s6's supervision model adds a crucial piece that systemd's WatchdogSec approach misses: **readiness notification**. A service is supervised, but supervision means more than "is the process alive?" -- it means "is the service ready to do its job?"

s6's protocol: a service writes a newline character to a designated file descriptor (specified in the `notification-fd` file in the service directory) when it becomes operational. The supervisor (`s6-supervise`) catches this, updates the status file, and broadcasts a 'U' (up and ready) event to all subscribers via the event fifodir. Services that depend on this service can block until they receive the 'U' event.

The s6 documentation explicitly calls polling a "bad mechanism" -- it's inefficient and unreliable. Direct notification shifts responsibility to the daemon author, who knows when initialization is actually complete, rather than relying on external heuristics (port is open, PID file exists, etc.).

The readiness notification pattern is the complement to the watchdog pattern:

- **Watchdog**: "are you still healthy?" (ongoing)
- **Readiness**: "are you healthy yet?" (startup)

Both are needed. A service that starts its process but hasn't finished initialization is alive but not ready. A service that was ready but became stuck is ready-but-dead.

### What "detecting loss of user input" looks like

The foundations spec mentions the router detecting "loss of user input." Technically, this is a heartbeat absence:

1. The compositor is the source of user input events (keyboard, mouse, touch).
2. The compositor dispatches input events to focused panes and, for routable actions, sends route messages to the router.
3. If the router stops receiving input-related messages from the compositor for an unusual duration, something is wrong -- either the compositor is dead, or the communication channel between compositor and router is broken.

But this requires the router to distinguish "no user input because the user isn't doing anything" from "no user input because the compositor is dead." The answer is a heartbeat protocol between the compositor and the router, separate from the message traffic. The compositor periodically sends a heartbeat to the router. If the heartbeat stops, the router knows the compositor is unresponsive, even during periods of user inactivity. This is exactly the Erlang heart pattern applied at the inter-server level.

The heartbeat should be **in-band but distinguishable** -- a special message type on the existing session-typed channel between compositor and router. This way it doesn't require a separate communication channel, but the router can distinguish "no messages because the user is idle" from "no messages because the channel is dead." If the session type includes a heartbeat message that the compositor must send at regular intervals, the router can detect compositor death by heartbeat timeout.

### Heartbeat protocols between components

The general pattern for inter-component health monitoring in pane:

```
Component A                    Component B
    |                              |
    |--- Heartbeat(sequence_n) --->|
    |                              |  (B records timestamp)
    |<-- HeartbeatAck(sequence_n) -|
    |                              |
    |  (A records round-trip time) |
    |                              |
    ... (repeat at interval T) ... |
    |                              |
    |--- Heartbeat(sequence_m) --->|
    |                              |
    |  (timeout: no ack within 3T) |
    |                              |
    |  A declares B unresponsive   |
```

This is the standard heartbeat pattern, but the important design choices are:

- **Interval**: too short wastes bandwidth, too long delays failure detection. For a desktop environment, 1--5 seconds is reasonable. User-perceptible lag starts around 100ms; a 2-second heartbeat interval means worst-case 2-second detection latency.
- **Failure threshold**: one missed heartbeat could be a scheduling hiccup. Three consecutive misses (at the standard "3x interval" threshold) provides confidence with 6-second worst-case detection at a 2-second interval.
- **Asymmetry**: the router doesn't need to heartbeat every component. Critical infrastructure (compositor, roster) gets heartbeats. Ordinary clients are monitored by session liveness (socket errors, connection resets) -- the heartbeat overhead isn't worth it for potentially hundreds of client sessions.

---

## 3. Circuit Breaker Patterns: Protecting the Router from Unhealthy Downstream

### The circuit breaker state machine

The circuit breaker pattern, popularized by Michael Nygard in *Release It!* and implemented at scale by Netflix's Hystrix, protects a caller from a failing downstream dependency:

**Closed** (normal): requests pass through. The breaker maintains a rolling window of success/failure statistics. When the error rate exceeds a threshold AND the request volume exceeds a minimum (to avoid tripping on statistical noise), the breaker transitions to Open.

**Open** (rejecting): all requests immediately fail with a "circuit open" error. No requests reach the downstream service. After a configurable sleep window, the breaker transitions to Half-Open.

**Half-Open** (probing): a single test request passes through. If it succeeds, the breaker returns to Closed. If it fails, it returns to Open for another sleep window.

Hystrix's specific thresholds: `circuitBreakerRequestVolumeThreshold` (minimum request count in the rolling window -- prevents tripping on low traffic), `circuitBreakerErrorThresholdPercentage` (the failure rate that triggers opening), `circuitBreakerSleepWindowInMilliseconds` (how long to wait before the Half-Open probe).

### Bulkhead: isolating failure domains

Hystrix's bulkhead pattern isolates dependencies from each other using separate thread pools per dependency. If the payment service is slow and consuming all its allocated threads, the inventory service still has its own dedicated threads. Netflix ran 40+ thread pools with 5--20 threads each per API instance.

The bulkhead prevents a single slow downstream service from consuming all available resources and starving unrelated request paths. Without bulkheads, a slow dependency's requests pile up, exhaust the shared thread pool, and prevent requests to healthy dependencies from executing. The slow service has effectively taken down all other services.

### How the router implements these patterns

The router dispatches messages to handlers -- pane servers, applications, bridges. Each handler is a potential failure point. The router needs circuit breakers per handler:

**Per-handler health tracking.** The router maintains a rolling window of dispatch outcomes (success, timeout, error) for each handler port. When a handler's error rate crosses a threshold, the router opens its circuit:

- Messages for that handler are redirected to the dead letter queue (see section 4) rather than being dispatched to a black hole.
- The router logs the circuit opening and notifies the roster that the handler is unhealthy.
- After the sleep window, the router probes with one message. If the handler responds, the circuit closes.

**Bulkhead through per-handler resource limits.** The router allocates bounded resources per handler: a maximum number of in-flight messages, a maximum queue depth. If handler A is slow and its queue fills, messages to handler A are shed (dead-lettered), but messages to handlers B, C, D continue flowing normally. The slow handler cannot consume all the router's dispatch capacity.

**Timeout propagation.** Every dispatched message carries a deadline. If the message has been in the router's queue long enough that its deadline has passed, the router drops it rather than dispatching it (a stale message wastes the handler's resources for no benefit -- the originator has already given up). This is the Google SRE principle: "avoid doing work for which no credit will be granted."

### What the router doesn't do

The router does not implement retry logic. Retry belongs in the kit layer (pane-app), not in the routing infrastructure. If a message dispatch fails, the router reports the failure back to the sender as a typed error. The sender (via the kit) decides whether to retry, fall back, or degrade. The router's job is to detect unhealthy handlers and protect itself from them, not to paper over their failures.

---

## 4. The "Dispatch of Last Resort" Concept

### What happens when the normal dispatch path fails

The normal path: message arrives at the router, matches a rule, dispatches to the matched handler's port. Failure modes:

1. **No rule matches.** The message doesn't match any routing rule. This is not an error in the traditional sense -- it means the system hasn't been configured to handle this content type. The message goes to the dead letter queue with a "no match" annotation. If the user explicitly triggered the route action, they see a notification: "No handler for this content."

2. **Handler is unresponsive.** The matched handler's circuit is open, or dispatch times out. The message goes to the dead letter queue with a "handler unavailable" annotation. The user sees a notification if they triggered the route.

3. **Multiple handlers match but none can be reached.** All matched handlers are circuit-broken. Same dead letter + notification path.

4. **Transport failure.** The socket connection to the handler is broken. This triggers session cancellation (see error handling research), which the router handles by marking the handler as dead and circuit-breaking it.

### Dead letter queue: where undeliverable messages go

The dead letter queue is the routing system's safety net. Every message that can't be delivered ends up here, annotated with:

- The original message content and metadata
- The reason for non-delivery (no match, handler unavailable, timeout, circuit open)
- Timestamp of the delivery attempt
- Which rules were evaluated and what matched

The dead letter queue is itself a pane. It's queryable through the filesystem interface (`/srv/pane/router/dead-letters/`), scriptable, and visible to the user if they want to inspect it. An agent could monitor the dead letter queue and take corrective action -- "I see messages piling up for the `edit` port because the editor crashed; let me restart it."

The dead letter queue has a bounded size with a retention policy. Old dead letters are evicted FIFO. The queue doesn't grow unbounded -- that would be the router implementing Fred Hebert's critique in "Queues Don't Fix Overload": "creating a bigger buffer to accumulate data that is in-flight, only to lose it sooner or later." The dead letter queue is for diagnostic inspection, not for indefinite storage.

### Priority escalation: messages that must get through

Not all messages are equal. Under load, the router must prioritize:

1. **System health messages** (heartbeats, roster notifications, circuit breaker state changes): these must flow even when the router is shedding load. They are on a separate internal path, not subject to the routing rule engine.
2. **User input events** (route actions triggered by direct user interaction): these get priority over background/automated messages, because user-perceptible latency is the most damaging form of system degradation.
3. **Inter-server protocol messages** (compositor-to-router, roster-to-router): these maintain the infrastructure and get priority over application-level routing.
4. **Application messages** (automated routing, agent messages, background tasks): these are shed first under load.

Implementation: the router maintains a priority queue with at least two levels -- infrastructure and application. Infrastructure messages bypass the application queue entirely. Under load, the application queue sheds (drops the oldest messages, which are least likely to be useful by the time they'd be dispatched -- LIFO dispatch is better than FIFO under overload, per the Google SRE recommendation).

The critical insight from the Google SRE cascading failures chapter: "Graceful degradation shouldn't trigger very often" and requires regular testing, because unused code paths often fail when needed. The router's load shedding and priority paths must be exercised in tests, not left as theoretical emergency-only code.

---

## 5. Session Types and Router Robustness

### Why the router is "best positioned for robustness" through session types

The foundations spec claims the router is "best positioned for robustness, because session-typed design applies most directly to its core function." The reasoning:

The router's core operation is: receive a message, match it against rules, dispatch to a handler. This is a pure protocol operation -- it transforms input into output according to a rule set, with no persistent state beyond the rule set itself and the health tracking of handlers. The router doesn't maintain complex application state, doesn't render UI, doesn't manage filesystems. Its state is:

1. The rule set (loaded from filesystem, updated via pane-notify)
2. Per-handler health statistics (rolling windows of dispatch outcomes)
3. The message queues (transient, bounded)

The session type describes the router's conversation with each client precisely:

```
Client                            Router
  |                                  |
  |-- RouteMessage(content, ctx) --> |
  |                                  |  (match rules, select handler)
  |                                  |
  |<-- RouteResult(matched/none) --- |
  |                                  |
```

This is a simple request-response. The session type captures it completely. There are no complex protocol states, no multi-step negotiations, no branching conversations (beyond the Result type). The simpler the protocol, the more session types buy -- because the gap between "what the protocol should do" and "what the implementation does" is smallest when the protocol is simple.

### What session types catch at compile time

For the router specifically:

- **The router cannot forget to respond.** The session type `Recv<RouteMessage, Send<RouteResult, ...>>` means the router must produce a `RouteResult` after receiving a `RouteMessage`. A code path that consumes the message without producing a result is a compile error.
- **The router cannot send the wrong message type.** If the protocol says `Send<RouteResult>`, the router cannot accidentally send a `HeartbeatAck` or raw bytes.
- **The client cannot send messages out of order.** If the protocol requires sending a `RouteMessage` before receiving a `RouteResult`, a client that tries to receive first is a compile error.
- **Resource linearity.** Each session endpoint is used exactly once per protocol step (enforced by ownership in Rust + `#[must_use]` in par). A forgotten session endpoint generates a compiler warning.

### What failure modes remain

Session types eliminate protocol errors but not all failure modes:

**Transport failure.** The unix socket between client and router can die (process crash, kernel OOM, etc.). Session types operate above the transport layer. The session is typed; the socket is bytes. The bridge between them (serialization/deserialization, socket I/O) is outside the session type's guarantees. This is handled by the cancellation-aware session wrapper described in the error handling research.

**Resource exhaustion.** The router can run out of memory, file descriptors, or thread capacity. Session types don't prevent resource exhaustion -- they guarantee protocol correctness, not resource availability. This is handled by the operational hardening measures (section 7).

**Rule evaluation bugs.** The routing rules are data, not code checked by the session type system. A malformed rule, an infinite loop in pattern matching, a regex catastrophic backtrack -- these are runtime errors in the rule engine, not protocol errors. Mitigation: bounded evaluation time per rule, rule validation on load, and fallback to default dispatch if rule evaluation fails.

**Semantic errors.** The router dispatches a message to the wrong handler because the rules are misconfigured. The protocol was followed perfectly -- the right types in the right order -- but the routing decision was wrong. Session types guarantee protocol correctness, not semantic correctness. This is mitigable only by good rule design and user feedback (the user sees unexpected behavior and corrects the rules).

---

## 6. Graceful Degradation Under System Failure

### What "bracing against cascading failure" looks like

The foundations spec says the router can "trigger escalation procedures: writing the journal to disk, backing up elements of user state, bracing the system against cascading failure." Concretely:

**Journal to disk.** The router maintains an in-memory log of recent routing activity -- messages dispatched, handlers invoked, errors encountered. Under normal operation, this journal is useful for debugging and is periodically flushed. Under system failure (multiple handlers down, compositor unresponsive, resource exhaustion detected), the router force-flushes the journal to a well-known filesystem location. This is not a write-ahead log in the database sense -- it's a crash forensics log. After a system failure, the journal answers "what was happening when things went wrong?"

The flush must be fast and must not depend on infrastructure that might be failing. The target should be a pre-opened file descriptor (opened at startup, kept open), written with `write(2)` directly -- no buffered I/O, no filesystem path resolution, no allocation. Pre-allocate the journal buffer at startup. The write path must have zero dependencies on the rest of the system.

**User state backup.** "User state" in pane's context means: which panes are open, their layout, their content state (unsaved text, scroll positions, working directories). The router doesn't own this state -- the compositor owns layout, each client owns its content state. What the router CAN do is broadcast an emergency "persist your state NOW" message to all connected components. This is the pane equivalent of a `wall` message: a broadcast that says "we're in trouble, save what you can."

This broadcast must be on the same session-typed channels the router already has open to each component. No new infrastructure needed. The message type would be part of the infrastructure protocol -- something like `SystemAlert::PersistState { reason: FailureEscalation }`. Each component that receives this message writes its state to its own well-known location (compositor writes layout to `/run/pane/layout.state`, each client writes its state to its own state file). The router doesn't coordinate the state saving -- it just sends the alarm.

**Cascading failure prevention.** The router's circuit breakers (section 3) ARE the cascading failure prevention. When a handler fails, the router isolates it (opens its circuit), preventing messages from piling up and preventing the failure from consuming router resources. The router continues serving healthy handlers. This is the bulkhead principle: one compartment floods, the others stay dry.

The Google SRE book's guidance applies directly: limit per-client resource allocation (no single handler can consume more than X% of the router's dispatch capacity), fail fast when deadlines have passed (don't dispatch stale messages), and distinguish retriable from non-retriable errors (a handler crash is retriable after restart; a malformed message is not).

### The bootstrap problem: the router can't route its own health alerts through itself

The spec identifies a genuine paradox: if the router is the communication infrastructure, and it needs to send health alerts, who carries those alerts? The router can't route messages through itself when it's the thing that's failing.

The answer draws from how Erlang heart solves the same problem for the VM: **use a separate, simpler channel**.

The router's health alerts do NOT go through the routing infrastructure. They go through direct channels:

1. **Router to compositor**: a direct unix socket session, not mediated by routing rules. The router opens a session to the compositor at startup specifically for system alerts. If the router detects system-level failure (multiple handlers down, resource exhaustion), it sends an alert directly to the compositor, which displays it to the user. This bypasses the routing rule engine entirely.

2. **Router to init system**: the router reports its own health through the watchdog protocol (WatchdogSec on systemd, readiness notification on s6). If the router itself becomes unresponsive, the init system kills and restarts it.

3. **External watchdog**: the most extreme case. A minimal process (pane's equivalent of Erlang's heart) monitors the router via a direct pipe. If the router stops heartbeating, the watchdog writes the journal to disk (from the router's pre-opened fd) and triggers router restart via the init system.

The escalation hierarchy:

```
Layer 0: Router self-monitoring
  Router tracks its own metrics (queue depth, dispatch latency,
  memory usage). If thresholds are crossed, it initiates graceful
  degradation (load shedding, persist state broadcasts).

Layer 1: Direct peer monitoring
  Compositor heartbeats the router. Roster heartbeats the router.
  If they detect the router is unresponsive, they switch to direct
  communication (compositor talks directly to roster, bypassing
  the router) and display user-visible alerts.

Layer 2: Init system supervision
  pane-init monitors the router process. If it dies, pane-init
  restarts it. The restarted router re-reads its rules from the
  filesystem, re-opens sessions to all servers, and resumes
  dispatch. During the restart window, other servers handle
  the gap (compositor processes input locally, roster queues
  registration requests).

Layer 3: External watchdog (optional, for critical deployments)
  A minimal C program (like Erlang's heart) monitors the router
  via a pipe. If the router AND the init system both fail to
  maintain the router, the watchdog can trigger a full session
  persistence and system restart.
```

Each layer is simpler than the one it monitors. The router is complex (rule matching, circuit breakers, priority queues). The init system is simpler (process supervision). The external watchdog is simplest (pipe + timer). This is the fundamental principle: **the monitor must be simpler than the thing it monitors**, or it will have more failure modes than the thing it's supposed to protect.

---

## 7. Making the Router Unkillable

### The goal: not invulnerability, but maximum resilience within Linux's constraints

The router cannot be literally unkillable -- the kernel's OOM killer, a SIGKILL from root, or a hardware failure will always be able to take it down. The goal is to make the router the hardest component to kill accidentally, and the fastest component to restore when it is killed.

### Process priority and scheduling

The router should run at elevated priority but NOT real-time (`SCHED_FIFO` or `SCHED_RR`). Real-time priority is dangerous: a bug in a `SCHED_FIFO` process that enters a tight loop will block all normal-priority processes forever, including the terminal you'd use to kill it. The router should use a high `nice` value (e.g., `nice -10`) or a elevated `SCHED_OTHER` priority, but remain preemptible by the kernel.

The compositor, which handles user input and rendering, has a stronger case for elevated scheduling priority than the router. The router's latency requirements are less stringent -- a few milliseconds of dispatch delay is invisible to users, while a few milliseconds of input delay is perceptible.

### OOM killer protection

The router should set `oom_score_adj = -900` (not -1000). A value of -1000 makes the process completely immune to the OOM killer, which is dangerous: if the router has a memory leak, it will consume all available memory while the OOM killer helplessly kills everything else around it. A value of -900 means the router is among the last processes killed, but CAN be killed if it becomes the problem itself.

The compositor should have similar protection. User-facing applications should have default OOM scores.

For systemd-managed services: `OOMScoreAdjust=-900` in the unit file.

### Memory reservation

The router should pre-allocate its working memory at startup:

- **Message buffers**: a fixed pool of message buffers, sized for the maximum expected concurrent messages. No runtime allocation in the dispatch hot path.
- **Rule table**: loaded and compiled at startup. Rule updates (via pane-notify) are processed by building a new rule table and atomically swapping it in. The old table is freed only after the swap.
- **Circuit breaker state**: pre-allocated per handler, sized for the maximum expected number of handlers.
- **Journal buffer**: pre-allocated at startup, ring-buffer style. New entries overwrite the oldest.

The critical principle: **the dispatch hot path must not allocate**. Every allocation is a potential failure point (OOM), a latency source (allocator contention), and a fragmentation risk. Pre-allocation at startup, when memory is abundant and failure can be handled by refusing to start, eliminates these risks from the running system.

`mlockall(MCL_CURRENT | MCL_FUTURE)` locks the router's pages into RAM, preventing them from being swapped out. This is important when the system is under memory pressure -- a swapped-out router page would cause a page fault during message dispatch, introducing unpredictable latency spikes. The memory cost is small (the router is lean) and the reliability benefit is significant.

### Minimal dependencies

At runtime, the router needs:

- **The kernel**: unix sockets, file descriptors, timers.
- **Its rule set**: loaded from filesystem at startup, held in memory.
- **Socket connections**: to servers and clients that register with it.

It does NOT need:

- The filesystem (after startup -- rules are in memory, journal writes go to a pre-opened fd).
- DNS resolution.
- Dynamic library loading.
- Network connectivity (it's local-only on unix sockets).
- Any other pane server (the router can function in isolation, dispatching to whatever handlers are connected).

The router's dependency set is essentially: the kernel, sockets, and its rule set. This is about as minimal as a useful component can be on Linux.

### Should there be a secondary router?

No. Redundancy through duplication is the wrong answer for the router, for the same reason Erlang doesn't run two BEAM VMs in active-active mode. The complexity of coordinating two routers (rule synchronization, handler registration, session migration, split-brain resolution) exceeds the complexity of the router itself. The added complexity introduces more failure modes than it eliminates.

The right answer is: **make the router simple enough to be reliable, and make restart fast enough that downtime is barely perceptible.**

The router's restart sequence:

1. Init system detects death, starts new router process (< 100ms).
2. Router loads rules from filesystem (< 10ms for a reasonable rule set).
3. Router opens well-known unix socket, begins accepting connections.
4. Infrastructure servers (compositor, roster) detect the old session's death and reconnect (< 100ms).
5. Client kits detect session death and reconnect transparently (< 100ms).

Total: under 500ms from crash to full service restoration. During this window, the compositor handles input locally (text appears, focus changes, but route actions are queued), and client messages queue in their kit-layer send buffers. When the router comes back, queued messages drain.

A 500ms interruption in routing is imperceptible to the user in most scenarios. The exception -- a user presses a button that triggers a route action during the exact 500ms window -- results in a brief delay, not data loss. The kit layer retries the route once the router reconnects.

This is cheaper, simpler, and more reliable than running a hot standby router. Simplicity is the strongest form of reliability.

---

## How This Informs Pane's Design

### The router as immune system: a synthesis

The research reveals that "dispatch of last resort" and "system immune system" are two aspects of the same design:

1. **The router detects illness** through heartbeats from critical infrastructure (compositor, roster), through circuit breaker statistics on handlers, and through its own internal health metrics (queue depth, dispatch latency, memory usage).

2. **The router contains illness** through circuit breakers that isolate failing handlers, through priority queues that protect critical messages from application-level overload, and through load shedding that drops low-priority messages before the system drowns.

3. **The router triggers recovery** through escalation procedures: persist-state broadcasts, journal flushes, direct alerts to the compositor, and notifications to the roster about handler health.

4. **The router is itself protected** by external watchdogs (init system, optional heart-like process), by OOM killer resistance, by pre-allocated memory, and by the simplicity of its core function.

The pattern is the same at every level: the monitor is simpler than the thing it monitors, the communication channel for health is separate from the communication channel for work, and the response to failure is containment first, then recovery.

### Key design decisions emerging from this research

**1. Heartbeat protocol between infrastructure servers.** The compositor, router, and roster heartbeat each other on their direct session-typed channels. Each can detect the others' death within seconds and take appropriate fallback action. The heartbeat is a typed message in the session protocol, not a separate mechanism.

**2. Circuit breakers per handler in the router.** Each handler port gets a circuit breaker with configurable thresholds. Circuit state is visible through the filesystem interface (`/srv/pane/router/handlers/<name>/circuit-state`).

**3. Dead letter queue as a first-class pane.** Undeliverable messages go to a bounded, queryable dead letter queue visible at `/srv/pane/router/dead-letters/`. This serves diagnostics and enables automated recovery by agents.

**4. Priority-based dispatch.** Infrastructure messages (heartbeats, roster notifications, system alerts) bypass the application queue. User-triggered routes get priority over automated routes. Under load, the application queue sheds oldest-first.

**5. Pre-allocated, zero-allocation dispatch path.** The router pre-allocates its working memory at startup. The hot path (receive, match, dispatch) does not allocate. The journal is a pre-allocated ring buffer written to a pre-opened file descriptor.

**6. Direct alert channel.** The router maintains a direct session to the compositor for system alerts that bypasses the routing rule engine. This is the escape hatch for the bootstrap problem.

**7. Fast restart over redundancy.** No secondary router. The router is simple enough to restart in under 500ms, and the kit layer handles reconnection transparently. Simplicity over redundancy.

**8. External watchdog for the router (optional).** For deployments that need maximum reliability, a minimal heart-like process monitors the router via a pipe. This is not required for desktop use but provides defense in depth for server or embedded deployments of pane.

**9. Validation callback for heartbeats.** Following Erlang heart's pattern, the router's internal health check should verify not just process liveness but functional health: can the router actually match and dispatch a test message? Are its queues draining? This prevents the "alive but useless" failure mode.

**10. Graceful degradation is the router's primary emergency response, not graceful shutdown.** When things go wrong, the router sheds load, opens circuits, broadcasts persist-state alerts, and continues running in a degraded mode. Shutdown is the last resort, triggered externally (by the init system if the router is truly unrecoverable). The router fights to stay up, because the alternative -- no communication infrastructure -- is worse than degraded communication infrastructure.
