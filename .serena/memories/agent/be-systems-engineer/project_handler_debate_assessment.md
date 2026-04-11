---
name: Handler architecture debate final assessment
description: Final assessment (2026-03-31) of Pane-as-trait vs Handler+Message debate — typed ingress unified batch, defer trait split, fix Message::Clone, calloop multi-source is the key advance
type: project
---

Three-round debate between Be, Plan 9, and session-type positions on Handler architecture resolved.

**Consensus:** Typed ingress (per-protocol calloop channels), unified batch (convert to filterable events at dispatch boundary). All three positions agree on this mechanism for Phase 3.

**Key findings:**
- Handler growth IS concerning but evidence threshold not met for splitting into per-subsystem traits yet
- Clipboard methods belong on separate ClipboardCallbacks, not on Handler (matches BClipboard model)
- Message::Clone has 4 panicking variants — must split Message into clonable window-lifecycle events vs affine subsystem handles
- LooperMessage enum should freeze; new subsystems get typed calloop channels
- CompletionReplyPort inside Message enum is same smell as clipboard
- Pane-as-trait (compile-time subsystem composition) deferred until 3+ services prove the pattern
- MessageInterest (C5) duplicates what default method impls already provide — defer
- Runtime subsystem composition via channel registration will dominate over type-level composition
- Single-port BLooper was the root cause of BHandler/BWindow bloat; calloop multi-source IS the advance

**Disagreements retained:**
- With session-type: Pane-as-trait not necessarily the "ultimate destination"; runtime composition dominates
- With Plan 9: binding-time gap underspecified; filesystem projection is the right dynamic composition point

**Why:** The looper ingress topology — the boundary between general event loop and protocol-specific subsystems — is the single most important architectural commitment. Everything else (clipboard, observer, DnD, scripting, distribution) composes on top of getting this right.

**How to apply:** Phase 3 implementation should deliver register_channel<E>, ClipboardCallbacks extraction, Message enum split, and LooperMessage freeze. Pane-as-trait remains deferred.
