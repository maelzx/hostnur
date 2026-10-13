//! Git source policy: repository URL, ref and network restrictions (phase 13.1).
//!
//! Only `https` repositories are accepted by default. Loopback, link-local,
//! cloud-metadata and (unless explicitly allowed) RFC1918 destinations are
//! refused, credentials must not appear in the URL, and refs are validated
//! before any argument is constructed so option-like or revision-expression
//! tricks are rejected.

use std::net::IpAddr;
use std::str::FromStr;

/// A validated, credential-free HTTPS repository URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryUrl {
    pub host: String,
    pub port: Option<u16>,
    /// host[:port], suitable for sanitized logging.
    pub authority: String,
    /// Full URL without credentials.
    pub url: String,
}

/// A validated Git ref or commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitRef {
    /// A full 40- or 64-hex commit id (immutable once supplied).
    Commit(String),
    /// A branch or tag name (resolved at execution time).
    Name(String),
}

impl GitRef {
    pub fn as_str(&self) -> &str {
        match self {
            GitRef::Commit(value) | GitRef::Name(value) => value,
        }
    }
}

/// Source-policy failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitPolicyError {
    EmptyUrl,
    UnsupportedScheme(String),
    CredentialsInUrl,
    MissingHost,
    ForbiddenHost(String),
    UnsupportedPort(u16),
    InvalidRef(&'static str),
}

impl std::fmt::Display for GitPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitPolicyError::EmptyUrl => f.write_str("repository URL is empty"),
            GitPolicyError::UnsupportedScheme(scheme) => {
                write!(f, "repository scheme `{scheme}` is not allowed")
            }
            GitPolicyError::CredentialsInUrl => {
                f.write_str("credentials must not appear in the repository URL")
            }
            GitPolicyError::MissingHost => f.write_str("repository URL has no host"),
            GitPolicyError::ForbiddenHost(host) => {
                write!(f, "repository host `{host}` is not an allowed destination")
            }
            GitPolicyError::UnsupportedPort(port) => {
                write!(f, "repository port {port} is not allowed")
            }
            GitPolicyError::InvalidRef(reason) => write!(f, "invalid git ref: {reason}"),
        }
    }
}

impl std::error::Error for GitPolicyError {}

/// Operator network policy for outbound Git.
#[derive(Debug, Clone, Default)]
pub struct NetworkPolicy {
    /// Explicit allowlist of hosts; when non-empty, only these are allowed.
    pub allowed_hosts: Vec<String>,
    /// Permit RFC1918/private destinations (still refuses loopback/metadata).
    pub allow_private_networks: bool,
    /// Permit a non-443 port (e.g. a self-hosted Git host).
    pub allow_custom_port: bool,
}

/// Validate and normalize an HTTPS repository URL.
pub fn validate_repository_url(
    raw: &str,
    policy: &NetworkPolicy,
) -> Result<RepositoryUrl, GitPolicyError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(GitPolicyError::EmptyUrl);
    }
    if trimmed.chars().any(|c| c.is_control() || c == ' ') {
        return Err(GitPolicyError::InvalidRef(
            "url contains control characters",
        ));
    }

    let Some(rest) = trimmed.strip_prefix("https://") else {
        let scheme = trimmed.split("://").next().unwrap_or(trimmed);
        return Err(GitPolicyError::UnsupportedScheme(scheme.to_string()));
    };

    // Reject credentials (userinfo) in the authority.
    let (authority, _path) = match rest.split_once('/') {
        Some((authority, _path)) => (authority, _path),
        None => (rest, ""),
    };
    if authority.contains('@') {
        return Err(GitPolicyError::CredentialsInUrl);
    }
    if authority.is_empty() {
        return Err(GitPolicyError::MissingHost);
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => {
            let port: u16 = port.parse().map_err(|_| GitPolicyError::MissingHost)?;
            (host, Some(port))
        }
        _ => (authority, None),
    };

    if host.is_empty() {
        return Err(GitPolicyError::MissingHost);
    }
    let host = host.to_ascii_lowercase();

    if let Some(port) = port
        && port != 443
        && !policy.allow_custom_port
    {
        return Err(GitPolicyError::UnsupportedPort(port));
    }

    if !is_host_allowed(&host, policy) {
        return Err(GitPolicyError::ForbiddenHost(host));
    }

    let authority = match port {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };

    Ok(RepositoryUrl {
        host,
        port,
        authority,
        url: trimmed.to_string(),
    })
}

fn is_host_allowed(host: &str, policy: &NetworkPolicy) -> bool {
    if !policy.allowed_hosts.is_empty() {
        return policy
            .allowed_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host));
    }

    if host == "localhost" || host.ends_with(".localhost") {
        return false;
    }
    // IPv6 literals are bracketed in the authority.
    let candidate = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = IpAddr::from_str(candidate) {
        return is_ip_allowed(ip, policy.allow_private_networks);
    }
    true
}

fn is_ip_allowed(ip: IpAddr, allow_private: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Loopback, link-local, unspecified, broadcast, metadata.
            if v4.is_loopback() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast() {
                return false;
            }
            // 169.254.169.254 is covered by link-local; 100.100.100.200 (aliyun).
            if octets == [100, 100, 100, 200] {
                return false;
            }
            let private = v4.is_private();
            allow_private || !private
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() {
                return false;
            }
            let segments = v6.segments();
            // Unique-local fc00::/7 and link-local fe80::/10.
            let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
            let is_link_local = (segments[0] & 0xffc0) == 0xfe80;
            if is_link_local {
                return false;
            }
            allow_private || !is_unique_local
        }
    }
}

