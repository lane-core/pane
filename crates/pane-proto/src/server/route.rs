use crate::attrs::{AttrValue, PaneMessage};
use super::ServerVerb;
use super::views::{expect_verb, require_str, optional_str, TypedView, ViewError, Set, Unset};

// --- RouteCommand: "route this text fragment" ---

/// Typed view over a route-command inter-server message.
/// Required: data, wdir. Optional: src, content_type.
#[derive(Debug)]
pub struct RouteCommand<'a> {
    pub data: &'a str,
    pub wdir: &'a str,
    pub src: Option<&'a str>,
    pub content_type: Option<&'a str>,
}

impl<'a> TypedView<'a> for RouteCommand<'a> {
    fn parse(msg: &'a PaneMessage<ServerVerb>) -> Result<Self, ViewError> {
        expect_verb(msg, ServerVerb::Command)?;
        Ok(Self {
            data: require_str(msg, "data")?,
            wdir: require_str(msg, "wdir")?,
            src: optional_str(msg, "src")?,
            content_type: optional_str(msg, "content_type")?,
        })
    }
}

/// Builder for RouteCommand messages. Required fields enforced via typestate.
pub struct RouteCommandBuilder<D, W> {
    data: D,
    wdir: W,
    src: Option<String>,
    content_type: Option<String>,
}

impl RouteCommandBuilder<Unset, Unset> {
    pub fn new() -> Self {
        Self {
            data: Unset,
            wdir: Unset,
            src: None,
            content_type: None,
        }
    }
}

impl<W> RouteCommandBuilder<Unset, W> {
    pub fn data(self, data: impl Into<String>) -> RouteCommandBuilder<Set<String>, W> {
        RouteCommandBuilder {
            data: Set(data.into()),
            wdir: self.wdir,
            src: self.src,
            content_type: self.content_type,
        }
    }
}

impl<D> RouteCommandBuilder<D, Unset> {
    pub fn wdir(self, wdir: impl Into<String>) -> RouteCommandBuilder<D, Set<String>> {
        RouteCommandBuilder {
            data: self.data,
            wdir: Set(wdir.into()),
            src: self.src,
            content_type: self.content_type,
        }
    }
}

impl<D, W> RouteCommandBuilder<D, W> {
    pub fn src(mut self, src: impl Into<String>) -> Self {
        self.src = Some(src.into());
        self
    }

    pub fn content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }
}

impl RouteCommandBuilder<Set<String>, Set<String>> {
    pub fn into_message(self) -> PaneMessage<ServerVerb> {
        let mut msg = PaneMessage::new(ServerVerb::Command);
        msg.set_attr("data", AttrValue::String(self.data.0));
        msg.set_attr("wdir", AttrValue::String(self.wdir.0));
        if let Some(src) = self.src {
            msg.set_attr("src", AttrValue::String(src));
        }
        if let Some(ct) = self.content_type {
            msg.set_attr("content_type", AttrValue::String(ct));
        }
        msg
    }
}

/// Convenience entry point.
impl RouteCommand<'_> {
    pub fn build() -> RouteCommandBuilder<Unset, Unset> {
        RouteCommandBuilder::new()
    }
}

// --- RouteQuery: "what handlers match this content?" ---

/// Typed view over a route-query inter-server message.
/// Required: data. Optional: content_type.
#[derive(Debug)]
pub struct RouteQuery<'a> {
    pub data: &'a str,
    pub content_type: Option<&'a str>,
}

impl<'a> TypedView<'a> for RouteQuery<'a> {
    fn parse(msg: &'a PaneMessage<ServerVerb>) -> Result<Self, ViewError> {
        expect_verb(msg, ServerVerb::Query)?;
        Ok(Self {
            data: require_str(msg, "data")?,
            content_type: optional_str(msg, "content_type")?,
        })
    }
}

pub struct RouteQueryBuilder<D> {
    data: D,
    content_type: Option<String>,
}

impl RouteQueryBuilder<Unset> {
    pub fn new() -> Self {
        Self {
            data: Unset,
            content_type: None,
        }
    }

    pub fn data(self, data: impl Into<String>) -> RouteQueryBuilder<Set<String>> {
        RouteQueryBuilder {
            data: Set(data.into()),
            content_type: self.content_type,
        }
    }
}

impl<D> RouteQueryBuilder<D> {
    pub fn content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }
}

impl RouteQueryBuilder<Set<String>> {
    pub fn into_message(self) -> PaneMessage<ServerVerb> {
        let mut msg = PaneMessage::new(ServerVerb::Query);
        msg.set_attr("data", AttrValue::String(self.data.0));
        if let Some(ct) = self.content_type {
            msg.set_attr("content_type", AttrValue::String(ct));
        }
        msg
    }
}

impl RouteQuery<'_> {
    pub fn build() -> RouteQueryBuilder<Unset> {
        RouteQueryBuilder::new()
    }
}
