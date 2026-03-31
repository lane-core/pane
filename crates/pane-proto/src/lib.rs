//! Wire types and protocol definitions for compositor-client communication.
//!
//! pane-proto is the shared vocabulary between pane-comp (the compositor)
//! and pane-app (the application kit). Every message that crosses the unix
//! socket boundary is defined here. There is no BeOS equivalent — BeOS
//! used untyped kernel ports with `BMessage` and a `uint32 what` code;
//! pane-proto replaces that with typed Rust enums and a session-typed
//! handshake.
//!
//! The crate has three layers:
//!
//! - **[`protocol`]** — the handshake sequence and active-phase message
//!   enums ([`ClientToComp`], [`CompToClient`])
//! - **Payload types** — [`event`] (keyboard, mouse), [`tag`] (title,
//!   commands), [`color`], [`attrs`] (dynamic attribute values)
//! - **[`wire`]** — serialization (postcard) and length-prefixed framing
//!
//! Protocol changes are refactoring operations, not debugging expeditions:
//! if you add a variant or restructure the handshake, every client that
//! doesn't update fails to compile.
//!
//! # Related Crates
//!
//! - [`pane_session`] — session type primitives that pane-proto's
//!   handshake types compose
//! - `pane_app` — the developer-facing kit that wraps these wire
//!   types into a programming model

pub mod attrs;
pub mod color;
pub mod event;
pub mod message;
pub mod protocol;
pub mod tag;
pub mod wire;

pub use attrs::AttrValue;
pub use color::Color;
pub use event::{KeyEvent, MouseEvent, MouseButton, MouseEventKind, Modifiers, Key};
pub use message::PaneId;
pub use tag::{PaneTitle, CommandVocabulary, CommandGroup, Command, Completion};
pub use protocol::{ClientToComp, CompToClient, PaneGeometry, CreatePaneTag,
    ClientHello, ServerHello, ClientCaps, Accepted, Rejected,
    PeerIdentity, ConnectionTopology};
pub use wire::{deserialize, frame, frame_length, serialize};
