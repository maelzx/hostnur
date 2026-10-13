//! Scoped, transient HTTPS credentials for private Git (phase 13.3).
//!
//! A token is delivered to Git through a short-lived `GIT_ASKPASS` helper owned
//! by the application user, never through the URL, argv or a persisted global
//! config. The credential directory is removed when the value is dropped. The
//! token is never formatted, logged or included in error messages.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nix::unistd::{Gid, Uid, chown};

/// Credential-delivery failures.
#[derive(Debug)]
pub enum CredentialError {
    Io(String),
    Random,
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialError::Io(message) => write!(f, "credential io: {message}"),
            CredentialError::Random => f.write_str("could not obtain randomness"),
        }
    }
}

impl std::error::Error for CredentialError {}

/// A transient credential directory that cleans itself up on drop.
pub struct CredentialFile {
    dir: PathBuf,
}

impl CredentialFile {
    /// Write an askpass helper and token file owned by `uid`/`gid`.
    pub fn create(token: &str, uid: u32, gid: u32) -> Result<CredentialFile, CredentialError> {
        let mut suffix = [0u8; 8];
        getrandom::fill(&mut suffix).map_err(|_| CredentialError::Random)?;
        let name = suffix
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let dir = std::env::temp_dir().join(format!("hostnur-cred-{name}"));

        fs::create_dir(&dir).map_err(io)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(io)?;
        chown(&dir, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid))).map_err(nix_io)?;

        let token_path = dir.join("token");
        fs::write(&token_path, token.as_bytes()).map_err(io)?;
        fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).map_err(io)?;
        chown(
            &token_path,
            Some(Uid::from_raw(uid)),
            Some(Gid::from_raw(gid)),
        )
        .map_err(nix_io)?;

        // The token value is read from the file by the helper; it never appears
        // in the script text.
        let askpass = dir.join("askpass.sh");
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\n  *[Uu]sername*) printf '%s\\n' x-access-token ;;\n  *) cat {} ;;\nesac\n",
            shell_quote(&token_path)
        );
        fs::write(&askpass, script).map_err(io)?;
        fs::set_permissions(&askpass, fs::Permissions::from_mode(0o700)).map_err(io)?;
        chown(&askpass, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid))).map_err(nix_io)?;

        Ok(CredentialFile { dir })
    }

    /// Path to pass as `GIT_ASKPASS`.
    pub fn askpass(&self) -> PathBuf {
        self.dir.join("askpass.sh")
    }

    pub fn directory(&self) -> &Path {
        &self.dir
    }
}

impl Drop for CredentialFile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn io(error: std::io::Error) -> CredentialError {
    CredentialError::Io(error.to_string())
}

fn nix_io(error: nix::errno::Errno) -> CredentialError {
    CredentialError::Io(error.to_string())
}

fn shell_quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("'{}'", raw.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    fn current() -> (u32, u32) {
        (
            nix::unistd::getuid().as_raw(),
            nix::unistd::getgid().as_raw(),
        )
    }

    #[test]
    fn writes_private_files_and_cleans_up() {
        let (uid, gid) = current();
        let dir;
        {
            let credential = CredentialFile::create("s3cr3t-token", uid, gid).unwrap();
            dir = credential.directory().to_path_buf();

            assert_eq!(std::fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
            let token = std::fs::read(dir.join("token")).unwrap();
            assert_eq!(token, b"s3cr3t-token");
            assert_eq!(
                std::fs::metadata(dir.join("token")).unwrap().mode() & 0o777,
                0o600
            );

            let script = std::fs::read_to_string(credential.askpass()).unwrap();
            // The helper reads the token from a file; the value is not in the script.
            assert!(!script.contains("s3cr3t-token"));
            assert!(script.contains("x-access-token"));
        }
        assert!(
            !dir.exists(),
            "credential directory must be removed on drop"
        );
    }
}
