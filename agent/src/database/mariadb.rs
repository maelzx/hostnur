//! MariaDB provisioning and grants via the fixed `mysql` client (phase 18.2).
//!
//! The admin connection uses local `root` socket authentication owned by the
//! agent. Application principals are created only for a specific database and
//! get no global privileges, `GRANT OPTION`, or filesystem/program access.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::process::{CommandSpec, run};

use super::policy::{self, Engine};

/// Database failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseError {
    /// A name/id failed the safety policy.
    Policy(String),
    /// The engine tool failed.
    Tool(String),
    /// An unmanaged resource blocked a safe operation.
    Conflict(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::Policy(message) => write!(f, "database policy: {message}"),
            DatabaseError::Tool(message) => write!(f, "database tool: {message}"),
            DatabaseError::Conflict(message) => write!(f, "database conflict: {message}"),
        }
    }
}

impl std::error::Error for DatabaseError {}

/// Fixed MariaDB administration client.
#[derive(Debug, Clone)]
pub struct MariaDbAdmin {
    pub engine: Engine,
    pub mysql: PathBuf,
    pub host: String,
    pub port: u16,
    pub deadline: Duration,
}

impl Default for MariaDbAdmin {
    fn default() -> Self {
        Self {
            engine: Engine::MariaDb,
            mysql: PathBuf::from("/usr/bin/mysql"),
            host: "127.0.0.1".to_string(),
            port: 3306,
            deadline: Duration::from_secs(60),
        }
    }
}

impl MariaDbAdmin {
    /// Run SQL as the agent-owned local admin (`root` via unix socket).
    fn admin_sql(&self, sql: &str) -> Result<String, DatabaseError> {
        let mut spec = CommandSpec::new(&self.mysql, self.deadline);
        spec.args = vec![
            "--protocol=socket".to_string(),
            "-uroot".to_string(),
            "--batch".to_string(),
            "--skip-column-names".to_string(),
            "-e".to_string(),
            sql.to_string(),
        ];
        spec.env = tool_env();
        spec.output_cap_bytes = 65_536;
        let outcome = run(&spec).map_err(|error| DatabaseError::Tool(error.to_string()))?;
        if !outcome.succeeded() {
            return Err(DatabaseError::Tool("admin query failed".to_string()));
        }
        Ok(String::from_utf8_lossy(&outcome.stdout).into_owned())
    }

    pub fn server_version(&self) -> Result<String, DatabaseError> {
        Ok(self
            .admin_sql("SELECT VERSION();")?
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string())
    }

    pub fn database_exists(&self, db_name: &str) -> Result<bool, DatabaseError> {
        ensure_managed_database(db_name)?;
        let sql = format!(
            "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = '{db_name}';"
        );
        Ok(!self.admin_sql(&sql)?.trim().is_empty())
    }

    pub fn user_exists(&self, username: &str) -> Result<bool, DatabaseError> {
        ensure_managed_user(username)?;
        let sql = format!("SELECT User FROM mysql.user WHERE User = '{username}';");
        Ok(!self.admin_sql(&sql)?.trim().is_empty())
    }

    /// Create the managed database and principal, granting only that database.
    pub fn provision(
        &self,
        db_name: &str,
        username: &str,
        password: &str,
    ) -> Result<(), DatabaseError> {
        ensure_managed_database(db_name)?;
        ensure_managed_user(username)?;
        ensure_password(password)?;

        let sql = format!(
            "CREATE DATABASE IF NOT EXISTS `{db_name}`; \
             CREATE USER IF NOT EXISTS '{username}'@'localhost' IDENTIFIED BY '{password}'; \
             CREATE USER IF NOT EXISTS '{username}'@'127.0.0.1' IDENTIFIED BY '{password}'; \
             GRANT ALL PRIVILEGES ON `{db_name}`.* TO '{username}'@'localhost'; \
             GRANT ALL PRIVILEGES ON `{db_name}`.* TO '{username}'@'127.0.0.1';"
        );
        self.admin_sql(&sql)?;
        Ok(())
    }

