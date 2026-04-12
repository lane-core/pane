---
type: reference
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [adversarial, deadlock, head-of-line, partial-frame, port, timeout, BDirectMessageTarget, write_port_etc, FrameReader]
sources: [~/src/haiku/src/kits/app/LinkReceiver.cpp, ~/src/haiku/src/system/kernel/port.cpp, ~/src/haiku/src/servers/app/MessageLooper.cpp, ~/src/haiku/src/servers/app/ServerApp.cpp, ~/src/haiku/src/servers/app/Desktop.cpp, ~/src/haiku/src/servers/app/ServerWindow.cpp, ~/src/haiku/src/kits/app/Message.cpp, ~/src/haiku/src/kits/app/DirectMessageTarget.cpp, ~/src/haiku/src/servers/app/DelayedMessage.cpp]
verified_against: [haiku HEAD 2026-04-11, pane master bdf130e]
related: [reference/haiku/appserver_concurrency, reference/haiku/internals, architecture/session, decision/connection_source_design]
agents: [be-systems-engineer]
---

# Adversarial bug analysis: Haiku precedents

Three bugs from adversarial testing mapped to BeOS/Haiku
architectural answers.

## Bug 1: Partial frame blocks reader forever

**Haiku answer: port semantics make this impossible.**

BeOS kernel ports are *message-oriented*, not byte-stream. A
`read_port()` returns either a complete message or an error.
There is no concept of a "partial port message." The kernel
atomically enqueues the full buffer in `write_port_etc`
(port.cpp:1668: `portRef->messages.Add(message)`) — the reader
never sees partial data.

The `LinkReceiver` then parses multiple sub-messages from a
single port read (batching). `GetNextMessage()` first checks if
remaining bytes exist in the buffer (line 89: `remaining > 0`);
only if empty does it call `ReadFromPort()` which calls
`read_port[_etc]()` with a timeout parameter (line 80:
`GetNextMessage(int32 &code, bigtime_t timeout)`).

`ServerApp`'s port is owned by the client team
(ServerApp.cpp:130: `set_port_owner(fMessagePort, fClientTeam)`).
When the client team dies, the kernel calls
`delete_owned_ports()` (port.cpp:852), which deletes all ports
owned by the dead team. This fires
`read_condition.NotifyAll(B_BAD_PORT_ID)` (port.cpp:1084),
unblocking any thread waiting in `read_port()`. The
`MessageLooper` (line 146-153) sees `status < B_OK`, prints
"Someone deleted our message port!", and breaks out of its loop.

**Key insight:** The "partial frame" bug is impossible with
message-oriented IPC. With byte-stream (Unix sockets), the
architectural fix is a non-blocking state-machine reader
(WouldBlock-aware) with a per-connection inactivity timeout.
pane already has `FrameReader` in `ConnectionSource`
(connection_source.rs:79-200) which handles WouldBlock at byte
granularity. The remaining gap is the blocking `FrameCodec` path
in frame.rs:191 (`read_exact`), used by server reader threads.

**pane fix direction:** Server reader threads must either (a)
switch to non-blocking reads with a timeout, or (b) use
`read_exact` on a socket with `SO_RCVTIMEO` set, or (c) be
replaced by ConnectionSource-style event sources. Option (c) is
the architectural answer — it's what D12 started. For the server
specifically, reader threads + blocking reads can be retained
if the socket has an inactivity timeout that fires
`Disconnected` to the actor.

## Bug 2: Bidirectional buffer deadlock

**Haiku answer: multi-pronged prevention.**

Three mechanisms prevented symmetric write-blocking deadlock:

1. **BDirectMessageTarget (same-team shortcut).** For same-team
   delivery, `BMessage::_SendMessage()` (Message.cpp:2144-2268)
   bypasses the port entirely. It clones the message into the
   target's `BDirectMessageTarget::fQueue` (an unbounded
   `BMessageQueue`), then pokes the port with a zero-length
   wakeup if the queue was empty. Comment at line 2148: "This
   will also prevent possible deadlocks when the queue is full."
   This is the architectural fix for intra-process deadlock.

2. **Port capacity + B_RELATIVE_TIMEOUT.** For cross-team
   messages, `write_port_etc` with `B_RELATIVE_TIMEOUT` returns
   `B_WOULD_BLOCK` (or `B_TIMED_OUT`) instead of blocking
   indefinitely when the port is full (port.cpp:1594). Callers
   that can tolerate failure use timeout=0.

