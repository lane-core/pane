//! Dispatch watchdog: detects non-terminating handler callbacks.
//!
//! The looper updates a heartbeat timestamp at the top of each
//! event loop iteration. A watchdog thread checks the heartbeat
//! periodically; if it hasn't advanced within the timeout window,
//! the handler callback is presumed stuck (I2/I3 violation).
//!
//! The watchdog can detect but not prevent blocking — Rust has no
//! signal-based preemption of arbitrary &mut H code. Detection
//! is diagnostic: the callback fires with the stall duration so
//! the caller can log and/or abort.
//!
//! Design heritage: Plan 9 alarm(2) delivered a note to the
//! process after a timeout, but syscalls were interruptible.
//! Rust handler callbacks are not interruptible, so the watchdog
//! is a separate thread (not a signal). BeOS had no explicit
//! watchdog — BLooper relied on the convention that handlers
//! return promptly. pane adds detection where Be relied on
//! convention alone.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Shared heartbeat between the looper and watchdog threads.
/// The looper stores a monotonic tick counter; the watchdog
/// reads it periodically and fires if it hasn't advanced.
#[derive(Clone)]
pub(crate) struct Heartbeat {
    tick: Arc<AtomicU64>,
}

impl Heartbeat {
    pub(crate) fn new() -> Self {
        Heartbeat {
            tick: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Advance the heartbeat. Called by the looper at the top
    /// of each event loop iteration, before dispatch.
    pub(crate) fn beat(&self) {
        self.tick.fetch_add(1, Ordering::Release);
    }

    /// Read the current tick. Called by the watchdog thread.
    fn current(&self) -> u64 {
        self.tick.load(Ordering::Acquire)
    }
}

/// Watchdog that monitors a looper's heartbeat for stalls.
///
/// Spawns a background thread that checks the heartbeat at
/// `interval` intervals. If the heartbeat hasn't advanced in
/// `timeout` duration, fires the `on_stall` callback.
///
/// Dropped when the Looper exits — the background thread
/// detaches (no join, no blocking shutdown).
pub(crate) struct Watchdog {
    // Thread handle kept to prevent the thread from being
    // garbage-collected immediately. The thread exits when
    // it detects the heartbeat Arc has no other owners
    // (looper dropped) via a weak reference check, or it
    // simply runs until the process exits.
    _thread: Option<thread::JoinHandle<()>>,
}

impl Watchdog {
    /// Spawn a watchdog monitoring the given heartbeat.
    ///
    /// `timeout` — how long a stall must last before firing.
    /// `on_stall` — called with the stall duration when detected.
    ///   Runs on the watchdog thread, not the looper thread.
    pub(crate) fn spawn(
        heartbeat: Heartbeat,
        timeout: Duration,
        on_stall: impl Fn(Duration) + Send + 'static,
    ) -> Self {
        // Check interval: half the timeout, so we detect stalls
        // within 1.5x the timeout in the worst case.
        let check_interval = timeout / 2;

        let handle = thread::Builder::new()
            .name("pane-watchdog".into())
            .spawn(move || {
                let mut last_tick = heartbeat.current();
                let mut stall_start: Option<Instant> = None;

                loop {
                    thread::sleep(check_interval);

                    let current = heartbeat.current();
                    if current != last_tick {
                        // Heartbeat advanced — looper is alive.
                        last_tick = current;
                        stall_start = None;
                    } else {
                        // Heartbeat stale — looper may be stuck.
                        let start = *stall_start.get_or_insert_with(Instant::now);
                        let stall_duration = start.elapsed();
                        if stall_duration >= timeout {
                            on_stall(stall_duration);
                            // Reset stall tracking so we don't fire
                            // continuously — fire once per stall.
                            stall_start = None;
                            last_tick = current;
                        }
                    }

                    // If the heartbeat's Arc has been dropped (looper
                    // exited), the tick value stops changing. We could
                    // check Arc::strong_count, but that requires owning
                    // the Arc. Instead, just exit when the check fires
                    // but on_stall has already been called — the looper
                    // is gone, no point watching.
                    //
                    // Actually, we can't detect Arc drop from a clone.
                    // The thread just exits when the process exits.
                    // This is fine — the watchdog is lightweight.
                }
            })
            .ok();

        Watchdog { _thread: handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn heartbeat_advances_monotonically() {
        let hb = Heartbeat::new();
        assert_eq!(hb.current(), 0);
        hb.beat();
        assert_eq!(hb.current(), 1);
        hb.beat();
        assert_eq!(hb.current(), 2);
    }

    #[test]
    fn watchdog_does_not_fire_when_heartbeat_advances() {
        let hb = Heartbeat::new();
        let fired = Arc::new(AtomicBool::new(false));
        let fired2 = fired.clone();

        let _wd = Watchdog::spawn(hb.clone(), Duration::from_millis(50), move |_| {
            fired2.store(true, Ordering::SeqCst);
        });

        // Beat faster than the timeout.
        for _ in 0..10 {
            hb.beat();
            thread::sleep(Duration::from_millis(10));
        }

        assert!(
            !fired.load(Ordering::SeqCst),
            "watchdog must not fire when heartbeat is advancing"
        );
    }

    #[test]
    fn watchdog_fires_on_stall() {
        let hb = Heartbeat::new();
        let fired = Arc::new(AtomicBool::new(false));
        let fired2 = fired.clone();

        // Short timeout for testing.
        let _wd = Watchdog::spawn(hb.clone(), Duration::from_millis(50), move |duration| {
            assert!(
                duration >= Duration::from_millis(50),
                "stall duration must be >= timeout"
            );
            fired2.store(true, Ordering::SeqCst);
        });

        // Don't beat — simulate a stuck handler.
        thread::sleep(Duration::from_millis(150));

        assert!(
            fired.load(Ordering::SeqCst),
            "watchdog must fire when heartbeat stalls"
        );
    }

    #[test]
    fn watchdog_resets_after_firing() {
        let hb = Heartbeat::new();
        let fire_count = Arc::new(AtomicU64::new(0));
        let fire_count2 = fire_count.clone();

        let _wd = Watchdog::spawn(hb.clone(), Duration::from_millis(30), move |_| {
            fire_count2.fetch_add(1, Ordering::SeqCst);
        });

        // Stall long enough for one fire, then beat.
        thread::sleep(Duration::from_millis(80));
        let count_after_stall = fire_count.load(Ordering::SeqCst);
        assert!(
            count_after_stall >= 1,
            "watchdog must fire at least once. Got: {count_after_stall}"
        );

        // Resume beating — should not fire again.
        for _ in 0..5 {
            hb.beat();
            thread::sleep(Duration::from_millis(10));
        }

        let count_after_resume = fire_count.load(Ordering::SeqCst);
        assert_eq!(
            count_after_stall, count_after_resume,
            "watchdog must not fire after heartbeat resumes"
        );
    }
}
