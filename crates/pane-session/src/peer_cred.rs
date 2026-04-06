//! Peer credential extraction from unix domain sockets.
//!
//! Derives PeerAuth from the transport's kernel credentials.
//! Platform-specific: Linux uses SO_PEERCRED, macOS uses
//! getpeereid + LOCAL_PEERPID.

use std::os::unix::net::UnixStream;
use std::os::unix::io::AsRawFd;
use pane_proto::peer_auth::{PeerAuth, AuthSource};

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
    let fd = stream.as_raw_fd();

    #[cfg(target_os = "linux")]
    {
        peer_cred_linux(fd)
    }

    #[cfg(target_os = "macos")]
    {
        peer_cred_macos(fd)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "peer credentials not supported on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
fn peer_cred_linux(fd: i32) -> std::io::Result<PeerAuth> {
    // SO_PEERCRED: returns ucred { pid, uid, gid }
    #[repr(C)]
    struct UCred {
        pid: i32,
        uid: u32,
        gid: u32,
    }

    let mut cred: UCred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<UCred>() as u32;

    // SOL_SOCKET = 1, SO_PEERCRED = 17 on Linux
    let ret = unsafe {
        libc_getsockopt(fd, 1, 17, &mut cred as *mut _ as *mut _, &mut len)
    };

    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(PeerAuth::new(cred.uid, AuthSource::Kernel { pid: cred.pid as u32 }))
}

extern "C" {
    fn getsockopt(
        fd: i32,
        level: i32,
        optname: i32,
        optval: *mut std::ffi::c_void,
        optlen: *mut u32,
    ) -> i32;
}

#[cfg(target_os = "linux")]
use getsockopt as libc_getsockopt;

#[cfg(target_os = "macos")]
fn peer_cred_macos(fd: i32) -> std::io::Result<PeerAuth> {
    // getpeereid: uid + gid
    extern "C" {
        fn getpeereid(fd: i32, uid: *mut u32, gid: *mut u32) -> i32;
    }

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
    let ret = unsafe {
        getsockopt(fd, 0, 2, &mut pid as *mut _ as *mut _, &mut len)
    };
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

        // Should be our own uid and pid
        let my_uid = unsafe { libc_getuid() };
        let my_pid = std::process::id();

        assert_eq!(auth.uid, my_uid);
        // For a socketpair, peer pid is our own pid
        if let AuthSource::Kernel { pid } = auth.source {
            assert_eq!(pid, my_pid);
        } else {
            panic!("expected Kernel auth source");
        }
    }

    extern "C" {
        #[link_name = "getuid"]
        fn libc_getuid() -> u32;
    }
}
