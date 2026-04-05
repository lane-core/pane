//! Typed message filters.
//!
//! Filters are per-protocol (MessageFilter<M> where M: Message).
//! The Message trait requires Clone, which makes it not object-safe
//! (Clone requires Sized). This is by design — filters operate on
//! concrete protocol message types, not trait objects.
//!
//! The base filter chain is MessageFilter<LifecycleMessage>.
//! Per-service filter hooks are typed by their protocol's message:
//! MessageFilter<ClipboardMessage>, MessageFilter<DisplayMessage>, etc.
//!
//! Obligation handles bypass all filters.
//!
//! BeOS: BMessageFilter on BHandler/BLooper. Filters saw the raw
//! BMessage* including embedded reply ports — pane corrects this by
//! separating obligations from filterable messages.

use crate::message::Message;

/// A typed message filter for protocol message type M.
///
/// Registered via add_filter on the looper. Runs in registration
/// order. FilterHandle Drop removes.
pub trait MessageFilter<M: Message>: Send + 'static {
    /// Inspect the message and decide: pass, transform, or consume.
    fn filter(&mut self, msg: &M) -> FilterAction<M>;

    /// Pre-filter: does this filter care about this message?
    /// Default: yes. Override for efficiency (skip the filter
    /// for messages it won't touch).
    fn matches(&self, msg: &M) -> bool { let _ = msg; true }
}

/// What the filter decides.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction<M> {
    /// Pass the message through unchanged.
    Pass,
    /// Replace the message with a transformed version.
    /// Use case: shortcut filter transforms Key → CommandExecuted.
    Transform(M),
    /// Consume the message — handler never sees it.
    Consume,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::lifecycle::LifecycleMessage;

    struct ConsumeDisconnected;

    impl MessageFilter<LifecycleMessage> for ConsumeDisconnected {
        fn filter(&mut self, msg: &LifecycleMessage) -> FilterAction<LifecycleMessage> {
            match msg {
                LifecycleMessage::Disconnected => FilterAction::Consume,
                _ => FilterAction::Pass,
            }
        }

        fn matches(&self, msg: &LifecycleMessage) -> bool {
            matches!(msg, LifecycleMessage::Disconnected)
        }
    }

    #[test]
    fn filter_consumes_matching_message() {
        let mut f = ConsumeDisconnected;
        assert_eq!(
            f.filter(&LifecycleMessage::Disconnected),
            FilterAction::Consume,
        );
    }

    #[test]
    fn filter_passes_non_matching_message() {
        let mut f = ConsumeDisconnected;
        assert_eq!(
            f.filter(&LifecycleMessage::Ready),
            FilterAction::Pass,
        );
    }

    #[test]
    fn filter_matches_prefilter() {
        let f = ConsumeDisconnected;
        assert!(f.matches(&LifecycleMessage::Disconnected));
        assert!(!f.matches(&LifecycleMessage::Ready));
    }
}
