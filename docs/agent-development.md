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
| `serde` | 1.0.229 | derive-based (de)serialization for contract DTOs (phase 11.1) | MIT OR Apache-2.0 |
| `serde_json` | 1.0.154 | strict JSON request parsing and response serialization (phase 11.1) | MIT OR Apache-2.0 |
| `nix` | 0.31.3 | safe syscall wrappers: `SO_PEERCRED`, signals, `setgroups`/`setgid`/`setuid`, `openat2`-adjacent fs ops (phases 11.2, 12.2) | MIT |
| `libc` | 0.2.190 | `openat2(2)` beneath-resolution syscall and child-identity drop (phases 12.2, 12.3) | MIT OR Apache-2.0 |
| `rusqlite` | 0.40.2 | agent-owned execution journal and resource registry (phase 11.3); `bundled` compiles SQLite from source (C toolchain) | MIT |
| `getrandom` | 0.4.3 | OS CSPRNG for execution/resource UUIDs (phase 11.3) | MIT OR Apache-2.0 |
| `tempfile` (dev) | 3.27.0 | temporary config directories in tests | MIT OR Apache-2.0 |

`Cargo.lock` is committed. An async runtime (socket concurrency) and
process-control crates are introduced only when their phase uses them. The
contract DTOs/parser and length-prefixed framing live in `agent/src/contract/`
and are checked against `contracts/v1` fixtures in
`agent/tests/contract_fixtures.rs`.

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

## Host integration (phase 11.6)

`packaging/systemd/hostnur-agent.service` runs the agent as a dedicated
`hostnur` system user with a hardened unit (`ProtectSystem=strict`,
`NoNewPrivileges`, empty capability set, `RuntimeDirectory=hostnur`,
`StateDirectory=hostnur/agent`). An example config is
`packaging/systemd/agent.toml.example`; `allowed_peer_uids` must list the panel
identity.

Verified in the Lima Ubuntu 24.04 VM (session 026):

```bash
sudo groupadd -f hostnur
sudo useradd --system --no-create-home --shell /usr/sbin/nologin --gid hostnur hostnur
sudo install -m 0644 packaging/systemd/hostnur-agent.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now hostnur-agent
```

Observed: `/run/hostnur/agent.sock` is `mode=660 owner=hostnur:hostnur`; a user in
the `hostnur` group receives `agent.capabilities` (`mode=real`, available, the four
supported operations); an unrelated user is denied at `connect()`; and
`systemctl restart` recreates the socket and serves again. `server.snapshot`
returns live read-only metrics (load, memory, uptime, root-disk usage, CPU count),
hostname and OS from `/proc`/`statvfs` — no writable operation and no shell.

## Managed application isolation (phase 12)

`agent/src/provision/` turns an approved application UUID into an isolated Linux
identity and a managed layout:

- **Identity/namespace** (`policy.rs`): account name `hn_<12 hex of UUID>`,
  layout `/opt/hostnur/apps/<uuid>/{releases,shared,shared/secrets,current}`,
  and the `reserved -> account_created -> layout_prepared -> ready` lifecycle
  (plus `failed`/`drifted`). Names and paths are always derived from the UUID.
- **Accounts** (`accounts.rs`): `groupadd`/`useradd` via the allowlisted process
  primitive with a validated argv; the result is read back from NSS, not trusted
  from tool output. No login shell, no home, not in the panel socket group.
- **Safe filesystem** (`safe_fs.rs`): every managed write is resolved with
  `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS)` from a
  single trusted base descriptor; files publish with an atomic rename.
- **UID switching** (`process.rs`): the child clears supplementary groups with
  `setgroups([])` before `setgid`/`setuid`, so it never inherits the agent's
  groups.
- **Reconciliation** (`service.rs`): idempotent reserve/provision/reconcile;
  unmanaged accounts or paths are refused, identity drift is detected, and a
  re-verified layout is marked ready.

Privilege model for this phase: a **root-run harness** on the disposable VM, not
yet the hardened systemd service (which still runs as `hostnur`). Root-only tests
are `#[ignore]`d and executed directly:

```bash
cargo test --locked --no-run
sudo target/debug/deps/provision_vm-<hash> --ignored --nocapture
sudo target/debug/deps/process_runner-<hash> --ignored --nocapture
```

The vetted isolation runs verified on the arm64 Lima VM: two apps provisioned with
distinct UIDs, `shared` `0750` app-owned, `secrets` `0700`, `releases` root-owned
`0755`; app A read its own secret and was denied app B's; a symlinked parent was
refused; and a child ran as the target UID with no supplementary groups.

## Git source lifecycle (phase 13)

`agent/src/git/` implements the real public/private HTTPS source path:

- **Policy** (`policy.rs`): only `https://` URLs; credentials rejected in the
  URL; loopback/link-local/cloud-metadata and (by default) RFC1918 refused;
  explicit host allowlist and optional custom port; refs validated against
  `git check-ref-format` rules with option-like/revision-expression forms
  rejected and full 40/64-hex commit ids recognized.
- **Checkout** (`checkout.rs`): a fixed `git` binary runs as the application
  UID/GID with a minimal environment (`GIT_CONFIG_NOSYSTEM=1`,
  `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`, `GIT_SSH_COMMAND` and
  `GIT_ASKPASS` disabled unless supplied, `GIT_ALLOW_PROTOCOL=https`), a deadline
  and capped output. It `init`/`remote add`/`fetch --depth 1`/`rev-parse`/
  `checkout --detach`s and returns the pinned commit; a failure removes only the
  incomplete staging tree.
- **Credentials** (`credentials.rs`): a private token is delivered through a
  transient, app-owned `GIT_ASKPASS` helper + `0600` token file that is removed
  on drop; the value never appears in the URL, argv, config or error messages.

Chosen auth for this phase: **public HTTPS + private HTTPS scoped token** (SSH
deploy keys, P13.4, are deferred). Verified on the arm64 Lima VM with a root-run
harness (`agent/tests/git_vm.rs`): checkout ran as the app user, produced an
app-owned working tree, pinned a 40-hex commit, and cleaned up a failed staging
tree.

Deferred/recorded: live private-repository clone (offline VM) — the credential
mechanism is unit-tested for permissions, cleanup and non-leakage; SSH host-key
pinning (P13.4); and panel/task integration of real source results (P13.6), which
belongs with the deployment orchestration in phase 15.

## Blocked / deferred checks

Recorded, not claimed:

- Real GitHub Actions run: the workflow exists but cannot be observed until the
  branch is pushed to `origin` (these sessions committed locally only).
- The host integration above ran on the **arm64** Lima VM. amd64 systemd/socket
  behavior is covered by the CI build target but not yet run on an amd64 VM.
- Cross-language end-to-end (PHP panel → live Rust agent over the socket) was not
  run locally: PHP is not installed inside the VM. Both sides are tested against
  the shared contract and their own framing; a live PHP↔Rust test is a follow-up
  (CI or amd64 VM).
- Privileged mutation handlers (users, packages, services, Nginx, TLS) remain
  unimplemented and return `unsupported_operation`; they arrive in phases 12+.
- Release-target (x86_64) binary execution and packaging are validated in
  phase 20.

## Next slice

Phase 11 is complete. The next work is **phase 12** (Linux application isolation
and resource reconciliation), which needs a disposable Ubuntu 24.04 amd64 VM for
isolation evidence.
