//! Backup artifact foundation (phase 19.2).
//!
//! Captured content is sealed with **age** (X25519) to an operator-supplied
//! public key, so the decryption identity never has to live on the host. Each
//! artifact carries a small versioned manifest with the scope, sizes and an
//! integrity hash of the sealed bytes. Destination transfer (S3-compatible,
//! then SFTP) and restore orchestration build on this module.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::process::{CommandSpec, run};

/// Manifest/artifact format version understood by this build.
pub const BACKUP_FORMAT_VERSION: u32 = 1;

/// A backup artifact's metadata. The sealed artifact is the encrypted content;
/// the manifest is written alongside it (not secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupManifest {
    pub format_version: u32,
    pub artifact_id: String,
    pub scope: String,
    pub created_at: String,
    pub encryption: String,
    pub recipient: String,
    pub consistency: String,
    pub plaintext_bytes: u64,
    pub sealed_bytes: u64,
    pub sealed_sha256: String,
}

/// Seal `plaintext` with age to the given `age1...` recipient.
pub fn seal(recipient: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let parsed = recipient
        .parse::<age::x25519::Recipient>()
        .map_err(|_| "backup recipient is not a valid age public key".to_string())?;

    let encryptor = age::Encryptor::with_recipients(vec![Box::new(parsed)])
        .ok_or_else(|| "no backup recipient was provided".to_string())?;

    let mut sealed = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut sealed)
        .map_err(|error| error.to_string())?;
    writer
        .write_all(plaintext)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;

    Ok(sealed)
}

/// SHA-256 of the sealed artifact, hex encoded (integrity evidence).
pub fn seal_hash(sealed: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sealed);
    format!("{:x}", hasher.finalize())
}

/// Build a manifest describing a sealed artifact.
#[allow(clippy::too_many_arguments)]
pub fn manifest(
    artifact_id: &str,
    scope: &str,
    recipient: &str,
    consistency: &str,
    created_at: &str,
    plaintext_bytes: u64,
    sealed_bytes: u64,
    sealed_sha256: &str,
) -> BackupManifest {
    BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        artifact_id: artifact_id.to_string(),
        scope: scope.to_string(),
        created_at: created_at.to_string(),
        encryption: "age-x25519".to_string(),
        recipient: recipient.to_string(),
        consistency: consistency.to_string(),
        plaintext_bytes,
        sealed_bytes,
        sealed_sha256: sealed_sha256.to_string(),
    }
}

/// Open an age-sealed artifact with a raw X25519 identity string
/// (`AGE-SECRET-KEY-1...`). Used by restore; kept here so encryption and
/// decryption share one format.
pub fn open(identity: &str, sealed: &[u8]) -> Result<Vec<u8>, String> {
    let identity = identity
        .parse::<age::x25519::Identity>()
        .map_err(|_| "backup identity is not a valid age secret key".to_string())?;

    let decryptor = match age::Decryptor::new(sealed).map_err(|error| error.to_string())? {
        age::Decryptor::Recipients(decryptor) => decryptor,
        _ => return Err("unexpected age artifact".to_string()),
    };
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|error| error.to_string())?;

    let mut plaintext = Vec::new();
    reader
        .read_to_end(&mut plaintext)
        .map_err(|error| error.to_string())?;

    Ok(plaintext)
}

/// Create a gzip tar archive of `source_dir` at `archive_path` using the system
/// `tar` (bounded process; creating an archive, never extracting as root).
pub fn archive_directory(source_dir: &Path, archive_path: &Path) -> Result<(), String> {
    if !source_dir.is_dir() {
        return Err("backup source directory does not exist".to_string());
    }
    archive_paths(&[(source_dir, ".")], archive_path)
}

