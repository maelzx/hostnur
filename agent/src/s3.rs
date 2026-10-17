//! S3-compatible backup destination (phase 19.3/19.6).
//!
//! Minimal AWS Signature V4 client for `PUT Object` against S3-compatible object
//! storage (AWS S3, Cloudflare R2, MinIO, …). Only the two headers SigV4 needs
//! are signed; the payload hash is sent explicitly so the request can be verified
//! end-to-end. No long-lived credentials are logged.

use std::os::unix::fs::PermissionsExt;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// S3-compatible destination settings.
#[derive(Debug, Clone)]
pub struct S3Config {
    /// Base endpoint, e.g. `https://s3.amazonaws.com` or `https://<acct>.r2.cloudflarestorage.com`.
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// Key prefix within the bucket (`hostnur/backups` recommended).
    pub prefix: String,
    /// Use path-style addressing (`endpoint/bucket/key`); MinIO needs this.
    pub path_style: bool,
}

impl S3Config {
    pub fn is_configured(&self) -> bool {
        !self.endpoint.trim().is_empty()
            && !self.bucket.trim().is_empty()
            && !self.access_key.trim().is_empty()
            && !self.secret_key.is_empty()
    }

    /// Load destination settings from a small TOML file. Returns `None` when the
    /// file is absent or incomplete, so backups simply stay local.
    pub fn load(path: &std::path::Path) -> Option<S3Config> {
        // Destination credentials must not be readable by group/other.
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return None;
        }

        let text = std::fs::read_to_string(path).ok()?;
        let value: toml::Value = toml::from_str(&text).ok()?;
        let table = value.as_table()?;
        let string = |key: &str| {
            table
                .get(key)
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };

        let region = {
            let value = string("region");
            if value.is_empty() {
                "us-east-1".to_string()
            } else {
                value
            }
        };

        let config = S3Config {
            endpoint: string("endpoint"),
            region,
            bucket: string("bucket"),
            access_key: string("access_key"),
            secret_key: string("secret_key"),
            prefix: string("prefix"),
            path_style: table
                .get("path_style")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
        };

        config.is_configured().then_some(config)
    }

    pub fn object_key(&self, name: &str) -> String {
        let prefix = self.prefix.trim_matches('/');
        if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        }
    }
}

/// A computed Authorization header and the raw signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    pub authorization: String,
    pub signature: String,
    /// Exposed for tests/debugging (contains no secrets).
    pub canonical_request: String,
    pub string_to_sign: String,
}

/// Upload `body` as `key`, returning the storage path on success.
pub fn put_object(
    cfg: &S3Config,
    key: &str,
    body: &[u8],
    now_rfc3339: &str,
) -> Result<String, String> {
    let (amzdate, datestamp) = amz_dates(now_rfc3339)?;

    let payload_hash = to_hex(&Sha256::digest(body));
    let (scheme, endpoint_host) = split_endpoint(&cfg.endpoint)?;
    let encoded = encode_path(key);

    let (url, canonical_uri, host) = if cfg.path_style {
        let path = format!("/{}/{}", cfg.bucket, encoded);
        (
            format!("{scheme}://{endpoint_host}{path}"),
            path,
            endpoint_host.clone(),
        )
    } else {
        let host = format!("{}.{}", cfg.bucket, endpoint_host);
        (
            format!("{scheme}://{host}/{encoded}"),
            format!("/{encoded}"),
            host,
        )
    };

    let headers = vec![
        ("x-amz-content-sha256".to_string(), payload_hash.clone()),
        ("x-amz-date".to_string(), amzdate.clone()),
    ];

    let signed = sign(
        "PUT",
        &canonical_uri,
        &host,
        &headers,
        &payload_hash,
        &amzdate,
        &datestamp,
        &cfg.region,
        "s3",
        &cfg.access_key,
        &cfg.secret_key,
    );

    let response = ureq::request("PUT", &url)
        .set("x-amz-content-sha256", &payload_hash)
        .set("x-amz-date", &amzdate)
        .set("Authorization", &signed.authorization)
        .send_bytes(body);

    match response {
        Ok(response) if (200..300).contains(&response.status()) => Ok(cfg.object_key(key)),
        Ok(response) => Err(format!("S3 responded with status {}", response.status())),
        Err(ureq::Error::Status(code, _)) => Err(format!("S3 responded with status {code}")),
        Err(error) => Err(format!("S3 request failed: {error}")),
    }
}

