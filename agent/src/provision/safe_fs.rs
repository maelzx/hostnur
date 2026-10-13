//! Symlink-safe, beneath-root filesystem operations for the managed layout
//! (phase 12.3).
//!
//! Every write is resolved with Linux `openat2(RESOLVE_BENEATH |
//! RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)` relative to a single trusted
//! base directory descriptor. Caller-supplied strings never name a path: only
//! directories that already exist beneath the managed root can be reached, no
//! symlink is ever followed, and files are published with an atomic rename.

use std::ffi::CString;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;

use nix::sys::stat::{Mode, fchmod, mkdirat};
use nix::unistd::{Gid, Uid, fchown, unlinkat};

/// Upper bound for a single managed file write (e.g. a secret value).
pub const MAX_FILE_BYTES: usize = 65_536;

/// Observed mode/ownership of a managed entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryMeta {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}

const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;

const RESOLVE_STRICT: u64 = RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS | RESOLVE_BENEATH;

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

/// `openat2(2)` relative to `dirfd`, resolved strictly beneath it.
fn openat2(dirfd: RawFd, path: &str, flags: i32, mode: u32) -> io::Result<OwnedFd> {
    let cpath = CString::new(path)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let how = OpenHow {
        flags: flags as u64,
        mode: mode as u64,
        resolve: RESOLVE_STRICT,
    };

    // SAFETY: openat2 is a stable Linux syscall; the pointers are valid for the
    // duration of the call and the returned fd is valid on success.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dirfd,
            cpath.as_ptr(),
            &how as *const OpenHow,
            std::mem::size_of::<OpenHow>(),
        )
    };

    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd is a fresh, owned file descriptor returned by the kernel.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
}

/// A trusted base directory beneath which all managed paths are resolved.
#[derive(Debug)]
pub struct ManagedRoot {
    base: OwnedFd,
}

impl ManagedRoot {
    /// Open an existing, trusted base directory (e.g. `/opt/hostnur/apps`).
    pub fn open(path: &Path) -> io::Result<ManagedRoot> {
        let file = File::open(path)?;
        if !file.metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "managed root is not a directory",
            ));
        }
        Ok(ManagedRoot {
            base: OwnedFd::from(file),
        })
    }

    /// Read back the mode/ownership of a managed directory.
    pub fn metadata(&self, relative: &str) -> io::Result<EntryMeta> {
        let fd = self.open_dir(relative)?;
        let stat = nix::sys::stat::fstat(&fd)?;
        Ok(EntryMeta {
            mode: stat.st_mode & 0o777,
            uid: stat.st_uid,
            gid: stat.st_gid,
        })
    }

    fn open_dir(&self, relative: &str) -> io::Result<OwnedFd> {
        let path = if relative.is_empty() { "." } else { relative };
        openat2(
            self.base.as_raw_fd(),
            path,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0,
        )
    }

    /// Create a directory beneath the root (its parent must already exist).
    pub fn create_dir(
        &self,
        relative: &str,
        mode: u32,
        owner: Option<(u32, u32)>,
    ) -> io::Result<()> {
        let (parent, name) = split(relative)?;
        let parent_fd = self.open_dir(parent)?;

        match mkdirat(&parent_fd, name, Mode::from_bits_truncate(mode)) {
            Ok(()) => {}
            Err(nix::errno::Errno::EEXIST) => {}
            Err(err) => return Err(err.into()),
        }

        let dir_fd = self.open_dir(relative)?;
        if let Some((uid, gid)) = owner {
            fchown(&dir_fd, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))?;
        }
        fchmod(&dir_fd, Mode::from_bits_truncate(mode))?;
        Ok(())
    }

    /// Write a file beneath the root with an atomic rename and bounded size.
    pub fn write_file(
        &self,
        relative: &str,
        mode: u32,
        owner: Option<(u32, u32)>,
        contents: &[u8],
    ) -> io::Result<()> {
        if contents.len() > MAX_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file exceeds the managed write limit",
            ));
        }

        let (parent, name) = split(relative)?;
        let parent_fd = self.open_dir(parent)?;
        let temp_name = format!(".{name}.tmp.{}", std::process::id());

        let temp_fd = openat2(
            parent_fd.as_raw_fd(),
            &temp_name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )?;

        let result = (|| -> io::Result<()> {
            let mut file = File::from(temp_fd);
            file.write_all(contents)?;
            file.sync_all()?;
            if let Some((uid, gid)) = owner {
                fchown(&file, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))?;
            }
            fchmod(&file, Mode::from_bits_truncate(mode))?;
            Ok(())
        })();

        if let Err(error) = result {
            let _ = unlinkat(
                &parent_fd,
                temp_name.as_str(),
                nix::unistd::UnlinkatFlags::NoRemoveDir,
            );
            return Err(error);
        }

        // Atomic publish within the same trusted directory.
        nix::fcntl::renameat(&parent_fd, temp_name.as_str(), &parent_fd, name)?;
        Ok(())
    }
}

