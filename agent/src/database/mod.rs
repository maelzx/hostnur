//! Application database provisioning (phase 18), MariaDB first.

pub mod mariadb;
pub mod policy;

pub use mariadb::{DatabaseError, MariaDbAdmin};
pub use policy::{
    Engine, database_name, generate_password, is_managed_database, is_managed_username,
    is_protected, username,
};
