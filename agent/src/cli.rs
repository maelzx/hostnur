//! Command-line entry point.
//!
//! Startup is deliberately thin: argument parsing and dispatch only. All
//! validation and lifecycle logic lives in `config`/`runtime` so it can be
//! tested directly.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::config::AgentConfig;
use crate::runtime;
use crate::{PROTOCOL_VERSION, VERSION};

/// Production default configuration path (see `plan.md` phase 11).
pub const DEFAULT_CONFIG_PATH: &str = "/etc/hostnur/agent.toml";

/// Environment variable overriding the default configuration path.
pub const CONFIG_ENV: &str = "HOSTNUR_AGENT_CONFIG";

const EXIT_OK: u8 = 0;
const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;

/// Run using the process arguments (skipping `argv[0]`).
pub fn run_from_env() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}

/// Run with an explicit argument list. Returns the process exit code.
pub fn run(args: &[String]) -> ExitCode {
    let Some(command) = args.first().map(String::as_str) else {
        eprintln!("error: a command is required\n\n{}", help());
        return ExitCode::from(EXIT_USAGE);
    };

    match command {
        "-h" | "--help" => {
            println!("{}", help());
            ExitCode::from(EXIT_OK)
        }
        "-V" | "--version" => {
            println!("{}", version_line());
            ExitCode::from(EXIT_OK)
        }
        "check-config" => command_check_config(&args[1..]),
        "run" => command_run(&args[1..]),
        "renew-hook" => command_renew_hook(),
        other => {
            eprintln!("error: unknown command `{other}`\n\n{}", help());
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn command_check_config(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{}", command_help());
        return ExitCode::from(EXIT_OK);
    }

    let path = match parse_config_arg(args) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("error: {message}\n\n{}", command_help());
            return ExitCode::from(EXIT_USAGE);
        }
    };

    match AgentConfig::load(&path) {
        Ok(config) => {
            println!(
                "configuration valid: {} (version {})",
                path.display(),
                config.version
            );
            ExitCode::from(EXIT_OK)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn command_run(args: &[String]) -> ExitCode {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{}", command_help());
        return ExitCode::from(EXIT_OK);
    }

    let path = match parse_config_arg(args) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("error: {message}\n\n{}", command_help());
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let config = match AgentConfig::load(&path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    match runtime::run(&config) {
        Ok(()) => ExitCode::from(EXIT_OK),
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

/// Parse `--config PATH` / `--config=PATH`, falling back to the environment
/// override and then the production default.
fn parse_config_arg(args: &[String]) -> Result<PathBuf, String> {
    let mut path: Option<PathBuf> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = arg.strip_prefix("--config=") {
            if value.is_empty() {
                return Err("`--config` requires a path".to_string());
            }
            path = Some(PathBuf::from(value));
        } else if arg == "--config" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| "`--config` requires a path".to_string())?;
            path = Some(PathBuf::from(value));
            index += 1;
        } else {
            return Err(format!("unexpected argument `{arg}`"));
        }
        index += 1;
    }

    Ok(path.unwrap_or_else(default_config_path))
}

fn default_config_path() -> PathBuf {
    match std::env::var(CONFIG_ENV) {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(DEFAULT_CONFIG_PATH),
    }
}

/// certbot deploy hook: copy the renewed lineage (`RENEWED_LINEAGE`) to the
/// managed sites that use it and reload Nginx. Runs as root under certbot.
fn command_renew_hook() -> ExitCode {
    use crate::tls::{CertificateManager, TLS_DIR, renewal};

    let Some(lineage) = std::env::var_os("RENEWED_LINEAGE").filter(|value| !value.is_empty())
    else {
        eprintln!("error: RENEWED_LINEAGE is not set (run by certbot as a deploy hook)");
        return ExitCode::from(EXIT_USAGE);
    };
    let nginx = crate::provider::NginxManager::default();
    match renewal::refresh_renewed(
        Path::new(&lineage),
        Path::new(renewal::LETSENCRYPT_LIVE),
        Path::new(TLS_DIR),
        &CertificateManager::default(),
        || nginx.reload_checked().map_err(|error| error.to_string()),
    ) {
        Ok(sites) if sites.is_empty() => {
            println!("no Hostnur site uses {}", Path::new(&lineage).display());
            ExitCode::from(EXIT_OK)
        }
        Ok(sites) => {
            println!("refreshed {} and reloaded nginx", sites.join(", "));
            ExitCode::from(EXIT_OK)
        }
        Err(error) => {
            eprintln!("error: renewed certificate not installed: {error}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

fn version_line() -> String {
    format!(
        "{} {VERSION} (protocol {PROTOCOL_VERSION})",
        env!("CARGO_PKG_NAME")
    )
}

fn command_help() -> String {
    format!(
        "Usage: hostnur-agent <command> [options]\n\n\
         Commands:\n  \
         run                            Start the agent process (idle in this build)\n  \
         check-config [--config PATH]   Validate configuration and exit\n  \
         renew-hook                     certbot deploy hook (reads RENEWED_LINEAGE)\n\n\
         Options:\n  \
         --config PATH                  Config file ({CONFIG_ENV} or {DEFAULT_CONFIG_PATH})"
    )
}

fn help() -> String {
    format!(
        "hostnur-agent — Hostnur privileged operations agent\n\n\
         {}\n\n\
         Also: --version, -V   Print version and exit\n       \
         --help, -h      Print this help and exit\n\n\
         This build performs no privileged host operations.",
        command_help()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_line_includes_protocol() {
        let line = version_line();
        assert!(line.contains("hostnur-agent"));
        assert!(line.contains(VERSION));
        assert!(line.contains("protocol 1"));
    }

    #[test]
    fn parses_config_argument_forms() {
        assert_eq!(
            parse_config_arg(&["--config".into(), "/tmp/a.toml".into()]).unwrap(),
            PathBuf::from("/tmp/a.toml")
        );
        assert_eq!(
            parse_config_arg(&["--config=/tmp/b.toml".into()]).unwrap(),
            PathBuf::from("/tmp/b.toml")
        );
        assert!(parse_config_arg(&["--config".into()]).is_err());
        assert!(parse_config_arg(&["--nope".into()]).is_err());
    }

    #[test]
    fn unknown_command_is_a_usage_error() {
        assert_eq!(run(&["frobnicate".into()]), ExitCode::from(EXIT_USAGE));
    }

    #[test]
    fn missing_command_is_a_usage_error() {
        assert_eq!(run(&[]), ExitCode::from(EXIT_USAGE));
    }
}
