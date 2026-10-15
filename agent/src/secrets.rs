//! Root-owned, write-only secret storage for generated credentials (phase 18).
//!
//! Each secret is a single file named by resource id, mode `0600`, under a
//! root-only directory. Values are never returned by the agent, logged, or placed
//! in task payloads; the panel receives credential references, not values, and a
//! real connection check is performed as the application principal instead.

use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Store `value` at `<dir>/<id>` with directory `0700` and file `0600`.
pub fn write_secret(dir: &Path, id: &str, value: &str) -> io::Result<PathBuf> {
    write_bytes(dir, id, value.as_bytes())
}

/// Store bounded bytes at `<dir>/<id>` with directory `0700` and file `0600`.
/// Modes are applied *at creation* so there is no window where the file or
/// directory exists with umask-default (group/world-readable) permissions.
pub fn write_bytes(dir: &Path, id: &str, value: &[u8]) -> io::Result<PathBuf> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let path = dir.join(id);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(value)?;
    file.flush()?;
    // Re-assert on the (rare) path where the file pre-existed with looser modes.
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    Ok(path)
}

/// Read a stored secret (internal use only; never exposed over the transport).
pub fn read_secret(dir: &Path, id: &str) -> io::Result<String> {
    std::fs::read_to_string(dir.join(id))
}

/// Remove a stored secret.
pub fn remove_secret(dir: &Path, id: &str) -> io::Result<()> {
    match std::fs::remove_file(dir.join(id)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn writes_and_removes_private_secret() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("secrets");
        let path = write_secret(&base, "abc", "secret-value").unwrap();
        assert_eq!(
            std::fs::metadata(&base).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(read_secret(&base, "abc").unwrap(), "secret-value");
        remove_secret(&base, "abc").unwrap();
        assert!(!path.exists());
        // Removing an absent secret is fine.
        remove_secret(&base, "abc").unwrap();
    }
}
