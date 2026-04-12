---
type: reference
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [BApplication, ReadyToRun, lifecycle, app_server, routing, write_port, backpressure, head_of_line, DirectMessageTarget, SendMessageToClient]
sources: [haiku/src/kits/app/Application.cpp, haiku/src/kits/app/Looper.cpp, haiku/src/kits/app/Messenger.cpp, haiku/src/kits/app/Message.cpp, haiku/src/servers/app/ServerWindow.cpp, haiku/src/servers/app/MessageLooper.cpp, haiku/src/servers/app/Desktop.cpp, haiku/src/servers/app/DelayedMessage.cpp, haiku-website/static/legacy-docs/benewsletter/Issue1-13.html, haiku-website/static/legacy-docs/benewsletter/Issue2-50.html, haiku-website/static/legacy-docs/benewsletter/Issue4-29.html]
verified_against: [Haiku source commit at ~/src/haiku as of 2026-04-11]
agents: [be-systems-engineer]
related: [reference/haiku/appserver_concurrency, architecture/app, architecture/looper, decision/connection_source_design]
---

# BApplication lifecycle, port writes, and app_server routing

Analysis for pane's unified looper-first architecture. Addresses
four questions Lane posed 2026-04-11 about BApplication lifecycle,
app_server routing under hung clients, BLooper port write semantics,
and recommendation for pane.

## Q1: BApplication lifecycle — constructor → Run → ReadyToRun

**Sequence, verified against Haiku source:**

1. **Constructor** (`_InitData`, Application.cpp:353-542):
   - Creates port (`BLooper` constructor via `create_port`)
   - Registers with BRoster (team registration)
   - Posts `B_ARGV_RECEIVED` and `B_READY_TO_RUN` to **own** port
     (Application.cpp:500: `PostMessage(B_READY_TO_RUN, this)`)
   - Connects to app_server (`_InitGUIContext` →
     `_ConnectToServer` → `create_desktop_connection`)
   - All synchronous, all on the calling thread.
   - **Key: looper thread is NOT running yet.** Messages go to the
     port and wait.

2. **Between constructor and Run()** — user code can create
   BWindows, add BHandlers, set up state. All single-threaded.
   Messages accumulate in the port.

3. **Run()** (`BApplication::Run`, Application.cpp:590-599):
   - Calls `Loop()` (NOT `BLooper::Run()` which spawns a thread)
   - `Loop()` hijacks the **main thread** as the message loop
   - `task_looper()` starts dispatching

4. **ReadyToRun** fires as the first or near-first dispatched
   message (it was PostMessage'd in the constructor, so it's in
   the port queue). The handler fires INSIDE the running looper.

**Critical insight for pane:** The BApplication constructor did ALL
setup (app_server connection, roster registration) BEFORE any
message loop ran. `ReadyToRun` was a callback WITHIN the running
loop, not a pre-loop setup phase. User code that needed the looper
running (e.g., sending messages to BWindows) went in ReadyToRun.

**BLooper::Run() vs BLooper::Loop():**
- `Run()` (Looper.cpp:470): spawns a NEW thread, returns thread_id.
  Used by BWindow — window gets its own thread.
- `Loop()` (Looper.cpp:499): uses the CALLING thread as the
  looper. Used by BApplication — main thread becomes app loop.

## Q2: app_server routing — blocking semantics

**SendMessageToClient (ServerWindow.cpp:4399-4408):**
```cpp
ServerWindow::SendMessageToClient(const BMessage* msg, int32 target) const
{
    if (target == B_NULL_TOKEN) target = fClientToken;
    BMessenger reply;
    BMessage::Private messagePrivate((BMessage*)msg);
    return messagePrivate.SendMessage(fClientLooperPort, fClientTeam, target,
        0, false, reply);  // timeout = 0
}
```
**Timeout = 0** with `B_RELATIVE_TIMEOUT` means try-once,
non-blocking. If the client's port is full, write_port_etc
returns `B_WOULD_BLOCK` and the message is **dropped**.

