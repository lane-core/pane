# Clipboard Kit API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Kit-level clipboard types: Clipboard handle, ClipboardWriteLock/ReadLock typestates, ClipboardMetadata (sensitivity, locality, MIME), and the Handler integration (message variants, async lock grant).

**Architecture:** Types and Handler plumbing in pane-app. The clipboard service process and protocol channel wiring are deferred — they depend on multi-source select in the looper (noted in looper_message.rs as future C1 work). This plan builds the API surface that the service will plug into.

**Tech Stack:** Rust, pane-app, pane-proto (new message variants)

**Scope boundary:** This plan creates the types, traits, and Handler integration. It does NOT create the clipboard service process, the platform backends, or the pane-fs projection. Those are separate plans when their dependencies are ready.

---

### Task 1: Clipboard types and metadata

**Files:**
- Create: `crates/pane-app/src/clipboard.rs`
- Modify: `crates/pane-app/src/lib.rs` (add `pub mod clipboard` + re-exports)

- [ ] **Step 1: Write the compile test**

Create `crates/pane-app/tests/clipboard.rs`:

```rust
use pane_app::clipboard::{
    Clipboard, ClipboardMetadata, Sensitivity, Locality,
};
use std::time::Duration;

#[test]
fn clipboard_system_default() {
    let clip = Clipboard::system();
    assert_eq!(clip.name(), "system");
}

#[test]
fn clipboard_named() {
    let clip = Clipboard::named("kill-ring");
    assert_eq!(clip.name(), "kill-ring");
}

#[test]
fn metadata_normal() {
    let meta = ClipboardMetadata {
        content_type: "text/plain".into(),
        sensitivity: Sensitivity::Normal,
        locality: Locality::Any,
    };
    assert!(matches!(meta.sensitivity, Sensitivity::Normal));
    assert!(matches!(meta.locality, Locality::Any));
}

#[test]
fn metadata_secret_with_ttl() {
    let meta = ClipboardMetadata {
        content_type: "text/plain".into(),
        sensitivity: Sensitivity::Secret { ttl: Duration::from_secs(30) },
        locality: Locality::Local,
    };
    if let Sensitivity::Secret { ttl } = meta.sensitivity {
        assert_eq!(ttl, Duration::from_secs(30));
    } else {
        panic!("expected Secret");
    }
    assert!(matches!(meta.locality, Locality::Local));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test clipboard 2>&1 | tail -5`
Expected: compilation error — `clipboard` module doesn't exist.

- [ ] **Step 3: Create the clipboard module**

Create `crates/pane-app/src/clipboard.rs`:

```rust
//! Cross-platform clipboard with named clipboards, lazy MIME
//! negotiation, and declarative security policies.
//!
//! This module provides the kit API types. The clipboard service
//! (which holds the actual data and enforces policies) is a
//! separate component wired through the looper's clipboard channel.
//!
//! # BeOS
//!
//! Recovers BClipboard's transactional model (Lock/Clear/Commit)
//! with typestate enforcement, lazy writes, and security policies
//! that Be lacked.
//!
//! # Plan 9
//!
//! The filesystem projection at `/pane/clipboard/{name}/` follows
//! the `/dev/snarf` pattern: read bytes, get text. MIME negotiation
//! and metadata extend the model for modern content types.

use std::time::Duration;

/// A named clipboard handle.
///
/// Does not hold a connection — identifies which clipboard to
/// operate on. The actual data lives in the clipboard service.
///
/// "system" is the well-known default (platform clipboard bridge).
/// Other names are application-defined (kill-ring, registers, etc.).
#[derive(Debug, Clone)]
pub struct Clipboard {
    name: String,
}

impl Clipboard {
    /// The system clipboard (bridges to Wayland selection / NSPasteboard).
    pub fn system() -> Self {
        Clipboard { name: "system".into() }
    }

    /// A named clipboard for application-specific use.
    pub fn named(name: &str) -> Self {
        Clipboard { name: name.into() }
    }

    /// The clipboard's name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Metadata for a clipboard write.
#[derive(Debug, Clone)]
pub struct ClipboardMetadata {
    /// MIME type of the data (e.g., "text/plain", "text/html").
    pub content_type: String,
    /// Sensitivity and lifetime policy.
    pub sensitivity: Sensitivity,
    /// Whether this entry can be read by remote instances.
    pub locality: Locality,
}

/// Sensitivity policy for clipboard entries.
#[derive(Debug, Clone)]
pub enum Sensitivity {
    /// Normal clipboard data. No special handling.
    Normal,
    /// Sensitive data (passwords, tokens). Zeroized on clear,
    /// auto-cleared after TTL expires. The clipboard service
    /// emits `ClipboardCleared { reason: TtlExpired }` on expiry.
    Secret {
        /// Time-to-live. The service auto-clears after this duration.
        ttl: Duration,
    },
}

/// Locality constraint for clipboard entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locality {
    /// Readable from any instance (local or remote).
    Any,
    /// Readable only from the local instance. Remote namespaces
    /// do not see this entry (ENOENT, not empty — leaks less
    /// information per Plan 9 namespace-as-permission model).
    Local,
}
```

