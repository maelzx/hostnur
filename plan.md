# Hostnur — implementation plan and session handoff

Last updated: 2026-10-11.

## 1. Purpose and planning authority

Hostnur is a modular, native-first VPS control panel. The web panel manages durable,
audited operations; a separate Rust agent performs narrowly defined privileged
actions. Hosted applications remain independent of the panel's own language.

Source requirements: `vps-control-panel-plan.md`. Preserve that document as the
original product specification. This file records implementation phases, later
decisions, open questions, validation gates, and session handoffs. Explicit user
decisions recorded here supersede conflicting provisional choices in the source.

Current repository: `https://github.com/maelzx/hostnur`, branch `main`.
Initial commit: `b73f0a7` (`Initialize repository`). No application code existed
when this plan was created.

## 2. Confirmed decisions

| Area | Decision | Origin |
|---|---|---|
| Product name | Hostnur | User's initial request |
| Repository | One monorepo; independently buildable panel and agent | Specification |
| Panel | PHP / Laravel / Blade / Livewire | User's initial request |
| UI styling | Tailwind CSS | Clarification, 2026-10-11 |
| HTMX | Selective, separately owned flows; never manage the same DOM as Livewire | Clarification, 2026-10-11 |
| Privileged agent | Rust, separate systemd service | User and specification |
| Panel persistence | SQLite and database-backed queue with a dedicated worker | Clarification, 2026-10-11 |
| First hosted runtime | PHP/PHP-FPM, with a Laravel deployment workflow | Clarification; supersedes Python-first source roadmap |
| Initial source inputs | Public and private Git repositories | Clarification, 2026-10-11 |
| Beta access model | Single server, administrator-only access, MFA | Clarification, 2026-10-11 |
| Beta TLS | Let's Encrypt HTTP-01 and uploaded certificates | Clarification, 2026-10-11 |
| Initial validated platform | Ubuntu 24.04 LTS, amd64 | Specification |
| Deployment model | Native Linux users, systemd, Nginx, PHP-FPM | Specification |
| Session size | One small, verified slice per session, then handoff and commit/push | Clarification, 2026-10-11 |
| Destructive validation | Disposable Ubuntu VM; VM will be supplied later | Clarification, 2026-10-11 |

Engineering baseline selected in session 002: Laravel 13, Livewire 4, Tailwind 4,
PHP minimum 8.3 (local 8.4 allowed), Composer 2.10.3+, Node 24 LTS, Rust/Cargo
1.99.0 with initial MSRV 1.99 and edition 2024. See
[`docs/development-toolchain.md`](docs/development-toolchain.md) for evidence,
prerequisites, and the distinction between selected and actually validated versions.

### Beta boundary

The first public beta must include installation, safe administrator onboarding,
MFA, durable jobs, audit records, server overview, public/private Git deployment
of PHP/Laravel applications, dedicated application users, PHP-FPM/Nginx lifecycle,
domains/TLS, logs, health checks, supported rollback, approved packages, managed
services, basic scheduled jobs, and tested backup/restore and upgrades.

Laravel deployment must support environment configuration, persistent storage,
Composer, application-specific build steps where required, optional migrations,
and optional queue workers/scheduler. Builds and application commands run as the
application's Linux user. Database migration rollback is not automatically safe;
the UI must distinguish code rollback from database/data recovery.

Early application database provisioning will follow the first deployment slice;
that slice may use an administrator-supplied existing database. The panel's SQLite
database is separate from any database hosted for applications.

## 3. Architecture and implementation rules

- Panel runs unprivileged. Agent operations are allowlisted and resolve managed
  resource IDs independently; no arbitrary root command endpoint.
- Local Unix-domain socket with restrictive permissions and peer checks;
  versioned structured JSON protocol, framed messages, request IDs, bounded
  payloads, timeouts, and capability reporting. Exact framing is decided in phase 2.
- Persist intent before execution. Define job states, ownership, leases,
  cancellation semantics, idempotency, and reconciliation after worker/agent
  restart. Do not claim exactly-once execution across process boundaries.
