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
| Development order | Web panel first; Rust development later in another prepared environment | User steering, session 003 |
| Current environment | Use installed tools; no additional runtimes/system tools or tool upgrades | User steering, session 003 |

Framework baseline: Laravel 13, Livewire 4, Tailwind 4 and PHP minimum 8.3.
Current web development uses installed PHP 8.4.26, Composer 2.9.2, Node 24.12.0
and npm 11.6.2. Composer 2.10.3+ remains the intended CI/release baseline, not a
required local upgrade. Rust/Cargo 1.99.0, MSRV 1.99 and edition 2024 are deferred
to the agent environment and will be rechecked there. See
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

### Web-first development boundary

- Build and test the panel independently using a PHP agent-client interface and
  deterministic fixtures/fakes for development/tests. No local Rust prerequisite.
- Keep the fake adapter opt-in and restricted to local/testing environments, with
  visible simulated status/data. Production defaults to agent unavailable until a
  real adapter is configured; reject privileged actions server-side in that state.
- The web stage delivers authentication, persistent application configuration,
  request validation, tasks/audit, and working UI flows. Fixture-driven deployment,
  metrics and service results are not proof of actual host operations.
- Never substitute PHP root commands or direct host mutations for the missing agent.
- Freeze operation schemas and representative fixtures early; actual transport,
  version negotiation, registry and cross-language compatibility are validated later.
- Use existing PHP/Composer/Node/npm. Project-local Composer/npm dependencies are
  part of building the panel; do not install or upgrade runtimes, system packages,
  Rust, Docker, QEMU, or global development tools in this environment.
- If a concrete dependency/tool incompatibility occurs, record the exact blocker
  and continue independent web work where possible. Do not silently install tools
  or disable package advisory checks to force dependency resolution.

### Full-system rules

- Panel runs unprivileged. Agent operations are allowlisted and resolve managed
  resource IDs independently; no arbitrary root command endpoint.
- Local Unix-domain socket with restrictive permissions and peer checks;
  versioned structured JSON protocol, framed messages, request IDs, bounded
  payloads, timeouts, and capability reporting. Initial schema is in phase 3;
  final transport compatibility is validated in phase 11.
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

Status values: **pending**, **in progress**, **deferred**, **blocked**, **complete**. A phase is
complete only when its exit checks pass. Phase 1 is in progress; later implementation
phases are pending. Dependencies reference phase numbers; partial scaffolding can happen
earlier, but dependencies must pass before declaring the dependent phase complete.

### Phase 0 — Requirements, decisions, and session protocol
- Scope: review the source, clarify priorities, record this phased plan and handoff.
- Exit: decisions and remaining questions documented; planning committed/pushed.
- Status: complete; planning commit `f654136` was pushed and remote-verified.

Roadmap reordered in session 003: phases 1–9 deliver the independent web panel;
phases 10–11 begin Rust development in the prepared environment; phases 12–21
integrate and validate real host operations. Phase numbers below replace the prior
session's numbering. All phases after 1 are pending unless explicitly noted.

### Phase 1 — Web-only foundation
- Status: in progress; baseline and runnable panel scaffold complete. Web-only
  CI/static checks and actual PHP 8.3 validation remain.
- Depends on: 0.
- Slices: Laravel 13 in `panel/`, Livewire 4, Tailwind 4/Vite, SQLite and DB queue
  defaults, lockfiles, local run instructions, formatting and web-only CI.
- Exit: panel boots, migrations/tests/asset build pass with installed local tools;
  PHP 8.3/8.4 CI checks use documented versions. No Rust job gates web CI.

### Phase 2 — Administrator onboarding, authentication and MFA
- Depends on: 1.
- Slices: local one-time administrator bootstrap, login, TOTP/recovery codes,
  sessions, rate limits, CSRF and reauthentication for consequential changes.
- Exit: setup/login/recovery work; no public registration or default password;
  unauthenticated users cannot invoke administrative actions.

### Phase 3 — PHP agent abstraction and initial operation schemas
- Depends on: 1.
- Slices: agent-client interface, versioned request/response schemas, request IDs,
  errors/capabilities, operation catalog, deterministic fixtures, fake and
  unavailable adapters. Define correlation, retries and cancellation semantics.
