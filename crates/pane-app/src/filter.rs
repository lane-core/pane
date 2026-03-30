use crate::event::Message;

/// What a filter decides to do with an event.
///
/// # BeOS
///
/// Replaces `filter_result` (`B_DISPATCH_MESSAGE` / `B_SKIP_MESSAGE`),
/// renamed for clarity.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction {
    /// Pass the event through (possibly modified).
    Pass(Message),
    /// Consume the event — the handler never sees it.
    Consume,
}

/// A filter that intercepts events before the handler sees them.
///
/// Filters are composable, cross-cutting concerns: key remapping,
/// logging, rate limiting, access control. They run in registration
/// order via a [`FilterChain`]. Any filter can observe, transform,
/// or consume an event.
///
/// # BeOS
///
/// `BMessageFilter`. Key changes:
/// - Trait instead of class (no separate `filter_hook` function pointer)
/// - `wants()` pre-filter replaces the `message_delivery` /
///   `message_source` enum criteria — more general, same purpose
/// - Returns [`FilterAction`] enum instead of `filter_result`
pub trait MessageFilter: Send + 'static {
    /// Process an event. Return `Pass(event)` to continue dispatch
    /// (possibly with a modified event), or `Consume` to swallow it.
    fn filter(&mut self, event: Message) -> FilterAction;

    /// Whether this filter is interested in this event type.
    /// Returns true by default (filter sees everything). Override
    /// to skip events your filter doesn't care about — a key-remap
    /// filter can return false for mouse events, etc.
    fn wants(&self, _event: &Message) -> bool {
        true
    }
}

/// An ordered chain of filters. Events pass through each filter
/// in sequence; any filter can consume the event.
pub struct FilterChain {
    filters: Vec<Box<dyn MessageFilter>>,
}

impl Default for FilterChain {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterChain {
    /// Create an empty filter chain.
    pub fn new() -> Self {
        FilterChain { filters: Vec::new() }
    }

    /// Append a filter to the chain.
    pub fn add(&mut self, filter: impl MessageFilter) {
        self.filters.push(Box::new(filter));
    }

    /// Run the event through all filters. Returns the (possibly modified)
    /// event if it survived, or None if any filter consumed it.
    pub fn apply(&mut self, mut event: Message) -> Option<Message> {
        for filter in &mut self.filters {
            if !filter.wants(&event) {
                continue;
            }
            match filter.filter(event) {
                FilterAction::Pass(e) => event = e,
                FilterAction::Consume => return None,
            }
        }
        Some(event)
    }
}

impl std::fmt::Debug for FilterChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterChain")
            .field("filter_count", &self.filters.len())
            .finish()
    }
}