- Agent maintains a durable managed-resource registry with an explicit recovery
  and reconciliation strategy; caller-supplied paths/service names are not trusted.
- Validate managed paths, symlink behavior, names, repositories, references,
  configuration values, and child-process arguments at the appropriate boundary.
- Separate privileged provisioning from unprivileged source checkout, builds,
  Composer scripts, Artisan commands, and runtime execution.
- Configuration changes use staged writes, validation, backups, and recovery.
  Account for global Nginx validation failures and partial PHP-FPM reload failures.
- Application secrets, Git credentials, and TLS keys are restricted and redacted
  from task output/audit metadata. Durable audit records contain useful attribution
  without storing secret values.
- Livewire owns interactive panel components. Introduce HTMX only for a concrete
  benefit in a separate flow, with the same authentication/CSRF/authorization rules.
- SQLite is a starting decision, not an untested scaling claim. Measure queue,
  monitoring, and audit-write contention before beta; document migration triggers.
- Security, meaningful tests, and failure handling belong to each phase. Later
  release-gate phases validate the complete system rather than postponing safety.
- No destructive installer/service/firewall experiments in the development host.

### Planned repository structure

```text
panel/                      Laravel application, Blade/Livewire UI, panel tests
agent/                      Rust crate, operation handlers, agent tests
contracts/                  Protocol versions, schemas, fixtures, operation catalog
modules/                    First-party manifests and runtime metadata
packaging/                  Debian packages, systemd units, bootstrap installer
tests/integration/          Cross-process and disposable-VM scenarios
docs/                       Architecture decisions, developer/operator documentation
.github/workflows/          CI and release workflows
README.md                   Project overview and entry points
plan.md                     Roadmap, decisions, blockers, session handoff
vps-control-panel-plan.md   Original requirements
```

## 4. Token-aware working and Git policy

1. Start each session by reading this handoff and inspecting repository status.
   Load only the files and guidance relevant to the selected slice.
2. Select one narrowly scoped deliverable and its acceptance checks. A phase may
   require several sessions; phase numbers are not a promise of session size.
3. Record the active slice before substantial work. Avoid beginning an additional
   feature after the selected slice is complete.
4. Complete relevant checks. Record exact commands, outcomes, and any unexecuted
   environment-dependent gates; do not mark blocked checks as passed.
5. Update this file with completed work, next action, blockers, and changed decisions.
   Keep durable architecture detail in `docs/` as it grows and link it here.
6. Inspect `git status`, `git diff`, and recent history. Stage only intended files,
   commit in repository style, and push to the configured remote every session.
   Do not include secrets or unrelated user work, amend/force-push, or skip hooks.
7. Verify remote synchronization and report the commit plus checks. If push fails,
   preserve the local commit, record the blocker, and report it explicitly.
8. Stop at the checkpoint and resume from the handoff in the next session.

The agent cannot observe remaining account credit or guarantee a checkpoint if
the service terminates abruptly. Keep slices small and save progress early. If
interrupted, inspect the actual working tree and commit state before continuing;
do not assume the last attempted operation completed.

## 5. Phased roadmap

Status values: **pending**, **in progress**, **blocked**, **complete**. A phase is
complete only when its exit checks pass. Phase 1 is in progress; later implementation
phases are pending. Dependencies reference phase numbers; partial scaffolding can happen
earlier, but dependencies must pass before declaring the dependent phase complete.

### Phase 0 — Requirements, decisions, and session protocol
- Scope: review the source, clarify priorities, record this phased plan and handoff.
- Exit: decisions and remaining questions documented; planning committed/pushed.
- Status: complete; planning commit `f654136` was pushed and remote-verified.

### Phase 1 — Monorepo and reproducible development foundation
- Status: in progress; toolchain/compatibility baseline slice complete in session
  002. Tool provisioning, runnable scaffolds, lockfiles and CI remain.
