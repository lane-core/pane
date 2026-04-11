---
name: Kit API improvements session-type review (2026-03-31)
description: Analysis of filter mutation, quit protocol, and pending_creates typestate. Filter = message-based like AddTimer. Quit = scatter-gather with no-Messenger callback to prevent deadlock. PaneCreateFuture = degenerate Recv<CreateResponse, End> with cancel-sender Drop.
type: project
---

Reviewed three kit API improvements on 2026-03-31.

**1. Filter mutation:** Message-based (LooperMessage::AddFilter/RemoveFilter), following AddTimer pattern. Mutations take effect between batches, not mid-batch. Returns FilterToken like TimerToken. Filters as stream transformers are Kleisli arrows -- affine gap (ReplyPort/CompletionReplyPort Drop) makes filter consumption safe. No C1 interaction.

**2. Quit protocol:** App-internal scatter-gather, not compositor-routed. DLfActRiS Theorem 5.4 partial-order requirement means quit handler MUST NOT do inter-pane communication. Enforce via `fn quit_requested(&self) -> bool` (no &Messenger parameter). Separate from CloseRequested. Panes needing UI interaction must veto and re-initiate.

**3. PaneCreateFuture:** Degenerate session type Recv<CreateResponse, End>. Drop must handle orphan-pane race: replace pending_creates entry with cancel-sender (sends RequestClose on PaneCreated arrival), not just remove the entry. Current code (app.rs lines 175-183) silently drops PaneCreated with no pending entry -- orphan pane leak if future is dropped after CreatePane sent.

**Why:** These three items are in PLAN.md Tier 2 / session-type debt.

**How to apply:** Reference when implementing these features. The quit protocol's no-Messenger constraint is load-bearing for deadlock freedom. The cancel-sender pattern for PaneCreateFuture is the critical implementation detail.