Add to `crates/pane-app/src/lib.rs`:

```rust
pub mod clipboard;
```

And in re-exports:

```rust
pub use clipboard::{Clipboard, ClipboardMetadata, Sensitivity, Locality};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-app --test clipboard 2>&1 | tail -5`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pane-app/src/clipboard.rs crates/pane-app/src/lib.rs \
  crates/pane-app/tests/clipboard.rs
git commit -m "Clipboard kit types: Clipboard, ClipboardMetadata, Sensitivity, Locality"
```

---

### Task 2: ClipboardWriteLock typestate handle

**Files:**
- Modify: `crates/pane-app/src/clipboard.rs`
- Modify: `crates/pane-app/tests/clipboard.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/pane-app/tests/clipboard.rs`:

```rust
use pane_app::clipboard::ClipboardWriteLock;

#[test]
fn write_lock_commit_consumes() {
    // ClipboardWriteLock is created by the service (simulated here).
    // commit() consumes the lock. After commit, the lock is gone.
    // This is a compile-time test — if it compiles, the typestate works.
    let (tx, _rx) = std::sync::mpsc::channel();
    let lock = ClipboardWriteLock::new_for_test("system".into(), tx);

    lock.commit(
        b"hello".to_vec(),
        ClipboardMetadata {
            content_type: "text/plain".into(),
            sensitivity: Sensitivity::Normal,
            locality: Locality::Any,
        },
    );
    // lock is consumed — using it again would be a compile error.
}

#[test]
fn write_lock_revert_consumes() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let lock = ClipboardWriteLock::new_for_test("system".into(), tx);
    lock.revert();
}

#[test]
fn write_lock_drop_reverts() {
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let _lock = ClipboardWriteLock::new_for_test("system".into(), tx);
        // dropped without commit — should send revert
    }
    // The revert message should have been sent via the channel
    let msg = rx.try_recv();
    assert!(msg.is_ok(), "drop should send revert");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test clipboard write_lock 2>&1 | tail -5`
Expected: compilation error — `ClipboardWriteLock` not defined.

- [ ] **Step 3: Implement ClipboardWriteLock**

Add to `crates/pane-app/src/clipboard.rs`:

```rust
use std::sync::mpsc;

/// Internal message from ClipboardWriteLock to the clipboard service.
#[derive(Debug)]
pub(crate) enum ClipboardCommand {
    Commit {
        clipboard: String,
        data: Vec<u8>,
        metadata: ClipboardMetadata,
    },
    Revert {
        clipboard: String,
    },
}

/// A typestate handle for writing to a clipboard.
///
/// Created by the clipboard service when a write lock is granted.
/// Must be consumed by `commit()` or `revert()`. Drop without
/// commit = revert (affine gap compensation, same pattern as
/// ReplyPort and PaneCreateFuture).
///
/// # Session type
///
/// Degenerate `Send<CommitOrRevert, End>` — one action, then done.
///
/// # BeOS
///
/// BClipboard Lock/Clear/Commit/Unlock collapsed into a single
/// typestate: lock is implicit in handle creation, commit consumes.
#[must_use = "dropping without commit reverts the clipboard write"]
pub struct ClipboardWriteLock {
    clipboard: String,
    command_tx: mpsc::Sender<ClipboardCommand>,
    consumed: bool,
}

impl ClipboardWriteLock {
    /// For testing — creates a lock backed by a channel.
    #[doc(hidden)]
    pub fn new_for_test(
        clipboard: String,
        command_tx: mpsc::Sender<ClipboardCommand>,
    ) -> Self {
        ClipboardWriteLock {
            clipboard,
            command_tx,
            consumed: false,
        }
    }

    /// Commit data to the clipboard. Consumes the lock.
    pub fn commit(mut self, data: Vec<u8>, metadata: ClipboardMetadata) {
        self.consumed = true;
        let _ = self.command_tx.send(ClipboardCommand::Commit {
            clipboard: self.clipboard.clone(),
            data,
            metadata,
        });
    }

    /// Explicitly revert. Consumes the lock.
    pub fn revert(mut self) {
        self.consumed = true;
        let _ = self.command_tx.send(ClipboardCommand::Revert {
            clipboard: self.clipboard.clone(),
        });
    }
}