- Depends on: 0.
- Slices: inspect PHP/Composer/Node/Rust toolchains; choose mutually compatible,
  supported Laravel/Livewire/PHP versions and an MSRV; scaffold panel and agent;
  add setup instructions, lockfiles, formatting/static checks, and initial CI.
- Exit: panel boots locally, agent builds, each component's baseline checks pass,
  and CI uses documented versions. No production installation is implied.

### Phase 2 — Protocol and operation contract
- Depends on: 1.
- Slices: define envelopes, framing, error taxonomy, capabilities/version rules,
  authentication boundary, request IDs, and operations; add shared fixtures and
  cross-language contract checks; decide job/agent correlation and retry semantics.
- Exit: PHP and Rust agree on valid/invalid messages and unsupported-version errors.

### Phase 3 — Agent transport and execution boundary
- Depends on: 2.
- Slices: socket service, peer validation, managed-resource registry, validated
  dispatch, bounded subprocess runner, health/capabilities and read-only inventory.
- Exit: permitted callers can invoke predefined operations; unknown operations,
  malformed requests, unauthorized peers, unsafe paths, and oversized input fail.

### Phase 4 — Administrator onboarding, authentication, and MFA
- Depends on: 1.
- Slices: local one-time bootstrap, administrator login, TOTP MFA/recovery codes,
  secure sessions, rate limits, CSRF, reauthentication for consequential changes.
- Exit: no default password/public registration; setup and recovery flows work;
  unauthorized requests cannot reach administrative actions.

### Phase 5 — Durable jobs, audit, and reconciliation
- Depends on: 2, 3, 4.
- Slices: task/audit schema, DB worker, state transitions, bounded/redacted logs,
  actor attribution, progress views, leases/idempotency, restart reconciliation.
- Exit: browser disconnect does not stop a job; worker/agent interruption produces
  a recoverable state and cannot blindly duplicate privileged mutations.

### Phase 6 — Panel shell and server overview
- Depends on: 3, 4, 5.
- Slices: responsive navigation, accessible common components, CPU/RAM/disk/uptime,
  OS information, managed-service status, job history and failure presentation.
- Exit: authenticated overview uses real agent data with bounded polling and clear
  stale/unavailable states; no unbounded metrics writes to SQLite.

### Phase 7 — Application inventory and Linux isolation
- Depends on: 3, 5.
- Slices: app identities, runtime configuration, dedicated Linux users, managed
  directory layouts, ownership/permissions, secret files, registry reconciliation.
- Exit: isolated apps can be provisioned safely and retries preserve identity;
  traversal/symlink cases and cross-application secret access are tested.

### Phase 8 — Public/private Git sources and credential lifecycle
- Depends on: 5, 7.
- Slices: public HTTPS sources; encrypted credential references; private source
  access; strict SSH host-key verification where used; ref/commit pinning; timeouts,
  bounded checkout, credential rotation/deletion, redacted diagnostics.
- Exit: public and private repositories produce reproducible source snapshots as
  the app user; secrets do not leak into logs, URLs, or command-line diagnostics.

### Phase 9 — PHP runtime, PHP-FPM, and Laravel build provider
- Depends on: 7, 8.
- Slices: approved PHP packages/extensions and supported versions; isolated FPM
  pools; Composer/build execution as app user; environment/shared storage; Artisan
  preparation; provider lifecycle interface; optional worker/scheduler definitions.
- Exit: a sample Laravel release builds and runs through its FPM pool without
  privileged build scripts or using the panel's own credentials.

### Phase 10 — Transactional Nginx sites and domain routing
- Depends on: 3, 7, 9.
- Slices: domain validation/conflict detection, managed templates, staged config,
  `nginx -t`, reload/recovery, PHP routing and static sites, panel-route isolation.
- Exit: an application is reachable by HTTP; invalid configuration or reload
  failure preserves/reinstates the last working managed configuration.

### Phase 11 — Deployment orchestration, health, logs, and rollback
- Depends on: 5, 8, 9, 10.
- Slices: release directories, complete deployment job, health gates, controlled
  current-symlink switch, restart/log views, release retention, worker handling,
  explicit migration policy, code rollback and partial-failure reconciliation.