/// Create a gzip tar archive holding each `(base, relative)` member, stored
/// under `relative` (e.g. `(/opt/hostnur, "apps/<id>/shared")`).
pub fn archive_paths(members: &[(&Path, &str)], archive_path: &Path) -> Result<(), String> {
    if members.is_empty() {
        return Err("nothing to archive".to_string());
    }

    // Pre-create the (still plaintext) archive with owner-only permissions so
    // there is no world-readable window before sealing. `tar -f` truncates the
    // existing file and keeps its mode.
    if let Err(error) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(archive_path)
    {
        return Err(error.to_string());
    }
    let _ = std::fs::set_permissions(archive_path, std::fs::Permissions::from_mode(0o600));

    let mut spec = CommandSpec::new(Path::new("/usr/bin/tar"), Duration::from_secs(1800));
    spec.args = vec!["-czf".to_string(), archive_path.display().to_string()];
    for (base, relative) in members {
        spec.args.push("-C".to_string());
        spec.args.push(base.display().to_string());
        spec.args.push((*relative).to_string());
    }
    spec.output_cap_bytes = 8_192;
    let mut env = BTreeMap::new();
    env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    env.insert("LC_ALL".to_string(), "C".to_string());
    spec.env = env;

    let outcome = run(&spec).map_err(|error| error.to_string())?;
    if outcome.succeeded() {
        let _ = std::fs::set_permissions(archive_path, std::fs::Permissions::from_mode(0o600));
        Ok(())
    } else {
        let _ = std::fs::remove_file(archive_path);
        Err(format!("tar exited with code {:?}", outcome.exit_code))
    }
}

/// Write a transactionally consistent copy of the SQLite database at `source`
/// to `dest` (`VACUUM INTO`, read-only on the source), mode 0600.
pub fn snapshot_sqlite(source: &Path, dest: &Path) -> Result<(), String> {
    let connection = rusqlite::Connection::open_with_flags(
        source,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| error.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(10))
        .map_err(|error| error.to_string())?;
    connection
        .execute("VACUUM INTO ?1", [dest.display().to_string()])
        .map_err(|error| error.to_string())?;
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| error.to_string())
}

/// Copy the tree at `source` to `dest` (which must not exist), keeping
/// permission bits. Symlinks and special files are skipped, never followed.
pub fn copy_tree(source: &Path, dest: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    let mode = metadata.permissions().mode() & 0o777;
    if metadata.is_dir() {
        std::fs::create_dir(dest).map_err(|error| error.to_string())?;
        for entry in std::fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        std::fs::copy(source, dest).map_err(|error| error.to_string())?;
    } else {
        return Ok(());
    }
    std::fs::set_permissions(dest, std::fs::Permissions::from_mode(mode))
        .map_err(|error| error.to_string())
}

/// Stream `input` through age into `output_path`, returning the sealed byte
/// count and its SHA-256. Content is never buffered in memory wholesale.
pub fn seal_file(
    input: &Path,
    output_path: &Path,
    recipient: &str,
) -> Result<(u64, String), String> {
    let parsed = recipient
        .parse::<age::x25519::Recipient>()
        .map_err(|_| "backup recipient is not a valid age public key".to_string())?;
    let encryptor = age::Encryptor::with_recipients(vec![Box::new(parsed)])
        .ok_or_else(|| "no backup recipient was provided".to_string())?;

    let mut reader = std::fs::File::open(input).map_err(|error| error.to_string())?;
    let output = std::fs::File::create(output_path).map_err(|error| error.to_string())?;
    let mut writer = encryptor
        .wrap_output(output)
        .map_err(|error| error.to_string())?;
    std::io::copy(&mut reader, &mut writer).map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;

    // The sealed artifact's on-disk size (age adds a header/stream overhead).
    let sealed_bytes = std::fs::metadata(output_path)
        .map_err(|error| error.to_string())?
        .len();
    let hash = file_sha256(output_path)?;
    Ok((sealed_bytes, hash))
}

/// SHA-256 of a file, read in bounded chunks.
pub fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Write a manifest as pretty JSON next to the artifact.
pub fn write_manifest(dir: &Path, manifest: &BackupManifest) -> Result<PathBuf, String> {
    let path = dir.join(format!("{}.manifest.json", manifest.artifact_id));
    let json = serde_json::to_string_pretty(manifest).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| error.to_string())?;
    Ok(path)
}

/// Upper bound on entries unpacked from one archive.
const MAX_RESTORE_ENTRIES: usize = 100_000;

