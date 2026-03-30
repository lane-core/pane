Pane adopts BeOS's per-component threading model: each component (looper) has its own thread and message queue, processes messages sequentially. Concurrency comes from many such threads, not async I/O within a thread.

calloop is scoped to the Wayland/smithay core (polling Wayland socket, DRM, input devices). It does NOT define the system-wide concurrency model. Per-client sessions, pane-route, pane-roster, and other servers use std::thread + channels — the direct Rust equivalent of BLooper.

Open prototype question: whether smithay's calloop main thread can coexist with per-client session threads in the compositor. (Answer from Phase 4: yes, it works — the frame timer polls client messages via mpsc channels.)

Why: Lane wants to be as close to BeOS's model as sanity permits. calloop appears only in compositor Wayland core context, not as system-wide event model.
