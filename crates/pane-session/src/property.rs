//! Property: optic-backed state access for the namespace projection.
//!
//! Each property is a named lens from handler state to a value.
//! pane-fs reads `/pane/<n>/attrs/<name>` via the getter;
//! writes via the setter. The optic laws (GetPut, PutGet, PutPut)
//! guarantee consistency between the namespace view and the
//! handler's internal state.
//!
//! Uses fp-library's profunctor-encoded Lens.

use fp_library::types::optics::{Lens, Getter};
use fp_library::brands::RcBrand;

/// A named property backed by a Lens into handler state S.
///
/// The lens focuses on a value of type A within state S.
/// GetPut: reading then writing back is identity.
/// PutGet: writing then reading returns the written value.
/// PutPut: two writes collapse to the last write.
pub struct Property<'a, S: 'a, A: 'a> {
    pub name: &'static str,
    pub lens: Lens<'a, RcBrand, S, S, A, A>,
}

impl<'a, S: 'a, A: 'a> Property<'a, S, A> {
    /// Create a property from a name, getter, and setter.
    pub fn new(
        name: &'static str,
        get: impl Fn(S) -> A + 'a,
        set: impl Fn((S, A)) -> S + 'a,
    ) -> Self
    where
        S: Clone,
    {
        Property {
            name,
            lens: Lens::from_view_set(get, set),
        }
    }

    /// Read the property value from state.
    pub fn view(&self, state: S) -> A {
        self.lens.view(state)
    }

    /// Write a new value, returning the updated state.
    pub fn set(&self, state: S, value: A) -> S {
        self.lens.set(state, value)
    }
}

/// A read-only property backed by a Getter.
/// For computed/derived values that don't have a setter.
pub struct ReadOnlyProperty<'a, S: 'a, A: 'a> {
    pub name: &'static str,
    pub getter: Getter<'a, RcBrand, S, S, A, A>,
}

impl<'a, S: 'a, A: 'a> ReadOnlyProperty<'a, S, A> {
    pub fn new(name: &'static str, get: impl Fn(S) -> A + 'a) -> Self {
        ReadOnlyProperty {
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
    fn property_get_put() {
        let cursor = Property::new(
            "cursor",
            |s: EditorState| s.cursor,
            |(s, c): (EditorState, usize)| EditorState { cursor: c, ..s },
        );

        let state = EditorState {
            cursor: 42,
            buffer: "hello".into(),
        };

        // GetPut: view then set back is identity
        let val = cursor.view(state.clone());
        let state2 = cursor.set(state.clone(), val);
        assert_eq!(state, state2);
    }

    #[test]
    fn property_put_get() {
        let cursor = Property::new(
            "cursor",
            |s: EditorState| s.cursor,
            |(s, c): (EditorState, usize)| EditorState { cursor: c, ..s },
        );

        let state = EditorState {
            cursor: 42,
            buffer: "hello".into(),
        };

        // PutGet: set then view returns the set value
        let state2 = cursor.set(state, 99);
        assert_eq!(cursor.view(state2), 99);
    }

    #[test]
    fn readonly_property() {
        let length = ReadOnlyProperty::new(
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
