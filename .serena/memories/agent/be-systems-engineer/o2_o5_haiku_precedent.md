---
type: analysis
status: complete
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [O2, O5, backpressure, cancel, port_capacity, BDirectMessageTarget, RemoveHandler, handshake, cancel-by-token]
related: [decision/connection_source_design, reference/haiku/internals, agent/be-systems-engineer/_hub]
sources: [haiku/headers/os/app/Looper.h, haiku/src/kits/app/Looper.cpp, haiku/src/kits/app/Messenger.cpp, haiku/src/kits/app/Message.cpp, haiku/src/kits/app/DirectMessageTarget.cpp, haiku/src/system/kernel/port.cpp, haiku/headers/os/kernel/OS.h, haiku-website/benewsletter/Issue2-50.html, haiku-website/bebook/BLooper.html]
verified_against: [haiku source tree 2026-04-11]
agents: [be-systems-engineer]
supersedes: []
---

# O2 and O5 Haiku precedent analysis

## O2: Handshake-Negotiated Cap

### Port capacity was never negotiable
Kernel API has no `set_port_queue_size` or `resize_port`.
`create_port(capacity, name)` is one-shot, immutable after creation.
BLooper constructor passes capacity to `_InitData()` → `create_port()`.
If `portCapacity <= 0`, clamped to default. No renegotiation.

**BeOS R5 default: 100.** Haiku doubled to **200**.
(Be Book: `#define B_LOOPER_PORT_DEFAULT_CAPACITY 100`;
 Haiku: same macro = 200.)

### Message exemption from port cap
**BeOS R5:** all messages through port, all counted uniformly.
**Haiku added BDirectMessageTarget:** same-team messages bypass port,
go directly into BMessageQueue (unbounded linked list). Comment in
Message.cpp:2148-2150: "This will also prevent possible deadlocks
when the queue is full."

This is Haiku's equivalent of pane's ctl-plane exemption (D7).
Haiku discovered in practice that some messages must not be blocked
by data-plane congestion.

### 128 default assessment
128 is reasonable:
- Counts only send_request and send_notification (ctl exempt per D7)
- 4x Plan 9 MAXRPC=32, appropriate since pane has notifications
- BeOS 100 counted everything; pane 128 counts less
- 0=unlimited escape hatch important for compositor protocol

### Byte cap concern
"Derived from max_outstanding_requests × max_message_size" is simple
but may need revisiting. BeOS port stored message data in kernel
memory (per-message allocation). Pane's Unix socket buffer is shared
across all queued messages. A large request consuming the socket
buffer while 127 small requests queue behind it is a different
pathology than BeOS faced.

## O5: Cancel Scope

### No cancel mechanism in Haiku — confirmed
Searched Looper.cpp, Handler.h, all headers/os/app/. No cancel,
flush, or abort for in-flight messages. B_CANCEL ('_CNC') in
AppDefs.h is a UI message code for file panel close, not protocol
cancel.

### RemoveHandler does NOT cancel pending messages
RemoveHandler(handler) removes handler from list, sets
handler->Looper() = NULL. Does NOT purge BMessageQueue.
Messages to removed handler: at dispatch time, token lookup
yields handler with Looper() != this → handler set to NULL →
message silently dropped and deleted.

De facto "cancel pending messages for handler X" but:
- Not explicit — side effect of token lookup failure
- No sender notification
- No bulk cancel operation

### Cancel-by-token is the right primitive
Haiku's lack of cancel was a real limitation. Plan 9's Tflush(oldtag)
by-tag was sufficient for 30+ years. Wider scopes should be
library-level compositions:
- cancel_all_for_service: iterate DispatchEntries for service
- cancel_all_for_connection: iterate DispatchEntries for connection
- cancel_by_selector: dangerous spec surface area, defer

Cancel-by-token gives what Haiku lacked: cancel specific long-running
request without tearing down handler/connection. In Haiku, only
out-of-band state (shared atomic flag) could signal "stop working
on that message."