/// Compute the SigV4 Authorization header for a request.
#[allow(clippy::too_many_arguments)]
pub fn sign(
    method: &str,
    canonical_uri: &str,
    host: &str,
    headers: &[(String, String)],
    payload_hash: &str,
    amzdate: &str,
    datestamp: &str,
    region: &str,
    service: &str,
    access_key: &str,
    secret_key: &str,
) -> Signed {
    let mut all: Vec<(String, String)> = headers.to_vec();
    all.push(("host".to_string(), host.to_string()));
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical_headers: String = all.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed_headers: String = all
        .iter()
        .map(|(k, _)| k.clone())
        .collect::<Vec<_>>()
        .join(";");

    // Empty query line after the URI, and a blank line after the header block.
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    let scope = format!("{datestamp}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amzdate}\n{scope}\n{}",
        to_hex(&Sha256::digest(canonical_request.as_bytes()))
    );

    let key = signing_key(secret_key, datestamp, region, service);
    let signature = to_hex(&hmac(&key, string_to_sign.as_bytes()));

    Signed {
        authorization: format!(
            "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
        ),
        signature,
        canonical_request,
        string_to_sign,
    }
}

fn signing_key(secret: &str, datestamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), datestamp.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, service.as_bytes());
    hmac(&k_service, b"aws4_request")
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `(scheme, host)` from an endpoint URL.
fn split_endpoint(endpoint: &str) -> Result<(String, String), String> {
    let trimmed = endpoint.trim().trim_end_matches('/');
    let (scheme, rest) = trimmed
        .split_once("://")
        .ok_or_else(|| "endpoint must start with https:// or http://".to_string())?;
    if rest.is_empty() {
        return Err("endpoint is missing a host".to_string());
    }
    if rest.contains('/') {
        return Err("endpoint must be scheme://host[:port] without a path".to_string());
    }
    Ok((scheme.to_string(), rest.to_string()))
}

/// `(amz_date, date)` from an RFC3339 timestamp like `2026-10-16T14:06:26Z`.
fn amz_dates(rfc3339: &str) -> Result<(String, String), String> {
    let value = rfc3339.trim().trim_end_matches('Z');
    let (date, time) = value
        .split_once('T')
        .ok_or_else(|| "timestamp is not RFC3339".to_string())?;
    let date: String = date.chars().filter(|c| *c != '-').collect();
    let time: String = time.chars().filter(|c| *c != ':').collect();
    if date.len() != 8 || time.len() < 6 {
        return Err("timestamp is not RFC3339".to_string());
    }
    Ok((format!("{date}T{time}Z"), date))
}

/// Percent-encode a key for a canonical URI, keeping `/` unencoded.
fn encode_path(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for byte in key.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigv4_signature_matches_the_reference_for_a_put_example() {
        // Canonical inputs from the AWS "PUT object" signing example; the
        // signature is cross-checked against an independent SigV4 reference.
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let payload_hash = "44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072";
        let headers = vec![
            ("content-type".to_string(), "text/plain".to_string()),
            ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
            ("x-amz-date".to_string(), "20130524T000000Z".to_string()),
        ];

        let signed = sign(
            "PUT",
            "/test%24file.text",
            "examplebucket.s3.amazonaws.com",
            &headers,
            payload_hash,
            "20130524T000000Z",
            "20130524",
            "us-east-1",
            "s3",
            "AKIAIOSFODNN7EXAMPLE",
            secret,
        );

        assert_eq!(
            signed.signature,
            "922c40083861768d8a7028c01137570f2861fae4e51a522845070b1e33068731"
        );
    }

    #[test]
    fn sigv4_signature_matches_the_reference_for_a_get_example() {
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let payload_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let headers = vec![("x-amz-date".to_string(), "20130524T000000Z".to_string())];
        let signed = sign(
            "GET",
            "/",
            "example.amazonaws.com",
            &headers,
            payload_hash,
            "20130524T000000Z",
            "20130524",
            "us-east-1",
            "service",
            "AKIDEXAMPLE",
            secret,
        );
        assert_eq!(
            signed.signature,
            "6b52c88578c3a6e09e219aaa1b56c53f15bc2ad1a7d2bc31287ecaf19c2d2174"
        );
    }

    #[test]
    fn encodes_keys_like_s3() {
        assert_eq!(encode_path("test$file.text"), "test%24file.text");
        assert_eq!(
            encode_path("hostnur/backups/a.tar.gz.age"),
            "hostnur/backups/a.tar.gz.age"
        );
    }

    #[test]
    fn derives_amz_dates() {
        assert_eq!(
            amz_dates("2026-10-16T14:06:26Z").unwrap(),
            ("20261016T140626Z".to_string(), "20261016".to_string())
        );
    }

    #[test]
    fn builds_prefixed_object_keys() {
        let cfg = S3Config {
            endpoint: "https://s3.example.com".into(),
            region: "us-east-1".into(),
            bucket: "b".into(),
            access_key: "a".into(),
            secret_key: "s".into(),
            prefix: "/hostnur/backups/".into(),
            path_style: true,
        };
        assert_eq!(cfg.object_key("x.age"), "hostnur/backups/x.age");
        assert!(cfg.is_configured());
    }
}
