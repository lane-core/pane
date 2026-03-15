use std::fmt;

use crate::attrs::PaneMessage;
use super::ServerVerb;

/// Errors from parsing a `PaneMessage<ServerVerb>` into a typed view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// Message has the wrong verb for this view.
    WrongVerb { expected: ServerVerb, got: ServerVerb },
    /// A required attr field is missing.
    MissingField(&'static str),
    /// An attr field has the wrong AttrValue variant.
    WrongFieldType { field: &'static str, expected: &'static str },
    /// An attr field value is invalid (e.g., unrecognized enum string).
    InvalidValue { field: &'static str, detail: String },
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongVerb { expected, got } => {
                write!(f, "wrong verb: expected {:?}, got {:?}", expected, got)
            }
            Self::MissingField(field) => write!(f, "missing required field: {}", field),
            Self::WrongFieldType { field, expected } => {
                write!(f, "field '{}': expected {}", field, expected)
            }
            Self::InvalidValue { field, detail } => {
                write!(f, "field '{}': invalid value: {}", field, detail)
            }
        }
    }
}

impl std::error::Error for ViewError {}

/// Parse a `PaneMessage<ServerVerb>` into a validated typed struct.
///
/// Typed views borrow the message and provide typed accessor methods.
/// Raw `msg.attr("key")` access stays inside `parse()` — consumers
/// use the view's methods.
pub trait TypedView<'a>: Sized {
    fn parse(msg: &'a PaneMessage<ServerVerb>) -> Result<Self, ViewError>;
}

/// Typestate marker: field has been set.
pub struct Set<T>(pub T);
/// Typestate marker: field has not been set.
pub struct Unset;

/// Helper: verify the message verb matches expectations.
pub(crate) fn expect_verb(
    msg: &PaneMessage<ServerVerb>,
    expected: ServerVerb,
) -> Result<(), ViewError> {
    if msg.core != expected {
        Err(ViewError::WrongVerb {
            expected,
            got: msg.core,
        })
    } else {
        Ok(())
    }
}

/// Helper: get a required string attr.
/// Distinguishes MissingField (absent) from WrongFieldType (present, not String).
pub(crate) fn require_str<'a>(
    msg: &'a PaneMessage<ServerVerb>,
    field: &'static str,
) -> Result<&'a str, ViewError> {
    let val = msg.attr(field).ok_or(ViewError::MissingField(field))?;
    val.as_str().ok_or(ViewError::WrongFieldType {
        field,
        expected: "String",
    })
}

/// Helper: get an optional string attr.
/// Returns Ok(None) if absent, Err if present but wrong type.
pub(crate) fn optional_str<'a>(
    msg: &'a PaneMessage<ServerVerb>,
    field: &'static str,
) -> Result<Option<&'a str>, ViewError> {
    match msg.attr(field) {
        None => Ok(None),
        Some(val) => val.as_str().map(Some).ok_or(ViewError::WrongFieldType {
            field,
            expected: "String",
        }),
    }
}