3. **Asymmetric channel structure.** The app_server protocol is
   fundamentally asymmetric: clients write to ServerApp's port
   (requests), ServerApp replies to client's reply port. They
   don't write to each other's ports symmetrically. The
   `fLink.SetSenderPort(fClientReplyPort)` /
   `fLink.SetReceiverPort(fMessagePort)` split in
   ServerApp.cpp:125-126 means app_server reads from A, writes
   to B — never both to the same port, never in a cycle.

**Could the deadlock occur with kernel ports?** Yes, in
principle: if A writes to B's port and B writes to A's port, and
both ports are full, both block. Port capacity defaults:
`DEFAULT_MONITOR_PORT_SIZE = 50` (ServerConfig.h:52). But in
practice it almost never happened because:
- BDirectMessageTarget handles same-team (eliminates the most
  common case)
- app_server's asymmetric request/reply structure avoids cycles
- BLooper drains its port rapidly (it's the only consumer)
- Port capacity 50 means 50 *messages*, not bytes — substantial
  headroom

**pane fix direction:** pane's architecture already has the D12
pattern: server actor uses `try_send` (non-blocking) to bounded
writer channels. Overflow → connection teardown (server.rs:234).
This is equivalent to `write_port_etc(timeout=0)` + port
deletion. The bidirectional deadlock arises specifically when the
*publisher* blocks on the socket write while the server is also
trying to send replies. Fix: the publisher's write path must also
be non-blocking or async. Either (a) the publisher uses a writer
thread (same D12 pattern), or (b) ConnectionSource's non-blocking
FrameWriter handles both directions on the looper thread without
ever blocking.

## Bug 3: Head-of-line blocking during fan-out

**Haiku answer: independent per-window threads + non-blocking
notifications.**

Desktop's `BroadcastToAllWindows()` (Desktop.cpp:628-636) loops
through all windows calling `PostMessage(code)` on each.
`PostMessage` (MessageLooper.cpp:92) uses `LinkSender::Flush()`
which calls `write_port_etc()`. The default timeout is
`B_INFINITE_TIMEOUT` — so if a window's port is full, Desktop
would block.

However, several mitigations:

1. **Port capacity 50.** Each ServerWindow has its own port with
   capacity 50. A window would need 50 unprocessed messages to
   be "full."

2. **Fire-and-forget notifications.** ServerWindow::RequestRedraw
   (ServerWindow.cpp:378-386): `PostMessage(AS_REDRAW, 0)` with
   timeout 0. Comment: "we don't care if this fails - it's only
   a notification, and if it fails, there are obviously enough
   messages in the queue already." This is the idempotent
   notification pattern — if the window already has a redraw
   pending, another one is redundant.

3. **DelayedMessage system.** (DelayedMessage.cpp) A separate
   sender thread with merge semantics (DM_MERGE_REPLACE,
   DM_MERGE_CANCEL, DM_MERGE_DUPLICATES). Multi-target broadcast
   with per-port timeout of 1 second (line 618:
   `sender.Flush(1000000)`). Failed ports are reported via
   callback, not blocking.

4. **Desktop thread only does routing.** The Desktop thread
   dispatches to ServerApps/ServerWindows via their ports — it
   doesn't do the actual rendering. Each ServerWindow has its own
   thread that does the work. So even if one window is slow to
   process, the Desktop thread's broadcast to other windows
   isn't blocked because the port write succeeds (port isn't full
   — the window thread drains it).

**pane fix direction:** pane's server actor already uses
`try_send` to bounded channels (WRITER_CHANNEL_CAPACITY = 4096
frames). If a channel is full, `try_enqueue` records the
connection for teardown (server.rs:233-234). This is exactly the
fire-and-forget pattern. The sequential loop in `process_service`
is not a problem because `try_send` is O(1) non-blocking.
Head-of-line blocking only arises if the enqueue itself blocks
— which it doesn't with `try_send`.

The remaining gap is the *looper-side* path: ConnectionSource's
write queue. D12 status mentions "VecDeque highwater cap (8
frames) enables backpressure propagation." This is the per-pane
equivalent of ServerWindow's port capacity.
