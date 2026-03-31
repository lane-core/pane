//! Application kit for the pane desktop environment.
//!
//! The developer's primary interface for building pane-native applications.
//! This is the descendant of BeOS's Application Kit (`BApplication`,
//! `BLooper`, `BHandler`, `BMessenger`), redesigned for Rust with session
//! types underneath and Haiku's 25 years of refinement as reference
//! (see `reference/haiku-book/app/`).
//!
//! The kit provides five core types:
//!
//! - **[`App`]** — the application entry point. Connects to the compositor,
//!   creates panes. One per process.
//! - **[`Pane`]** — a single surface in the compositor. Configure it, then
//!   consume it by calling [`Pane::run`] or [`Pane::run_with`].
//! - **[`Messenger`]** — a cloneable, `Send` handle for communicating with
//!   the compositor and posting events to a pane's own looper from any thread.
//! - **[`Handler`]** — trait for structured event handling. Override what you
//!   understand; all methods default to continuing the event loop.
//! - **[`Message`]** — typed event enum delivered to handlers. Replaces
//!   BeOS's runtime `BMessage` `what` dispatch with exhaustive matching.
//!
//! # Threading
//!
//! Each pane gets its own looper thread running a sequential message loop.
//! Heavy work goes in spawned threads; the looper stays responsive.
//! Worker threads post results back via [`Messenger::post_app_message`](crate::Messenger::post_app_message)
//! (for application-defined types) or [`Messenger::send_message`]
//! (for system events), and the looper picks them up on its next
//! iteration. This is the `BLooper` model — one thread, one message
//! queue, sequential processing — with the borrow checker replacing
//! `Lock()`/`Unlock()`.
//!
//! # Quick Start
//!
//! ```ignore
//! use pane_app::{App, Tag, cmd, Message};
//!
//! fn main() -> pane_app::Result<()> {
//!     let app = App::connect("com.example.hello")?;
//!     let pane = app.create_pane(
//!         Tag::new("Hello")
//!             .command(cmd("close", "Close").shortcut("Alt+W")),
//!     )?.wait()?;
//!     pane.run(|_messenger, msg| match msg {
//!         Message::Key(key) if key.is_escape() => Ok(false),
//!         Message::CloseRequested => Ok(false),
//!         _ => Ok(true),
//!     })
//! }
//! ```
//!
//! # Related Crates
//!
//! - [`pane_proto`] — wire types and protocol definitions that this kit wraps
//! - [`pane_session`] — session-typed channels used for the compositor handshake

pub mod app;
pub mod completions;
pub(crate) mod connection;
pub mod error;
pub(crate) mod identity;
pub mod event;
pub mod exit;
pub mod filter;
pub mod handler;
pub mod mock; // pub for integration tests
pub mod pane;
pub mod proxy;
pub mod reply;
pub mod shortcuts;
pub mod tag;

// --- Internal modules: hidden from docs, accessible from integration tests ---

#[doc(hidden)]
pub mod looper;
pub(crate) mod looper_message;
#[doc(hidden)]
pub use looper_message::LooperMessage;

// Future API (Phase 6 stubs — not yet implemented)
#[doc(hidden)]
pub mod routing;
#[doc(hidden)]
pub mod scripting;

// --- Developer-facing API ---

pub use app::{App, PaneCreateFuture};
pub use error::{Error, ConnectError, PaneError, ScriptError, Result};
pub use event::Message;
pub use exit::ExitReason;
pub use filter::{MessageFilter, FilterAction, FilterToken};
pub use handler::Handler;
pub use pane::Pane;
pub use proxy::{Messenger, TimerToken};
pub use completions::CompletionReplyPort;
pub use reply::ReplyPort;
pub use shortcuts::KeyCombo;
pub use tag::{Tag, CommandBuilder, cmd};

// --- Test support (hidden) ---

#[doc(hidden)]
pub use connection::{run_client_handshake, HandshakeResult};
#[doc(hidden)]
pub use routing::{RouteTable, RouteResult, RouteCandidate};
#[doc(hidden)]
pub use scripting::{
    AttrValue, DynOptic, OpKind, PropertyInfo, Resolution, ScriptOp,
    ScriptQuery, ScriptReply, ScriptResponse, ScriptableHandler, Specifier,
    SpecifierForm, ValueType,
};
