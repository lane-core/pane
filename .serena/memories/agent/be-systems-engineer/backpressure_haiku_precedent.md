---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [backpressure, send_request, PostMessage, SendMessage, write_port_etc, coalescing, cancel, three_tier, O1]
related: [decision/connection_source_design, reference/haiku/source, reference/haiku/appserver_concurrency]
agents: [be-systems-engineer]
verified_against: haiku source as of 2026-04-11
sources:
  - src/kits/app/Looper.cpp (task_looper lines 1162-1280, _PostMessage line 896-904)
  - src/kits/app/Messenger.cpp (SendMessage overloads lines 154-215)
  - src/kits/app/Message.cpp (_SendMessage lines 2131-2280, sync variant 2285-2390)
  - src/kits/app/LinkSender.cpp (Flush lines 424-461, StartMessage 83-121)
  - headers/private/app/LinkSender.h (full API)
  - src/kits/app/DirectMessageTarget.cpp (unbounded queue)
  - src/system/kernel/port.cpp (write_port_etc lines 1546-1625, B_WOULD_BLOCK on full)
  - src/servers/app/ServerWindow.cpp (RequestRedraw line 378-386)
  - src/servers/app/Window.cpp (MarkContentDirtyAsync lines 834-847)
  - src/servers/app/MessageLooper.cpp (PostMessage lines 92-97)
  - src/kits/interface/View.cpp (SetViewColor lines 2718-2739)
  - haiku-website benewsletter-wisdom.md (George Hoffman Issue 2-36, lines 57-68)
---

# Backpressure precedent analysis for O1 (three-tier proposal)

## 1. BLooper::PostMessage — always fallible

`_PostMessage` (Looper.cpp:896) creates a BMessenger and calls
`messenger.SendMessage(msg, replyTo, 0)` with **timeout = 0**.
This is non-blocking.

Two code paths inside `BMessage::_SendMessage`:
- **Local same-team:** bypasses port entirely, enqueues via
  `BDirectMessageTarget::AddMessage` into an **unbounded**
  BMessageQueue linked list. Only fails if target is closed
  (returns false, deletes message). Effectively infallible for
  live targets.
- **Remote/cross-team:** calls `write_port_etc(port, ...,
  B_RELATIVE_TIMEOUT, 0)`. Kernel returns `B_WOULD_BLOCK` if
  port full (port.cpp:1594-1595). **Fallible.**

Return type: `status_t`. All PostMessage overloads return status_t.
**PostMessage was always fallible at the API level.** But in
practice, local-team sends almost never failed because the direct
queue was unbounded.

## 2. BMessenger::SendMessage — five overloads, all fallible

All return `status_t`. Three async overloads (fire-and-forget),
two sync (with reply). The timeout parameter on async overloads
defaults to `B_INFINITE_TIMEOUT` (blocks until delivered). Timeout
= 0 means try-once-and-return.

**On error, the BMessage is not consumed.** The caller still owns
it. This is exactly pane's `try_send_*` returning the request on
error. Haiku never had a "message consumed on error" pattern.

## 3. LinkSender::Flush — server-side, also fallible

`Flush(timeout)` (LinkSender.cpp:424) calls `write_port_etc` with
the timeout. Default is `B_INFINITE_TIMEOUT` = blocking.
Returns status_t.

Callers that want non-blocking behavior pass timeout = 0:
- `ServerWindow::RequestRedraw()` calls
  `MessageLooper::PostMessage(AS_REDRAW, 0)` — non-blocking,
  **ignores failure** with comment: "we don't care if this
  fails — it's only a notification."

## 4. No cancellation mechanism

Haiku had no equivalent to Plan 9's Tflush. No `AS_CANCEL`,
no cancel-by-tag, no cancel-by-token. Once a message entered
a port or queue, it would be processed. The only exit was
quitting the looper entirely (`_QUIT_` / `B_QUIT_REQUESTED`).

LinkSender::CancelMessage() cancels in-buffer construction,
not a pending request — it's a "never mind, don't flush this
message I was building."

## 5. Coalescing precedents

### 5a. Dirty region accumulation (app_server side)
`Window::MarkContentDirtyAsync` (Window.cpp:834-847):
accumulates into `fDirtyRegion` and sends RequestRedraw()
only if the region was previously empty. Multiple
invalidations collapse into one redraw. This is **server-side
coalescing**, not client-side.

### 5b. Client-side dedup (not coalescing)
`BView::SetViewColor` (View.cpp:2722-2724): skips sending
if the value is already the same. Per-call dedup, not queue
coalescing — distinct values each generate their own message.

### 5c. LinkSender buffer batching (transport-level)
LinkSender accumulates messages in a buffer and flushes at
a watermark or on explicit Flush(). This is transport-level
batching, not semantic coalescing — all messages are
preserved, just shipped together.

**No Haiku precedent for write-queue coalescing of the kind
pane's `set_content` proposes** (last-write-wins in the
send queue). The dirty-region pattern is the closest analog
but operates server-side on accumulated state, not by
dropping superseded messages from a send queue.

## 6. task_looper dispatch loop and the 1273-1276 break

`task_looper()` (Looper.cpp:1162-1280):
1. Read one message from port (blocking).
2. Drain remaining port messages into queue (non-blocking).
3. Inner loop: dispatch from queue until empty.
4. On each inner iteration, check `port_count(fMsgPort) > 0`
   (line 1273). If new messages arrived on port, break inner
   loop and go back to step 1.

The line-1273 break favors **cross-source responsiveness** over
drain-to-completion. Pane's Inv-CS1 (drain per-source) deliberately
diverges from this.

## 7. Port capacity

`B_LOOPER_PORT_DEFAULT_CAPACITY = 200` (Looper.h:26).
BWindow uses the same default. This is the kernel-enforced
bound on queued messages for remote senders. Local senders
bypass it via BDirectMessageTarget's unbounded queue.

## 8. George Hoffman on queue-full behavior (Be Newsletter 2-36)

"If a window thread becomes unresponsive, and the user continues
to provide input... its message queue will fill up. If this
happens, the app_server will start to throw away update messages
and input intended for the window, and this can cause erratic
behavior."

This is Be's original backpressure policy: drop on overflow,
with the philosophy that an unresponsive window is already broken
so losing messages is the lesser evil.
