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
    spec.args = vec![
        "-C".to_string(),
        source_dir.display().to_string(),
        "-czf".to_string(),
        archive_path.display().to_string(),
        ".".to_string(),
    ];
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

/// Safely restore a sealed archive into `dest` using the operator identity.
///
/// Rejects absolute paths and `..` traversal, skips symlinks/hardlinks/device
/// nodes, and never extracts outside `dest`.
pub fn restore_archive(sealed_path: &Path, identity: &str, dest: &Path) -> Result<usize, String> {
    let sealed = std::fs::read(sealed_path).map_err(|error| error.to_string())?;
    let plaintext = open(identity, &sealed)?;

    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(plaintext));
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(false);
    archive.set_preserve_mtime(false);
    archive.set_overwrite(true);

    std::fs::create_dir_all(dest).map_err(|error| error.to_string())?;

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

        entry.unpack_in(dest).map_err(|error| error.to_string())?;
        count += 1;

        if count > 100_000 {
            return Err("archive has too many entries".to_string());
        }
    }

    Ok(count)
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
