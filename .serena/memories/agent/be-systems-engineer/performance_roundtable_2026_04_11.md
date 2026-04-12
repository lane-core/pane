---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [performance, dispatch, routing, batching, LinkSender, BLooper, app_server, ServerWindow, DrawingEngine, kWatermark, direct_message]
sources: [Looper.cpp:1162-1280, LinkSender.cpp:49-461, LinkReceiver.cpp:80-131, link_message.h:17-18, Message.cpp:2142-2275, Messenger.cpp:180-188, ServerWindow.cpp:4218-4385, MessageLooper.cpp:140-165, DrawingEngine.h:37-62, HWInterface.h:56, Issue2-36.html, Issue1-5.html]
verified_against: [~/src/haiku/ as of 2026-04-11]
related: [reference/haiku/appserver_concurrency, reference/haiku/internals, decision/server_actor_model, architecture/looper, architecture/session]
agents: [be-systems-engineer]
---

# Performance roundtable analysis (2026-04-11)

Lane asked three performance questions about pane given 200K msg/sec,
20us P50 benchmarks. Analysis below is grounded in verified Haiku source.

## 1. Single-threaded dispatch

**BLooper** (Looper.cpp:1162-1280): `task_looper()` is a single-thread
message loop. Reads from port, enqueues to MessageQueue, dispatches one
at a time with Lock/Unlock bracketing. No multi-threaded dispatch within
a single BLooper — ever.

**Key optimization in task_looper**: After getting one message from
the port (blocking), it immediately drains `port_count(fMsgPort)`
additional messages without blocking (timeout=0), batching them into
the queue before dispatching any. This amortizes the port read syscall.

**app_server's ServerWindow** (ServerWindow.cpp:4218-4385): Overrides
`_MessageLooper()` with two critical additions:
1. **Batch limit**: Processes at most 70 messages per inner loop
   iteration, or 10ms wall time (line 4357-4358). This prevents one
   chatty window from starving Desktop lock access.
2. **Desktop read-lock held across batch**: Uses
   `LockSingleWindow()` (ReadLock on Desktop.fWindowLock) for most
   operations, upgraded to `LockAllWindows()` (WriteLock) only for
   mutations. Read-lock is held across inner-loop iterations, dropped
   when batch limit hit.

**DrawingEngine and parallel access** (DrawingEngine.h:54-58,
HWInterface.h:56): `LockParallelAccess()` = `ReadLock()` on HWInterface
— a reader-writer lock on the framebuffer. Multiple ServerWindow threads
can draw simultaneously (read-lock). Exclusive access only for
framebuffer mutations (resize, mode change). This is how CPU-intensive
drawing didn't block the message loop — drawing operations hold a
read-lock that doesn't block other drawers.

**Key insight**: Be's approach was NOT to make dispatch itself
multithreaded. It was to ensure drawing operations (the expensive part)
could proceed in parallel across windows via reader-writer locks on
shared resources, while each window's message loop remained
single-threaded.

## 2. Server routing hop

**BMessenger::SendMessage** (Messenger.cpp:180-188): Calls
`BMessage::Private::SendMessage(fPort, fTeam, fHandlerToken, ...)`.
The fPort is the TARGET looper's port. app_server is NOT in this path.

**BMessage::_SendMessage** (Message.cpp:2142-2275): Two paths:
- **Local (same team)**: `BDirectMessageTarget` — enqueues message
  directly into target looper's MessageQueue (no port write at all
  unless queue was empty and looper needs wakeup). Line 2262-2274.
- **Remote (different team)**: `write_port_etc(port, ...)` directly
  to the target looper's kernel port. Line 2246-2249.

**Neither path goes through app_server.** The kernel port system IS
the router for inter-app messages. app_server only handles display
protocol messages (drawing, window management). Inter-window BMessage
communication is direct port-to-port.

**Critical difference for pane**: BeOS got free routing from kernel
ports (each looper has a port, senders write directly to it). pane
has no kernel port equivalent — connections go through ProtocolServer's
Unix sockets. The routing hop is inherent to pane's transport topology.

## 3. Write batching (LinkSender)

**Constants** (link_message.h:17-18):
- `kInitialBufferSize` = 2048 bytes
- `kMaxBufferSize` = 65536 bytes

**kWatermark** (LinkSender.cpp:50-51):
```cpp
static const size_t kWatermark = kInitialBufferSize - 24;
// = 2024 bytes
```
Comment: "if a message is started after this mark, the buffer is
flushed automatically"

**Mechanism** (LinkSender.cpp:83-121, StartMessage):
1. `StartMessage(code)` first calls `EndMessage()` on any in-progress
   message.
2. If buffer in use AND (new message won't fit OR `fCurrentStart >= kWatermark`),
   calls `Flush()`.
3. Otherwise appends message_header to buffer, advances fCurrentEnd.

**Flush** (LinkSender.cpp:424-461): Single `write_port()` call sends
entire buffer contents. Resets fCurrentEnd and fCurrentStart to 0.

**EndMessage** (LinkSender.cpp:125-141): Records final message size in
header, advances fCurrentStart to fCurrentEnd (ready for next message).

**FlushCompleted** (LinkSender.cpp:393-420): Used when a new Attach()
exceeds buffer space. Hides the in-progress message, flushes completed
messages, then moves the incomplete message to buffer start. This is
the key: it never flushes an incomplete message.

**LinkReceiver side** (LinkReceiver.cpp:80-131): `GetNextMessage()`
reads from buffer first; only calls `ReadFromPort()` when buffer is
exhausted. `HasMessages()` checks both buffer remaining AND
`port_count()`. This means the receiver processes multiple messages
from a single port read.

**Batching effect**: N async drawing calls (StrokeLine, FillRect, etc.)
accumulate in a ~2KB buffer. One write_port() delivers them all.
Receiver unpacks them sequentially without additional syscalls. The
watermark of 2024 bytes means roughly 20-50 small drawing commands
per port write.

**Sync calls kill batching**: `FlushWithReply()` (ServerLink) forces
an immediate Flush + blocking read. This is why Be Newsletter Issue
2-36 warned "Synchronous calls are Bad" — they drain the batch buffer
and serialize the pipeline.

**Ordering**: Strictly preserved. Messages within a buffer are
sequential (fCurrentStart advances monotonically). No reordering
possible. Receiver processes in buffer order.

## Implications for pane

See main response to Lane for the three recommendations.