/// Decrypt and unpack a sealed archive into `staging`, which must not exist
/// yet and is created root-only (0700). Streams file → age → gzip → tar, so
/// the archive is never held in memory.
///
/// Rejects absolute paths and `..` traversal and skips symlinks, hardlinks and
/// device nodes. Because `staging` is new and contains no links, nothing can
/// redirect a write outside it. File modes keep their permission bits (so
/// executables stay executable) but never setuid/setgid/sticky, and the owner
/// always keeps access.
pub fn restore_archive(
    sealed_path: &Path,
    identity: &str,
    staging: &Path,
) -> Result<usize, String> {
    use std::os::unix::fs::DirBuilderExt;

    let identity = identity
        .parse::<age::x25519::Identity>()
        .map_err(|_| "backup identity is not a valid age secret key".to_string())?;
    let sealed = std::fs::File::open(sealed_path).map_err(|error| error.to_string())?;
    let decryptor = match age::Decryptor::new(std::io::BufReader::new(sealed))
        .map_err(|error| error.to_string())?
    {
        age::Decryptor::Recipients(decryptor) => decryptor,
        _ => return Err("unexpected age artifact".to_string()),
    };
    let plaintext = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|error| error.to_string())?;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(plaintext));
    archive.set_preserve_permissions(false);
    archive.set_preserve_mtime(false);
    archive.set_overwrite(true);

    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(staging)
        .map_err(|error| format!("creating restore staging: {error}"))?;

    let mut count = 0usize;

    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let path = entry
            .path()
            .map_err(|error| error.to_string())?
            .into_owned();

        if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err("archive contains an unsafe path".to_string());
        }

        let kind = entry.header().entry_type();
        if kind.is_symlink()
            || kind.is_hard_link()
            || kind.is_block_special()
            || kind.is_character_special()
            || kind.is_fifo()
        {
            continue;
        }

        let header_mode = entry.header().mode().unwrap_or(0o644);
        if !entry
            .unpack_in(staging)
            .map_err(|error| error.to_string())?
        {
            continue;
        }

        let unpacked = staging.join(&path);
        let owner_access = if kind.is_dir() { 0o700 } else { 0o600 };
        if unpacked != staging {
            std::fs::set_permissions(
                &unpacked,
                std::fs::Permissions::from_mode((header_mode & 0o777) | owner_access),
            )
            .map_err(|error| error.to_string())?;
        }

        count += 1;
        if count > MAX_RESTORE_ENTRIES {
            return Err("archive has too many entries".to_string());
        }
    }

    Ok(count)
}

/// `lchown` every entry under `root` (and `root` itself) to `uid`/`gid`.
/// Links are never followed.
pub fn chown_tree(root: &Path, uid: u32, gid: u32) -> Result<(), String> {
    std::os::unix::fs::lchown(root, Some(uid), Some(gid)).map_err(|error| error.to_string())?;
    let metadata = std::fs::symlink_metadata(root).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            chown_tree(&entry.path(), uid, gid)?;
        }
    }
    Ok(())
}

/// The outcome of restoring an application's `shared/` tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedRestore {
    pub entries: usize,
    pub target: PathBuf,
    /// Where the replaced `shared/` tree was kept, if one existed.
    pub previous: Option<PathBuf>,
}