**BroadcastToAllWindows (Desktop.cpp:628-636):**
```cpp
Desktop::BroadcastToAllWindows(int32 code) {
    AutoReadLocker _(fWindowLock);
    for (Window* window = fAllWindows.FirstWindow(); window != NULL;
            window = window->NextWindow(kAllWindowList)) {
        window->ServerWindow()->PostMessage(code);
    }
}
```
ServerWindow's `PostMessage` uses `MessageLooper::PostMessage`
which defaults to `B_INFINITE_TIMEOUT` — blocking. BUT this is
server-internal (ServerWindow threads), not to client apps.

**Server-to-client path is non-blocking, fire-and-forget.**
If a client's port is full, the app_server drops the message.
The app_server NEVER blocks on a client. This is the answer to
head-of-line blocking.

**DelayedMessage system (Haiku addition):**
For broadcast-style messages where drop is unacceptable, Haiku
added a `DelayedMessage` pattern with a 1-second timeout
(DelayedMessage.cpp:618: `sender.Flush(1000000)`) and retry
semantics. Not original BeOS — this is Haiku solving a problem
Be probably had but tolerated.

## Q3: BLooper port write — blocking vs non-blocking

**BMessenger::SendMessage default: B_INFINITE_TIMEOUT (blocking)**
(Messenger.h:47: `bigtime_t timeout = B_INFINITE_TIMEOUT`).
Calls `write_port` (the blocking variant) when port is full.

**BLooper::PostMessage: non-blocking**
Actually goes through BMessenger now (Looper.cpp:896-904), which
means it IS blocking. But the Be Newsletter (Issue 4-29:772-773)
documents original PostMessage as returning B_WOULD_BLOCK immediately.
Haiku's implementation unified them through BMessenger.

**Port capacity: 100 slots** (BLooper) or 200 in some sources.
Newsletter Issue 2-50:105: `B_LOOPER_PORT_DEFAULT_CAPACITY = 100`.

**Direct target optimization (Message.cpp:2262-2274):**
For **same-process** messages, bypasses the port entirely:
```cpp
direct->AddMessage(copy);  // write to in-memory queue
if (direct->Queue()->IsNextMessage(copy) && port_count(port) <= 0) {
    write_port_etc(port, 0, NULL, 0, B_RELATIVE_TIMEOUT, 0);
}
```
The queue is `BMessageQueue` (unbounded linked list). The port
write is zero-byte, non-blocking, used ONLY as a wake-up signal.
**Same-process messages cannot be backpressured by port capacity.**

**Newsletter rationale (Issue 1-13:42-48):**
"If the port fills, the next call to write_port will block. [...]
We chose this design to prevent an orphaned port [...] from
consuming all of system memory."

## Q4: Recommendation for pane

**Three distinct write-path answers from Be:**

| Path | Semantics | Rationale |
|------|-----------|-----------|
| App→App (BMessenger::SendMessage) | Block w/ timeout | Sender should slow down |
| Server→Client (SendMessageToClient) | Non-blocking, drop | Server must not stall |
| Same-process (DirectMessageTarget) | Queue bypass, non-blocking | Port is just a wake-up |

**Mapping to pane's three problems:**

1. **connect() blocking on Looper ack:** Be solved this by doing
   ALL connection setup in the constructor (before Run). ReadyToRun
   was a looper-internal callback for post-setup work. pane should
   match: connect in the builder, callback for post-connect work.

2. **Server head-of-line blocking:** Be's app_server used
   non-blocking writes to clients with message drop on full port.
   pane's ProtocolServer should NOT do synchronous write_frame on
   the actor thread. It should either:
   (a) Non-blocking try_write + drop (Be's model), or
   (b) Per-connection write queues drained by writer threads
       (separates routing from I/O).

3. **Multiple buffer layers:** Be's three-layer answer was
   port (bounded kernel buffer) → BMessageQueue (unbounded
   in-memory) → dispatch. Same-process messages skipped the port.
   pane has: SyncSender → mpsc → VecDeque → FrameWriter → fd.
   The mpsc + VecDeque layers can collapse if ConnectionSource
   reads directly from fd into the looper's event queue.