- Exit: PHP validates contracts; fakes are local/test-only and visibly simulated;
  unavailable mode fails closed. Rust compatibility is deferred to phase 11.

### Phase 4 — Panel tasks, database queue and audit
- Depends on: 2, 3.
- Slices: persisted intent, state transitions, dedicated DB worker, bounded/redacted
  output, actor attribution, progress views, leases/idempotency and interruption
  tests using deterministic fake operations.
- Exit: refresh/disconnect preserves task history; authorization and redaction
  pass. Real agent crash/reconciliation evidence is deferred to phases 11 and 15.

### Phase 5 — Panel shell, dashboard and agent availability
- Depends on: 2, 3, 4.
- Slices: responsive/accessibility baseline, navigation, overview cards, task/audit
  history, CPU/RAM/disk/uptime and service views with simulated/stale/offline states.
- Exit: useful authenticated UI without a running agent; no fake metrics presented
  as live production readings; polling and database writes are bounded.

### Phase 6 — Application, source and secret configuration
- Depends on: 2, 3, 4.
- Slices: app inventory, runtime selection, public/private Git repository/ref
  configuration, encrypted credential references, environment/secrets, domains,
  persistent-data/health settings and deployment configuration validation.
- Exit: authorized CRUD and secret masking work; configuration remains distinct
  from agent-provisioned resources. No Linux users or source builds created yet.

### Phase 7 — Deployment workflows and application operation UI
- Depends on: 4, 5, 6.
- Slices: Laravel deployment wizard, preflight/confirmation, task progress, history,
  logs, restart/rollback requests, worker/scheduler settings and migration policy.
- Exit: fake success/failure/timeout flows are meaningfully tested; unavailable
  agent cannot produce a successful real deployment. Code/data rollback distinction
  is clear and uploads/secrets are never exposed in task output.

### Phase 8 — Server administration configuration and views
- Depends on: 4, 5, 6.
- Slices: domains/TLS requests, managed-service/package views, schedules, firewall
  status, database and backup settings, version/capability-aware unavailable states.
- Exit: validated panel data/actions and simulated UI flows work without direct
  privileged execution. Actual provisioning, renewal and restore remain deferred.

### Phase 9 — Web milestone, quality checks and integration handoff
- Depends on: 1–8.
- Slices: web regression, authorization/CSRF/MFA checks, responsive/accessibility
  review, queue/audit failure tests, schema/fixture review, documented adapter map
  and instructions for the future agent environment.
- Exit: web-only CI passes and the panel is independently runnable; contracts and
  acceptance cases are ready for Rust. This is a web milestone, not public beta.

### Phase 10 — Rust foundation in the prepared environment
- Status: deferred until web milestone and user-provided agent environment.
- Depends on: 9.
- Slices: recheck Rust pin/MSRV/dependencies, scaffold `agent/`, build/test,
  rustfmt/Clippy, agent-specific CI and environment handoff.
- Exit: agent independently builds/checks in that environment; web development
  remains independent of local Rust installation.

### Phase 11 — Real protocol, transport and execution boundary
- Depends on: 3, 10.
- Slices: final framing/version rules, cross-language fixture checks, Unix socket
  peer/permission checks, PHP real adapter, agent resource registry, validated
  dispatch, health/inventory, bounded subprocess runner and restart correlation.
- Exit: PHP/Rust agree on envelopes/errors; unauthorized peers, unknown operations,
  unsafe input/paths and oversized messages fail. No arbitrary root command API.

### Phase 12 — Linux application isolation and resource reconciliation
- Depends on: 6, 11.
- Slices: app-owned users/directories, permissions, secret files, registry mapping,
  safe retries, traversal/symlink handling and interrupted provisioning recovery.
- Exit: identities reconcile reliably; cross-app secret access tests pass on VM.

### Phase 13 — Real public/private Git source lifecycle
- Depends on: 6, 12.
- Slices: unprivileged checkout, HTTPS/scoped tokens and/or SSH deploy keys,
  verified host keys, commit pinning, time/output limits, rotation and deletion.
- Exit: public/private snapshots are reproducible as app user; logs and diagnostics
  do not leak credentials. Confirm Git authentication modes before implementation.