/// Restore an application's `shared/` tree from a sealed archive without ever
/// writing into the live, application-writable tree.
///
/// The archive is unpacked into a fresh root-only staging directory beside
/// `shared/` (inside the root-owned application directory, out of the app
/// account's reach), given the managed layout's modes and the application's
/// ownership, and swapped in by rename. The replaced tree is kept as
/// `.shared-before-<backup>` for the operator to inspect or delete; a failed
/// swap puts it back.
pub fn restore_shared(
    sealed_path: &Path,
    identity: &str,
    app_dir: &Path,
    backup_id: &str,
    owner: (u32, u32),
) -> Result<SharedRestore, String> {
    use std::os::unix::fs::DirBuilderExt;

    let target = app_dir.join("shared");
    let staging = app_dir.join(format!(".restore-{backup_id}"));
    let previous = app_dir.join(format!(".shared-before-{backup_id}"));

    // A failed earlier attempt may have left staging behind; it is root-owned
    // and inside the root-owned application directory, so removing it is safe.
    remove_if_present(&staging)?;

    let staged = (|| -> Result<usize, String> {
        let entries = restore_archive(sealed_path, identity, &staging)?;
        let secrets = staging.join("secrets");
        if !secrets.exists() {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&secrets)
                .map_err(|error| error.to_string())?;
        }
        std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o750))
            .map_err(|error| error.to_string())?;
        chown_tree(&staging, owner.0, owner.1)?;
        Ok(entries)
    })();
    let entries = match staged {
        Ok(entries) => entries,
        Err(message) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(message);
        }
    };

    remove_if_present(&previous)?;
    let had_previous = std::fs::symlink_metadata(&target).is_ok();
    if had_previous {
        std::fs::rename(&target, &previous).map_err(|error| {
            let _ = std::fs::remove_dir_all(&staging);
            format!("moving the current shared tree aside: {error}")
        })?;
    }
    if let Err(error) = std::fs::rename(&staging, &target) {
        if had_previous {
            let _ = std::fs::rename(&previous, &target);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("activating the restored shared tree: {error}"));
    }

    Ok(SharedRestore {
        entries,
        target,
        previous: had_previous.then_some(previous),
    })
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
        Ok(metadata) if metadata.is_dir() => {
            std::fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => std::fs::remove_file(path).map_err(|error| error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::ExposeSecret;

    #[test]
    fn seals_and_opens_round_trip() {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();

        let plaintext = b"panel control state";
        let sealed = seal(&recipient.to_string(), plaintext).unwrap();
        assert_ne!(&sealed, plaintext);

        let secret = identity.to_string();
        let opened = open(secret.expose_secret(), &sealed).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn rejects_invalid_recipient() {
        assert!(seal("not-a-key", b"x").is_err());
    }

    #[test]
    fn archives_seals_and_writes_a_manifest() {
        let work = tempfile::tempdir().unwrap();
        let source = work.path().join("shared");
        std::fs::create_dir_all(source.join("storage/app")).unwrap();
        std::fs::write(source.join("storage/app/file.txt"), b"hello").unwrap();

        let archive = work.path().join("data.tar.gz");
        archive_directory(&source, &archive).unwrap();
        let archive_len = std::fs::metadata(&archive).unwrap().len();

        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public().to_string();
        let sealed = work.path().join("data.tar.gz.age");
        let (sealed_bytes, hash) = seal_file(&archive, &sealed, &recipient).unwrap();
        assert!(sealed_bytes > 0);
        assert_eq!(hash, file_sha256(&sealed).unwrap());

        let m = manifest(
            "11111111-1111-4111-8111-111111111111",
            "application-data",
            &recipient,
            "quiesced",
            "2026-10-16T00:00:00Z",
            archive_len,
            sealed_bytes,
            &hash,
        );
        let manifest_path = write_manifest(work.path(), &m).unwrap();
        assert!(manifest_path.exists());

        // The archive is recoverable with the matching identity.
        let identity_str = identity.to_string();
        let opened = open(
            identity_str.expose_secret(),
            &std::fs::read(&sealed).unwrap(),
        )
        .unwrap();
        assert!(!opened.is_empty());
    }

    #[test]
    fn restores_a_sealed_archive() {
        let work = tempfile::tempdir().unwrap();
        let source = work.path().join("shared");
        std::fs::create_dir_all(source.join("storage/app")).unwrap();
        std::fs::write(source.join("storage/app/file.txt"), b"payload").unwrap();

        let archive = work.path().join("a.tar.gz");
        archive_directory(&source, &archive).unwrap();

        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public().to_string();
        let sealed = work.path().join("a.tar.gz.age");
        seal_file(&archive, &sealed, &recipient).unwrap();

        let dest = work.path().join("restored");
        let secret = identity.to_string();
        let entries = restore_archive(&sealed, secret.expose_secret(), &dest).unwrap();

        assert!(entries >= 1);
        assert_eq!(
            std::fs::read_to_string(dest.join("storage/app/file.txt")).unwrap(),
            "payload"
        );
    }

    /// Seal a gzip tar built from `(path, mode, contents)` entries.
    fn sealed_archive(dir: &Path, entries: &[(&str, u32, &[u8])]) -> (PathBuf, String) {
        let archive = dir.join("built.tar.gz");
        {
            let file = std::fs::File::create(&archive).unwrap();
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
            let mut builder = tar::Builder::new(encoder);
            for (path, mode, contents) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_mode(*mode);
                header.set_cksum();
                builder.append_data(&mut header, path, *contents).unwrap();
            }
            builder.into_inner().unwrap().finish().unwrap();
        }
        let identity = age::x25519::Identity::generate();
        let sealed = dir.join("built.tar.gz.age");
        seal_file(&archive, &sealed, &identity.to_public().to_string()).unwrap();
        (sealed, identity.to_string().expose_secret().to_string())
    }

    fn me() -> (u32, u32) {
        (
            nix::unistd::getuid().as_raw(),
            nix::unistd::getgid().as_raw(),
        )
    }

    fn mode(path: &Path) -> u32 {
        std::fs::symlink_metadata(path)
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    }

    #[test]
    fn restore_refuses_an_existing_staging_directory() {
        let work = tempfile::tempdir().unwrap();
        let (sealed, identity) = sealed_archive(work.path(), &[("a.txt", 0o644, b"a")]);
        let staging = work.path().join("staging");
        std::fs::create_dir(&staging).unwrap();
        assert!(restore_archive(&sealed, &identity, &staging).is_err());
    }

    #[test]
    fn restore_keeps_permission_bits_but_never_setuid() {
        let work = tempfile::tempdir().unwrap();
        let (sealed, identity) = sealed_archive(
            work.path(),
            &[
                ("bin/run", 0o4755, b"#!/bin/sh"),
                ("notes.txt", 0o400, b"n"),
            ],
        );
        let staging = work.path().join("staging");
        restore_archive(&sealed, &identity, &staging).unwrap();

        assert_eq!(mode(&staging), 0o700);
        assert_eq!(mode(&staging.join("bin/run")), 0o755);
        // The owner always keeps read/write access.
        assert_eq!(mode(&staging.join("notes.txt")), 0o600);
    }

    #[test]
    fn restore_shared_never_writes_through_a_planted_symlink() {
        let work = tempfile::tempdir().unwrap();
        let app_dir = work.path().join("app");
        let outside = work.path().join("outside");
        std::fs::create_dir_all(app_dir.join("shared")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        // The app account plants a link in its writable tree.
        std::os::unix::fs::symlink(&outside, app_dir.join("shared/uploads")).unwrap();

        let (sealed, identity) = sealed_archive(
            work.path(),
            &[
                ("uploads/new.txt", 0o644, b"restored"),
                ("secrets/app.key", 0o600, b"k"),
            ],
        );
        let restored = restore_shared(&sealed, &identity, &app_dir, "b1", me()).unwrap();

        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
        let shared = app_dir.join("shared");
        assert!(
            std::fs::symlink_metadata(shared.join("uploads"))
                .unwrap()
                .is_dir()
        );
        assert_eq!(
            std::fs::read_to_string(shared.join("uploads/new.txt")).unwrap(),
            "restored"
        );
        assert_eq!(mode(&shared), 0o750);
        assert_eq!(mode(&shared.join("secrets")), 0o700);
        assert_eq!(restored.entries, 2);

        // The replaced tree (with the planted link) is kept aside, untouched.
        let previous = restored.previous.unwrap();
        assert!(
            std::fs::symlink_metadata(previous.join("uploads"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!app_dir.join(".restore-b1").exists());
    }

    #[test]
    fn failed_restore_leaves_the_live_tree_untouched() {
        let work = tempfile::tempdir().unwrap();
        let app_dir = work.path().join("app");
        std::fs::create_dir_all(app_dir.join("shared")).unwrap();
        std::fs::write(app_dir.join("shared/live.txt"), b"live").unwrap();
        let (sealed, _) = sealed_archive(work.path(), &[("a.txt", 0o644, b"a")]);
        let wrong = age::x25519::Identity::generate();

        let result = restore_shared(
            &sealed,
            wrong.to_string().expose_secret(),
            &app_dir,
            "b2",
            me(),
        );

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(app_dir.join("shared/live.txt")).unwrap(),
            "live"
        );
        assert!(!app_dir.join(".restore-b2").exists());
        assert!(!app_dir.join(".shared-before-b2").exists());
    }

    #[test]
    fn manifest_records_integrity_and_sizes() {
        let sealed = vec![1u8, 2, 3, 4];
        let m = manifest(
            "11111111-1111-4111-8111-111111111111",
            "application-data",
            "age1abc",
            "quiesced",
            "2026-10-16T00:00:00Z",
            10,
            4,
            &seal_hash(&sealed),
        );
        assert_eq!(m.format_version, BACKUP_FORMAT_VERSION);
        assert_eq!(m.sealed_bytes, 4);
        assert_eq!(m.plaintext_bytes, 10);
        assert_eq!(m.sealed_sha256, seal_hash(&sealed));
        assert_eq!(m.encryption, "age-x25519");
    }
}
