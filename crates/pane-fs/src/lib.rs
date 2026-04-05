//! Filesystem namespace for pane.
//!
//! Projects pane state into `/pane/` for scripting and inspection.
//! Each pane appears as a directory with structured entries:
//!
//!   /pane/<id>/tag          title text
//!   /pane/<id>/body         content (semantic, not rendered)
//!   /pane/<id>/attrs/<name> named attributes via optics
//!   /pane/<id>/ctl          line-oriented command interface
//!
//! Attributes are read through the optic layer — each Attribute<S,A>
//! is a lens from handler state to a value. Reads clone the handler
//! state snapshot; writes produce a new state that the looper applies.
//!
//! Design heritage: Plan 9 /proc (read-only lens onto process state),
//! rio's per-window synthetic files, BeOS hey scripting.

pub mod attrs;
pub mod namespace;
