//! Namespace: the /pane/ directory structure.
//!
//! Each pane registers with the namespace, providing:
//!   - An id (directory name under /pane/)
//!   - A state snapshot (Clone-able, periodically updated by looper)
//!   - An AttrSet for /pane/<id>/attrs/ reads
//!
//! The namespace is a read-side projection. It never holds &mut
//! to the handler — it reads from snapshots. Writes go through
//! the looper as commands (ctl file).

use crate::attrs::{AttrSet, AttrValue};

/// A registered pane in the namespace.
/// S is the handler's state type.
pub struct PaneEntry<S> {
    pub id: u64,
    pub tag: String,
    pub attrs: AttrSet<S>,
    /// State snapshot, updated by the looper after each dispatch cycle.
    pub state: S,
}

impl<S> PaneEntry<S> {
    /// Read an attribute from the current state snapshot.
    pub fn read_attr(&self, name: &str) -> Option<AttrValue> {
        self.attrs.read(name, &self.state)
    }

    /// Update the state snapshot. Called by the looper.
    pub fn update_state(&mut self, state: S) {
        self.state = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::AttrReader;

    #[derive(Clone, Debug)]
    struct StatusState {
        status: String,
        uptime_secs: u64,
    }

    #[test]
    fn pane_entry_reads_attrs_from_snapshot() {
        let mut attrs = AttrSet::new();
        attrs.add(AttrReader::new("status", |s: &StatusState| s.status.clone()));
        attrs.add(AttrReader::new("uptime", |s: &StatusState| s.uptime_secs));

        let entry = PaneEntry {
            id: 1,
            tag: "Server Status".into(),
            attrs,
            state: StatusState {
                status: "online".into(),
                uptime_secs: 3600,
            },
        };

        assert_eq!(entry.read_attr("status").unwrap().0, "online");
        assert_eq!(entry.read_attr("uptime").unwrap().0, "3600");
    }

    #[test]
    fn pane_entry_reflects_state_updates() {
        let mut attrs = AttrSet::new();
        attrs.add(AttrReader::new("status", |s: &StatusState| s.status.clone()));

        let mut entry = PaneEntry {
            id: 1,
            tag: "Server Status".into(),
            attrs,
            state: StatusState {
                status: "online".into(),
                uptime_secs: 0,
            },
        };

        assert_eq!(entry.read_attr("status").unwrap().0, "online");

        // Looper updates the snapshot after handler processes an event
        entry.update_state(StatusState {
            status: "degraded".into(),
            uptime_secs: 120,
        });

        assert_eq!(entry.read_attr("status").unwrap().0, "degraded");
    }
}
