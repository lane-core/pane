---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [RevokeInterest, ServiceHandle, Drop, cleanup, BMessenger, BHandler, token, port, process_disconnect]
related: [reference/haiku/internals, decision/connection_source_design, architecture/session]
sources:
  - ~/src/haiku/src/kits/app/Messenger.cpp (destructor line 81-83: empty)
  - ~/src/haiku/src/kits/app/Handler.cpp (destructor line 133-155: RemoveToken + RemoveHandler)
  - ~/src/haiku/src/kits/app/Looper.cpp (destructor line 120-165: close_port + drain + RemoveHandler)
  - ~/src/haiku/src/kits/app/Handler.cpp (_SendNotices line 726-747: lazy purge of stale observers)
  - ~/src/haiku/src/servers/app/ServerApp.cpp (_MessageLooper line 3616-3624: port-death detection)
  - pane crates/pane-app/src/service_handle.rs (Drop impl line 270-284)
  - pane crates/pane-session/src/server.rs (process_disconnect line 416-475)
  - pane crates/pane-session/src/bridge.rs (single write channel, WRITE_CHANNEL_CAPACITY=128)
verified_against: [haiku src commit as of 2026-04-11, pane master b938d55]
agents: [be-systems-engineer]
---

# RevokeInterest channel analysis

## Question

Should ServiceHandle::Drop send RevokeInterest through the ctl
channel (guaranteed delivery) instead of the data channel (lossy
try_send)?

## Haiku precedent: three cleanup mechanisms

### 1. BMessenger destruction — NOTHING sent

BMessenger::~BMessenger() is empty (Messenger.cpp:81-83). It stores
{port_id, handler_token, team_id} — all copyable scalars. Destroying
a BMessenger sends no message, notifies nobody. The messenger is just
an address; destroying the address doesn't destroy the thing it
points at.

### 2. BHandler destruction — LOCAL state mutation, no wire message

BHandler::~BHandler() (Handler.cpp:133-155):
1. LockLooper() + RemoveHandler(this) — removes self from parent
   looper's handler list (direct pointer manipulation, looper-local)
2. delete fFilters, delete fObserverList — local cleanup
3. gDefaultTokens.RemoveToken(fToken) — invalidates the token in
   the process-global token space

No message is sent to the looper's port. No message crosses a process
boundary. The cleanup is entirely local state mutation under the
looper lock. The token is simply invalidated — future sends targeting
that token will fail at delivery time (B_BAD_HANDLER).

### 3. Port death — server detects, cleans up reactively

ServerApp::_MessageLooper() (ServerApp.cpp:3616-3624): when
receiver.GetNextMessage() returns an error (port deleted because
client died), the server tells the desktop to delete the app. No
graceful "I'm leaving" message from the client is required.

BLooper destructor (Looper.cpp:123-144): close_port() first, then
drain remaining messages. The port closure is what the server
detects — not a message.

### 4. Lazy purge of stale references

ObserverList::_SendNotices (Handler.cpp:726-747) calls
_ValidateHandlers() before sending, which checks IsValid() on each
stored BMessenger and erases invalid ones. Cleanup happens lazily
at next use, not eagerly at invalidation time.

## Key insight: Be never sent "cleanup messages"

BeOS had NO pattern of "send a message to notify the other side I'm
going away." The cleanup model was:

- **Local side:** Direct state mutation (RemoveHandler, RemoveToken)
- **Remote side:** Detect port death reactively, then clean up

This was deliberate. A cleanup message is a paradox: if you're dying,
you might not be able to send it. If you can send it, you're not
really dying yet. The system should not depend on messages from dying
entities.

## Recommendation for pane

**Option 3: looper-local, no wire message. Let process_disconnect
handle it.**

Reasoning:

1. **Be precedent is unambiguous.** Be never sent cleanup messages.
   BMessenger destruction was a no-op. BHandler destruction was local
   state mutation. The server detected client death reactively via
   port closure. pane's process_disconnect already does exactly this.

2. **The current code has a correctness bug by Be standards.** Sending
   RevokeInterest on the data channel via try_send means the server
   might process it OR might not. This creates a race: the server may
   have already started processing frames for a session that the client
   thinks is revoked. Worse, if the channel is full, the revocation is
   silently lost, and the only cleanup path is process_disconnect —
   which is the path that should have been primary all along.

3. **Moving to ctl doesn't fix the real problem.** Even if RevokeInterest
   goes through a guaranteed-delivery ctl channel, the server still
   needs process_disconnect for crash cleanup. So you have two cleanup
   paths that must produce identical results. Dual cleanup paths are a
   maintenance hazard — when they diverge, you get bugs.

4. **The optimization argument doesn't hold yet.** The argument for
   eager RevokeInterest is "free the server-side route entry sooner."
   But route entries are tiny (two ConnectionId + two u16). The server
   cleans them up on disconnect anyway. Early cleanup is an optimization
   that adds complexity for no measurable benefit at current scale.

5. **If eager notification is needed later, send it from the looper,
   not from Drop.** The looper knows when a ServiceHandle is dropped
   (it owns the handler state). If pane ever needs the server to know
   about individual service teardowns before connection close, the
   looper can batch these into its next write flush — similar to how
   BLooper's destructor drained its port. This keeps the "wire message"
   path under the looper's control, not scattered across arbitrary
   threads running Drop.

### Concrete change

- Remove the Drop impl's try_send of RevokeInterest
- Keep the RevokeInterest variant in ControlMessage (wire format
  stability, and it may be useful later for explicit client-initiated
  teardown that isn't tied to Drop)
- Ensure process_disconnect covers all cleanup (it already does)
- Document that ServiceHandle::Drop is purely local — "destroying the
  address doesn't destroy the service binding; connection close does"
