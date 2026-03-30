use std::ffi::OsString;
use std::path::PathBuf;

/// Stable identity of a filesystem node (device + inode).
///
/// Immutable across renames and moves within a device. Use this to
/// track a file's identity independent of its current name or location.
///
/// # BeOS
///
/// Equivalent to `node_ref` (`dev_t` + `ino_t`). On Linux, this is
/// the `st_dev` + `st_ino` pair from stat(2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeRef {
    pub device: u64,
    pub inode: u64,
}

/// A filesystem event delivered to a consumer.
///
/// Each event identifies the affected node, its path (resolved at
/// delivery time), and what happened. Events are delivered in the
/// order the kernel reports them.
///
/// # BeOS
///
/// Equivalent to the `B_NODE_MONITOR` `BMessage`. Unlike Be's
/// `BMessage`, which used dynamic field lookup, this is a typed
/// enum — the compiler enforces that each event kind carries
/// exactly the right fields.
#[derive(Debug, Clone)]
pub struct Event {
    /// What happened, with kind-specific payload.
    pub kind: EventKind,
    /// Path of the affected file or directory.
    pub path: PathBuf,
    /// Identity of the affected node.
    pub node: NodeRef,
}

/// The kinds of filesystem events pane-notify can report.
///
/// Each variant carries the minimum fields needed to act on it without
/// a round-trip to the filesystem. This follows Haiku's evolution:
/// original BeOS had thin events that forced cache-and-diff; Haiku
/// added cause/fields/attr-name because the original was inadequate.
///
/// # BeOS
///
/// Maps to the opcode field of `B_NODE_MONITOR` messages, with
/// Haiku's detail extensions (attr name, cause, stat fields bitmask).
#[derive(Debug, Clone)]
pub enum EventKind {
    /// A new entry was created in a watched directory.
    ///
    /// # BeOS: `B_ENTRY_CREATED`
    Created {
        /// Name of the new entry within its parent directory.
        name: OsString,
        /// Node ref of the parent directory.
        directory: NodeRef,
    },

    /// An entry was removed (unlinked) from a watched directory.
    ///
    /// The node may still be open elsewhere — the entry is gone but
    /// the data persists until the last fd is closed (POSIX semantics).
    ///
    /// # BeOS: `B_ENTRY_REMOVED`
    Removed {
        /// Name of the removed entry.
        name: OsString,
        /// Node ref of the parent directory.
        directory: NodeRef,
    },

    /// An entry was moved away from a watched directory (source side).
    ///
    /// Paired with `MovedTo` via `cookie`. inotify guarantees the pair
    /// is adjacent in the event stream. A consumer watching only the
    /// source directory will see only `MovedFrom`.
    ///
    /// # BeOS: `B_ENTRY_MOVED`
    ///
    /// BeOS emitted a single atomic event with both directories.
    /// Linux splits moves into two events per directory. The cookie
    /// is inotify's correlation mechanism.
    MovedFrom {
        name: OsString,
        directory: NodeRef,
        /// Correlation cookie. Match with a `MovedTo` bearing the
        /// same cookie to reconstruct the full move.
        cookie: u32,
    },

    /// An entry appeared in a watched directory (destination side).
    ///
    /// Paired with `MovedFrom` via `cookie`.
    ///
    /// # BeOS: `B_ENTRY_MOVED`
    MovedTo {
        name: OsString,
        directory: NodeRef,
        /// Correlation cookie. Match with a `MovedFrom` bearing the
        /// same cookie to reconstruct the full move.
        cookie: u32,
    },

    /// One or more fields of the node's stat structure changed.
    ///
    /// Content modification shows up here as `SIZE | MODIFICATION_TIME`
    /// — there is no separate "content changed" event, following the
    /// Be/Haiku model.
    ///
    /// # BeOS: `B_STAT_CHANGED`
    ///
    /// Original BeOS did not report which fields changed. Haiku added
    /// the `fields` bitmask. We follow Haiku.
    StatChanged {
        /// Bitmask of which stat fields changed.
        fields: StatFields,
    },

    /// An attribute (xattr) on the node was created, modified, or removed.
    ///
    /// # BeOS: `B_ATTR_CHANGED`
    ///
    /// Original BeOS did not report the attribute name or cause. Haiku
    /// added both. We carry the fields, but neither inotify nor fanotify
    /// can fill them today — `attr` will be `None` and `cause` will be
    /// `Unknown` until a supplementary mechanism (e.g., eBPF) is added.
    AttrChanged {
        /// Name of the affected attribute (e.g. "user.pane.tags").
        /// `None` when the kernel interface doesn't provide it.
        attr: Option<OsString>,
        /// What happened to the attribute.
        cause: AttrCause,
    },
}

bitflags::bitflags! {
    /// Bitmask of stat fields that changed.
    ///
    /// # BeOS
    ///
    /// Equivalent to the `fields` field added by Haiku to
    /// `B_STAT_CHANGED` messages.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StatFields: u32 {
        const MODE              = 0x0001;
        const UID               = 0x0002;
        const GID               = 0x0004;
        const SIZE              = 0x0008;
        const ACCESS_TIME       = 0x0010;
        const MODIFICATION_TIME = 0x0020;
        const CREATION_TIME     = 0x0040;
        const CHANGE_TIME       = 0x0080;
        /// Coalescing hint for high-frequency writes. Consumers that
        /// don't need real-time tracking can skip interim updates.
        ///
        /// # BeOS: `B_STAT_INTERIM_UPDATE`
        const INTERIM_UPDATE    = 0x1000;
    }
}

bitflags::bitflags! {
    /// What kinds of events to watch for.
    ///
    /// Passed to [`Watcher::watch_path`] and [`Watcher::watch_mount`].
    /// Separates the "what to watch" question from the "what happened"
    /// answer — `WatchFlags` is the subscription, [`EventKind`] is the
    /// notification.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WatchFlags: u32 {
        /// File/directory creation.
        const CREATE = 0x01;
        /// File/directory removal.
        const REMOVE = 0x02;
        /// File/directory moves (both from and to sides).
        const MOVE   = 0x04;
        /// Stat changes (size, permissions, timestamps).
        const STAT   = 0x08;
        /// Attribute (xattr) changes.
        const ATTR   = 0x10;
        /// All event kinds.
        const ALL    = 0x1F;
    }
}

/// What happened to an attribute.
///
/// # BeOS
///
/// Equivalent to the `cause` field added by Haiku to `B_ATTR_CHANGED`:
/// `B_ATTR_CREATED`, `B_ATTR_REMOVED`, `B_ATTR_CHANGED`.
///
/// Neither inotify nor fanotify distinguishes attr create/modify/remove.
/// The `Unknown` variant covers this case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrCause {
    /// A new attribute was added.
    Created,
    /// An existing attribute's value was modified.
    Modified,
    /// An attribute was removed.
    Removed,
    /// The cause could not be determined from the kernel interface.
    Unknown,
}