/// Split into (parent, name), rejecting empty, `.` and `..` names.
fn split(relative: &str) -> io::Result<(&str, &str)> {
    let (parent, name) = match relative.rsplit_once('/') {
        Some((parent, name)) => (parent, name),
        None => ("", relative),
    };
    if name.is_empty() || name == "." || name == ".." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid managed path name",
        ));
    }
    Ok((parent, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn creates_nested_dirs_with_modes() {
        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();

        root.create_dir("app", super::super::policy::MODE_ROOT_DIR, None)
            .unwrap();
        root.create_dir("app/releases", super::super::policy::MODE_RELEASES, None)
            .unwrap();
        root.create_dir("app/shared", super::super::policy::MODE_SHARED, None)
            .unwrap();
        root.create_dir(
            "app/shared/secrets",
            super::super::policy::MODE_SECRETS,
            None,
        )
        .unwrap();

        let mode_of = |p: &str| {
            std::fs::symlink_metadata(dir.path().join(p))
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode_of("app"), super::super::policy::MODE_ROOT_DIR);
        assert_eq!(
            mode_of("app/shared/secrets"),
            super::super::policy::MODE_SECRETS
        );
    }

    #[test]
    fn writes_private_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        root.create_dir("app", super::super::policy::MODE_ROOT_DIR, None)
            .unwrap();
        root.create_dir("app/shared", super::super::policy::MODE_SHARED, None)
            .unwrap();
        root.create_dir(
            "app/shared/secrets",
            super::super::policy::MODE_SECRETS,
            None,
        )
        .unwrap();

        root.write_file(
            "app/shared/secrets/app.key",
            super::super::policy::MODE_SECRET_FILE,
            None,
            b"base64:secret",
        )
        .unwrap();

        let path = dir.path().join("app/shared/secrets/app.key");
        assert_eq!(std::fs::read(&path).unwrap(), b"base64:secret");
        assert_eq!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            super::super::policy::MODE_SECRET_FILE
        );
    }

    #[test]
    fn refuses_to_follow_a_symlinked_component() {
        let dir = tempfile::tempdir().unwrap();
        let victim = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        symlink(victim.path(), dir.path().join("escape")).unwrap();

        let error = root
            .create_dir("escape/sub", 0o755, None)
            .expect_err("symlinked parent must be refused");
        assert_ne!(error.kind(), io::ErrorKind::NotFound);
        assert!(!victim.path().join("sub").exists());
    }

    #[test]
    fn refuses_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();

        assert!(root.create_dir("../escape", 0o755, None).is_err());
    }

    #[test]
    fn refuses_oversized_write() {
        let dir = tempfile::tempdir().unwrap();
        let root = ManagedRoot::open(dir.path()).unwrap();
        let big = vec![0u8; MAX_FILE_BYTES + 1];
        assert!(root.write_file("big", 0o600, None, &big).is_err());
    }
}
