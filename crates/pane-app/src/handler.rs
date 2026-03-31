use std::any::Any;

use pane_proto::event::{KeyEvent, MouseEvent};
use pane_proto::message::PaneId;
use pane_proto::protocol::PaneGeometry;

use crate::error::Result;
use crate::event::Message;
use crate::exit::ExitReason;
use crate::proxy::Messenger;

/// Trait for structured pane event handling.
///
/// Implement the methods you care about; all have defaults that
/// continue the event loop. `close_requested` defaults to accepting
/// the close (returning `Ok(false)` to stop the loop).
///
/// Each method corresponds to one [`Message`] variant.
/// The looper dispatches to the matching method automatically, so you
/// never need to write a top-level match yourself — that's what
/// [`Pane::run`](crate::Pane::run)'s closure form is for.
///
/// # Return Convention
///
/// All methods return `Result<bool>`:
/// - `Ok(true)` — continue the event loop
/// - `Ok(false)` — exit cleanly (the compositor is notified)
/// - `Err(e)` — exit with an error
///
/// The default for most methods is `Ok(true)` (keep going).
/// The exception is [`close_requested`](Handler::close_requested),
/// which defaults to `Ok(false)` (accept the close).
///
/// # BeOS
///
/// Descends from `BHandler` (see also Haiku's
/// [BHandler documentation](reference/haiku-book/app/BHandler.dox)).
/// Key changes:
/// - Per-event methods replace single `MessageReceived(BMessage*)`
///   dispatch — exhaustive: every event reaches a method or the
///   `fallback` catch-all
/// - Return value controls the event loop (Be used side-effects
///   like `PostMessage(B_QUIT_REQUESTED)`)
/// - Trait with default methods replaces virtual class hierarchy
///
/// # BeOS Divergences
///
/// **No handler chain.** Be's `SetNextHandler`/`NextHandler` delegated
/// events through a handler chain within a looper. pane has one handler
/// per pane — event delegation uses the [`fallback`](Handler::fallback)
/// method instead. This is intentional: the handler chain was a
/// workaround for C++ single-dispatch that Rust's trait system replaces.
///
/// **No observer pattern on Handler.** Be's `StartWatching`/`SendNotices`
/// built pub/sub into every handler. pane uses filesystem attributes for
/// observable state: write to `/pane/{id}/attr/{property}`, watch with
/// `pane-notify`. This gives persistence, crash recovery, scriptability,
/// and solves the initial-value problem that Be's approach didn't.
///
/// **Runtime filter mutation via Messenger.** Be allowed adding/removing
/// filters at runtime via `AddFilter`/`RemoveFilter` under the looper
/// lock, which had a latent self-modification-during-iteration bug.
/// pane defers filter mutation via the looper's message channel:
/// [`Messenger::add_filter`](crate::Messenger) and
/// [`Messenger::remove_filter`](crate::Messenger). Mutations take
/// effect before the next event batch, not mid-batch.
pub trait Handler: Send + 'static {
    /// Called once when the pane is ready and its initial geometry is known.
    ///
    /// This is the first event your handler sees. Use it for one-time
    /// setup that depends on geometry: initializing a rendering buffer,
    /// starting a pulse timer, loading initial content.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    fn ready(&mut self, _proxy: &Messenger, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane was resized.
    fn resized(&mut self, _proxy: &Messenger, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane gained input focus.
    ///
    /// Override to update visual state (cursor style, selection
    /// highlight) or start input capture.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::WindowActivated` with `active == true`. pane splits
    /// the `bool active` parameter into separate `activated` /
    /// `deactivated` hooks.
    fn activated(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// The pane lost input focus.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::WindowActivated` with `active == false`.
    fn deactivated(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// Keyboard input.
    fn key(&mut self, _proxy: &Messenger, _event: KeyEvent) -> Result<bool> {
        Ok(true)
    }

    /// Mouse input.
    fn mouse(&mut self, _proxy: &Messenger, _event: MouseEvent) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was activated.
    fn command_activated(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was dismissed.
    fn command_dismissed(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// A command was executed from the command surface.
    fn command_executed(&mut self, _proxy: &Messenger, _command: &str, _args: &str) -> Result<bool> {
        Ok(true)
    }

    /// The compositor requests completions for the command input.
    ///
    /// Reply via the [`CompletionReplyPort`](crate::CompletionReplyPort).
    /// If you drop the reply port without calling `.reply()`, an empty
    /// completion list is sent to the compositor (the command surface
    /// shows "no completions" rather than hanging).
    fn completion_request(&mut self, _proxy: &Messenger, _input: &str, _reply: crate::CompletionReplyPort) -> Result<bool> {
        Ok(true)
    }

    /// Periodic pulse. Override for animations, status polling, clock ticks.
    ///
    /// Delivered at the interval set by [`Messenger::set_pulse_rate`].
    /// Not delivered until you set a rate.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::Pulse`.
    fn pulse(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// The compositor requests this pane to close.
    ///
    /// Override to implement save-before-close, confirmation dialogs,
    /// or to refuse the close. Return `Ok(false)` to accept (stop
    /// the loop) or `Ok(true)` to reject and keep running.
    ///
    /// Default: accepts the close (`Ok(false)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::QuitRequested` — same semantics but the return
    /// convention is inverted: Be returned `true` to accept,
    /// pane returns `Ok(false)` to stop the loop.
    fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }

    /// A clipboard write lock was granted.
    ///
    /// Use the lock to write data and commit. Drop without commit
    /// automatically reverts.
    ///
    /// Default: reverts (drops the lock without writing).
    fn clipboard_lock_granted(
        &mut self,
        _proxy: &Messenger,
        _lock: crate::clipboard::ClipboardWriteLock,
    ) -> Result<bool> {
        Ok(true)
    }

    /// A clipboard write lock was denied.
    ///
    /// Default: continues the event loop.
    fn clipboard_lock_denied(
        &mut self,
        _proxy: &Messenger,
        _clipboard: &str,
        _reason: &str,
    ) -> Result<bool> {
        Ok(true)
    }

    /// A watched clipboard changed.
    ///
    /// Default: continues the event loop.
    fn clipboard_changed(
        &mut self,
        _proxy: &Messenger,
        _clipboard: &str,
        _source: pane_proto::message::PaneId,
    ) -> Result<bool> {
        Ok(true)
    }

    /// The application is requesting to quit.
    ///
    /// Called by [`App::request_quit`](crate::App::request_quit) on all
    /// panes. Return `true` to allow the quit or `false` to veto.
    /// If any pane vetoes, no panes are closed.
    ///
    /// The signature is `&self` (not `&mut self`, not `&Messenger`) —
    /// this is load-bearing for deadlock freedom. The handler can
    /// inspect its state ("do I have unsaved data?") but cannot send
    /// messages or mutate state. If user interaction is needed (save
    /// dialog), veto the quit and handle it internally.
    ///
    /// Default: allows quit (`true`).
    ///
    /// # BeOS
    ///
    /// `BApplication::QuitRequested` → `BWindow::QuitRequested`. Be
    /// gave full access (the handler could lock other windows, send
    /// messages), leading to the unlock-during-iteration and stale-
    /// pointer complexity. pane's `&self` constraint eliminates this.
    fn quit_requested(&self) -> bool {
        true
    }

    /// The connection to the compositor was lost.
    fn disconnected(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }

    /// A monitored pane exited. Default: continue.
    fn pane_exited(&mut self, _proxy: &Messenger, _pane: PaneId, _reason: ExitReason) -> Result<bool> {
        Ok(true)
    }

    /// Handle an application-defined message.
    ///
    /// Worker threads post these via [`Messenger::post_app_message`](crate::Messenger::post_app_message).
    /// Downcast to your application types:
    ///
    /// ```ignore
    /// fn app_message(&mut self, proxy: &Messenger, msg: Box<dyn Any + Send>) -> Result<bool> {
    ///     if let Some(result) = msg.downcast_ref::<DownloadResult>() {
    ///         self.handle_download(proxy, result)?;
    ///     }
    ///     Ok(true)
    /// }
    /// ```
    ///
    /// For compile-time exhaustiveness within your app, use a private
    /// enum as the payload type — one downcast, then pattern match:
    ///
    /// ```ignore
    /// enum WorkerResult { Download(PathBuf), Parse(Data) }
    /// // Worker: proxy.post_app_message(WorkerResult::Download(path))
    /// // Handler: downcast to WorkerResult, then match
    /// ```
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BHandler::MessageReceived` for app-defined `what` codes.
    fn app_message(&mut self, _proxy: &Messenger, _msg: Box<dyn Any + Send>) -> Result<bool> {
        Ok(true)
    }

    /// A reply to a request sent via [`Messenger::send_request`].
    ///
    /// The `token` matches the one returned by `send_request`. Downcast
    /// the payload to your expected reply type.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// Equivalent to receiving a reply `BMessage` via async `SendMessage`
    /// with a reply handler target.
    fn reply_received(&mut self, _proxy: &Messenger, _token: u64, _payload: Box<dyn Any + Send>) -> Result<bool> {
        Ok(true)
    }

    /// A pending request failed — the target dropped its [`ReplyPort`](crate::ReplyPort)
    /// without replying, or the target pane exited.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    fn reply_failed(&mut self, _proxy: &Messenger, _token: u64) -> Result<bool> {
        Ok(true)
    }

    /// A request was received that expects a reply. Call
    /// [`ReplyPort::reply`](crate::ReplyPort::reply) to respond.
    ///
    /// If you drop the `reply_port` without calling `.reply()`, the
    /// requestor receives `Message::ReplyFailed` (same contract as
    /// Be's `BMessage` destructor sending `B_NO_REPLY`).
    ///
    /// Default: drops the reply port (sends ReplyFailed), continues.
    fn request_received(
        &mut self,
        _proxy: &Messenger,
        _msg: Message,
        _reply_port: crate::ReplyPort,
    ) -> Result<bool> {
        Ok(true)
    }

    /// Available scripting properties. Default: none.
    ///
    /// Override in handlers that implement [`ScriptableHandler`](crate::ScriptableHandler)
    /// to return the full property list. This method exists on Handler
    /// (not just ScriptableHandler) so that every handler is at least
    /// minimally introspectable — an empty result is a clean answer,
    /// not a type error at the dispatch layer.
    ///
    /// # BeOS
    ///
    /// `BHandler::GetSupportedSuites` — every handler responded, even
    /// if only with the base `suite/vnd.Be-handler` suite.
    fn supported_properties(&self) -> &[crate::scripting::PropertyInfo] {
        &[]
    }

    /// Catch-all for events not handled by other methods.
    fn fallback(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }
}
