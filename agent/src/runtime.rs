//! Process lifecycle: startup, signal handling and controlled shutdown.
//!
//! `run` binds the local socket, installs SIGTERM/SIGINT handlers and serves
//! bounded connections until shutdown. The wait loop is factored out so
//! lifecycle behaviour can be tested without launching a daemon or sending real
//! signals. No privileged operation handlers are enabled in this build.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;

use crate::VERSION;
use crate::config::AgentConfig;
use crate::contract::Catalog;
use crate::dispatch::AgentHandler;
use crate::error::{AgentError, ErrorCode};
use crate::journal::Journal;
use crate::logging::{self, Level};
use crate::transport::{self, Listener, ServerContext};

/// Poll interval for the shutdown flag.
pub const TICK: Duration = Duration::from_millis(100);

/// Install SIGTERM/SIGINT handlers that set `shutdown` when a signal arrives.
///
/// Returns the shared flag. The flag starts `false` and is only ever set by the
/// signal handlers, so a clean `run` without a signal waits indefinitely.
pub fn install_shutdown_signals() -> Result<Arc<AtomicBool>, AgentError> {
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(|err| lifecycle("registering SIGTERM handler", &err))?;
    flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(|err| lifecycle("registering SIGINT handler", &err))?;
    Ok(shutdown)
}

/// Wait until `shutdown` is set, or until `max` elapses when given.
///
/// Returns `true` when a shutdown was requested, `false` on timeout.
pub fn wait_for_shutdown(shutdown: &AtomicBool, max: Option<Duration>) -> bool {
    let started = Instant::now();
    loop {
        if shutdown.load(Ordering::Relaxed) {
            return true;
        }
        if let Some(max) = max
            && started.elapsed() >= max
        {
            return false;
        }
        std::thread::sleep(TICK);
    }
}

/// Run the agent process until a shutdown signal is received.
///
/// Binds the local socket (refusing symlink substitution, restrictive mode),
/// then serves peer-checked, framed, bounded connections. Operation handlers are
/// still placeholders, so a valid request is accepted and closed without a
/// mutation.
pub fn run(config: &AgentConfig) -> Result<(), AgentError> {
    logging::init(config.log_level);
    logging::log(
        Level::Info,
        "startup",
        "agent.starting",
        &format!("hostnur-agent {VERSION} starting"),
        None,
        None,
    );

    let shutdown = install_shutdown_signals()?;

    std::fs::create_dir_all(&config.state_dir).map_err(|err| {
        AgentError::new(
            ErrorCode::Journal,
            format!("creating state directory: {}", err.kind()),
        )
    })?;
    let journal = Journal::open(&config.state_dir.join("agent.db"))
        .map_err(|err| AgentError::new(ErrorCode::Journal, format!("opening journal: {err}")))?;

    let listener = Listener::bind(&config.socket_path, config.socket_mode)
        .map_err(|err| AgentError::new(ErrorCode::Transport, format!("binding socket: {err}")))?;

    logging::log(
        Level::Info,
        "startup",
        "agent.ready",
        "agent listening; read-only operations only in this build",
        None,
        None,
    );

    let catalog = Catalog::embedded();
    let handler = AgentHandler::new(&journal);
    let context = ServerContext {
        catalog: &catalog,
        handler: &handler,
        max_message_bytes: config.max_message_bytes,
        read_timeout: Duration::from_secs(config.connection_read_timeout_seconds),
        allowed_peer_uids: &config.allowed_peer_uids,
    };
    transport::serve(&listener, &context, &shutdown)
        .map_err(|err| AgentError::new(ErrorCode::Transport, format!("serving: {err}")))?;

    logging::log(
        Level::Info,
        "shutdown",
        "agent.stopped",
        "hostnur-agent stopped",
        None,
        None,
    );
    Ok(())
}

fn lifecycle(context: &str, err: &std::io::Error) -> AgentError {
    AgentError::new(ErrorCode::Lifecycle, format!("{context}: {}", err.kind()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn wait_returns_immediately_when_already_shutting_down() {
        let flag = AtomicBool::new(true);
        let started = Instant::now();
        assert!(wait_for_shutdown(&flag, Some(Duration::from_secs(5))));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn wait_honours_timeout_without_shutdown() {
        let flag = AtomicBool::new(false);
        let started = Instant::now();
        assert!(!wait_for_shutdown(&flag, Some(Duration::from_millis(150))));
        assert!(started.elapsed() >= Duration::from_millis(100));
    }

    #[test]
    fn wait_observes_flag_set_by_another_thread() {
        let flag = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            setter.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        assert!(wait_for_shutdown(&flag, Some(Duration::from_secs(5))));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn installs_signal_handlers() {
        let flag = install_shutdown_signals().expect("signal handlers install");
        assert!(!flag.load(Ordering::Relaxed));
    }
}