impl Drop for ClipboardWriteLock {
    fn drop(&mut self) {
        if !self.consumed {
            let _ = self.command_tx.send(ClipboardCommand::Revert {
                clipboard: self.clipboard.clone(),
            });
        }
    }
}
```

Update re-exports in `lib.rs` to include `ClipboardWriteLock`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-app --test clipboard 2>&1 | tail -5`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pane-app/src/clipboard.rs crates/pane-app/src/lib.rs \
  crates/pane-app/tests/clipboard.rs
git commit -m "ClipboardWriteLock: typestate handle with commit/revert/drop"
```

---

### Task 3: Handler clipboard message variants

**Files:**
- Modify: `crates/pane-app/src/event.rs` (new Message variants)
- Modify: `crates/pane-app/src/handler.rs` (new handler methods)

- [ ] **Step 1: Add Message variants**

Add to the `Message` enum in `crates/pane-app/src/event.rs`:

```rust
    /// A clipboard write lock was granted. Handle the write and commit.
    ClipboardLockGranted(crate::clipboard::ClipboardWriteLock),
    /// A clipboard write lock was denied.
    ClipboardLockDenied {
        clipboard: String,
        reason: String,
    },
    /// A watched clipboard changed.
    ClipboardChanged {
        clipboard: String,
        source: pane_proto::message::PaneId,
    },
```

- [ ] **Step 2: Add Handler methods**

Add to the `Handler` trait in `crates/pane-app/src/handler.rs`:

```rust
    /// A clipboard write lock was granted.
    ///
    /// Use the lock to write data and commit. Drop without commit
    /// automatically reverts.
    ///
    /// Default: reverts (drops the lock without writing).
    fn clipboard_lock_granted(
        &mut self,
        _proxy: &Messenger,
        _lock: crate::clipboard::ClipboardWriteLock,
    ) -> Result<bool> {
        Ok(true) // default: drop lock (auto-revert), continue
    }

    /// A clipboard write lock was denied.
    ///
    /// Default: continues the event loop.
    fn clipboard_lock_denied(
        &mut self,
        _proxy: &Messenger,
        _clipboard: &str,
        _reason: &str,
    ) -> Result<bool> {
        Ok(true)
    }

    /// A watched clipboard changed.
    ///
    /// Default: continues the event loop.
    fn clipboard_changed(
        &mut self,
        _proxy: &Messenger,
        _clipboard: &str,
        _source: pane_proto::message::PaneId,
    ) -> Result<bool> {
        Ok(true)
    }
```

- [ ] **Step 3: Add dispatch in looper**

Add to `dispatch_to_handler` in `crates/pane-app/src/looper.rs`:

```rust
        Message::ClipboardLockGranted(lock) =>
            handler.clipboard_lock_granted(proxy, lock),
        Message::ClipboardLockDenied { ref clipboard, ref reason } =>
            handler.clipboard_lock_denied(proxy, clipboard, reason),
        Message::ClipboardChanged { ref clipboard, source } =>
            handler.clipboard_changed(proxy, clipboard, source),
```

- [ ] **Step 4: Run full test suite**

Run: `cargo test 2>&1 | grep -E "FAILED|test result:" | head -20`
Expected: all pass. (The new variants are added but no test exercises the dispatch yet — that requires the service.)

- [ ] **Step 5: Commit**

```bash
git add crates/pane-app/src/event.rs crates/pane-app/src/handler.rs \
  crates/pane-app/src/looper.rs
git commit -m "Handler clipboard integration: message variants + dispatch"
```

---

### Task 4: Doc comments, PLAN.md

**Files:**
- Modify: `crates/pane-app/src/clipboard.rs` (verify doc comments)
- Modify: `PLAN.md`

- [ ] **Step 1: Run cargo doc**

Run: `cargo doc -p pane-app --no-deps 2>&1 | grep warning`
Expected: zero warnings.

- [ ] **Step 2: Update PLAN.md**

Add under API Tier 2:

```markdown
- [x] **Clipboard kit types** — Clipboard, ClipboardWriteLock (typestate), ClipboardMetadata (sensitivity/TTL/locality), Handler integration. Spec: `docs/superpowers/specs/2026-03-31-clipboard-design.md`. Service process and pane-fs projection deferred.
```

- [ ] **Step 3: Full test suite**

Run: `cargo test 2>&1 | grep -E "FAILED|test result:"` — all green.

- [ ] **Step 4: Commit**

```bash
git add PLAN.md crates/pane-app/src/clipboard.rs
git commit -m "Clipboard kit types: docs and PLAN.md update"
```
