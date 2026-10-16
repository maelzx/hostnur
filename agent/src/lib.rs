//! Hostnur agent core library.
//!
//! The privileged Hostnur agent, intentionally independent of the Laravel
//! panel: it builds, tests and runs without PHP or Node. It serves a
//! peer-checked, length-prefixed JSON protocol on a local Unix socket and
//! executes cataloged operations (snapshot, deployment, providers, TLS,
//! databases) through the durable journal's accept→execute→receipt pipeline.
//!
//! Callers are only ever the panel process whose uid is on the configured
//! allowlist; identities, paths and unit names are derived from canonical
//! UUIDs, and every external tool runs through the bounded process primitive.

pub mod appstate;
pub mod backup;
pub mod cli;
pub mod clock;
pub mod config;
pub mod contract;
pub mod cron;
pub mod database;
pub mod deploy;
pub mod dispatch;
pub mod error;
pub mod firewall;
pub mod git;
pub mod ids;
pub mod journal;
pub mod logging;
pub mod observe;
pub mod packages;
pub mod process;
pub mod provider;
pub mod provision;
pub mod runtime;
pub mod secrets;
pub mod services;
pub mod tls;
pub mod transport;

/// Agent protocol version implemented by this build. Mirrors
/// `contracts/v1/catalog.json` `protocol_versions`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Agent crate/package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
