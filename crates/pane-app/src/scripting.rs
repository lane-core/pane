//! Scripting protocol — structured, discoverable access to pane state.
//!
//! Optics + session types = the recovery of BeOS's ResolveSpecifier.
//! Every pane is automatable through the same protocol it uses for
//! everything else.
//!
//! The theory (profunctor optics, dependent session types, separation
//! logic) makes the system sound; app developers don't need to know
//! about it. They see `PropertyInfo`, `ScriptableHandler`, and
//! `#[derive(Scriptable)]` (when it arrives).
//!
//! See `docs/optics-design-brief.md` for the full design rationale.

use std::any::Any;
use std::fmt;

use crate::error::ScriptError;

// ---------------------------------------------------------------------------
// Value types at the scripting boundary
// ---------------------------------------------------------------------------

/// How a property's value is represented at the scripting boundary.
///
/// The enum discriminant IS the type code. No separate numeric codes
/// needed — Rust's exhaustive matching provides type safety that Be's
/// `u32` type codes (`B_STRING_TYPE`, `B_INT32_TYPE`) couldn't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    String,
    Bool,
    Int,
    Float,
    Bytes,
    Rect,
}

/// Values that cross the scripting wire. Shared with filesystem attributes.
///
/// A closed enum — validation is exhaustive pattern matching, not
/// runtime type code checking (which was the source of Be's "type
/// confusion at wire boundary" problem). Custom types go through
/// `Bytes` with application-defined serialization.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    String(std::string::String),
    Bool(bool),
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Rect { x: f64, y: f64, w: f64, h: f64 },
}

impl AttrValue {
    /// The value type discriminant.
    pub fn value_type(&self) -> ValueType {
        match self {
            AttrValue::String(_) => ValueType::String,
            AttrValue::Bool(_) => ValueType::Bool,
            AttrValue::Int(_) => ValueType::Int,
            AttrValue::Float(_) => ValueType::Float,
            AttrValue::Bytes(_) => ValueType::Bytes,
            AttrValue::Rect { .. } => ValueType::Rect,
        }
    }
}

// ---------------------------------------------------------------------------
// Scripting operations and specifiers
// ---------------------------------------------------------------------------

/// Which scripting operations a property supports (declaration).
///
/// Used in `PropertyInfo` to declare capabilities. The actual
/// operation with its payload is `ScriptOp`.
///
/// # BeOS
///
/// `B_GET_PROPERTY`, `B_SET_PROPERTY`, etc. as bitmask flags.
/// Pane uses a typed enum instead of `uint32` bitmask constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Get,
    Set,
    Count,
    Execute,
    Create,
    Delete,
    ListProperties,
}

/// A scripting operation with its payload.
///
/// The operation to perform on a property. `OpKind` is the
/// declaration form (what's supported); `ScriptOp` is the
/// request form (what to do, with data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptOp {
    /// Get a property's value.
    Get,
    /// Set a property's value.
    Set(Vec<u8>),
    /// Count items in a collection property.
    Count,
    /// Execute an action property (no return value).
    Execute,
    /// Create a new item in a collection property.
    Create(Vec<u8>),
    /// Delete an item from a collection property.
    Delete,
    /// List available properties (GetSupportedSuites equivalent).
    ListProperties,
}

impl ScriptOp {
    /// The operation kind (strips payload).
    pub fn kind(&self) -> OpKind {
        match self {
            ScriptOp::Get => OpKind::Get,
            ScriptOp::Set(_) => OpKind::Set,
            ScriptOp::Count => OpKind::Count,
            ScriptOp::Execute => OpKind::Execute,
            ScriptOp::Create(_) => OpKind::Create,
            ScriptOp::Delete => OpKind::Delete,
            ScriptOp::ListProperties => OpKind::ListProperties,
        }
    }
}

/// Specifier forms for addressing a property target.
///
/// # BeOS
///
/// `B_DIRECT_SPECIFIER`, `B_INDEX_SPECIFIER`, `B_NAME_SPECIFIER`,
/// plus reverse index, range, reverse range, and ID. Pane starts
/// with the three most common forms; others can be added as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecifierForm {
    /// The property itself (lens — target always exists).
    Direct,
    /// Nth element of a collection (traversal → affine).
    Index,
    /// Keyed lookup in a collection (affine — may not exist).
    Name,
}

