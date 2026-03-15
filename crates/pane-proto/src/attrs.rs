use serde::{Deserialize, Serialize};

use crate::polarity::Value;

/// A dynamically-typed attribute value for the PaneMessage attrs bag.
/// Supports nesting for complex payloads (drag-and-drop, file metadata).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Bytes(Vec<u8>),
    /// Nested key-value attributes.
    Attrs(Vec<(String, AttrValue)>),
}

impl AttrValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AttrValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            AttrValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            AttrValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttrValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            AttrValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_attrs(&self) -> Option<&[(String, AttrValue)]> {
        match self {
            AttrValue::Attrs(a) => Some(a),
            _ => None,
        }
    }
}

/// A protocol message wrapping a typed core with an open attributes bag.
///
/// The core provides compile-time exhaustiveness checking.
/// The attrs bag provides BMessage-style extensibility — servers and clients
/// can attach additional key-value pairs that flow through the system.
/// Receivers handle attrs they understand and ignore the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneMessage<T> {
    /// The typed message core.
    pub core: T,
    /// Open-ended attributes. Ordered, duplicates allowed.
    pub attrs: Vec<(String, AttrValue)>,
}

impl<T> PaneMessage<T> {
    /// Wrap a core message with no attrs.
    pub fn new(core: T) -> Self {
        Self {
            core,
            attrs: Vec::new(),
        }
    }

    /// Wrap a core message with the given attrs.
    pub fn with_attrs(core: T, attrs: Vec<(String, AttrValue)>) -> Self {
        Self { core, attrs }
    }

    /// Get the first attr value matching `key`.
    pub fn attr(&self, key: &str) -> Option<&AttrValue> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Get all attr values matching `key`.
    pub fn attrs_all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a AttrValue> + 'a {
        self.attrs.iter().filter(move |(k, _)| k == key).map(|(_, v)| v)
    }

    /// Set the first attr matching `key` to `value`, or append if absent.
    pub fn set_attr(&mut self, key: impl Into<String>, value: AttrValue) {
        let key = key.into();
        if let Some(entry) = self.attrs.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.attrs.push((key, value));
        }
    }

    /// Append an attr (allows duplicates).
    pub fn insert_attr(&mut self, key: impl Into<String>, value: AttrValue) {
        self.attrs.push((key.into(), value));
    }
}

impl Value for AttrValue {}

impl<T> From<T> for PaneMessage<T> {
    fn from(core: T) -> Self {
        Self::new(core)
    }
}
