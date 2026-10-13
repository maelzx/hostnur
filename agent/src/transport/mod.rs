//! Local Unix-domain transport boundary.
//!
//! Phase 11.2: endpoint ownership (no symlink substitution, restrictive mode),
//! kernel-verified peer UID allowlisting and bounded framed reads with
//! timeouts, all ordered before dispatch. Operation semantics are added later.

pub mod endpoint;
pub mod peer;
pub mod server;

use std::fmt;

pub use crate::contract::FrameError;
pub use endpoint::Listener;
pub use peer::{Peer, peer_credentials};
pub use server::{
    ConnectionOutcome, NoopHandler, RequestHandler, ServerContext, handle_connection, serve,
};

/// Failures of the socket/peer boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Io(String),
    /// The configured socket path is a symlink; a substituted socket is refused.
    PathIsSymlink,
    /// A non-socket file already exists at the socket path.
    PathOccupied,
    /// Binding, permission or non-blocking configuration failed.
    Bind(String),
    /// Peer credentials could not be obtained via `SO_PEERCRED`.
    PeerCredentials(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Io(message) => write!(f, "io: {message}"),
            TransportError::PathIsSymlink => f.write_str("socket path is a symlink"),
            TransportError::PathOccupied => f.write_str("socket path is occupied by a non-socket"),
            TransportError::Bind(message) => write!(f, "bind: {message}"),
            TransportError::PeerCredentials(message) => {
                write!(f, "peer credentials: {message}")
            }
        }
    }
}

impl std::error::Error for TransportError {}
