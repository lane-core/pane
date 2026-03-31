use crate::event::Message;

/// What a filter decides to do with an event.
///
/// # BeOS
///
/// Replaces `filter_result` (`B_DISPATCH_MESSAGE` / `B_SKIP_MESSAGE`),
/// renamed for clarity.
#[derive(Debug)]
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
/// - `matches()` pre-filter replaces the `message_delivery` /
///   `message_source` enum criteria — more general, same purpose
/// - Returns [`FilterAction`] enum instead of `filter_result`
/// - No retargeting (`SetTarget`/`Target`). Be's retargeting redirected
///   a filtered message to a different handler within the same looper.
///   pane has one handler per pane, so retargeting is meaningless.
pub trait MessageFilter: Send + 'static {
    /// Process an event. Return `Pass(event)` to continue dispatch
    /// (possibly with a modified event), or `Consume` to swallow it.
    fn filter(&mut self, event: Message) -> FilterAction;

    /// Whether this filter is interested in this event type.
    /// Returns true by default (filter sees everything). Override
    /// to skip events your filter doesn't care about — a key-remap
    /// filter can return false for mouse events, etc.
    fn matches(&self, _event: &Message) -> bool {
        true
    }
}

/// Unique filter identifier. Returned by
/// [`Messenger::add_filter`](crate::Messenger::add_filter)
/// inside a [`FilterToken`].
pub type FilterId = u64;

/// Token for removing a runtime filter.
///
/// Created by [`Messenger::add_filter`](crate::Messenger::add_filter). Pass to
/// [`Messenger::remove_filter`](crate::Messenger::remove_filter) to uninstall the
/// filter. Analogous to [`TimerToken`](crate::TimerToken) for timers.
#[derive(Debug, Clone)]
pub struct FilterToken {
    /// The filter's unique ID.
    pub(crate) id: FilterId,
}

/// An ordered chain of filters. Events pass through each filter
/// in sequence; any filter can consume the event.
///
/// Filters are keyed by [`FilterId`] so they can be removed at
/// runtime via [`Messenger::remove_filter`](crate::Messenger).
pub struct FilterChain {
    filters: Vec<(FilterId, Box<dyn MessageFilter>)>,
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

    /// Append a filter to the chain (no ID — for static filters
    /// added before the loop starts).
    pub fn add(&mut self, filter: impl MessageFilter) {
        self.filters.push((0, Box::new(filter)));
    }

    /// Append a filter with a specific ID (for runtime mutation).
    pub(crate) fn add_with_id(&mut self, id: FilterId, filter: Box<dyn MessageFilter>) {
        self.filters.push((id, filter));
    }

    /// Remove a filter by ID. Returns true if found.
    pub(crate) fn remove_by_id(&mut self, id: FilterId) -> bool {
        let before = self.filters.len();
        self.filters.retain(|(fid, _)| *fid != id);
        self.filters.len() < before
    }

    /// Run the event through all filters. Returns the (possibly modified)
    /// event if it survived, or None if any filter consumed it.
    pub fn apply(&mut self, mut event: Message) -> Option<Message> {
        for (_, filter) in &mut self.filters {
            if !filter.matches(&event) {
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
