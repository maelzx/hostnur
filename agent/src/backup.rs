//! Backup artifact foundation (phase 19.2).
//!
//! Captured content is sealed with **age** (X25519) to an operator-supplied
//! public key, so the decryption identity never has to live on the host. Each
//! artifact carries a small versioned manifest with the scope, sizes and an
//! integrity hash of the sealed bytes. Destination transfer (S3-compatible,
//! then SFTP) and restore orchestration build on this module.

use std::io::{Read, Write};

use serde::Serialize;
use sha2::{Digest, Sha256};

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
pub fn manifest(
    artifact_id: &str,
    scope: &str,
    recipient: &str,
    consistency: &str,
    created_at: &str,
    plaintext_bytes: usize,
    sealed: &[u8],
) -> BackupManifest {
    BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        artifact_id: artifact_id.to_string(),
        scope: scope.to_string(),
        created_at: created_at.to_string(),
        encryption: "age-x25519".to_string(),
        recipient: recipient.to_string(),
        consistency: consistency.to_string(),
        plaintext_bytes: plaintext_bytes as u64,
        sealed_bytes: sealed.len() as u64,
        sealed_sha256: seal_hash(sealed),
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
    fn manifest_records_integrity_and_sizes() {
        let sealed = vec![1u8, 2, 3, 4];
        let m = manifest(
            "11111111-1111-4111-8111-111111111111",
            "application-data",
            "age1abc",
            "quiesced",
            "2026-10-16T00:00:00Z",
            10,
            &sealed,
        );
        assert_eq!(m.format_version, BACKUP_FORMAT_VERSION);
        assert_eq!(m.sealed_bytes, 4);
        assert_eq!(m.plaintext_bytes, 10);
        assert_eq!(m.sealed_sha256, seal_hash(&sealed));
        assert_eq!(m.encryption, "age-x25519");
    }
}
