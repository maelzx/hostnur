//! Unix socket endpoint ownership and creation.
//!
//! Binding is defensive: a symlink or non-socket already at the path is refused
//! rather than followed or overwritten, and a stale socket owned by the agent is
//! replaced. The socket is created with a restrictive mode. The parent directory
//! is expected to be provisioned by the service (root-owned `/run/hostnur`); the
//! agent does not create or chmod privileged directories.

use std::fs::Permissions;
use std::io;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use super::TransportError;

/// A bound Unix socket listener that removes its socket on drop.
#[derive(Debug)]
pub struct Listener {
    inner: UnixListener,
    path: PathBuf,
}

impl Listener {
    /// Bind `path` with the given permission bits.
    pub fn bind(path: &Path, mode: u32) -> Result<Listener, TransportError> {
        if path.as_os_str().is_empty() {
            return Err(TransportError::Bind("empty socket path".to_string()));
        }

        match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    return Err(TransportError::PathIsSymlink);
                }
                if !file_type.is_socket() {
                    return Err(TransportError::PathOccupied);
                }
                // A leftover socket from a previous run: remove and rebind.
                std::fs::remove_file(path)
                    .map_err(|err| TransportError::Bind(format!("removing stale socket: {err}")))?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(TransportError::Bind(format!(
                    "inspecting socket path: {err}"
                )));
            }
        }

        let inner = UnixListener::bind(path)
            .map_err(|err| TransportError::Bind(format!("binding socket: {err}")))?;

        std::fs::set_permissions(path, Permissions::from_mode(mode))
            .map_err(|err| TransportError::Bind(format!("setting socket mode: {err}")))?;

        Ok(Listener {
            inner,
            path: path.to_path_buf(),
        })
    }

    /// Accept the next connection, blocking until one arrives.
    pub fn accept(&self) -> io::Result<(UnixStream, std::os::unix::net::SocketAddr)> {
        self.inner.accept()
    }

    /// Switch the listener to non-blocking acceptance.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current permission bits of the bound socket (tests and diagnostics).
    pub fn mode(&self) -> io::Result<u32> {
        let metadata = std::fs::symlink_metadata(&self.path)?;
        Ok(metadata.permissions().mode() & 0o777)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        // Only remove the path if it is still the socket we created.
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
