# Structural Gaps: EAct Analysis of Pane's Actor Model

Four gaps identified by analyzing Fowler et al. EventActors against pane's current architecture. Each gap includes current state, what EAct reveals, and recommended resolution.

## Gap 1: Untyped Active Phase Has Structured Sub-Protocols

**Current state:** Session types govern the handshake (`ClientHandshake`/`ServerHandshake`), then `finish()` drops into free-form `ClientToComp`/`CompToClient` enums. Both sides send freely.

**What EAct reveals:** The active phase contains sub-protocols with real structure:
- `CreatePane → PaneCreated` (request-response, currently serialized via pending_creates queue)
- `CompletionRequest → CompletionResponse` (token-correlated)
- `Close → CloseAck` (negotiation with handler veto)
- Future: clipboard lock/commit, DnD enter/over/drop, observer start_watching/notify

**Resolution:** Don't session-type the transport. Apply typestate at the API surface (principle C2). Messenger methods that begin sub-protocols return typestate handles that enforce the interaction pattern. The wire stays as enums with correlation IDs.

## Gap 2: Single-Session Looper

**Current state:** Each pane's looper reads from one `mpsc::Receiver<LooperMessage>`. All events (compositor, self-delivery, timer, monitor notifications) are multiplexed into this single channel as LooperMessage variants.

**What EAct reveals:** Actors must handle multiple heterogeneous sessions via the event loop. Pane needs this for:
- Clipboard protocol (separate session)
- Inter-pane messaging (peer sessions)
- System services (audio, notifications — each its own protocol)
- Observer pattern (watcher relationships)

**Resolution:** Evolve the looper to multi-source select (principle C1). Each protocol relationship is a separate typed channel. The looper selects across all channels and dispatches to appropriate handlers. This is the biggest architectural evolution identified.

## Gap 3: Per-Conversation Failure Missing

**Current state:** `monitor()` + `Message::PaneExited { pane, reason }` tells you who died. No information about which pending interaction failed.

**What EAct reveals:** EAct's `suspend` takes a failure callback per conversation. When a peer crashes mid-interaction, the specific conversation's failure handler fires — not just a global death signal. This is critical for request-response patterns between panes.

**Resolution:** Layer conversation-level failure on top of PaneExited (principle C3). When inter-pane request-response is implemented, pending requests should resolve to failure when the peer exits. PaneExited remains the actor-level signal.

## Gap 4: No Cascading Failure / Queue Cleanup

**Current state:** When a pane exits, `ExitBroadcaster` notifies watchers. But messages queued for the dead pane in the dispatcher's routing table are silently dropped (the channel is gone). Messages from the dead pane that are already in other panes' queues will be delivered normally.

**What EAct reveals:** EAct's "zapper threads" propagate failure through sessions — E-CancelMsg drains queued messages for cancelled roles, E-CancelH invokes failure callbacks in all participants waiting on the cancelled role. This ensures no session gets stuck.

**Resolution:** When a pane exits: (1) the dispatcher should drain pending messages for that pane, (2) panes waiting on responses from the dead pane should be notified per-conversation (ties to Gap 3), (3) the exit reason should propagate through any session chains. Current implementation is adequate for the compositor-only model but needs revision when inter-pane sessions exist.
