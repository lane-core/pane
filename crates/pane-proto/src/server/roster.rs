use crate::attrs::{AttrValue, PaneMessage};
use super::ServerVerb;
use super::views::{expect_verb, require_str, optional_str, TypedView, ViewError, Set, Unset};

/// Whether a registered entity is infrastructure (init-supervised) or application (roster-supervised).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Infrastructure,
    Application,
}

impl ServerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Infrastructure => "infrastructure",
            Self::Application => "application",
        }
    }

    fn from_str(s: &str) -> Result<Self, ViewError> {
        match s {
            "infrastructure" => Ok(Self::Infrastructure),
            "application" => Ok(Self::Application),
            other => Err(ViewError::InvalidValue {
                field: "kind",
                detail: format!("expected 'infrastructure' or 'application', got '{}'", other),
            }),
        }
    }
}

// --- RosterRegister: "I'm a server, register me" ---

/// Typed view over a roster-register inter-server message.
/// Required: signature, kind. Optional: socket.
#[derive(Debug)]
pub struct RosterRegister<'a> {
    pub signature: &'a str,
    pub kind: ServerKind,
    pub socket: Option<&'a str>,
}

impl<'a> TypedView<'a> for RosterRegister<'a> {
    fn parse(msg: &'a PaneMessage<ServerVerb>) -> Result<Self, ViewError> {
        expect_verb(msg, ServerVerb::Notify)?;
        let kind_str = require_str(msg, "kind")?;
        let kind = ServerKind::from_str(kind_str)?;
        Ok(Self {
            signature: require_str(msg, "signature")?,
            kind,
            socket: optional_str(msg, "socket")?,
        })
    }
}

pub struct RosterRegisterBuilder<S, K> {
    signature: S,
    kind: K,
    socket: Option<String>,
}

impl RosterRegisterBuilder<Unset, Unset> {
    pub fn new() -> Self {
        Self {
            signature: Unset,
            kind: Unset,
            socket: None,
        }
    }
}

impl<K> RosterRegisterBuilder<Unset, K> {
    pub fn signature(self, sig: impl Into<String>) -> RosterRegisterBuilder<Set<String>, K> {
        RosterRegisterBuilder {
            signature: Set(sig.into()),
            kind: self.kind,
            socket: self.socket,
        }
    }
}

impl<S> RosterRegisterBuilder<S, Unset> {
    pub fn kind(self, kind: ServerKind) -> RosterRegisterBuilder<S, Set<ServerKind>> {
        RosterRegisterBuilder {
            signature: self.signature,
            kind: Set(kind),
            socket: self.socket,
        }
    }
}

impl<S, K> RosterRegisterBuilder<S, K> {
    pub fn socket(mut self, socket: impl Into<String>) -> Self {
        self.socket = Some(socket.into());
        self
    }
}

impl RosterRegisterBuilder<Set<String>, Set<ServerKind>> {
    pub fn into_message(self) -> PaneMessage<ServerVerb> {
        let mut msg = PaneMessage::new(ServerVerb::Notify);
        msg.set_attr("signature", AttrValue::String(self.signature.0));
        msg.set_attr("kind", AttrValue::String(self.kind.0.as_str().to_owned()));
        if let Some(socket) = self.socket {
            msg.set_attr("socket", AttrValue::String(socket));
        }
        msg
    }
}

impl RosterRegister<'_> {
    pub fn build() -> RosterRegisterBuilder<Unset, Unset> {
        RosterRegisterBuilder::new()
    }
}

// --- RosterServiceRegister: "I can do this operation on this content type" ---

/// Typed view over a roster-service-register inter-server message.
/// Required: operation, content_type, description.
#[derive(Debug)]
pub struct RosterServiceRegister<'a> {
    pub operation: &'a str,
    pub content_type: &'a str,
    pub description: &'a str,
}

impl<'a> TypedView<'a> for RosterServiceRegister<'a> {
    fn parse(msg: &'a PaneMessage<ServerVerb>) -> Result<Self, ViewError> {
        expect_verb(msg, ServerVerb::Notify)?;
        Ok(Self {
            operation: require_str(msg, "operation")?,
            content_type: require_str(msg, "content_type")?,
            description: require_str(msg, "description")?,
        })
    }
}

pub struct RosterServiceRegisterBuilder<O, C, D> {
    operation: O,
    content_type: C,
    description: D,
}

impl RosterServiceRegisterBuilder<Unset, Unset, Unset> {
    pub fn new() -> Self {
        Self {
            operation: Unset,
            content_type: Unset,
            description: Unset,
        }
    }
}

impl<C, D> RosterServiceRegisterBuilder<Unset, C, D> {
    pub fn operation(self, op: impl Into<String>) -> RosterServiceRegisterBuilder<Set<String>, C, D> {
        RosterServiceRegisterBuilder {
            operation: Set(op.into()),
            content_type: self.content_type,
            description: self.description,
        }
    }
}

impl<O, D> RosterServiceRegisterBuilder<O, Unset, D> {
    pub fn content_type(self, ct: impl Into<String>) -> RosterServiceRegisterBuilder<O, Set<String>, D> {
        RosterServiceRegisterBuilder {
            operation: self.operation,
            content_type: Set(ct.into()),
            description: self.description,
        }
    }
}

impl<O, C> RosterServiceRegisterBuilder<O, C, Unset> {
    pub fn description(self, desc: impl Into<String>) -> RosterServiceRegisterBuilder<O, C, Set<String>> {
        RosterServiceRegisterBuilder {
            operation: self.operation,
            content_type: self.content_type,
            description: Set(desc.into()),
        }
    }
}

impl RosterServiceRegisterBuilder<Set<String>, Set<String>, Set<String>> {
    pub fn into_message(self) -> PaneMessage<ServerVerb> {
        let mut msg = PaneMessage::new(ServerVerb::Notify);
        msg.set_attr("operation", AttrValue::String(self.operation.0));
        msg.set_attr("content_type", AttrValue::String(self.content_type.0));
        msg.set_attr("description", AttrValue::String(self.description.0));
        msg
    }
}

impl RosterServiceRegister<'_> {
    pub fn build() -> RosterServiceRegisterBuilder<Unset, Unset, Unset> {
        RosterServiceRegisterBuilder::new()
    }
}
