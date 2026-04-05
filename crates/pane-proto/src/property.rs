//! Optic-backed named attributes for namespace projection.
//!
//! Each attribute is a named lens from handler state to a value.
//! pane-fs reads `/pane/<n>/attrs/<name>` through the getter;
//! writes through the setter. Optic laws (GetPut, PutGet, PutPut)
//! guarantee consistency between the namespace view and the
//! handler's internal state.

use fp_library::types::optics::{Lens, Getter};
use fp_library::brands::RcBrand;

/// A named read-write attribute backed by a Lens.
///
/// Focuses on a value of type A within handler state S.
pub struct Attribute<'a, S: 'a, A: 'a> {
    pub name: &'static str,
    pub lens: Lens<'a, RcBrand, S, S, A, A>,
}

impl<'a, S: 'a, A: 'a> Attribute<'a, S, A> {
    pub fn new(
        name: &'static str,
        get: impl Fn(S) -> A + 'a,
        set: impl Fn((S, A)) -> S + 'a,
    ) -> Self
    where
        S: Clone,
    {
        Attribute {
            name,
            lens: Lens::from_view_set(get, set),
        }
    }

    /// Read the attribute value from state.
    pub fn view(&self, state: S) -> A {
        self.lens.view(state)
    }

    /// Write a new value, returning the updated state.
    pub fn set(&self, state: S, value: A) -> S {
        self.lens.set(state, value)
    }
}

/// A named read-only attribute backed by a Getter.
/// For computed or derived values with no setter.
pub struct ReadOnlyAttribute<'a, S: 'a, A: 'a> {
    pub name: &'static str,
    pub getter: Getter<'a, RcBrand, S, S, A, A>,
}

impl<'a, S: 'a, A: 'a> ReadOnlyAttribute<'a, S, A> {
    pub fn new(name: &'static str, get: impl Fn(S) -> A + 'a) -> Self {
        ReadOnlyAttribute {
            name,
            getter: Getter::new(get),
        }
    }

    pub fn view(&self, state: S) -> A {
        self.getter.view(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct EditorState {
        cursor: usize,
        buffer: String,
    }

    #[test]
    fn attribute_get_put() {
        let cursor = Attribute::new(
            "cursor",
            |s: EditorState| s.cursor,
            |(s, c): (EditorState, usize)| EditorState { cursor: c, ..s },
        );

        let state = EditorState {
            cursor: 42,
            buffer: "hello".into(),
        };

        let val = cursor.view(state.clone());
        let state2 = cursor.set(state.clone(), val);
        assert_eq!(state, state2);
    }

    #[test]
    fn attribute_put_get() {
        let cursor = Attribute::new(
            "cursor",
            |s: EditorState| s.cursor,
            |(s, c): (EditorState, usize)| EditorState { cursor: c, ..s },
        );

        let state = EditorState {
            cursor: 42,
            buffer: "hello".into(),
        };

        let state2 = cursor.set(state, 99);
        assert_eq!(cursor.view(state2), 99);
    }

    #[test]
    fn attribute_put_put() {
        let cursor = Attribute::new(
            "cursor",
            |s: EditorState| s.cursor,
            |(s, c): (EditorState, usize)| EditorState { cursor: c, ..s },
        );

        let state = EditorState {
            cursor: 42,
            buffer: "hello".into(),
        };

        // PutPut: two writes collapse to the last write (full state)
        let left = cursor.set(cursor.set(state.clone(), 10), 20);
        let right = cursor.set(state, 20);
        assert_eq!(left, right, "PutPut violated on full state");
    }

    #[test]
    fn readonly_attribute() {
        let length = ReadOnlyAttribute::new(
            "buffer_length",
            |s: EditorState| s.buffer.len(),
        );

        let state = EditorState {
            cursor: 0,
            buffer: "hello world".into(),
        };

        assert_eq!(length.view(state), 11);
    }
}
