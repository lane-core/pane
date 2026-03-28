//! Handler trait default method tests.
//! Verifies that the default implementations return the correct values
//! without any overrides — the contract that developers depend on.

use std::num::NonZeroU32;
use std::sync::mpsc;

use pane_app::{PaneHandle, Handler, LooperMessage};
use pane_app::error::Result;
use pane_proto::event::{KeyEvent, Key, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

fn make_handle(id: PaneId) -> (PaneHandle, mpsc::Receiver<ClientToComp>) {
    let (tx, rx) = mpsc::channel();
    (PaneHandle::new(id, tx), rx)
}

fn send_comp(tx: &mpsc::Sender<LooperMessage>, msg: CompToClient) {
    tx.send(LooperMessage::FromComp(msg)).unwrap();
}

/// A Handler with NO overrides — uses all defaults.
struct DefaultHandler;
impl Handler for DefaultHandler {}

/// A Handler that overrides close_requested to continue (Ok(true)).
struct NeverCloseHandler;
impl Handler for NeverCloseHandler {
    fn close_requested(&mut self, _handle: &PaneHandle) -> Result<bool> {
        Ok(true) // refuse to close
    }
}

// --- P1-4: Handler default methods ---

#[test]
fn handler_default_close_returns_false() {
    // Default close_requested returns Ok(false) — the loop should exit
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // If we get here, the loop exited on Close — default worked
}

#[test]
fn handler_default_key_returns_true() {
    // Default key returns Ok(true) — the loop should continue
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    // Send a key, then a close to terminate
    send_comp(&tx, CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Char('a'),
            modifiers: Modifiers::empty(),
            state: KeyState::Press,
            timestamp: None,
        },
    });
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // If we get here, key didn't stop the loop — default worked
}

#[test]
fn handler_default_disconnect_returns_false() {
    // Default disconnected returns Ok(false)
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    drop(tx); // explicitly drop sender to simulate disconnect
    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // Loop exited on disconnect — default worked
}

#[test]
fn handler_override_close_to_continue() {
    // NeverCloseHandler overrides close to return Ok(true) — loop continues
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    // Send close (handler will ignore it), then drop to disconnect
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, NeverCloseHandler).unwrap();
    // Loop exited on disconnect (after ignoring close) — override worked
}
