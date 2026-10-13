use std::process::ExitCode;

fn main() -> ExitCode {
    hostnur_agent::cli::run_from_env()
}