- Exit: deploy/redeploy/restart/inspect/roll back a sample Laravel application
  through the UI; persistent uploads survive and unsafe migration rollback is not
  implied. Local checks precede real-system checks on a disposable Ubuntu VM.
- Milestone: first complete native application vertical slice.

### Phase 12 — TLS lifecycle
- Depends on: 10, 11.
- Slices: certificate-provider interface, Let's Encrypt staging then HTTP-01,
  renewal scheduling/expiry visibility, uploaded certificates/key validation,
  transactional activation, failure recovery and secure key storage.
- Exit: HTTPS and renewal work on a VM with a real DNS name; failed issuance or
  renewal does not remove a working certificate or expose private keys.

### Phase 13 — Managed services, approved packages, and scheduled jobs
- Depends on: 3, 5, 7, 11.
- Slices: allowlisted package operations with package-manager locking; managed
  service controls; app-user timers/jobs, history and bounded logs; read-only
  firewall status. Exclude arbitrary root execution and unmanaged-service control.
- Exit: valid operations survive failure/retry; schedules run as the intended user;
  simultaneous package operations are coordinated.

### Phase 14 — Early application database provisioning
- Depends on: 3, 5, 7, 11; database engine decision required.
- Slices: first engine module, app-scoped database/users, credential delivery and
  rotation, connection checks, import/export limitations, explicit deletion rules.
- Exit: a Laravel app can use an isolated provisioned database; one app cannot
  access another app's data. Panel storage remains independently managed.

### Phase 15 — Backup, restore, and recovery
- Depends on: 5, 7, 11, 12, 13; include 14 when enabled.
- Slices: consistency/retention policy; panel DB/config, agent registry, keys,
  application metadata/data and database backups; encryption/key custody; restore
  orchestration; disk-full handling and operator recovery documentation.
- Exit: restore onto a separate disposable VM and verify panel access, resource
  identity, application health, secrets, and supported database state.

### Phase 16 — Debian packaging, installer, and production onboarding
- Depends on: 4, 11, 12, 15.
- Slices: `hostnur-agent`/`hostnur` packages, systemd permissions/units, runtime
  dependencies, architecture/OS detection, verified artifact bootstrap, local setup
  and SSH-tunnel instructions, idempotent install and explicit existing-host policy.
- Exit: clean Ubuntu 24.04 amd64 installation reaches a healthy panel/agent and
  supports deployment without exposing an unauthenticated setup page publicly.

### Phase 17 — Signed releases and upgrade lifecycle
- Depends on: 16; signing identity/domain/hosting decisions required.
- Slices: version tags, CI artifacts/signatures/checksums, GitHub Releases, verifier,
  coordinated panel/agent compatibility, pre-upgrade backup, migrations, health
  checks, failed-upgrade recovery; signed APT hosting before wider adoption.
- Exit: install verified built artifacts, upgrade a previous package version,
  preserve apps/data, and test documented recovery after a failed upgrade.

### Phase 18 — Beta integration, boundary review, and release gate
- Depends on: 1–17 and disposable VM access.
- Slices: complete cross-process/VM regression; filesystem/socket/operation abuse
  cases; app isolation; job disconnect/crash behavior; certificate and Nginx failure
  recovery; SQLite contention/load checks; release verification and restore drill.
- Exit: every beta requirement in section 7 has evidence; unresolved blockers are
  fixed or scope changes explicitly agreed. Do not label mocked checks as VM proof.
- Milestone: first public beta, after an explicit release decision.

### Phase 19 — First-party module lifecycle
- Depends on: 18 (provider interfaces already introduced in phase 9).
- Slices: versioned manifests, compatibility/capabilities/dependencies, install
  state, reviewed upgrade/uninstall plans, data preservation, first-party catalog.
- Exit: a first-party module can be installed/upgraded/removed safely without
  treating its manifest as a sandbox or deleting user data by default.

