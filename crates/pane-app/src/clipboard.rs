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
    /// auto-cleared after TTL expires.
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
    /// do not see this entry (ENOENT, not empty).
    Local,
}

use std::sync::mpsc;

/// Internal message from ClipboardWriteLock to the clipboard service.
#[derive(Debug)]
#[doc(hidden)]
pub enum ClipboardCommand {
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
