# Agent development (phase 10)

The Hostnur agent is an independent Rust crate in `agent/`. It builds, tests and
runs without PHP, Node, Laravel or any privileged host operation. Phase 10 delivers
a thin, testable process foundation (configuration, diagnostics, lifecycle,
logging); transport and privileged handlers arrive in phase 11 and later.

This document records the prepared environment, the exact toolchain and target
decisions, the commands to run, and the checks that remain blocked.

## Prepared environment

Local development uses a **Lima** Ubuntu 24.04 VM (arm64 native on Apple
Silicon). The amd64 deployment target is validated in GitHub Actions on
`ubuntu-24.04` (see `.github/workflows/agent.yml`).

Create the VM from a Lima template that mounts the repository writable, then from
inside the guest:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config ca-certificates
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --default-toolchain stable --profile minimal -c clippy -c rustfmt
```

Run any command in the guest with the repository mounted at its host path:

```bash
limactl shell <instance> -- bash -lc 'cd "/path/to/hostnur/agent" && cargo test --locked'
```

## Toolchain and target decisions

Verified in the prepared VM on 2026-10-13:

| Item | Value |
|---|---|
| Guest OS | Ubuntu 24.04.5 LTS, aarch64 |
| rustc | 1.99.0 (b940084d7 2026-09-28) |
| cargo | 1.99.0 (5f94df478 2026-08-27) |
| rustup | 1.29.1 |
| C compiler / linker | cc (Ubuntu 13.3.0), GNU Make 4.3 |
| Edition | 2024 |
| MSRV | 1.99 (the pinned channel is also the declared minimum) |
| Local target | aarch64-unknown-linux-gnu (native) |
| CI / deployment target | x86_64-unknown-linux-gnu (Ubuntu 24.04 amd64) |

`rust-toolchain.toml` at the repository root pins channel `1.99.0` with the
`clippy` and `rustfmt` components and a minimal profile. This was re-checked in
the prepared environment rather than assumed: stable resolved to 1.99.0
(released 2026-10-01), which supports edition 2024. The pin is unchanged.

A successful build against a newer host's glibc does not automatically run on the
Ubuntu 24.04 amd64 target. Release-target compatibility, static-linking choices
and deterministic packaging are finalized in phase 20. CI builds and tests the
x86_64 target on every change so the deployment architecture is not left
untested.

## Dependency policy

Only crates actually used by the current slice are added. Each direct dependency
below has no native build step and does not raise the MSRV beyond the pin.

| Crate | Version | Role | License |
|---|---|---|---|
| `toml` | 1.1.8 | typed configuration parsing (`toml::Value`) | MIT OR Apache-2.0 |
| `signal-hook` | 0.4.5 | SIGTERM/SIGINT flag registration | MIT OR Apache-2.0 |
| `tempfile` (dev) | 3.27.0 | temporary config directories in tests | MIT OR Apache-2.0 |

`Cargo.lock` is committed. `serde`/`serde_json` (contracts), an async runtime
(socket/process concurrency) and process-control crates are introduced only when
phase 11 uses them.

The crate has no `license` field yet: the project-wide license is an open
decision (see `plan.md` section 6) that must be resolved before publication.
`publish = false` prevents accidental release.

## Commands

Run from `agent/` inside the prepared VM:

```bash
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

Run the binary:

```bash
cargo run --locked -- --version
cargo run --locked -- check-config --config /path/to/agent.toml
cargo run --locked -- run --config /path/to/agent.toml
```

`check-config` validates a file and exits 0/2; it creates nothing. `run` starts
the idle process and exits cleanly on SIGTERM/SIGINT. Exit codes: `0` success,
`2` usage or configuration error, `1` runtime error.

## Configuration

A typed TOML file (proposed production path `/etc/hostnur/agent.toml`, ratified
in phase 11). Schema version 1 accepts:

```toml
version = 1
socket_path = "/run/hostnur/agent.sock"   # absolute, <= 107 bytes, no ".."
state_dir = "/var/lib/hostnur/agent"      # absolute, no "..", not "/"
log_level = "info"                        # error | warn | info | debug
max_message_bytes = 1048576               # 1024..1048576
shutdown_grace_seconds = 10               # 1..300
```

Unknown settings, missing required settings, wrong types, unsupported schema
versions, relative paths, `..` traversal and out-of-range limits are rejected with
a stable code (`config_not_found` / `config_parse` / `config_invalid`). Error
messages never echo configuration values. Agent configuration is deliberately
separate from hosted applications' environment variables.

## Behaviour and security boundaries

- `run` initializes logging, reports readiness and waits for a shutdown signal.
  It binds **no** socket, writes no server configuration, installs no packages,
  provisions no users and starts no hosted applications.
- Structured logs are single-line JSON on stderr with a fixed field allowlist
  (`ts_ms`, `level`, `component`, `event`, `message`, optional `code`,
  `correlation_id`). Configuration, environment, request bodies and credentials
  are never logged.
- The process is a normal unprivileged process. No root socket, privileged
  handler or systemd unit exists in this phase.
- Development checks use temporary directories; tests do not mutate the host.

## Blocked / deferred checks

Recorded, not claimed:

- Real GitHub Actions run: the workflow exists but cannot be observed until the
  branch is pushed to `origin` (this session committed locally only).
- Privileged host operations, socket/peer identity, systemd hardening and
  installation tests require a disposable Ubuntu 24.04 amd64 VM (phase 11+).
- Release-target (x86_64) binary execution and packaging are validated in
  phase 20.

## Next slice

**P11.1** — cross-language contract: Rust DTOs/parser, framing/version
decisions and shared fixtures agreeing with `contracts/v1` and
`docs/agent-integration.md`.
