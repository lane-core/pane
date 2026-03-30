use serde::{Deserialize, Serialize};

/// A dynamically-typed attribute value.
///
/// Used for filesystem attributes (pane-store), configuration metadata,
/// and any context where runtime-typed key-value data is needed.
///
/// This is NOT part of the session-typed protocol layer — protocol
/// messages are typed Rust enums. AttrValue is for the filesystem/store
/// layer where dynamic typing is appropriate.
///
/// # BeOS
///
/// Descends from `property_info`'s untyped data fields and BeOS's
/// filesystem attribute system (`fs_write_attr` / `fs_read_attr`).
/// pane modernizes the value representation as a Rust enum with
/// typed accessors instead of raw byte buffers with type codes.
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
    /// Borrow as `&str` if this is `String`, else `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AttrValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Return the `i64` if this is `Int`, else `None`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            AttrValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Return the `f64` if this is `Float`, else `None`.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            AttrValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Return the `bool` if this is `Bool`, else `None`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttrValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Borrow as `&[u8]` if this is `Bytes`, else `None`.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            AttrValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Borrow as `&[(String, AttrValue)]` if this is `Attrs`, else `None`.
    pub fn as_attrs(&self) -> Option<&[(String, AttrValue)]> {
        match self {
            AttrValue::Attrs(a) => Some(a),
            _ => None,
        }
    }
}