### Phase 20 — Python, generic services, and additional runtime providers
- Depends on: 19.
- Slices: Python venv/systemd deployment first; generic non-root build/start
  service; then Node.js, Go, Java, and .NET according to demand and tested support.
  Each provider is its own set of small sessions, with health/rollback checks.
- Exit: each advertised provider passes the same isolation/deployment contract.

### Phase 21 — Extended monitoring, backups, and integrations
- Depends on: 19; subsequent scope decided from beta feedback.
- Slices: bounded metric retention/alerts, remote backup providers, second database
  engine, DNS-01 providers, optional archive uploads and deployment hooks.
- Exit: enabled integrations have documented credentials, limits, recovery paths,
  and tests; no support claim is made for unvalidated integrations.

### Phase 22 — Opt-in AI-assisted operations
- Depends on: 18 and stable authorized operation interfaces.
- Slices: provider-neutral configuration, evidence-grounded diagnostics, redaction,
  spending limits, plan proposals, approvals, structured tool calls, health
  verification and audit attribution. Core features continue without an AI key.
- Exit: AI can diagnose/propose/invoke authorized operations with required approval;
  neither model output nor untrusted logs grant arbitrary privileged execution.

### Phase 23 — Expanded platforms and ecosystem
- Depends on: 18–20 and demonstrated demand.
- Slices: validate Ubuntu 26.04/arm64 before advertising; explore team RBAC,
  optional Docker module, multi-server control and reviewed/signed external modules
  as separately planned projects after the single-server experience is reliable.
- Exit: each added capability has a dedicated agreed scope and support matrix.
- Deferred: full email hosting, browser root terminal, unrestricted marketplace,
  unreviewed autonomous AI, and broad untested Linux compatibility.

## 6. Open decisions and recommended starting defaults

These are not user-confirmed decisions. Resolve each before its implementation;
ask only when the choice materially changes scope, cost, access, or product behavior.

| Decision | Proposed direction / outstanding question | Resolve by |
|---|---|---|
| Framework/tool versions | Baseline selected in session 002; verify resolved dependencies, lockfiles, actual PHP 8.3/8.4 runtime checks and pinned Rust build | Remaining phase 1 slices |
| Protocol framing and registry | Bounded length-prefixed JSON on UDS; agent-owned durable registry; compare implementation complexity and recovery needs | Phases 2–3 |
| Git credentials | Prefer read-only per-repository deploy keys and/or scoped HTTPS tokens, verified host keys, no credential in repository URL; confirm required Git providers and auth modes | Phase 8 |
| App database engine | MariaDB is a proposed first engine for Laravel users; confirm MariaDB versus PostgreSQL | Phase 14 |
| Secret and backup keys | Encrypt panel-managed secrets using established primitives; keep keys outside DB and define encrypted off-host recovery-key handling, rotation and restore | Before phase 8; complete in 15 |
| Installer on existing hosts | Propose clean supported VPS only for beta; do not adopt or overwrite existing unmanaged services/configuration implicitly | Phase 16 |
| Product domain | User to supply official panel/install/docs domain; use no advertised placeholder installer URL | Phases 12, 16–17 |
| Release signing and hosting | GitHub Releases initially; choose verifiable signing identity and signed APT repository hosting before wider distribution | Phase 17 |
| VM access and DNS | Disposable Ubuntu 24.04 amd64 VM plus test domain/DNS for real deployment, TLS, upgrade and separate restore tests | Before VM portion of phase 11 |
| Licensing/public beta | Confirm repository license and public-beta distribution terms before publishing releases | Before phase 18 release |

## 7. Beta acceptance checklist