/// A specifier — one step in a property access chain.
///
/// # BeOS
///
/// Part of the `BMessage` specifier stack (AddSpecifier/GetCurrentSpecifier).
/// Pane uses an immutable vec with a separate cursor instead of
/// mutating the message in flight (avoids Be's mutable-message
/// anti-pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Specifier {
    /// Access a property by name (B_DIRECT_SPECIFIER).
    Direct(std::string::String),
    /// Access the nth item in a collection (B_INDEX_SPECIFIER).
    Index(std::string::String, usize),
    /// Access an item by key in a collection (B_NAME_SPECIFIER).
    Named(std::string::String, std::string::String),
}

impl Specifier {
    /// The property name this specifier targets.
    pub fn property(&self) -> &str {
        match self {
            Specifier::Direct(name) => name,
            Specifier::Index(name, _) => name,
            Specifier::Named(name, _) => name,
        }
    }
}

/// A scripting query received from a client or tool.
#[derive(Debug, Clone)]
pub struct ScriptQuery {
    /// The specifier chain — which property, possibly nested.
    pub specifiers: Vec<Specifier>,
    /// The operation (get, set, count, etc.).
    pub operation: ScriptOp,
}

// ---------------------------------------------------------------------------
// Property introspection
// ---------------------------------------------------------------------------

/// Property declaration — what a handler exposes for scripting.
///
/// Static declarations generated by `#[derive(Scriptable)]` (when it
/// arrives) or hand-written. All fields are `&'static` — property
/// tables are known at compile time, matching Be's static
/// `property_info` tables.
///
/// # BeOS
///
/// `property_info` (see `sWindowPropInfo` in `Window.cpp:125-184`).
/// Pane adds descriptions and uses typed enums instead of `u32`
/// bitmasks. Be's `BPropertyInfo::FindMatch` integer dispatch
/// (fragile, reorder-sensitive) is replaced by named optic lookup.
pub struct PropertyInfo {
    /// Property name (e.g., "title", "content", "selection").
    pub name: &'static str,
    /// Human-readable description for tooling.
    pub description: &'static str,
    /// The value type at the scripting boundary.
    pub value_type: ValueType,
    /// Which operations this property supports.
    pub operations: &'static [OpKind],
    /// Which specifier forms can address this property.
    pub specifier_forms: &'static [SpecifierForm],
}

impl Clone for PropertyInfo {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for PropertyInfo {}

impl PartialEq for PropertyInfo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value_type == other.value_type
    }
}

impl Eq for PropertyInfo {}

impl fmt::Debug for PropertyInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyInfo")
            .field("name", &self.name)
            .field("value_type", &self.value_type)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Scripting response
// ---------------------------------------------------------------------------

/// Response to a scripting query.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptResponse {
    /// Direct answer — the property value.
    Value(AttrValue),
    /// Set/execute succeeded.
    Ok,
    /// Property list (GetSupportedSuites equivalent).
    Properties(Vec<PropertyInfo>),
    /// Item count for a collection property.
    Count(usize),
    /// Error.
    Error(ScriptError),
}

// ---------------------------------------------------------------------------
// ScriptableHandler trait
// ---------------------------------------------------------------------------

/// The result of resolving one specifier against a handler.
pub enum Resolution {
    /// This handler owns the property. Here's the optic.
    Resolved(Box<dyn DynOptic>),
    /// Not my property.
    NotFound,
}

/// A handler that exposes scriptable properties.
///
/// Companion trait to [`Handler`](crate::Handler) — not a supertrait.
/// Implement for handlers that want to be automatable through the
/// scripting protocol.
///
/// # BeOS
///
/// `BHandler::ResolveSpecifier` + `BHandler::GetSupportedSuites`.
/// Separated into its own trait because pane has no handler chain
/// (the chain walk that justified these on `BHandler` is now
/// inter-process in pane).
///
/// # BeOS Divergences
///
/// **Separate trait, not on Handler.** Be had `ResolveSpecifier` and
/// `GetSupportedSuites` as virtual methods on `BHandler` because
/// every handler participated in the scripting chain. Pane has one
/// handler per pane; the chain walk is inter-process (deferred to
/// Phase 6 mediator). Non-scriptable handlers avoid the `type State`
/// associated type entirely.
pub trait ScriptableHandler {
    /// The handler's state type (for internal optic access).
    type State: 'static;