/// Validate a ref name or full commit id.
pub fn validate_ref(raw: &str) -> Result<GitRef, GitPolicyError> {
    // Do not trim: leading/trailing whitespace is itself invalid.
    let value = raw;
    if value.is_empty() {
        return Err(GitPolicyError::InvalidRef("empty ref"));
    }
    if value.len() > 1024 {
        return Err(GitPolicyError::InvalidRef("ref is too long"));
    }
    if value.starts_with('-') {
        return Err(GitPolicyError::InvalidRef("ref must not start with `-`"));
    }
    if value.chars().any(|c| c.is_control()) {
        return Err(GitPolicyError::InvalidRef(
            "ref contains control characters",
        ));
    }

    if (value.len() == 40 || value.len() == 64) && value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(GitRef::Commit(value.to_ascii_lowercase()));
    }

    if !is_valid_ref_name(value) {
        return Err(GitPolicyError::InvalidRef("not a valid branch or tag name"));
    }
    Ok(GitRef::Name(value.to_string()))
}

/// Approximate `git check-ref-format` rules for a single ref name.
fn is_valid_ref_name(name: &str) -> bool {
    if name == "@" || name.starts_with('/') || name.ends_with('/') || name.ends_with('.') {
        return false;
    }
    if name.contains("..") || name.contains("//") || name.contains("@{") {
        return false;
    }
    if name.contains('\\') {
        return false;
    }
    const FORBIDDEN: [char; 8] = [' ', '~', '^', ':', '?', '*', '[', '\x7f'];
    if name.chars().any(|c| FORBIDDEN.contains(&c)) {
        return false;
    }
    name.split('/').all(|component| {
        !component.is_empty() && !component.starts_with('.') && !component.ends_with(".lock")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_public_https() {
        let policy = NetworkPolicy::default();
        let repo =
            validate_repository_url("https://github.com/maelzx/hostnur.git", &policy).unwrap();
        assert_eq!(repo.host, "github.com");
        assert_eq!(repo.authority, "github.com");
        assert_eq!(repo.port, None);
    }

    #[test]
    fn refuses_non_https_schemes() {
        let policy = NetworkPolicy::default();
        for url in [
            "http://github.com/a/b.git",
            "git://github.com/a/b.git",
            "ssh://git@github.com/a/b.git",
            "file:///tmp/repo",
            "ftp://example.com/x",
        ] {
            let error = validate_repository_url(url, &policy).unwrap_err();
            assert!(
                matches!(error, GitPolicyError::UnsupportedScheme(_)),
                "{url} -> {error:?}"
            );
        }
    }

    #[test]
    fn refuses_credentials_in_url() {
        let policy = NetworkPolicy::default();
        let error =
            validate_repository_url("https://user:token@github.com/a/b.git", &policy).unwrap_err();
        assert_eq!(error, GitPolicyError::CredentialsInUrl);
    }

    #[test]
    fn refuses_loopback_link_local_and_metadata() {
        let policy = NetworkPolicy::default();
        for url in [
            "https://localhost/a/b.git",
            "https://127.0.0.1/a/b.git",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/a/b.git",
            "https://10.0.0.5/a/b.git",
            "https://192.168.1.10/a/b.git",
        ] {
            let error = validate_repository_url(url, &policy).unwrap_err();
            assert!(
                matches!(error, GitPolicyError::ForbiddenHost(_)),
                "{url} -> {error:?}"
            );
        }
    }

    #[test]
    fn explicit_allowlist_overrides_defaults() {
        let policy = NetworkPolicy {
            allowed_hosts: vec!["git.internal.example".to_string()],
            allow_private_networks: true,
            allow_custom_port: true,
        };
        let repo =
            validate_repository_url("https://git.internal.example:8443/a/b.git", &policy).unwrap();
        assert_eq!(repo.port, Some(8443));

        // A host not on the allowlist is refused even if public.
        assert!(validate_repository_url("https://github.com/a/b.git", &policy).is_err());
    }

    #[test]
    fn validates_refs() {
        assert_eq!(
            validate_ref("main").unwrap(),
            GitRef::Name("main".to_string())
        );
        assert_eq!(
            validate_ref("refs/heads/main").unwrap(),
            GitRef::Name("refs/heads/main".to_string())
        );
        assert!(matches!(validate_ref("v1.2.3").unwrap(), GitRef::Name(_)));
        assert_eq!(
            validate_ref("0f6c5f2a4c4b4f1e9c9b2f1c8a7d6e550f6c5f2a").unwrap(),
            GitRef::Commit("0f6c5f2a4c4b4f1e9c9b2f1c8a7d6e550f6c5f2a".to_string())
        );
    }

    #[test]
    fn refuses_unsafe_refs() {
        for value in [
            "",
            "-x",
            "main\n",
            "main..other",
            "main~1",
            "main^",
            "a b",
            "refs//heads",
            ".hidden",
            "branch.lock",
            "@",
        ] {
            assert!(validate_ref(value).is_err(), "{value:?} should be rejected");
        }
    }
}