### Phase 14 — PHP/Laravel provider and transactional Nginx routing
- Depends on: 12, 13.
- Slices: approved PHP packages/extensions, isolated FPM pools, provider interface,
  Composer/build/Artisan as app user, shared storage, worker/scheduler services,
  domains, Nginx templates, staged validation/reload/recovery and static sites.
- Exit: sample Laravel app runs via its FPM pool and HTTP site; failed Nginx/FPM
  changes preserve/recover working config; build scripts receive no root privilege.

### Phase 15 — Real deployment orchestration, logs, health and rollback
- Depends on: 4, 7, 11, 14.
- Slices: release directories, health gates, symlink/traffic switch, retention,
  migrations policy, worker handling, restart/logs, code rollback and partial-failure
  reconciliation including actual worker/agent interruption.
- Exit: UI deploy/redeploy/restart/logs/rollback pass on disposable Ubuntu VM;
  persistent data survives. No automatic database rollback claim.
- Milestone: first complete native Laravel deployment vertical slice.

### Phase 16 — Real TLS lifecycle
- Depends on: 8, 15.
- Slices: provider abstraction, Let's Encrypt staging/HTTP-01, renewal/expiry,
  uploaded certificate/key validation, transactional activation and key storage.
- Exit: real-DNS HTTPS/renewal pass; failure preserves working certificates.

### Phase 17 — Real services, packages and scheduled jobs
- Depends on: 8, 12, 15.
- Slices: allowlisted packages with package-manager locking, managed services,
  app-user timers/jobs and logs, read-only firewall status, safe failure/retry.
- Exit: app-scoped jobs and managed actions execute correctly; package operations
  coordinate; unmanaged services and arbitrary root commands are rejected.

### Phase 18 — Application database provisioning
- Depends on: 8, 12, 15; engine decision required.
- Slices: first engine module, isolated app databases/users, credentials/rotation,
  connection checks, import/export constraints and explicit deletion semantics.
- Exit: Laravel app uses a provisioned database; cross-app database access fails.

### Phase 19 — Backup, restore and recovery
- Depends on: 15, 16, 17; include 18 for managed databases.
- Slices: panel DB/config, agent registry, keys, app metadata/data and databases;
  consistency/retention, encrypted backup/key custody, disk-full handling and runbook.
- Exit: separate-VM restore verifies access, identities, secrets and app health.

### Phase 20 — Packages, installer, signed releases and upgrades
- Depends on: 2, 16, 19; signing/domain/hosting decisions required.
- Slices: `hostnur`/`hostnur-agent` Debian packages, systemd units/permissions,
  idempotent clean-host installation, verified bootstrap, secure local onboarding,
  release CI/artifacts/signatures, version compatibility, pre-upgrade backups,
  migration/health recovery and signed APT hosting before wider distribution.
- Exit: clean Ubuntu installation, verified artifacts, data-preserving upgrade
  and failed-upgrade recovery pass. Each substantial package/release task is a
  separate session; no unauthenticated public setup page.

### Phase 21 — Integrated beta validation and release gate
- Depends on: 1–20 and disposable VM access.
- Slices: full cross-process/VM regression, privilege/path/socket abuse cases,
  app isolation, crashes/disconnects, Nginx/TLS failures, SQLite contention,
  signed release verification, restore/recovery drill and operator documentation.
- Exit: every section 7 beta requirement has actual evidence; fakes do not satisfy
  host-operation gates. Resolve blockers and make an explicit beta release decision.

### Phase 22 — First-party module lifecycle
- Depends on: 21; provider interfaces introduced in phase 14.
- Slices: manifests, capabilities/dependencies/compatibility, version/install state,
  reviewed upgrade/uninstall plans, preservation of user data and module catalog.
- Exit: module lifecycle passes; metadata is not treated as a root-code sandbox.

### Phase 23 — Python, generic services and additional runtimes
- Depends on: 22.
- Slices: Python venv/systemd first, generic non-root build/start services, then
  Node.js/Go/Java/.NET according to demand; each provider gets separate small slices.
- Exit: advertised providers pass shared deployment/isolation/health contracts.

### Phase 24 — Extended monitoring, backups and integrations
- Depends on: 22; scope selected from beta feedback.
- Slices: bounded metrics/alerts, remote backups, second DB engine, DNS-01 providers,
  archive uploads and deployment hooks with credential/recovery/limit documentation.