    /// Real connection check as the application principal over TCP.
    pub fn check_connection(
        &self,
        db_name: &str,
        username: &str,
        password: &str,
    ) -> Result<(), DatabaseError> {
        ensure_managed_database(db_name)?;
        ensure_managed_user(username)?;

        let mut spec = CommandSpec::new(&self.mysql, self.deadline);
        spec.args = vec![
            "-h".to_string(),
            self.host.clone(),
            "-P".to_string(),
            self.port.to_string(),
            "-u".to_string(),
            username.to_string(),
            db_name.to_string(),
            "--batch".to_string(),
            "--skip-column-names".to_string(),
            "-e".to_string(),
            "SELECT 1;".to_string(),
        ];
        let mut env = tool_env();
        // Password via env, never argv.
        env.insert("MYSQL_PWD".to_string(), password.to_string());
        spec.env = env;
        spec.output_cap_bytes = 4096;
        let outcome = run(&spec).map_err(|error| DatabaseError::Tool(error.to_string()))?;
        if !outcome.succeeded() {
            return Err(DatabaseError::Tool(
                "application connection check failed".to_string(),
            ));
        }
        Ok(())
    }

    pub fn grants(&self, username: &str) -> Result<Vec<String>, DatabaseError> {
        ensure_managed_user(username)?;
        let sql = format!("SHOW GRANTS FOR '{username}'@'localhost';");
        Ok(self
            .admin_sql(&sql)?
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect())
    }

    /// Drop the managed database and principal (explicit destructive operation).
    pub fn remove(&self, db_name: &str, username: &str) -> Result<(), DatabaseError> {
        ensure_managed_database(db_name)?;
        ensure_managed_user(username)?;
        if policy::is_protected(db_name) {
            return Err(DatabaseError::Conflict(
                "refusing to drop a protected database".to_string(),
            ));
        }
        let sql = format!(
            "DROP DATABASE IF EXISTS `{db_name}`; \
             DROP USER IF EXISTS '{username}'@'localhost'; \
             DROP USER IF EXISTS '{username}'@'127.0.0.1';"
        );
        self.admin_sql(&sql)?;
        Ok(())
    }
}

fn tool_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        "/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
    );
    env.insert("LC_ALL".to_string(), "C".to_string());
    env
}

fn ensure_managed_database(db_name: &str) -> Result<(), DatabaseError> {
    if policy::is_protected(db_name) {
        return Err(DatabaseError::Policy(format!(
            "`{db_name}` is a protected schema"
        )));
    }
    if !policy::is_managed_database(db_name) {
        return Err(DatabaseError::Policy(format!(
            "`{db_name}` is not a managed database name"
        )));
    }
    Ok(())
}

fn ensure_managed_user(username: &str) -> Result<(), DatabaseError> {
    if !policy::is_managed_username(username) {
        return Err(DatabaseError::Policy(format!(
            "`{username}` is not a managed principal"
        )));
    }
    Ok(())
}

fn ensure_password(password: &str) -> Result<(), DatabaseError> {
    if password.len() != policy::PASSWORD_BYTES * 2
        || !password.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(DatabaseError::Policy(
            "credential must be bounded hex".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_unmanaged_names_without_touching_engine() {
        let admin = MariaDbAdmin::default();
        assert!(matches!(
            admin.provision("mysql", "hn_u_aaaaaaaabbbb", &"a".repeat(48)),
            Err(DatabaseError::Policy(_))
        ));
        assert!(matches!(
            admin.database_exists("evil"),
            Err(DatabaseError::Policy(_))
        ));
        assert!(matches!(
            admin.grants("root"),
            Err(DatabaseError::Policy(_))
        ));
        assert!(matches!(
            admin.remove("hn_db_aaaaaaaabbbb", "root"),
            Err(DatabaseError::Policy(_))
        ));
    }

    #[test]
    fn refuses_bad_credentials() {
        let admin = MariaDbAdmin::default();
        // not hex / wrong length
        assert!(matches!(
            admin.provision("hn_db_aaaaaaaabbbb", "hn_u_aaaaaaaabbbb", "short"),
            Err(DatabaseError::Policy(_))
        ));
    }
}
