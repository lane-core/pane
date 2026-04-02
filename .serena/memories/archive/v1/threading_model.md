Pane adopts BeOS's per-component threading model: each component (looper) has its own thread and message queue, processes messages sequentially. Concurrency comes from many such threads, not async I/O within a thread.

calloop is scoped to the Wayland/smithay core (polling Wayland socket, DRM, input devices). It does NOT define the system-wide concurrency model. Per-client sessions, pane-route, pane-roster, and other servers use std::thread + channels — the direct Rust equivalent of BLooper.

Open prototype question: whether smithay's calloop main thread can coexist with per-client session threads in the compositor. (Answer from Phase 4: yes, it works — the frame timer polls client messages via mpsc channels.)

Evolution path (from EAct analysis, 2026-03-29): The current looper uses a single `mpsc::Receiver<LooperMessage>` channel multiplexing compositor events and self-delivery. When Tier 2 features arrive (clipboard, inter-pane messaging, observer pattern, system services), each protocol relationship should be a separate typed channel into the looper, selected via multi-source select (crossbeam-channel or calloop multi-source). This does NOT change the threading model — still one thread per pane — it changes the channel topology within that thread. See serena memory `pane/session_type_design_principles` principle C1 (heterogeneous session loop) and C6 (looper = concurrency boundary, session types = type boundary — keep orthogonal).

Why: Lane wants to be as close to BeOS's model as sanity permits. calloop appears only in compositor Wayland core context, not as system-wide event model.
