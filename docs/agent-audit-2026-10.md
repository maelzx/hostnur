# Agent code audit & fixes (2026-10-15)

Static review of `agent/` (root-run Linux host agent) covering transport,
contract, dispatch, journal, process primitive, provision, git, packages,
providers, deploy, TLS and database modules. Findings were verified against
the actual source before any fix was applied. No runtime/server was started
for the audit; verification was `cargo fmt --check`,
`cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked`
(non-root; privileged `*_vm` tests stay `#[ignore]`d) inside the Lima
`hostnur-rust` Ubuntu 24.04 VM.

## Fixed

| # | Severity | Finding | Fix location |
|---|---|---|---|
| 1 | High | Systemd unit injection: schedule `program` was only checked for `is_absolute()`; newlines/`%` specifiers could open new directives (`User=root`, `ExecStartPre=`) inside root-managed units | `dispatch.rs` (`is_safe_path_token` at registration), `provider/worker.rs` (allowlist in `validate_common`/`exec_line`, `%`→`%%` escaping, `ReadWritePaths` validation, unit file restored/removed when `systemd-analyze verify` fails) |
| 2 | High | MariaDB passwords leaked through world-readable `/proc/<pid>/cmdline`: admin DDL (`CREATE USER ... IDENTIFIED BY`) passed as `-e` argv | `database/mariadb.rs` — all admin SQL now delivered via stdin (`CommandSpec.stdin`) |
| 3 | High | `database.export` silently truncated dumps at 256 KiB and reported success (corrupt "backups") | `database/mariadb.rs` — `stdout_truncated` now fails with `Conflict`; cap documented |
| 4 | High | Git SSRF bypasses: legacy numeric hosts (`127.1`, `2130706433`, `0x7f000001`, `0177.0.0.1`) passed as "hostnames"; IPv4-mapped IPv6 (`[::ffff:127.0.0.1]`) skipped v4 rules; `0.0.0.0/8` allowed; AWS IPv6 IMDS allowed with private nets | `git/policy.rs` — numeric-alphabet literals refused, `to_ipv4()` unwrapping re-checks mapped addresses, `0/8` and `fd00:ec2::/64` refused; regression tests added |
| 5 | High | Repository URL stored raw (untrimmed) after validation, so the journal value could differ from the validated one | `dispatch.rs` — stores `RepositoryUrl::url` (normalized, credential-free) |
| 6 | High | Frame reads bounded only per-syscall: a drip of 1 byte per timeout could hold the single-threaded serve loop for ~max_body×timeout | `transport/server.rs` — 30 s overall `FRAME_READ_DEADLINE` (`DeadlineReader`); failure to install socket timeouts now closes the connection instead of leaving it unbounded |
| 7 | High | Shared fixed temp names raced between concurrent writers: `.current.tmp` (symlink switch), `*.conf.tmp` (nginx), `*.conf.tmp` (fpm) — one writer could rename another's target/contents into place | `deploy/mod.rs` `switch_current` (unique nonce name + temp cleanup on rename failure), `provider/nginx.rs` / `provider/php_fpm.rs` `write_atomic` (pid+nanosecond temp, removed on failure) |
| 8 | High | Unbounded OS thread spawn per deploy from a misbehaving panel identity | `dispatch.rs` — `DeployTicket` RAII counter, `MAX_CONCURRENT_DEPLOYS = 4`, extra requests get `agent_unavailable` before the journal accepts |
| 9 | Med | `agent.cancel` was a no-op: worker never observed it and overwrote `cancelled` with its own terminal status | `deploy/mod.rs` — `DeployInput.execution_id` + cancellation boundaries (before source fetch and before activation); journal `execution_status()` helper; cancel appends an event (`dispatch.rs`); worker preserves `cancelled` when it aborts pre-activation and records the true outcome with an explanatory event if activation already happened |
| 10 | Med | Journal: second connection per deploy worker with no `busy_timeout`; `append_event` MAX+INSERT+UPDATE untransactional (sequence collisions); terminal write failures silently dropped; failed `Journal::open` in `run_worker` returned without a trace | `journal.rs` — 5 s `busy_timeout`, `BEGIN IMMEDIATE` transaction in `append_event`; `deploy/mod.rs` — terminal-write and open failures logged at error level |
| 11 | Med | Migrations were documented fail-closed but actually ran `artisan migrate --force` pre-activation | `deploy/mod.rs` — `run_migrations=true` now returns `MigrationPrerequisiteMissing` (variant previously existed unused); matches docs and the root VM test expectation |
| 12 | Med | `rollback()` left the bad target current when reload/health failed | `deploy/mod.rs` — restores the previous target and reloads, same discipline as `deploy()`; also gained the settle-poll |
| 13 | Med | Health probe had no connect timeout (a DROPping firewall rule blocked each attempt for the OS default) | `deploy/health.rs` — `TcpStream::connect_timeout`; 3xx-healthy behaviour kept deliberately (Laravel redirects `/` to `/login`) and documented |
| 14 | Med | Certificate hostname coverage was substring matching: `evil-app.example.com` covered `app.example.com`; legitimate wildcard certs were rejected | `tls/certificate.rs` — `dns_names()` SAN extraction, exact + one-label wildcard matching, CN fallback parsing; unit tests added |
| 15 | Med | Git credential token written 0644-then-chmod, and leaked in `/tmp` on error paths before the Drop guard existed | `git/credentials.rs` — dir created 0700 (`DirBuilder::mode`), token created 0600 (`create_new` + `OpenOptionsExt::mode`), `populate()` failure removes the directory |
| 16 | Med | `process.rs` ran the child as root when only one of uid/gid was set (silently) | `process.rs` `validate_spec` — uid/gid must be set together; env NUL check added |
| 17 | Med | Provisioning wedged permanently after partial account creation (groupadd exits 9/10 on retry); `Accounts::delete` had no name validation; NSS readback didn't verify shell/primary group as documented | `provision/accounts.rs` — idempotent `create()` (skips existing group/user via NSS first), readback verifies group-gid and nologin shell (`IdentityDrift` otherwise), `delete()` validates the name |
| 18 | Med | certbot argv option-injection surface: hostnames/emails starting with `-` were acceptable tokens | `provider/policy.rs` — `validate_hostname` no longer trims (rejects leading/trailing whitespace, requires alphanumeric ends, rejects `.−`/`-.`/`*`), aligned with the panel's `Hostname` support which rejects wildcards; `tls/certbot.rs` — email must not start with `-` |
| 19 | Low | `string_array` parameter validator inverted (rejected arrays, accepted objects) | `contract/request.rs` — fixed via `as_array()` (latent only: no catalog entry uses it yet) |
| 20 | Low | Advertised operations drifted from served ones: 5 reachable ops (incl. destructive `agent.remove_database`) unadvertised; module docs claimed handlers were placeholders | `dispatch.rs` — `SUPPORTED_OPERATIONS` now lists all 18 unconditional ops; stale docs in `lib.rs`/`runtime.rs` updated |
| 21 | Low | Secrets/dumps chmod-after-write window; `upsert_env` newline injection; `packages.rs` truncation mangled versions; checkout early-rejections left the staging dir | `secrets.rs` (creation-time modes), `deploy/mod.rs` (`upsert_env` rejects CR/LF), `packages.rs` (truncated → error), `git/checkout.rs` (cleanup parity) |

