//! Kernel-reported peer identity for Unix domain socket connections.
//!
//! The agent must never trust a JSON `user_id`, PID claim or group membership.
//! Linux `SO_PEERCRED` returns the credentials of the process that connected,
//! which the kernel sets and the peer cannot forge. We check the UID against an
//! explicit allowlist configured by the operator.

use std::os::unix::net::UnixStream;

use nix::sys::socket::getsockopt;
use nix::sys::socket::sockopt::PeerCredentials;

use super::TransportError;

/// Effective kernel-reported identity of the connecting process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peer {
    pub uid: u32,
    pub gid: u32,
    pub pid: i32,
}

impl Peer {
    /// Whether this peer UID is explicitly allowed. An empty allowlist denies
    /// everyone (fail closed).
    pub fn is_allowed(&self, allowed_uids: &[u32]) -> bool {
        allowed_uids.contains(&self.uid)
    }
}

/// Read the connecting process's credentials via `SO_PEERCRED`.
pub fn peer_credentials(stream: &UnixStream) -> Result<Peer, TransportError> {
    let creds = getsockopt(stream, PeerCredentials)
        .map_err(|err| TransportError::PeerCredentials(err.to_string()))?;
    Ok(Peer {
        uid: creds.uid(),
        gid: creds.gid(),
        pid: creds.pid(),
    })
}
