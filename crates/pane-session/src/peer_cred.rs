//! Peer credential extraction from unix domain sockets.
//!
//! Derives PeerAuth from the transport's kernel credentials.
//! Platform-specific: Linux uses SO_PEERCRED (via rustix),
//! macOS uses getpeereid + LOCAL_PEERPID.
//!
//! Design heritage: Plan 9 factotum(4) resolved credentials to
//! a username — the AuthInfo, produced after the auth conversation
//! completed via the authinfo RPC (reference/plan9/man/4/factotum),
//! carried the resolved identity. BeOS had no equivalent —
//! identity was self-reported (team_id stuffed into AS_CREATE_APP,
//! src/kits/app/Application.cpp:1414). pane's peer_cred is
//! stronger: the kernel asserts identity, the peer cannot lie
//! (SO_PEERCRED/getpeereid are kernel-verified).

use pane_proto::peer_auth::{AuthSource, PeerAuth};
use std::os::unix::net::UnixStream;

/// Extract peer credentials from a connected unix socket.
///
/// Returns PeerAuth with uid from the kernel and pid from
/// platform-specific mechanisms.
///
/// # Errors
///
/// Returns io::Error if the credential extraction fails
/// (shouldn't happen on a connected unix socket).
pub fn peer_cred(stream: &UnixStream) -> std::io::Result<PeerAuth> {
    #[cfg(target_os = "linux")]
    {
        peer_cred_linux(stream)
    }

    #[cfg(target_os = "macos")]
    {
        peer_cred_macos(stream)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = stream;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "peer credentials not supported on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
fn peer_cred_linux(stream: &UnixStream) -> std::io::Result<PeerAuth> {
    use std::os::unix::io::AsFd;

    let cred =
        rustix::net::sockopt::socket_peercred(stream.as_fd()).map_err(std::io::Error::from)?;

    Ok(PeerAuth::new(
        cred.uid.as_raw(),
        AuthSource::Kernel {
            pid: cred.pid.as_raw_nonzero().get() as u32,
        },
    ))
}

#[cfg(target_os = "macos")]
fn peer_cred_macos(stream: &UnixStream) -> std::io::Result<PeerAuth> {
    use std::os::unix::io::AsRawFd;

    // rustix doesn't wrap getpeereid or LOCAL_PEERPID on macOS.
    extern "C" {
        fn getpeereid(fd: i32, uid: *mut u32, gid: *mut u32) -> i32;
        fn getsockopt(
            fd: i32,
            level: i32,
            optname: i32,
            optval: *mut std::ffi::c_void,
            optlen: *mut u32,
        ) -> i32;
    }

    let fd = stream.as_raw_fd();

    let mut uid: u32 = 0;
    let mut gid: u32 = 0;
    let ret = unsafe { getpeereid(fd, &mut uid, &mut gid) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // LOCAL_PEERPID: pid (macOS 10.8+)
    // SOL_LOCAL = 0, LOCAL_PEERPID = 2
    let mut pid: i32 = 0;
    let mut len = std::mem::size_of::<i32>() as u32;
    let ret = unsafe { getsockopt(fd, 0, 2, &mut pid as *mut _ as *mut _, &mut len) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(PeerAuth::new(uid, AuthSource::Kernel { pid: pid as u32 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_cred_from_socket_pair() {
        let (a, _b) = UnixStream::pair().unwrap();
        let auth = peer_cred(&a).expect("peer_cred failed");

        let my_uid = rustix::process::getuid().as_raw();
        let my_pid = std::process::id();

        assert_eq!(auth.uid, my_uid);
        if let AuthSource::Kernel { pid } = auth.source {
            assert_eq!(pid, my_pid);
        } else {
            panic!("expected Kernel auth source");
        }
    }
}
