//! Application kit for the pane desktop environment.
//!
//! The developer's primary interface for building pane-native applications.
//! Equivalent to BeOS's Application Kit — BApplication, BLooper, BHandler,
//! BMessenger translated to Rust with session types underneath.
//!
//! # Quick Start
//!
//! ```ignore
//! use pane_app::{App, Tag, cmd, Message, KeyCombo, Builtin};
//!
//! fn main() -> pane_app::Result<()> {
//!     let app = App::connect("com.example.hello")?;
//!     let mut pane = app.create_pane(
//!         Tag::new("Hello")
//!             .command(cmd("close", "Close").shortcut("Alt+W").built_in(Builtin::Close)),
//!     )?;
//!     pane.add_shortcut(KeyCombo::new(Key::Named(NamedKey::Escape), Modifiers::empty()), "close", "");
//!     pane.run(|_messenger, msg| match msg {
//!         Message::CommandExecuted { ref command, .. } if command == "close" => Ok(false),
//!         _ => Ok(true),
//!     })
//! }
//! ```

pub mod app;
pub(crate) mod connection;
pub mod error;
pub mod event;
pub mod exit;
pub mod filter;
pub mod handler;
pub mod mock; // pub for integration tests
pub mod pane;
pub mod proxy;
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

pub use app::App;
pub use error::{Error, ConnectError, PaneError, Result};
pub use event::Message;
pub use exit::ExitReason;
pub use filter::{Filter, FilterAction};
pub use handler::Handler;
pub use pane::Pane;
pub use proxy::{Messenger, TimerToken};
pub use shortcuts::KeyCombo;
pub use tag::{Tag, CommandBuilder, cmd};

// --- Test support (hidden) ---

#[doc(hidden)]
pub use connection::{run_client_handshake, HandshakeResult};
#[doc(hidden)]
pub use routing::{RouteTable, RouteResult, RouteCandidate};
#[doc(hidden)]
pub use scripting::{Attribute, ScriptQuery, ScriptOp, ScriptReplyToken};
