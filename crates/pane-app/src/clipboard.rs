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