    /// Resolve one specifier against this handler's properties.
    ///
    /// Returns `Resolution::Resolved(optic)` if the specifier matches
    /// a known property, `Resolution::NotFound` otherwise.
    ///
    /// # BeOS
    ///
    /// `BHandler::ResolveSpecifier(BMessage*, int32, BMessage*, int32, const char*)`
    fn resolve_specifier(&self, spec: &Specifier) -> Resolution;

    /// List all available properties.
    ///
    /// # BeOS
    ///
    /// `BHandler::GetSupportedSuites(BMessage*)` — accumulated the
    /// property tables from each handler in the chain. Pane returns
    /// a flat slice; suite composition is deferred to Phase 6.
    fn supported_properties(&self) -> &'static [PropertyInfo];

    /// Access the handler's state mutably.
    fn state_mut(&mut self) -> &mut Self::State;
}

// ---------------------------------------------------------------------------
// DynOptic — type-erased optic at the protocol boundary
// ---------------------------------------------------------------------------

/// Type-erased optic for dynamic dispatch at the scripting boundary.
///
/// This is where static, monomorphic optics meet dynamic specifier
/// chains from the wire. Each `DynOptic` wraps a concrete optic
/// (from `pane-optic`) and handles serialization to/from `AttrValue`.
///
/// No ownership/authority annotations — `&`/`&mut` on state is the
/// resource transfer semantics. Rust's type system is sufficient.
///
/// # Invariant
///
/// Implementations MUST return `Err(ScriptError)` on `dyn Any`
/// downcast failure, never panic. The `#[derive(Scriptable)]` proc
/// macro (deferred) will enforce this mechanically.
pub trait DynOptic: Send + Sync {
    /// The property name (for discovery and error messages).
    fn name(&self) -> &str;

    /// Get the property value as an `AttrValue`.
    fn get(&self, state: &dyn Any) -> Result<AttrValue, ScriptError>;

    /// Set the property value from an `AttrValue`.
    fn set(&self, state: &mut dyn Any, value: AttrValue) -> Result<(), ScriptError>;

    /// Whether this optic supports set (lenses yes, getters no).
    fn is_writable(&self) -> bool;

    /// For traversals: count of targets.
    fn count(&self, state: &dyn Any) -> Result<usize, ScriptError>;

    /// The value type this property produces/accepts.
    fn value_type(&self) -> ValueType;

    /// Which operations this property supports.
    fn operations(&self) -> &'static [OpKind];

    /// Which specifier forms can address this property.
    fn specifier_forms(&self) -> &'static [SpecifierForm];
}

// ---------------------------------------------------------------------------
// ScriptReply — reply handle for scripting queries
// ---------------------------------------------------------------------------

/// Reply handle for scripting queries. Newtype over [`ReplyPort`](crate::ReplyPort).
///
/// Consumed by [`ok`](ScriptReply::ok) or [`error`](ScriptReply::error).
/// If dropped without replying, the underlying `ReplyPort::drop` sends
/// `ReplyFailed` to the requestor.
///
/// Does NOT implement custom `Drop` — transparency to `ReplyPort::drop`
/// is what makes panic composition sound. The `panic = unwind` invariant
/// must hold in all pane binaries for Drop-based cleanup to fire.
#[must_use = "dropping without replying sends ReplyFailed to requestor"]
pub struct ScriptReply(crate::ReplyPort);

impl ScriptReply {
    /// Create a ScriptReply wrapping a ReplyPort.
    pub(crate) fn new(reply_port: crate::ReplyPort) -> Self {
        Self(reply_port)
    }

    /// Reply with a successful value.
    pub fn ok(self, value: AttrValue) {
        self.0.reply(ScriptResponse::Value(value));
    }

    /// Reply with an error.
    pub fn error(self, err: ScriptError) {
        self.0.reply(ScriptResponse::Error(err));
    }

    /// The conversation token (for logging/debugging).
    pub fn token(&self) -> u64 {
        self.0.token()
    }
}

impl fmt::Debug for ScriptReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptReply")
            .field("token", &self.0.token())
            .finish()
    }
}
