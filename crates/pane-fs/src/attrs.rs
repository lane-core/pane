//! Attribute access: reading and writing pane state through optics.
//!
//! The core problem: the handler state lives on the looper thread
//! (&mut self), but pane-fs reads from a FUSE thread. We need a
//! way to access attributes without holding &mut across threads.
//!
//! Solution: the looper maintains a Clone-able state snapshot.
//! pane-fs reads attributes from the snapshot (no lock contention
//! with the looper). Writes go through the looper as commands.
//!
//! This module defines the AttributeSet — a type-erased collection
//! of named attributes that pane-fs can enumerate and read.

use std::collections::HashMap;
use std::fmt;

/// A type-erased attribute value, serialized to string for
/// the filesystem interface. pane-fs reads `/pane/<n>/attrs/cursor`
/// and gets back "42" as text.
///
/// The serialization format is the attribute's Display impl,
/// matching Plan 9's convention of text-based ctl/status files.
#[derive(Debug, Clone)]
pub struct AttrValue(pub String);

impl fmt::Display for AttrValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A type-erased attribute reader. Constructed from an
/// Attribute<S, A> by capturing the view closure and a
/// Display-based serializer.
///
/// This is the boundary between the typed optic world
/// (Attribute<S, A> in pane-proto) and the string-based
/// filesystem world (pane-fs reads text).
pub struct AttrReader<S> {
    pub name: &'static str,
    reader: Box<dyn Fn(&S) -> AttrValue + Send + Sync>,
}

impl<S> AttrReader<S> {
    /// Create a reader from a name and a function that extracts
    /// the attribute value as a string from a state reference.
    ///
    /// Note: takes &S (borrow), not S (owned). The caller
    /// provides a reference to the state snapshot. The Attribute's
    /// by-value view() is called inside the closure with a clone.
    pub fn new<A: fmt::Display + 'static>(
        name: &'static str,
        view: impl Fn(&S) -> A + Send + Sync + 'static,
    ) -> Self {
        AttrReader {
            name,
            reader: Box::new(move |s| AttrValue(view(s).to_string())),
        }
    }

    /// Read the attribute from a state reference.
    pub fn read(&self, state: &S) -> AttrValue {
        (self.reader)(state)
    }
}

/// A collection of named attribute readers for one pane.
/// pane-fs uses this to serve `/pane/<n>/attrs/`.
pub struct AttrSet<S> {
    readers: HashMap<&'static str, AttrReader<S>>,
}

impl<S> AttrSet<S> {
    pub fn new() -> Self {
        AttrSet {
            readers: HashMap::new(),
        }
    }

    pub fn add(&mut self, reader: AttrReader<S>) {
        self.readers.insert(reader.name, reader);
    }

    /// Read a named attribute. Returns None if the name doesn't exist.
    pub fn read(&self, name: &str, state: &S) -> Option<AttrValue> {
        self.readers.get(name).map(|r| r.read(state))
    }

    /// List all attribute names (for readdir on /pane/<n>/attrs/).
    pub fn names(&self) -> Vec<&'static str> {
        self.readers.keys().copied().collect()
    }
}

impl<S> Default for AttrSet<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct EditorState {
        cursor: usize,
        buffer: String,
    }

    #[test]
    fn attr_reader_reads_from_state_ref() {
        let reader = AttrReader::new("cursor", |s: &EditorState| s.cursor);

        let state = EditorState {
            cursor: 42,
            buffer: "hello".into(),
        };

        assert_eq!(reader.read(&state).0, "42");
    }

    #[test]
    fn attr_set_serves_multiple_attributes() {
        let mut attrs = AttrSet::new();
        attrs.add(AttrReader::new("cursor", |s: &EditorState| s.cursor));
        attrs.add(AttrReader::new("buffer_length", |s: &EditorState| {
            s.buffer.len()
        }));

        let state = EditorState {
            cursor: 7,
            buffer: "hello world".into(),
        };

        assert_eq!(attrs.read("cursor", &state).unwrap().0, "7");
        assert_eq!(attrs.read("buffer_length", &state).unwrap().0, "11");
        assert!(attrs.read("nonexistent", &state).is_none());
    }

    #[test]
    fn attr_set_lists_names() {
        let mut attrs = AttrSet::<EditorState>::new();
        attrs.add(AttrReader::new("cursor", |s: &EditorState| s.cursor));
        attrs.add(AttrReader::new("title", |s: &EditorState| s.buffer.clone()));

        let mut names = attrs.names();
        names.sort();
        assert_eq!(names, vec!["cursor", "title"]);
    }
}
