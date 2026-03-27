use std::path::PathBuf;

/// The kinds of filesystem events pane-notify can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// A file or directory was created.
    Create,
    /// A file or directory was deleted.
    Delete,
    /// A file's content was modified.
    Modify,
    /// A file's metadata changed (xattr, chmod, chown, utimes).
    /// Consumers must re-read and diff to determine what changed —
    /// the kernel does not distinguish which attribute was modified.
    Attrib,
    /// A file or directory was moved (source side).
    MovedFrom,
    /// A file or directory was moved (destination side).
    MovedTo,
}

/// A filesystem event delivered to a consumer.
#[derive(Debug, Clone)]
pub struct Event {
    /// The kind of change.
    pub kind: EventKind,
    /// The path of the affected file or directory.
    /// For fanotify events this is resolved from the file handle
    /// via /proc/self/fd.
    pub path: PathBuf,
}
