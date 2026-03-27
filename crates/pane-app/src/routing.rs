//! Content routing — pattern-matched dispatch of content to handlers.
//!
//! Routing rules are TOML files in well-known directories:
//! - `/etc/pane/route/rules/` — system rules
//! - `~/.config/pane/route/rules/` — user rules (override system)
//!
//! The kit loads rules via pane-notify for live updates.
//! Drop a file, gain a behavior. Delete it, lose it.
//!
//! TODO(phase-6): implement rule loading, evaluation, and pane-roster
//! service registry queries for multi-match scenarios.

/// Result of evaluating routing rules against content.
#[derive(Debug)]
pub enum RouteResult {
    /// A single handler matched.
    Match(RouteCandidate),
    /// Multiple handlers matched — present options to the user.
    MultiMatch(Vec<RouteCandidate>),
    /// No handler matched.
    NoMatch,
}

/// A candidate handler for routed content.
#[derive(Debug, Clone)]
pub struct RouteCandidate {
    /// Application signature of the handler.
    pub signature: String,
    /// Human-readable description.
    pub description: String,
    /// Quality rating (0.0 - 1.0). Translation Kit pattern.
    pub quality: f32,
}

/// The routing table — loaded from rule files, evaluated locally.
///
/// TODO(phase-6): load rules from filesystem, watch via pane-notify,
/// query pane-roster's service registry for multi-match resolution.
pub struct RouteTable {
    _rules: Vec<()>, // placeholder
}

impl RouteTable {
    /// Create an empty route table (no rules loaded).
    pub fn new() -> Self {
        RouteTable { _rules: Vec::new() }
    }

    /// Evaluate routing rules against content.
    ///
    /// TODO(phase-6): implement rule matching, content transformation,
    /// and quality-based selection.
    pub fn route(&self, _content: &str, _content_type: &str) -> RouteResult {
        RouteResult::NoMatch
    }
}

impl Default for RouteTable {
    fn default() -> Self {
        Self::new()
    }
}