- [ ] Clean supported-VM installation, secure onboarding, MFA and session recovery.
- [ ] Public and private Git Laravel deployments, dedicated users and PHP-FPM pools.
- [ ] Build/application commands execute without root privileges.
- [ ] Persistent uploads and secrets survive supported redeployment/rollback.
- [ ] Deployment health, logs, managed restarts, queue workers and scheduler work.
- [ ] Invalid Nginx changes cannot silently replace the working configuration.
- [ ] Automatic TLS issuance/renewal and uploaded certificates work safely.
- [ ] Server overview, approved packages, managed services and schedules work.
- [ ] Jobs remain inspectable after browser disconnect and process interruption.
- [ ] Agent cannot be used as an arbitrary root command runner.
- [ ] Socket/resource/path checks and cross-application secret isolation pass.
- [ ] Audit attribution and log/secret redaction are verified.
- [ ] SQLite queue/audit/metrics contention stays within documented limits.
- [ ] Upgrade preserves panel data, application settings and credentials.
- [ ] Restore and documented recovery work on a separate VM.
- [ ] Published artifacts and repository metadata have verifiable authenticity.
- [ ] Support matrix and operator instructions match actual tested behavior.

## 8. Current handoff

### Session 002 — Toolchain and compatibility baseline (2026-10-11)

- Completed slice: inspected installed tools, selected compatible framework/tool
  versions, and documented prerequisites in `docs/development-toolchain.md`.
- Inspection: PHP 8.4.26, Composer 2.9.2, Node 24.12.0, npm 11.6.2 and Git
  2.43.0 are available. Rust/Cargo/rustup, Docker and QEMU are absent from PATH.
- PHP extension probe passed for all required extensions; SQLite is 3.45.1.
- Selected Laravel 13/Livewire 4/Tailwind 4, PHP 8.3+, Composer 2.10.3+, Node 24
  LTS and Rust 1.99.0. Added `.nvmrc`, `rust-toolchain.toml`, and project README.
- Validation executed: tool `--version` probes, `php -m`, PHP extension/SQLite/PDO
  probe, `php --ini`, `uname -sm`, and `composer diagnose`. The Composer diagnostic
  has failures/warnings: missing verification keys and advisories in the installed
  Composer 2.9.2. Packagist/GitHub connectivity passed.
- Checked upstream Laravel release policy and scaffold metadata, Livewire Composer
  compatibility, Ubuntu PHP package metadata, and the Rust stable manifest.
- Reviewed all new documentation/version files; `git diff --check` passed.
- No applications scaffolded, tools installed, application tests run, or VM gates
  executed. This session completes only the baseline slice, not phase 1.
- Publication: commit/push at session close; verify this session's commit on
  `origin/main`. The final response records the push outcome.

### Session 001 — Planning (2026-10-11)

- Reviewed all 571 lines of `vps-control-panel-plan.md` and repository state.
- No repository-local `AGENTS.md` or application scaffolding was present.
- Confirmed seven clarification answers; recorded decisions in section 2.
- Created phases 0–23, acceptance gates, token-aware session rules, and open decisions.
- Preserve and version the original requirements alongside this plan.
- GitHub authentication is available; `origin` is `maelzx/hostnur` and the working
  branch is `main`. Verify publication using this planning commit's presence on
  `origin/main`; the final session response records the push result.
- Validation: full plan review and `git diff --check --no-index /dev/null plan.md`
  passed. Inspected `git status --short --branch`, `git diff`, and
  `git log --oneline -10`; only the two planning documents are intended for staging.
- No application tests apply yet. No VM checks have been performed.

### Next session's single slice

Begin **phase 1, slice 2: provision development tools**. Obtain a verified user-local
Composer 2.10.3+ with verification keys, check/use patched Node 24, install pinned
Rust 1.99.0 plus rustfmt/Clippy, and repeat the relevant diagnostics. Keep the setup
user-local and record executable paths for the next session. Once tool checks pass,
the following slice is minimal Laravel/Livewire and Rust scaffolding. Record exact
commands/results, update this handoff, commit, push, and stop at the checkpoint.

### Current blockers

- PHP and Node are present, but Composer tooling needs updating/key setup and Rust
  is missing. Resolve these prerequisites before dependency installation/scaffolding.
- Disposable VM access is unavailable; real install/systemd/TLS/upgrade/restore
  gates remain unverified until supplied.
- Official domain, Git authentication details, database engine, release signing,
  and license await the decision checkpoints listed in section 6.