- Exit: each enabled integration passes its tests and actual compatibility gates.

### Phase 25 — Opt-in AI assistance
- Depends on: 21 and stable authorized operation interfaces.
- Slices: provider-neutral setup, redaction/spending limits, evidence-grounded
  diagnostics/plans, required approvals, structured operations, verification/audit.
- Exit: useful approved operations without arbitrary privileged commands; core
  features remain useful without an AI key.

### Phase 26 — Expanded platforms and ecosystem
- Depends on: 21–23 and demonstrated demand.
- Slices: validate Ubuntu 26.04/arm64; separately scope team RBAC, optional Docker,
  multi-server management and reviewed/signed third-party modules.
- Exit: each advertised capability has agreed scope and tested support matrix.
- Deferred: full email hosting, browser root terminal, unrestricted marketplace,
  unreviewed autonomous AI and untested broad Linux support.

## 6. Open decisions and recommended starting defaults

These are not user-confirmed decisions. Resolve each before its implementation;
ask only when the choice materially changes scope, cost, access, or product behavior.

| Decision | Proposed direction / outstanding question | Resolve by |
|---|---|---|
| Web framework/tool compatibility | Use installed tools without upgrades; verify actual dependency resolution/lockfiles; PHP 8.3 checks belong in CI | Phase 1 |
| Rust environment | User supplies prepared environment; recheck stored pin/MSRV before implementation | Phase 10 |
| Protocol framing and registry | Proposed length-prefixed JSON on UDS and agent-owned registry; finalize with Rust implementation | Schema in 3; transport in 11 |
| Git credentials | Read-only deploy keys and/or scoped HTTPS tokens, verified host keys, no credential in repository URL; confirm providers/auth modes | Config in 6; execution in 13 |
| App database engine | Proposed MariaDB first for Laravel; confirm MariaDB versus PostgreSQL | Before 18 |
| Secret and backup keys | Established encryption primitives, key outside DB, rotation and encrypted off-host recovery-key handling | Config in 6; recovery in 19 |
| Existing-host installer | Propose clean supported VPS beta; no implicit adoption/overwrite of unmanaged services | Phase 20 |
| Product domain | Official panel/install/docs domain from user | Before real TLS in 16 and installer in 20 |
| Release signing/hosting | GitHub Releases initially; select signing identity and signed APT hosting before wider distribution | Phase 20 |
| VM access and DNS | Disposable Ubuntu 24.04 amd64 VM, test DNS and separate restore VM | Host checks from 12; TLS in 16 |
| License/public beta | Confirm license and distribution terms before publishing releases | Phases 20–21 |

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

### Session 004 — Web panel scaffolding (2026-10-11)

- Completed Laravel scaffold `panel/` (upstream skeleton v13.11.0), locked Laravel
  13.35.0, Livewire 4.4.7, Tailwind 4.3.3 and Vite 8.3.4 with existing tools.
- Added a typed full-page Livewire foundation component and shared Blade layout,
  Hostnur identity, system-font styling and a visibly disconnected-agent message.
  No administrator or host-operation functionality is implied by this page.
- SQLite, database-backed sessions/cache/queue and the baseline user/cache/job
  migrations are configured. Local database and application key were initialized;
  secrets/state/dependency directories/builds remain ignored by Git.
- Added PHP platform 8.3.0 resolution, dependency lockfiles, Node 24 engine metadata,
  project guidance and local setup/run/check instructions in `panel/README.md`.
- Removed the template's default-user seed; administrator creation belongs to the
  forthcoming one-time bootstrap flow, without default credentials.
- Verified `php artisan db:seed --no-interaction` and `php artisan db:show --counts
  --no-interaction`: users table remains empty. Reran formatter and baseline tests
  after the seeder edit; both passed.
- Laravel skeleton `AGENTS.md` required project-local Laravel Boost setup; installed
  Boost 2.10.3 as a dev dependency and generated framework guidance. Replaced its
  prerequisite-install bootstrap instructions with Hostnur environment constraints.
- Used `composer create-project laravel/laravel panel "^13.0" --no-install
  --no-scripts --no-interaction`, then `composer require laravel/boost --dev
  --no-interaction` and `composer require livewire/livewire "^4.0" --no-interaction`.
