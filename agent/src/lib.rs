//! Hostnur agent core library.
//!
//! The foundation scaffold for the privileged Hostnur agent. This crate is
//! intentionally independent of the Laravel panel: it builds, tests and runs
//! without PHP, Node or any host mutation. Phase 10 provides a thin, testable
//! process foundation (configuration, diagnostics, lifecycle and logging);
//! transport and privileged operation handlers arrive in phase 11 and later.
//!
//! Nothing here binds a socket, provisions users, writes server configuration or
//! starts hosted applications. The default `run` path is an idle, non-root
//! process that waits for a shutdown signal.

pub mod cli;
pub mod config;
pub mod error;
pub mod logging;
pub mod runtime;

/// Agent protocol version implemented by this build. Mirrors
/// `contracts/v1/catalog.json` `protocol_versions`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Agent crate/package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
