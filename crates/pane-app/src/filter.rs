use crate::event::PaneEvent;

/// What a filter decides to do with an event.
pub enum FilterAction {
    /// Pass the event through (possibly modified).
    Pass(PaneEvent),
    /// Consume the event — the handler never sees it.
    Consume,
}

/// A message filter that intercepts events before the handler sees them.
///
/// Filters are the BMessageFilter equivalent: composable, cross-cutting
/// concerns that can observe, transform, or consume events. Common uses:
/// key remapping, logging, rate limiting, access control.
///
/// Filters run in registration order. A consumed event skips all
/// remaining filters and the handler.
pub trait Filter: Send + 'static {
    /// Process an event. Return `Pass(event)` to continue dispatch
    /// (possibly with a modified event), or `Consume` to swallow it.
    fn filter(&mut self, event: PaneEvent) -> FilterAction;
}

/// An ordered chain of filters. Events pass through each filter
/// in sequence; any filter can consume the event.
pub struct FilterChain {
    filters: Vec<Box<dyn Filter>>,
}

impl FilterChain {
    pub fn new() -> Self {
        FilterChain { filters: Vec::new() }
    }

    pub fn add(&mut self, filter: impl Filter) {
        self.filters.push(Box::new(filter));
    }

    /// Run the event through all filters. Returns the (possibly modified)
    /// event if it survived, or None if any filter consumed it.
    pub fn apply(&mut self, mut event: PaneEvent) -> Option<PaneEvent> {
        for filter in &mut self.filters {
            match filter.filter(event) {
                FilterAction::Pass(e) => event = e,
                FilterAction::Consume => return None,
            }
        }
        Some(event)
    }
}
