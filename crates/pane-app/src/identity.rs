//! Local peer identity for remote connections.
//!
//! Populates `PeerIdentity` from the local system — username, UID,
//! hostname. Used by `connect_remote` so the server knows who is
//! connecting over TCP. Local unix connections skip this (the kernel
//! provides identity via `SO_PEERCRED`).

use pane_proto::protocol::PeerIdentity;

/// Build a `PeerIdentity` from the local system.
///
/// Falls back to reasonable defaults if env vars or syscalls fail
/// (e.g., `"unknown"` for username, `"localhost"` for hostname).
pub(crate) fn local_identity() -> PeerIdentity {
    PeerIdentity {
        username: username(),
        // SAFETY: getuid has no preconditions and always succeeds.
        uid: unsafe { libc::getuid() },
        hostname: hostname(),
    }
}

fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: buf is a valid [u8; 256] on the stack; len matches its size.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return "localhost".into();
    }
    // Find the null terminator
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..len]).into_owned()
}
