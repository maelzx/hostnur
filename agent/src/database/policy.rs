//! MariaDB engine policy: identifier derivation, protected names and credential
//! generation (phase 18.1).
//!
//! Database/user identifiers are derived from managed resource UUIDs, so no
//! caller-supplied name is ever interpolated into DDL. Generated credentials are
//! hex only (no quoting characters) and bounded.

use crate::contract::Uuid;

/// Managed database-object prefixes.
pub const DB_PREFIX: &str = "hn_db_";
pub const USER_PREFIX: &str = "hn_u_";

/// MariaDB identifier limits.
pub const MAX_DATABASE_NAME: usize = 64;
pub const MAX_USERNAME: usize = 80;

/// Credential length in bytes (hex-encoded, so 2× characters).
pub const PASSWORD_BYTES: usize = 24;

/// System/protected database names that must never be targeted.
const PROTECTED: [&str; 4] = ["mysql", "information_schema", "performance_schema", "sys"];

/// The single supported engine for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    MariaDb,
}

impl Engine {
    pub const fn as_str(self) -> &'static str {
        match self {
            Engine::MariaDb => "mariadb",
        }
    }

    pub fn parse(value: &str) -> Option<Engine> {
        match value {
            "mariadb" => Some(Engine::MariaDb),
            _ => None,
        }
    }
}

/// Derive the managed database name for a resource UUID.
pub fn database_name(resource_id: &Uuid) -> String {
    format!("{DB_PREFIX}{}", fragment(resource_id))
}

/// Derive the managed principal (user) name for a resource UUID.
pub fn username(resource_id: &Uuid) -> String {
    format!("{USER_PREFIX}{}", fragment(resource_id))
}

fn fragment(resource_id: &Uuid) -> String {
    resource_id
        .as_str()
        .chars()
        .filter(|c| *c != '-')
        .take(12)
        .collect()
}

/// Whether a name is a managed database name (prefix + hex fragment).
pub fn is_managed_database(name: &str) -> bool {
    name.strip_prefix(DB_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Whether a name is a managed principal name.
pub fn is_managed_username(name: &str) -> bool {
    name.strip_prefix(USER_PREFIX)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Whether a database name is a protected system schema.
pub fn is_protected(name: &str) -> bool {
    PROTECTED
        .iter()
        .any(|protected| name.eq_ignore_ascii_case(protected))
}

/// Generate a bounded, hex-only credential from the OS CSPRNG.
pub fn generate_password() -> Result<String, String> {
    let mut bytes = [0u8; PASSWORD_BYTES];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> Uuid {
        Uuid::parse("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee").unwrap()
    }

    #[test]
    fn derives_bounded_identifiers() {
        assert_eq!(database_name(&id()), "hn_db_aaaaaaaabbbb");
        assert_eq!(username(&id()), "hn_u_aaaaaaaabbbb");
        assert!(is_managed_database(&database_name(&id())));
        assert!(is_managed_username(&username(&id())));
        assert!(database_name(&id()).len() <= MAX_DATABASE_NAME);
        assert!(username(&id()).len() <= MAX_USERNAME);
    }

    #[test]
    fn rejects_unmanaged_and_protected() {
        assert!(!is_managed_database("mysql"));
        assert!(!is_managed_database("hn_db_zzzz"));
        assert!(!is_managed_username("root"));
        assert!(is_protected("mysql"));
        assert!(is_protected("information_schema"));
        assert!(!is_protected("hn_db_aaaaaaaabbbb"));
    }

    #[test]
    fn password_is_hex_bounded() {
        let password = generate_password().unwrap();
        assert_eq!(password.len(), PASSWORD_BYTES * 2);
        assert!(password.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(password, generate_password().unwrap());
    }
}
