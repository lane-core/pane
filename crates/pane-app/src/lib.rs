//! Application kit for the pane desktop environment.
//!
//! The developer's primary interface for building pane-native applications.
//! Equivalent to BeOS's Application Kit — BApplication, BLooper, BHandler,
//! BMessenger translated to Rust with session types underneath.
//!
//! # Quick Start
//!
//! ```ignore
//! use pane_app::{App, Tag, cmd, BuiltIn};
//!
//! fn main() -> pane_app::Result<()> {
//!     let app = App::connect("com.example.hello")?;
//!     let pane = app.create_pane(
//!         Tag::new("Hello").commands(vec![
//!             cmd("close", "Close this pane")
//!                 .shortcut("Alt+W")
//!                 .built_in(BuiltIn::Close),
//!         ]),
//!     )?;
//!     pane.run(|_proxy, event| match event {
//!         pane_app::PaneEvent::Key(key) if key.is_escape() => Ok(false),
//!         pane_app::PaneEvent::Close => Ok(false),
//!         _ => Ok(true),
//!     })
//! }
//! ```

pub mod app;
pub(crate) mod connection;
pub mod error;
pub mod event;
pub mod filter;
pub mod handler;
pub mod looper;
pub mod mock; // pub for integration tests
pub mod pane;
pub mod proxy;
pub mod routing;
pub mod scripting;
pub mod tag;

pub use app::App;
pub use error::{Error, ConnectError, PaneError, Result};
pub use event::PaneEvent;
pub use filter::{Filter, FilterAction};
pub use handler::Handler;
pub use pane::Pane;
pub use proxy::PaneHandle;
pub use routing::{RouteTable, RouteResult, RouteCandidate};
pub use scripting::{PropertyDecl, ScriptQuery, ScriptOp, ScriptReplyToken};
pub use tag::{Tag, CommandBuilder, cmd};
pub use pane_proto::tag::BuiltIn;