- Verification passed: `composer validate` (license warning),
  `composer check-platform-reqs`, `composer audit`, `php artisan migrate
  --no-interaction`, `php artisan migrate:status --no-interaction`,
  `php artisan test --compact` (2 baseline tests / 2 assertions),
  `php artisan route:list --except-vendor`, `php artisan view:cache --no-interaction`,
  `php artisan queue:work --once --no-interaction`, `vendor/bin/pint --dirty
  --format agent`, full `vendor/bin/pint --format agent`, `npm audit` and
  `npm run build`. Cleared cached views afterward for development.
- Confirmed effective config: `app.name=Hostnur`, `database.default=sqlite`,
  `queue.default=database`. Application Composer/npm audits reported no advisories.
- Real local HTTP smoke: root page, `/up`, Livewire JavaScript and built Tailwind
  stylesheet all returned 200. Temporary server/process group stopped afterward.
- Reproducible installs verified with `composer install --no-interaction` and
  `npm ci --no-fund`, followed by `npm run build`. First `npm ci` exposed a missing
  optional WASI dependency in the generated npm lock; `npm install --include=optional
  --no-fund` repaired the lock, then clean installation/build passed without tool
  upgrades or advisory-check bypasses.
- Initial migration failed because a no-script scaffold does not create SQLite;
  created the ignored database and reran successfully. Setup documentation includes
  this prerequisite. Removed unused template font fetching from Vite.
- `composer validate --strict` reports the deferred license warning and does not
  pass yet; normal validation passes with that warning. License remains a user
  decision. Existing Composer-tool diagnostics are distinct from application audits.
- No runtime/global-tool installations or upgrades; Rust, actual PHP 8.3 runtime,
  browser visual checks and disposable-VM host gates remain unverified/deferred.
- Reviewed staged source/documentation and intended file list;
  `git diff --cached --check` passed. Git ignores local `.env`, SQLite, vendor,
  node_modules and generated asset manifests.
- Publication: commit/push at session close; verify the commit on `origin/main`.

### Session 003 — Web-first roadmap revision (2026-10-11)

- User requested web panel first, Rust later in a prepared environment, and no
  additional tooling installations in the current environment.
- Reordered phases 0–26: web phases 1–9, deferred Rust phases 10–11, real host
  integration/release phases 12–21, subsequent capabilities 22–26.
- Defined the local/test fake adapter, production-unavailable behavior and clear
  simulated-data labeling; real deployment/VM gates remain required for beta.
- Replaced local tool provisioning with panel scaffolding using installed tools.
  Existing Composer diagnostic findings remain recorded; no upgrades or installs
  were performed. Project-local panel dependencies are the next build step.
- Updated `docs/development-toolchain.md` and `README.md` to match this sequence.
- Historical session notes below record prior decisions; this session's steering
  supersedes their tool-provisioning and Rust-first prerequisites.
- Validation: reviewed documentation, phase dependencies and references;
  `git diff --check` passed. No application tests apply to this planning revision.
- Publication: commit/push at session close; verify the commit on `origin/main`.

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

Begin **phase 1, slice 3: web-only CI and static checks**. Add GitHub Actions for
locked Composer/npm installation, PHP 8.3/8.4, Node 24, migrations/tests, PHP
format/style and asset build. Use maintained tools in CI without modifying local
tooling. Confirm what runs on pull requests/pushes and inspect actual CI outcomes;
do not claim PHP 8.3 validation until its job passes. Keep Rust out of web CI.
Handle the known license-warning status honestly until the user selects a license.
Then update this handoff, commit, push and stop. Administrator bootstrap/login/MFA
is the following phase, split into small slices.

### Current blockers

- No blocker to local web work; scaffold/dependency resolution/build/tests now pass.
  Prior Composer-tool diagnostic findings remain recorded, and the shared tool is
  not to be changed under the user's environment constraint.
- Actual PHP 8.3 runtime validation awaits CI; strict Composer validation awaits a
  project license decision. Neither is recorded as passing.
- Missing Rust does not block web work; Rust development waits for phase 10 and the
  prepared environment. The existing toolchain pin is metadata for that later work.
- Disposable VM access is unavailable; real install/systemd/TLS/upgrade/restore
  gates remain unverified until supplied.
- Official domain, Git authentication details, database engine, release signing,
  and license await the decision checkpoints listed in section 6.