## Deliberately kept / not a defect

- One connection at a time in `serve` (bounded concurrency is the current
  design; see deferrals below).
- Health probe treats 3xx as healthy (Laravel front-controller redirect).
- `MYSQL_PWD` env delivery for the app principal (visible only to the same
  uid/root, unlike argv).

## Deferred / recorded for the next slice

1. ~~**Long inline operations**~~ — **resolved (session 050)**: journaled
   mutations are accepted on the serve loop and executed by a serialized
   mutation worker (`dispatch::serve_jobs`); the panel polls `agent.inspect`.
2. ~~**`timeout_seconds`**~~ — **resolved (session 050)**: the request's
   timeout becomes a thread-scoped operation deadline (`process::with_deadline`)
   capping every subprocess; expiry yields status `timed_out` / code `timeout`.
3. **DNS-resolution-time SSRF**: URL policy is point-in-time at registration;
   git resolves later and follows HTTP redirects (`http.maxRedirects` not
   capped). Needs a resolve-and-check hook or egress firewall; `nip.io`-style
   records bypass any registration-time check.
4. **Unqualified refs**: `git fetch origin main` can resolve a tag when a
   branch was meant (`refs/tags/` precedence). Callers can pass fully
   qualified refs today (`validate_ref` accepts `refs/heads/main`); deciding
   heads-vs-tags default is a contract question.
5. **Per-step deadlines, no overall budget**: deploy gives each subprocess its
   own deadline (5×300 s git steps, 1200 s build steps); add a total budget
   tied to the task deadline.
6. **Journal hygiene**: `SCHEMA_VERSION` is written but never enforced;
   executions/events are never pruned (unbounded growth).
7. **Transport hardening left in place**: socket bind→chmod race and
   symlink-substitution window at startup rely on the root-owned `/run/hostnur`
   parent directory (systemd `RuntimeDirectory`); `Drop` removes the socket
   path without inode comparison.
8. **TLS**: no issuer/chain verification (`openssl verify`) — deliberate for
   `install_supplied` private-CA flows; expiry and key-match are enforced.
9. **Credential plumbing**: `askpass` is not wired into production deploys
   yet (`deploy/mod.rs` passes `askpass: None`); the GitHub-specific username
   `x-access-token` needs to become per-provider when private repos land.
10. **Root VM regression**: the `#[ignore]`d `*_vm` suites (provision, git,
    provider, worker, deploy, tls) must be re-run on the Lima VM as root to
    re-validate the systemd/nginx/fpm/mariadb paths touched here — the
    unit-test run above does not cover them.

## Test-suite note

`tests/dispatch.rs` had 4 failures **pre-existing at HEAD** (commit
`c4115c6` wrapped task operations as accept/receipt/inspect but those tests
still expected immediate error bodies; confirmed against a clean HEAD
worktree). Updated to assert receipt → `agent.inspect` → recorded
`result.error.code`, matching the contract the panel uses.

## Verification (exact)

- `cargo fmt --all -- --check` → clean.
- `cargo clippy --locked --all-targets -- -D warnings` → 0 warnings.
- `cargo test --locked` → lib 125 passed / 0 failed; integration: cli 2,
  contract_fixtures 13, dispatch 9, git_vm 0 (1 ignored), journal 8,
  process_runner 7 (1 ignored), provider_vm 0, provision_vm 0 (2 ignored),
  tls_cert 7 (1 ignored), tls_vm 0, transport_boundary 8, worker_vm 0 — all
  passed. Root-only VM suites not re-run in this session (see deferred #10).
