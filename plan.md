# Hostnur — implementation plan and session handoff

Last updated: 2026-10-13.

**Development handoff:** this plan records phases **1–9 complete** (web milestone)
and **phase 10 complete** (Rust foundation) with blocked manual/VM checks retained
in sessions 023 and 025. The next implementation slice is **P11.1** (cross-language
contract), which needs the prepared agent environment (now a Lima Ubuntu 24.04 VM).
Detailed phases 10–20 are expanded directly under their headings in section 5.
Section 9 remains the web implementation playbook. Host/package designs below are
proposed work, not implemented features or verified host behavior.

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
| Local web port | 9000 (`php artisan serve --port=9000`) | User report, session 005 |

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

Status values: **pending**, **in progress**, **deferred**, **blocked**, **complete**.
Phases 1–9 are recorded complete, with the outstanding manual/VM checks retained
in session 023. Phases 10 onward are planned/deferred unless explicitly noted.
Writing a detailed plan does not complete an implementation phase. Dependencies
must pass before declaring a dependent phase complete.

### Phase 0 — Requirements, decisions, and session protocol
- Scope: review the source, clarify priorities, record this phased plan and handoff.
- Exit: decisions and remaining questions documented; planning committed/pushed.
- Status: complete; planning commit `f654136` was pushed and remote-verified.

Roadmap reordered in session 003: phases 1–9 deliver the independent web panel;
phases 10–11 begin Rust development in the prepared environment; phases 12–21
integrate and validate real host operations. Phase numbers below replace the prior
session's numbering. Phases 10 onward remain planned/deferred unless explicitly noted.

### Phase 1 — Web-only foundation
- Status: complete; scaffold, local checks and actual PHP 8.3/8.4 CI passed.
  Session 005 records the successful run and repaired-lock commit.
- Depends on: 0.
- Slices: Laravel 13 in `panel/`, Livewire 4, Tailwind 4/Vite, SQLite and DB queue
  defaults, lockfiles, local run instructions, formatting and web-only CI.
- Exit: panel boots, migrations/tests/asset build pass with installed local tools;
  PHP 8.3/8.4 CI checks use documented versions. No Rust job gates web CI.

### Phase 2 — Administrator onboarding, authentication and MFA
- Status: complete. One-time CLI bootstrap (session 006), P2.2 login/logout
  (session 008), P2.3 MFA enrollment (session 009), P2.4 MFA challenge/enforcement
  (session 010), P2.5 recent-auth/security settings (session 011) and P2.6 local
  recovery/runbook (session 012) all passed local checks and real PHP 8.3/8.4 CI.
- Depends on: 1.
- Slices: local one-time administrator bootstrap, login, TOTP/recovery codes,
  sessions, rate limits, CSRF and reauthentication for consequential changes.
- Exit: met — setup/login/recovery work; no public registration or default
  password; unauthenticated users cannot invoke administrative actions.

### Phase 3 — PHP agent abstraction and initial operation schemas
- Status: complete. P3.1 (DTOs/catalog/schemas/validation), P3.2 (AgentClient
  interface, unavailable adapter and mode guard), P3.3 (read-only fake snapshots),
  P3.4 (durable fake execution) and P3.5 (contract regression + compatibility
  notes) all passed local checks and real PHP 8.3/8.4 CI.
- Depends on: 1.
- Exit: met — PHP validates contracts; fakes are local/test-only and visibly
  simulated; unavailable mode fails closed. Rust compatibility is deferred to
  phase 11.

### Phase 4 — Panel tasks, database queue and audit
- Status: complete. P4.1 (schema/models/states), P4.2 (authorized intent/approval),
  P4.3 (worker/outbox/leases), P4.4 (progress/log persistence), P4.5
  (failure/cancel/retry) and P4.6 (audit UI + auth integration) all passed local
  checks and real PHP 8.3/8.4 CI.
- Depends on: 2, 3.
- Exit: met — refresh/disconnect preserves task history; authorization and
  redaction pass. Real agent crash/reconciliation evidence is deferred to phases 11
  and 15.

### Phase 5 — Panel shell, dashboard and agent availability
- Status: complete. P5.1 (protected shell/components), P5.2 (task/audit screens),
  P5.3 (dashboard adapter states) and P5.4 (responsive/accessibility) all passed
  local checks and real PHP 8.3/8.4 CI.
- Depends on: 2, 3, 4.
- Exit: met — useful authenticated UI without a running agent; unavailable values
  are not presented as zero; polling and database writes are bounded.

### Phase 6 — Application, source and secret configuration
- Status: complete. P6.1–P6.5 all passed local checks and real PHP 8.3/8.4 CI.
- Depends on: 2, 3, 4.
- Exit: met — authorized CRUD and secret masking work; configuration remains
  distinct from agent-provisioned resources. No Linux users or source builds
  created.

### Phase 7 — Deployment workflows and application operation UI
- Status: complete. P7.1–P7.5 all passed local checks and real PHP 8.3/8.4 CI.
- Depends on: 4, 5, 6.
- Exit: met — fake success/failure/timeout flows are meaningfully tested; an
  unavailable agent cannot produce a successful real deployment; code/data rollback
  distinction is clear and uploads/secrets are never exposed in task output.

### Phase 8 — Server administration configuration and views
- Status: complete. P8.1–P8.6 all passed local checks and real PHP 8.3/8.4 CI.
- Depends on: 4, 5, 6.
- Exit: met — an authenticated admin can configure/request all agreed views through
  the same validated workflow in fake mode; unavailable/unsupported mode remains
  closed; every screen states what is configured versus simulated versus observed.

### Phase 9 — Web milestone, quality checks and integration handoff
- Status: complete. P9.1–P9.5 all passed local checks and real PHP 8.3/8.4 CI.
- Depends on: 1–8.
- Slices: web regression, authorization/CSRF/MFA checks, responsive/accessibility
  review, queue/audit failure tests, schema/fixture review, documented adapter map
  and instructions for the future agent environment.
- Exit: met — web-only CI passes and the panel is independently runnable; contracts
  and acceptance cases are ready for Rust. This is a web milestone, not public beta.

Detailed implementation, file maps, slice IDs, schemas and test requirements for
phases 1–9 are in **section 9** below. Use that playbook to continue development.

### Phase 10 — Rust foundation in the prepared environment
- Status: **complete (P10.1–P10.5)**, local checks passed in the prepared Lima
  Ubuntu 24.04 aarch64 VM; real CI run pending push. Session 025.
- Depends on: 9.

#### Objective and entry gate

Create an independently buildable Rust service foundation in the same monorepo.
Reuse the web milestone's existing operation catalog, DTO responsibilities and
fixtures; do not re-scaffold the panel or replace its task/authentication layers.
Phase 9 is recorded complete, but its blocked browser review and later VM gates
remain outstanding. Rust builds alone do not satisfy those gates.

Before implementation, identify the user-approved development environment, target
Linux architecture, available compiler/linker and disposable-test VM access. Do
not install Rust or system packages in the current web environment. A prepared
Rust workstation is sufficient for this phase's unit/build checks; root/systemd
and installation tests require separate disposable Ubuntu 24.04 amd64 VMs later.

#### Proposed crate and dependency approach

```text
agent/Cargo.toml              Package metadata, edition, MSRV and minimal dependencies
agent/Cargo.lock              Committed dependency resolution for the agent executable
agent/src/main.rs             CLI/process entry point and orderly startup/shutdown
agent/src/lib.rs              Testable core, independent of daemon process startup
agent/src/config.rs           Typed agent-owned configuration and validation
agent/src/error.rs            Stable internal errors mapped to contract codes later
agent/tests/                 Integration tests that do not require root by default
.github/workflows/agent.yml   Separate agent build/lint/test checks
docs/agent-development.md     Environment, commands, targets and blocked host checks
```

Create transport/registry/operations modules when phase 11 uses them, not empty
handler trees for every future runtime. Start with safe Rust and reject unnecessary
`unsafe` in project code; use established safe syscall wrappers when later needed.
Suggested dependencies, introduced only when used: serde/serde_json for contracts,
an error library, structured logging, and a bounded async runtime if socket/process
concurrency justifies it. Select storage and process-control crates in phase 11.
Record each dependency's role, license, native build requirements and MSRV impact.

The existing Rust 1.99.0 / edition 2024 / MSRV 1.99 direction is a stored proposal,
not proof that the prepared environment has it. Recheck the pin and crate support;
choose one compatible stable toolchain, record the decision and use it in CI.
Changing MSRV requires a tested reason, not merely choosing whatever is newest.
Retain the Ubuntu 24.04 amd64 deployment target; a successful build against a newer
host's glibc does not automatically run on that target. Release-target compatibility
and deterministic packaging are finalized in phase 20.

#### Startup and configuration behavior

- Provide `--version`, help and a configuration-check/diagnostic mode with safe
  output. Configuration check must not provision users, write Nginx, install
  packages, contact Git or start hosted applications.
- Parse a documented, typed TOML configuration; reject unknown/invalid settings,
  unsupported limits and unsafe state/socket paths rather than use permissive
  defaults. Agent configuration is distinct from hosted apps' environment variables.
- Development checks use temporary directories and explicit non-root configuration.
  No production root socket or mutation handler is activated by this scaffold.
- Structured logs include service version, safe error code and correlation IDs.
  Never dump configuration, environment, future request bodies or credentials.
- Define signal handling and shutdown deadlines. A clean shutdown stops accepting
  new work; later phases decide how accepted work is drained or journaled.
- Keep executable startup thin so validation and lifecycle behavior can be tested
  without launching a root daemon or manipulating the development host.

#### Small implementation slices

| Slice | Deliverable | Acceptance evidence |
|---|---|---|
| P10.1 Environment/compatibility baseline | Prepared-environment inventory, Rust pin/MSRV and target decision | Exact versions/commands recorded; no current-host installation |
| P10.2 Minimal crate/CLI | Independent binary/library, lockfile, help/version | Locked build/test works; panel needs no Rust to run |
| P10.3 Typed configuration/errors | Config parser, safe diagnostics and startup checks | Missing/malformed/unknown/unsafe config fails without side effects |
| P10.4 Lifecycle/logging | Signals, shutdown and bounded safe logging | Controlled shutdown and invalid startup are observable/testable |
| P10.5 Agent-only CI/handoff | Format/Clippy/tests and target build documentation | Real CI run passes; root/VM-dependent checks explicitly separated |

Suggested checks once code exists: `cargo build --locked`, `cargo test --locked`,
`cargo fmt --all -- --check`, and `cargo clippy --locked --all-targets -- -D warnings`.
Check the declared MSRV in CI if a newer compiler is used as the main development
toolchain. Web CI remains independently runnable; coordinated release gates come
later. Use existing fixtures, not test files that only assert scaffold strings.

**Exit gate:** independently reproducible Rust build/tests/style/config lifecycle
checks pass in the prepared environment and CI; exact blocked host checks and the
first P11 slice are documented. No privileged operation support is advertised.

### Phase 11 — Real protocol, transport and execution boundary
- Status: **complete (P11.1–P11.6)** in session 026. The Rust agent builds/serves the
  real framed, peer-checked transport with a durable journal and bounded process
  primitive; the panel has the `RealAgentClient`. Real systemd/socket integration
  verified on the arm64 Lima VM (amd64 is CI-only for now). Privileged mutation
  handlers remain unimplemented by design (phases 12+).
- Depends on: 3, 10.

#### Objective and contract migration

Implement the real counterpart of the existing `AgentClient` responsibilities:
capabilities/snapshot/submit/inspect/cancel. Preserve `TaskService`, `RunTask`,
approval fingerprints, leases, event cursors and fake/unavailable modes. Future
implementation must inspect the actual completed `app/Agent` DTOs and
`contracts/v1` artifacts before selecting Rust types; this planning session only
uses the responsibilities recorded in this file.

Proposed transport: local Unix stream socket with a four-byte unsigned big-endian
length prefix followed by UTF-8 JSON, capped at the agreed message limit before
allocation. Finalize whether one request/response per connection or bounded
multi-request connections is simplest; document correlation and error behavior.
Reject zero/oversized frames, partial/time-out reads, malformed UTF-8/JSON, duplicate
keys, unknown fields, invalid numeric bounds and unsupported protocol versions.
Do not turn EOF midway through a message into an accepted operation.

Capabilities report protocol/schema version, agent build, stable managed-host identity,
boot identity, supported operations/runtime IDs, limits and retry/cancel semantics.
Only tested handlers are advertised. A successful connection is not evidence that
every web catalog operation exists. Unsupported versions fail before mutation;
upgrade compatibility is explicit rather than silently interpreting another schema.

#### Local process identity and service ownership

Suggested packaging-independent locations to ratify before use:

| Resource | Proposed location / owner |
|---|---|
| Socket | `/run/hostnur/agent.sock`; root-owned directory, restrictive socket group |
| Agent config | `/etc/hostnur/agent.toml`; root-owned, not web writable |
| Registry/journal | `/var/lib/hostnur/agent/`; root-only DB and sidecar files |
| Application root | `/opt/hostnur/apps/`; managed UUID layout finalized in phase 12 |

Use an explicit expected Unix peer UID allowlist and kernel peer credentials,
not a JSON `user_id`, PID claim or group membership alone. Panel and queue-worker
OS identities must be documented. A typical socket mode is 0660 with a dedicated
panel-access group; do not add hosted app users to it. Authenticate the agent endpoint
from the panel side as well, using trusted root-owned paths/ownership and expected
server peer credentials where supported. Reject socket substitution/symlinks.
No TCP listener, public HTTP admin endpoint or shared default token is needed.

Develop handlers unprivileged against temporary roots. Root-running integration
occurs only on the disposable VM. Select safe systemd service restrictions against
the actual required capabilities: a hardening directive is useful only if it does
not silently prevent UID switching, filesystem access or approved service control.
Do not claim an arbitrary plugin manifest makes root code sandboxed.

#### Registry, durable execution and dispatch

- Separate agent-owned registry storage from panel SQLite. Record managed resource
  UUID, kind, provider, owning UID/GID, approved paths/service IDs, revision and
  lifecycle state. Caller-supplied paths/unit names are never authoritative.
- Agent-owned execution journal records request/idempotency IDs, canonical payload
  fingerprint, resource/revision, state/checkpoints, event sequence and safe result.
  Use database constraints for identity conflicts. Same ID + same intent returns
  the original receipt/state; same ID + different intent is a conflict.
- Persist acceptance before returning a receipt or starting mutations. Work is owned
  by the agent, not the socket connection. Browser, worker or client disconnect must
  not implicitly cancel accepted execution.
- Distinguish received/accepted/running/terminal/unknown. On restart, reconcile the
  checkpoint against actual managed state. Never convert all interrupted work into
  success or automatically repeat a possibly completed mutation.
- Serialize conflicting operations by resource and global resources such as APT/
  Nginx reload. Keep storage transactions short; leases/locks cannot span a long
  child process while blocking all status inspection.
- Mutation authorization in PHP remains required. The agent independently validates
  the approved operation's safety, resource namespace, identity, revision and bounds;
  it does not trust browser-provided actor metadata as root authorization.

#### Sensitive inputs and subprocess boundary

Task requests and failed-job payloads continue to contain secret references only.
Design a documented sensitive-input delivery envelope on the authenticated local
channel, separate from durable intent: exact execution/request ID, resource,
reference/version, bounded material and expiry. Rust validates the binding and
accepts only fields required by that operation. Keep plaintext out of journals,
traces, errors, argv and environment diagnostics. Use dummy material in fixtures.

Do not make the agent fetch arbitrary HTTP secret URLs or silently decrypt/read the
panel database. For crash recovery, arrange controlled re-delivery of the approved
version or an explicitly encrypted agent credential store; choose/document the key
custody and paused-input state. Missing credentials must not trigger blind operation
resubmission or substitution with a newer unapproved secret version.

The subprocess runner takes an agent-selected executable and validated argument
array, explicit UID/GID/group policy, working-directory handle, minimal environment,
deadline, output cap and process-group termination policy. Root selects allowlisted
tools/arguments; arbitrary build commands are allowed only in an app-user runner
introduced later. Drain stdout/stderr without deadlock; redact before persistence.
Timeout/cancel kills the process group, records the actual outcome and does not
pretend package-manager/database effects were undone. No `sh -c` root executor.

#### Slices and verification

| Slice | Deliverable | Required proof |
|---|---|---|
| P11.1 Cross-language contract | Rust DTOs/parser, framing/version decisions, shared fixtures | PHP/Rust agree on all valid/invalid cases and limits |
| P11.2 Socket/peer boundary | Endpoint ownership, UID checks, connection limits/timeouts | Unauthorized peers/substituted sockets/partial frames fail before dispatch |
| P11.3 Registry/journal | Durable IDs, acceptance, dedupe, event cursor and restart state | Lost receipt/reconnect/restart cannot duplicate accepted work |
| P11.4 Safe process primitive | Unprivileged runner tests and fixed privileged-tool policy | Timeout/output flood/process tree/argv/env cases stay bounded |
| P11.5 PHP real adapter | Bind through existing task pipeline, capability/offline UI mapping | Auth/MFA/approval persist; production never falls back to fake |
| P11.6 VM service integration | systemd/socket permissions, live metrics, read-only health | Real panel/worker connects; app users cannot; safe service restart works |

Negative tests include cross-resource IDs, traversal/NUL/options, stale revisions,
duplicate identity conflicts, mismatched sensitive inputs, unsupported capabilities,
output floods, dropped receipts, crash windows and revocation before submission.
After acceptance, system-owned read-only reconciliation continues even if the
requesting user's approval expires; new mutations still require fresh authority.

**Exit gate:** real PHP/Rust contract/socket/identity and durable execution tests pass,
with privileged cases evidenced on the VM. The agent supports a narrow tested set
of read-only operations and safe primitives; later handlers remain unsupported.

### Phase 12 — Linux application isolation and resource reconciliation
- Status: **complete (P12.1–P12.5)** in session 027, verified with a root-run
  harness on the arm64 Lima VM. Privileged provisioning runs via the harness, not
  yet the hardened systemd service.
- Depends on: 6, 11.

#### Objective and resource identity

Turn an approved application UUID/revision into an isolated Linux identity and
managed filesystem layout. The panel's configuration already exists; agent-observed
provisioning state is a separate record. Do not assign a real Linux user or ready
state because the earlier fake deployment succeeded.

Use an agent-selected account namespace, for example a bounded `hn_<uuid-fragment>`
name, with collision handling and persistent UUID -> username/UID/GID mapping.
Application display name/slug is not an account name, path or privilege selector.
Accounts have no interactive login/sudo and do not join the panel socket group.
Existing unmanaged accounts/directories must not be silently adopted or overwritten.
UID/GID mismatch or reuse after external deletion is drift requiring reconciliation,
not permission to chown another owner's data into the app.

Provisioning can be an internal step of `application.deploy`. If a separate public
prepare operation is necessary, add a strictly typed schema/catalog/capability and
approval path; do not expose generic `user.add` or root filesystem-write commands.

#### Filesystem layout and permission matrix

Proposed stable layout, finalized with phase 14's provider:

```text
/opt/hostnur/apps/<application-uuid>/
  releases/<release-uuid>/       App-owned staged releases; published code policy explicit
  shared/                       Persistent storage/uploads and app-owned writable data
  shared/secrets/                Restricted app-specific runtime environment material
  current                       Agent-controlled activation pointer, introduced in phase 15
/var/lib/hostnur/agent/          Root-owned registry/journal, never app writable
```

Keep trusted namespace/activation/config parents agent/root-controlled. Writable
release/shared descendants are untrusted because application code can change them.
No hosted app can alter registry, agent config/socket, panel key/DB, another app's
private tree, generated root service configs or activation ownership metadata.

| Principal | Intended access |
|---|---|
| Agent | Validated management of registered resources; no unvalidated descendant traversal |
| App A | Own runtime files/secrets/shared data only; no sudo, peer B secrets or socket |
| App B | Equivalent own-app access; no app A private data |
| Panel/worker | Own panel state and authenticated agent IPC; no direct root provisioning |
| Nginx worker | Explicit public assets/traversal and FPM upstream socket only, not private app trees |

Document concrete owner/group/mode/ACL choices per directory and file. Environment
and credentials are typically 0600/0400 to the intended owner, not group/world
readable. Do not add Nginx or every application to broad shared groups merely to
make static routing work; grant the minimum public-subtree/socket access and test
private storage remains inaccessible. Root-readable credentials remain outside
app-owned staging unless that app specifically needs them.

#### Safe filesystem and UID switching approach

- Resolve root-managed IDs to paths internally. Validate relative identifiers,
  reject traversal/control/NUL/absolute input and limit path/depth/count/bytes.
- Use directory-file-descriptor-based operations and no-follow/beneath-resolution
  primitives appropriate to Linux. `canonicalize()` followed by a later ordinary
  open is not sufficient against app-controlled symlink/rename races.
- Reject unexpected symlinks, devices, FIFOs, hard-link ownership surprises and
  ancestor replacements during privileged writes/ownership changes. No recursive
  root `chown` over an arbitrary app-supplied tree or tar extraction as root.
- Root-generated files use staged writes in trusted parents, bounded permissions,
  same-filesystem atomic replacement and durability synchronization when required.
- Child-process identity setup clears supplementary groups before setting GID/UID,
  drops unnecessary capabilities and uses an explicit minimal environment/working
  directory. Test the resulting identity, not merely the command's requested UID.
- Resource changes are journaled before/after side effects. A crash can leave a
  user without directories; reconcile that exact managed user, not allocate a
  second identity. Rollback removes only proven newly created empty resources.

Suggested lifecycle: reserved -> account created -> layout prepared -> ready,
with partial/failed/drifted states and recorded recovery actions. Keep Linux-user
allocation/management serialized where needed. Panel archive does not automatically
delete host data, kill unrelated processes or free an account while tasks use it.

#### Slices and test matrix

| Slice | Deliverable | Required proof |
|---|---|---|
| P12.1 Registry/namespace policy | UUID/account/path ownership rules and lifecycle | Invalid IDs/collisions/unmanaged resources are refused |
| P12.2 Isolated account creation | No-login account/group and reliable UID switching | Children actually run as intended UID with no inherited extra groups |
| P12.3 Directory/secret provisioning | Trusted parents, private data and bounded file writes | Modes/ACLs meet the access matrix; secret output stays redacted |
| P12.4 Crash/drift reconciliation | Resume partial provisioning and detect identity changes | Repeated requests keep one identity and preserve existing data |
| P12.5 Host isolation/race tests | Disposable-VM app A/B plus malicious path fixtures | Cross-app secret/socket/config access and symlink races fail |

Run meaningful tests as the actual app UIDs, not just root checking mode bits.
Include reused UID, unexpected pre-existing directory, missing registry row,
concurrent provisioning, malicious symlink swaps, disk-full/write interruption,
and cleanup after a partial failure. Preserve evidence of files/accounts created
and removed in each VM scenario so cleanup cannot conceal leaked resources.

**Exit gate:** two independently provisioned apps pass the full permission matrix;
repeat/crash recovery preserves identity/data; no caller-controlled path crosses
the managed root. Publish the layout/ownership contract for Git and PHP providers.

### Phase 13 — Real public/private Git source lifecycle
- Status: **complete for the chosen scope** (public HTTPS + private HTTPS scoped
  token) in session 028, verified with a root-run harness on the arm64 Lima VM.
  SSH deploy keys (P13.4) deferred by decision; panel/task integration of real
  source results (P13.6) deferred to phase 15.
- Depends on: 6, 12.

#### Objective and source authority

Connect existing source/credential configuration to genuine bounded Git operations
under the application's Linux identity. Retain the existing URL/ref validation,
immutable app revision and secret-version references, then independently revalidate
them in Rust. A previously simulated connectivity check is not source evidence.

Confirm the required providers and supported authentication before implementing:
public HTTPS, private HTTPS with scoped token, and/or SSH with per-repository deploy
key. Prefer read-only repository scope. No personal all-repository credential is a
default requirement. Custom/private Git hosts require an explicit host/port/network
policy; do not assume all URLs or all internal network targets are approved.

#### Checkout pipeline

1. Resolve app/revision and the approved repository identity, ref and credential
   version. Verify the app identity/layout from phase 12 and acquire its source lock.
2. Resolve host/port through an outbound policy: public defaults exclude loopback,
   link-local/cloud metadata and other forbidden destinations. Explicit operator
   policy can allow a legitimate private Git host. Validate redirects/DNS changes
   against the same policy rather than validating only the original string.
3. Create a unique bounded staging checkout. Run the fixed Git binary as the app
   user with no root privilege, inherited SSH agent, panel environment or global
   credentials/config. Do not reuse an untrusted working tree's executable config.
4. Fetch the approved ref with strict transport settings, deadline and output/disk
   limits. Reject option-like refs and revision-expression tricks; branch, tag and
   full commit forms are distinguished and validated before argument construction.
5. Resolve and persist the full commit ID/repository identity before build handoff.
   Check an expected commit if supplied. Once resolved, retries of that accepted
   source intent use the same commit; they cannot silently move to a newer branch tip.
6. Export/stage release contents with documented symlink/file-type policy. Preserve
   provenance without runtime access to deployment credentials or unsafe `.git`
   configuration. A checked-out symlink is never authority for later root writes.
7. Remove transient credential material and failed staging artifacts according to
   retention rules. Record source success/failure and a safe diagnostic in the task.

A request approving a branch approves resolving that branch at execution time unless
the UI supplies an expected commit. Display that distinction; do not claim a moving
ref is immutable before resolution. Define behavior for unreachable/deleted refs,
unsupported Git object formats, force-pushed branches and annotated tags explicitly.
Shallow fetch is an optimization only if it preserves the required pinned commit.

#### Credential and transport handling

- HTTPS authentication uses a tightly scoped helper/channel, not token-in-URL,
  command-line headers or persisted global `.gitconfig`. Validate TLS and bound
  credential use to the approved host/repository. Reject insecure redirects.
- SSH uses strict known-host verification with an independently obtained approved
  host key/fingerprint. No `StrictHostKeyChecking=no`, first-seen silent trust or
  inheritance of the host operator's private keys/SSH configuration.
- Private keys/tokens arrive via phase 11's sensitive channel. Use restricted
  temporary descriptors/files only if the Git/SSH tool requires them; clear them
  after checkout and on error/cancel. Never persist plaintext in journal, source
  metadata, task payloads, process diagnostics or final release directories.
- Rotation/deletion is versioned. An accepted task cannot automatically consume a
  newly rotated credential without its defined approval/re-delivery policy. Report
  revoked/missing credentials safely and require a fresh request when appropriate.
- Application code is untrusted. Do not run dependency/build scripts while source
  credentials remain available. Read-only per-app credentials limit exposure from
  same-UID app processes; stricter separate deployment identity is a documented
  future option, not an unproven isolation claim.
- Disable unapproved external Git protocols, hooks, credential helpers, filters,
  fsmonitor, submodules and LFS execution. Support submodules/LFS only as explicit
  provider features with independent URL/credential/bounds validation.

#### Artifacts, progress and failure recovery

Proposed source manifest: app/revision/source IDs, sanitized repository identity,
requested ref, resolved commit/object format, staging/release ID, completion time,
mode=real and content/provenance checksum where practical. Credentials are references
only. Retain enough trusted metadata to resume fetch/export without re-running an
arbitrary app-owned Git command or confusing a partial tree with a ready source.

On network/auth failure, preserve the active app and discard/quarantine incomplete
staging only. Retry bounded read/fetch operations if they are known safe; do not
restart an entire deployment or delete a known-good release. Stream bounded redacted
progress, escaping server-supplied error text. Cancellation stops Git/SSH children
and their process group; a partial checkout never becomes deployable.

| Slice | Deliverable | Required proof |
|---|---|---|
| P13.1 Git/source policy | Provider/auth matrix, URL/ref/network restrictions | Unsupported protocols/options/private targets are refused correctly |
| P13.2 Public checkout/provenance | App-user fetch, commit pin, staging manifest | Actual UID, deterministic commit and failure cleanup verified |
| P13.3 Private HTTPS credentials | Scoped secret helper, TLS/redirect checks | Private clone works; no URL/argv/log/release credential leakage |
| P13.4 SSH deploy keys/host trust | Pinned host keys and versioned key delivery | Wrong/rotated host key fails; unrelated host keys cannot be used |
| P13.5 Recovery/rotation/cancel | Journaled partial fetch, revoke/rotate and limits | Interrupted/resumed source does not move commits or expose old keys |
| P13.6 Panel integration | Real source result and safe errors through existing tasks | Real/fake/not-checked remain distinct; no direct PHP Git executor |

Tests use controlled Git HTTP/SSH endpoints and disposable resources, not production
repositories or credentials. Cover auth failure, redirect/rebinding policy, malicious
refs/config/hooks/submodules, large repositories/output, wrong ownership, disk-full,
timeout, process interruption and credential cleanup. Limit claims to tested modes.

**Exit gate:** supported public/private repositories yield reproducible source
snapshots as the intended non-root identity, with pinned commits and verified
transport trust. Secrets do not survive in artifacts, history or diagnostics.

### Phase 14 — PHP/Laravel provider and transactional Nginx routing
- Status: planned; approved runtime/package and web-server policies must be ratified.
- Depends on: 12, 13.

#### Objective and provider contract

Implement the first real `php-laravel` provider against approved revision/source
manifests. Provider responsibilities: detect prerequisites, declare supported OS/
runtime/package versions, prepare dependencies, render bounded configuration,
stage a release, expose service/health/log references and describe supported rollback.
Keep this interface reusable for later modules without building the marketplace now.

Ubuntu 24.04's native PHP 8.3/PHP-FPM is the initial deployment baseline. The panel's
development PHP version is not a managed-app support promise. Additional versions
or external repositories need explicit support/installation policy and tested
compatibility; no automatic PPA addition or claims that any PHP version works.

Phase 14 needs an internal approved package-install primitive for PHP-FPM/extensions,
Nginx and required helpers. Introduce its allowlist, APT-lock coordination and safe
failure handling here; phase 17 later exposes broader catalog/UI operations. This
avoids a dependency cycle from phase 14 to the later server-administration phase.

#### App-user build and Laravel preparation

1. Verify source is ready/pinned, app UID/layout valid, runtime/extensions supported,
   and disk/memory budget sufficient. Record installed versions and prerequisites.
2. Run Composer with a trusted executable as the app user, preferably from committed
   lockfile with deterministic production flags. Never run app scripts/plugins as
   root, use panel credentials, ignore platform requirements or silently update the
   dependency lock. Unsupported source prerequisites produce a useful failure.
3. Treat Composer scripts/plugins and optional frontend builds as untrusted app
   execution. Enforce process/user/network/resource limits and approved dependency
   credential delivery; Git checkout credentials are already removed. Node-based
   app builds require a supported runtime/build policy, not a runtime install on the
   current web development environment.
4. Prepare app-specific environment/secrets with a tested serializer. Do not inject
   raw values into systemd/Nginx/FPM directives. Preserve an existing app `APP_KEY`;
   generate it only once when requested and store it as that application's secret,
   never copy or regenerate the panel key on each deployment.
5. Link approved persistent storage and writable caches; keep uploads/shared data
   outside ephemeral release directories. Run only provider-approved Artisan/cache
   preparation as the app user. Build artifacts contain no deployment tokens.
6. Migrations are explicitly opt-in and belong to the deployment checkpoint policy
   in phase 15. Do not run them accidentally through a privileged lifecycle script.

Document dependency credential scope separately from source Git credentials when
private Composer/npm packages are required. A missing package token is not a reason
to give every application the agent's or operator's global credentials.

#### FPM, service and static routing ownership

- Generate an isolated PHP-FPM pool/instance policy per application: actual UID/GID,
  registered socket, document root, working directories, process limits/timeouts,
  environment filtering and bounded error logs. Confirm the pool worker's identity.
- Keep FPM runtime socket parents and root-loaded pool definitions agent-owned.
  Grant Nginx only the registered upstream socket/public asset access; app A cannot
  connect to B's upstream or alter B's definitions. Shared root FPM management does
  not mean shared app workers or shared writable config.
- PHP/Laravel uses `public/` as the document root, front-controller routing and
  deny rules for `.env`, VCS/private files and unauthorized PHP paths. Prevent an
  uploaded `.php` under public storage becoming executable through a broad regex.
- Workers use root-owned systemd templates with `User`/`Group` set to the app,
  approved working directory, restart policy and resource limits. Arbitrary start
  commands remain app-user commands, never interpolated into a root shell unit.
- Laravel scheduler execution is app scoped, single-instance/overlap-aware and
  separate from the panel's own task-maintenance scheduler. Detailed schedule
  lifecycle is finalized in phase 17.
- Add static-site routing only through the same managed domain/path boundary.
  Nginx access to public assets must not imply access to private secrets/storage.

#### Transactional configuration workflow

Use versioned agent-selected templates for Nginx and FPM, not caller-uploaded root
configuration. Domain conflicts, port/socket collisions, directive escaping and
path resolution are independently validated against the registry. Preserve panel
routing and unmanaged host configuration; beta clean-host policy is not permission
to delete an unrelated site on an existing development VM.

For each config change: acquire the relevant per-resource/global lock -> render to
trusted staging -> retain known-good files -> validate candidate FPM and complete
Nginx configuration -> publish controlled files -> reload -> verify service/HTTP
health -> journal commit or restore. Define multi-file generation identity so a
crash cannot leave half of one version mixed with half of another.

`nginx -t` validates the global configuration, not just the new file. Unmanaged
syntax errors can block a managed change: report that dependency without editing
unmanaged files. Reload failure does not justify force-restarting a working server.
Restore the previous managed generation, revalidate, recover/reload as appropriate
and record whether old traffic actually remains healthy. Apply equivalent FPM
failure/recovery behavior, including a partial pool reload and socket mismatch.

Health requests use registered app endpoints/paths and deadlines, not arbitrary
caller URLs. HTTPS, renewal and certificate activation are phase 16; initial
component tests use explicitly HTTP test domains without pretending TLS is ready.
Full release activation and user-facing deploy/rollback orchestration are phase 15.

#### Proposed modules and slices

Rust modules may include `providers/php_laravel`, `operations/approved_packages`,
`config/nginx`, `config/php_fpm` and `services/systemd`, created only when needed.
Their outputs are typed configuration manifests, managed IDs and health evidence;
PHP services consume these through the existing task/real-adapter pipeline.

| Slice | Deliverable | Required proof |
|---|---|---|
| P14.1 Provider/runtime policy | Lifecycle types, native PHP matrix and package primitive | Only approved packages/repos/versions selectable; APT conflicts handled |
| P14.2 Unprivileged build | Composer/source checks, app env/storage and optional build policy | Scripts run as app UID; key/data persist; failures do not touch active app |
| P14.3 FPM/worker isolation | Pool/socket and root-owned app-user service templates | App A/B socket/code/secret isolation and process identities verified |
| P14.4 Nginx/domain templates | Managed public routing, conflict checks and escaped inputs | `.env`/VCS/uploads-PHP/root-path injection cannot bypass policy |
| P14.5 Config transaction/recovery | Validation, generation publish, reload/health rollback | Invalid global/FPM config, failed reload and crash retain/recover old service |
| P14.6 VM provider integration | Real sample Laravel/static site plus observed results | Actual reachable HTTP app and safe service/log evidence on Ubuntu VM |

Tests cover incompatible PHP/extensions/lockfile, failed dependency scripts, missing
DB config, malicious template values, key persistence, read-only storage, domain
duplication, Nginx/FPM permission failures, global invalid includes and reload crashes.
Use test apps with no production data. Boundary tests inspect actual process UIDs,
public/private access and service state, not just rendered configuration strings.

**Exit gate:** a sample Laravel release runs through its isolated FPM pool and
managed HTTP site; config validation/recovery and build isolation pass on the VM.
The provider records exact supported prerequisites and leaves traffic/release
promotion to the orchestration designed next.

### Phase 15 — Real deployment orchestration, logs, health and rollback
- Status: planned; first genuine end-to-end native deployment milestone.
- Depends on: 4, 7, 11, 14.

#### Objective and admission rules

Join the tested account/source/provider/config primitives into one durable operation
requested by the existing Laravel deployment/task UI. Preserve current approvals,
immutable revisions, idempotency and progress/event types; extend them only where
the real boundary needs additional observable states.

Before agent submission, the panel rechecks admin/MFA/recent approval, app ownership,
revision/secret versions, capability, archive state and resource conflicts. The
agent independently validates that approved plan against its registry. Accepted
work continues journaled reconciliation after browser/worker disconnection; no
later poll may accidentally submit another deployment.

Switching from fake to real mode must not convert fake releases, certificates or
services into registry evidence. Existing desired configuration may be used, but
the first real deployment provisions/observes actual resources. Pin task mode,
managed-host identity, app/release IDs and configuration fingerprint; reject mixed
mode/host/revision operation state rather than silently reinterpret it.

#### Release manifest and checkpoint sequence

Every release has an agent-generated ID and manifest containing app/revision,
provider/runtime, pinned source commit, secret-version references, build result,
health policy, migration decision, service/config generation and previous release.
Track staged, built, prepared, health-checked, activated and verified checkpoints.
Keep shared uploads/data outside releases. Materialize a per-release environment
snapshot of approved values so staging does not overwrite the live release's
environment; app keys remain stable unless explicitly rotated.

Recommended sequence:

1. Persist acceptance/plan and acquire the application deployment lock. Reject or
   queue another conflicting deploy/restart/rollback according to explicit policy.
2. Create a unique staging release, prepare source and dependencies as the app user,
   and journal ready artifacts. Failure here leaves active release/traffic unchanged.
3. Prepare runtime/services and validate candidate managed config without exposing
   unvalidated code publicly. Bound resource use so staging cannot exhaust the VPS.
4. Apply the approved migration policy at its documented irreversible checkpoint.
   Coordinate app workers/scheduler and maintenance mode when old/new code cannot
   safely share the same schema. Record DB impact even if later health fails.
5. Check candidate health through a private/registered route or provider probe.
   Health GETs are bounded, targeted and do not intentionally mutate user data.
6. Atomically change the agent-controlled activation pointer on its filesystem and
   publish any necessary managed config generation. Reload/restart FPM/workers and
   routing using phase 14's recovery protocol; journal each side effect.
7. Verify actual public routing and app/worker readiness, then mark the release
   active and the task successful. An accepted request, running process or symlink
   update alone is not success.
8. Retain the last known-good supported release and bounded history. Clean only
   eligible managed, inactive releases; never shared storage, active/pinned releases
   or artifacts still referenced by a running recovery/rollback task.

Do not promise zero downtime by default. Choose and test the first Laravel activation
strategy (such as controlled FPM/worker reload with brief maintenance when necessary).
Blue/green routing is a later optimization if resource/health requirements justify it.
Document OPcache/realpath cache and worker-version behavior: changing `current` does
not automatically guarantee old PHP workers execute the new code.

#### Migrations, rollback and cancellation semantics

- `run_migrations` is explicit in approved intent. Execute only as the app user with
  approved app database credentials, not the database administrator or root.
- A backup requirement can precede consequential migrations; until managed backups
  exist, require an explicit administrator-supplied recovery prerequisite or refuse
  that policy. Do not label a fake backup as satisfying it.
- A crash during migration can leave non-transactional DDL/data changes. Reconcile
  evidence and mark recovery-required/unknown where necessary; do not blindly run
  the migration command again because the execution receipt was lost.
- Supported rollback selects a real, same-app/same-host known release with compatible
  runtime/config/environment and acceptable DB schema. No arbitrary release path,
  fake release ID or automatic `migrate:rollback` reversal.
- Persistent uploads/database changes remain after code rollback. If old code is
  incompatible, retain a clear recovery-required state and operator runbook rather
  than falsely report restored health.
- Cancel before activation can stop/clean staging if safe. During package/migration/
  config activation, cancellation may be unsupported or delayed to a safe checkpoint.
  Never acknowledge full rollback solely because the child process was terminated.

#### Durable recovery and observability

Journal before/after irreversible actions and inspect actual pointer/config/service/
DB evidence on restart. Reconcile panel task/deployment and agent execution/release
records; the observed active release comes from the agent, not UI optimism. Unknown
execution never updates the active-release pointer in panel state.

Expose safe phase progress, elapsed time, bounded redacted stdout/stderr, structured
health/error codes and recovery notes through existing task events. Artifact IDs,
commit IDs and provider versions are safe metadata; decrypted env/build credentials
and database dumps are not logs. Inspect accepted work even after actor revocation;
new mutations/retries/cancel authority are independently validated.

Restart uses the registered app's services with health verification. Log inspection
is bounded by app/resource/file identity and retention, not a generic root file-reader.
Keep service/worker observations distinct from successful application HTTP health.

| Slice | Deliverable | Required proof |
|---|---|---|
| P15.1 Deployment admission/manifest | Real mode/host/revision binding and app lock | Fake/cross-app/stale/duplicate intents cannot activate resources |
| P15.2 Staging/build checkpoints | Repeatable release staging and per-release env | Failed build preserves live app, key and persistent data |
| P15.3 Migration/worker policy | Opt-in migrations, maintenance/worker coordination | DB-impact and interruption cases never claim automatic data rollback |
| P15.4 Health-gated activation | Pointer/config publish, FPM/worker update and final checks | No traffic to failed candidate; actual new release is healthy |
| P15.5 Rollback/retention/restart | Supported real-release rollback and safe cleanup | Old code restoration verified; active/shared/pinned data retained |
| P15.6 Crash/cancel reconciliation | Lost receipt, process kill and ambiguous checkpoints | One intent remains inspectable; no unsafe duplicate mutation |
| P15.7 Web-to-VM vertical slice | Real UI deploy/redeploy/log/restart/rollback | Browser reconnect and agent/worker restart preserve truthful state |

Failure-injection matrix: source/build failure, unhealthy candidate, global config
failure, failed FPM/worker reload, migration failure/unknown completion, disk-full,
crash before/after pointer switch, duplicate submit, role/revision change before
acceptance, cancel races, and cleanup while a release is referenced. Run at least
two releases of an app with persistent uploads and a database-backed feature.

**Exit gate / milestone:** an authenticated administrator can deploy a reachable
Laravel app, redeploy, inspect logs, restart and perform a supported code rollback
entirely through the real panel/agent on the VM. Persistent data survives, health
is evidenced and partial failures have tested recovery, not cosmetic success states.

### Phase 16 — Real TLS lifecycle
- Status: planned; requires a test domain/DNS and publicly reachable disposable VM.
- Depends on: 8, 15.

#### Approach and prerequisites

Connect existing certificate/domain intent to a bounded ACME provider and managed
certificate installation. Initial provider is Let's Encrypt HTTP-01; exercise its
staging service first. Confirm DNS, HTTP reachability, supported domain names,
operator contact, outbound access and ACME client/library before issuing real certs.
No wildcard/DNS-01 support or automatic DNS-account provisioning is claimed here.

Keep ACME account identity/key, certificate orders and private-key references in
restricted agent-owned state. The panel stores metadata and encrypted supplied
material, never public key-file URLs. Issuance/renewal executes as durable agent
work through the same approved task interface, not inline PHP shell commands.

#### Issuance, installation and renewal flow

1. Resolve the managed domain/app and approved provider; check conflicts, hostname,
   DNS/reachability evidence, existing certificate state and expiry policy.
2. Provision an agent-controlled HTTP-01 challenge location that cannot execute
   app PHP or allow the application to forge another domain's challenge. Preserve
   panel/app routing and serve only bounded challenge tokens for active orders.
3. Create/resume the ACME order with provider-aware backoff and rate-limit handling.
   Record authorization/order IDs and deadlines; reconnect never creates endless
   new orders or consumes provider limits unnecessarily.
4. Validate returned/supplied leaf and chain: private-key match, hostname coverage,
   validity, supported algorithm/strength and appropriate server-certificate use.
   Uploaded files are untrusted/bounded; error messages contain no key/passphrase.
5. Stage root-owned restricted cert/key files and a candidate Nginx generation;
   validate before activation, reload, and verify the served certificate via an
   actual TLS handshake. Persist success only after observed HTTPS health.
6. Schedule renewal early enough for retry/alerting, with one active order per
   domain/certificate scope. Renewals are system maintenance under a documented
   standing policy; changing domain/provider/key policy requires fresh approval.
7. Retain the working generation on failure, remove stale challenge material, and
   show real expiry/last attempt/next retry/error in the panel. Failed renewal is
   not permission to replace HTTPS with a broken certificate or silently enable HTTP.

Do not enable HSTS or irreversible redirect policies before actual HTTPS succeeds;
introduce them deliberately with recovery implications. Certificate removal/rotation
requires explicit intent. Account/key recovery and backup dependencies feed phase 19.

| Slice | Deliverable | Required evidence |
|---|---|---|
| P16.1 Provider/order state | ACME adapter, account policy, durable bounded orders | Staging issuance/resume and rate-limit errors are handled |
| P16.2 Challenge ownership | Managed HTTP-01 routing/token cleanup | Another app/domain cannot supply or read challenge secrets |
| P16.3 Certificate activation | Key/chain validation, file modes, transactional reload | Wrong key/name/expired chain denied; failed reload retains old HTTPS |
| P16.4 Renewal/alerts | Maintenance policy, scheduling, backoff and expiry UI | Renewal survives disconnect/restart; failure is visible before expiry |
| P16.5 Supplied certificates/VM proof | Integrate current upload model with real install | Supplied/encrypted-key paths and actual TLS handshake pass |

Test wrong DNS/port, challenge collision, revoked authorization, provider outage,
rate limit, bad/mismatched supplied material, expiry, permissions, crash between
issuance/activation, renewal duplicates and failed Nginx reload. Inspect private-key
access as panel/app users and ensure logs/journals contain references only.

**Exit gate:** actual domain HTTPS, staged-to-production issuance, supplied certs,
renewal and failure recovery pass on the VM; observed certificate identity/expiry
is accurate and the previous healthy configuration survives failed changes.

### Phase 17 — Real services, packages and scheduled jobs
- Status: planned; extends the internal package/service primitives from phase 14.
- Depends on: 8, 12, 15.

#### Service and package controls

Expose real observations/actions through the existing server UI and task pipeline.
Service IDs resolve to agent-owned unit definitions and app identity. Only explicitly
managed services support approved start/stop/restart; do not accept arbitrary unit
names or grant control over SSH, firewall, the host init system or unrelated services.
Panel/agent maintenance services use a separate upgrade/recovery policy in phase 20.

Package requests select an approved catalog ID/version policy, not APT arguments or
repository URLs. Reuse the phase-14 package primitive, serialize package-manager
access, respect external APT/dpkg locks and never delete lock files to force progress.
Record actual installed version, configured repository and completion evidence.
Installation failure can leave partial dpkg state: report/reconcile it and offer a
bounded approved repair procedure, not blind retry or a generic root terminal.

Review consequential package maintainer scripts, dependency removals, service restart
effects and disk/network/time limits before advertising catalog entries. Use configured
trusted Ubuntu repositories and normal authenticity verification; no unsigned package
downloads, implicit external repository addition or broad OS upgrade endpoint.

#### Hosted scheduled-job lifecycle

Map existing schedule UUID/revision to agent-owned systemd service/timer definitions
(preferred initial backend) or a deliberately selected cron backend. Each command
runs as its app UID with registered working directory and environment references.
No root owner option, panel-maintenance credentials or unbounded executable/path.

Validate expression/timezone and define conversion limits. Reject unsupported cron
semantics instead of approximating them silently. Specify missed-run behavior,
daylight-saving changes, overlap policy, execution deadline, output retention and
whether a manual run uses the same job identity/policy. Saving desired state alone
does not mean the timer has been installed or enabled.

Apply/update using staged unit validation, atomic root-owned files, daemon-reload,
enable/start and observed-state verification. Removal disables the registered timer
and handles any running job through its cancellation policy; retain history/data.
On app archive/decommission, identify schedules by registry association, not filename
patterns that may delete another app's jobs.

Firewall remains read-only: show supported backend/status with sampled time. No
automatic destructive rule changes or SSH access removal before a separate tested
network-recovery design. Metrics/log/service inspection uses bounded managed IDs.

| Slice | Deliverable | Required evidence |
|---|---|---|
| P17.1 Managed service observation/control | Real status/health and allowlisted lifecycle tasks | Cross-app/raw-unit targets denied; observed outcome matches systemd |
| P17.2 Approved package UI execution | Catalog mapping, lock coordination and installed state | Duplicate/concurrent requests do not fight APT or claim partial success |
| P17.3 Schedule rendering/installation | App-user unit/timer backend and expression policy | Real execution UID, timezone/overlap and safe root-owned config verified |
| P17.4 Schedule history/update/remove | Run IDs, bounded logs, disable/cancel and retention | Crash/update/missed-run behavior preserves history and ownership |
| P17.5 Firewall/status integration | Read-only backend adapter and error states | Offline/unsupported/stale status is accurate; no mutation path |

VM tests cover unrelated service targets, bad unit/command injection, blocked APT
locks, interrupted installation, failed reload/restart, app A/B schedule isolation,
DST/overlap, runaway jobs, log floods, archive/removal races and worker reconnect.

**Exit gate:** genuine managed actions, approved installs and app-owned schedules
work through the panel with bounded failure/recovery. Unmanaged/root commands and
firewall mutations remain unavailable; resulting state is observed, not assumed.

### Phase 18 — Application database provisioning
- Status: planned; user must choose the first managed engine before implementation.
- Depends on: 8, 12, 15; engine decision required.

#### Engine scope and privilege model

MariaDB is the proposed first Laravel-oriented engine, not a confirmed decision.
Implement/test one engine and version family on the supported OS before adding
PostgreSQL or multiple versions. Reuse the existing database-request model and
approved package/service primitives; hosted databases remain separate from panel
SQLite. Initial provisioning manages a local engine, not arbitrary remote DBA access.

An engine-admin connection belongs only to the agent and uses controlled local
socket/authentication or root-protected credentials. It is never an app secret,
task parameter, browser field or log. Application principals receive only their
registered database/schema privileges; no global administration, grant option,
server filesystem access or command/program-execution privileges.

Generate actual DB/user identifiers from managed resource IDs using engine-specific
length/quoting rules. Display labels are not interpolated SQL. Parameterize values
and use safe identifier construction for DDL that cannot bind identifiers. Reject
protected/system DB names regardless of input or a corrupted association.

#### Provision, rotate and remove workflow

1. Validate request UUID/app/engine/revision and provision policy; check engine
   readiness, disk/capacity and registered resource ownership. Existing unmanaged
   databases/users are conflicts, not candidates for silent adoption.
2. Journal allocation; create the managed DB/schema and application principal with
   a cryptographically generated, bounded credential. Grant only required app
   access, including approved migration DDL within that database where necessary.
3. Bind generated or pre-approved credential versions through the protected
   secret channel/storage. Perform a real connection check as the app principal.
   Record instance/DB/user IDs and actual readiness before panel reports success.
4. Deliver app environment references into a new deployment revision, not a live
   plaintext overwrite. Deploy/check the Laravel DB-backed feature with that role.
5. Rotation uses a tested engine-specific overlap/new-principal/maintenance policy;
   update app revision, verify connections and only then retire the old credential.
   Do not assume both engines support dual passwords or that existing pooled
   connections prove the next connection will succeed.
6. Delete/disable is a separate consequential approved operation. Check active
   deployment/schedule/backup references and recovery prerequisites. Drop only the
   exact registered DB/principal, retain audit/history, and never couple panel app
   archive to automatic database destruction.

Partial failures are journaled because DDL may commit independently of the operation.
Reconcile created DB/user/grants before retry; remove only proven newly created empty
resources. A failure after data is written requires recovery rather than destructive
cleanup. Passwords remain encrypted/write-only and absent from client argv/errors.

#### Data movement and isolation

Define the supported logical export/import format, engine/version compatibility,
size/deadline and consistency limits. Dumps are sensitive artifacts, not task logs.
Import app data as a restricted app role into an approved target/staging database;
never execute an untrusted dump using engine-admin privilege. Do not accept arbitrary
SQL, server filesystem paths or a generic database-console endpoint. Real scheduled
backup/restore consistency and encrypted retention are finalized in phase 19.

MariaDB testing must deny cross-database/global/FILE-like access. PostgreSQL testing,
if selected, must cover database/schema ownership, PUBLIC/default grants, connect
permissions and non-superuser/non-role-creation/non-server-file execution. Inspect
actual grants and query as app A/B; successful connections alone are not isolation.

| Slice | Deliverable | Required evidence |
|---|---|---|
| P18.1 Engine/policy selection | Confirmed engine/version, local admin auth, identifier/grant rules | Protected names and unsupported targets fail without SQL side effects |
| P18.2 Journaled provisioning | Registered database/user/grants and connection check | Partial/duplicate requests create one resource without adopting unrelated DBs |
| P18.3 App integration/rotation | Encrypted credential references and deploy/health cutover | Laravel data works; no credential leak or silent loss during rotation |
| P18.4 Bounded data movement/deletion | Supported import/export and explicit destructive policy | Dumps remain private; unknown/referenced/system resources cannot be dropped |
| P18.5 VM isolation/recovery | Two apps plus interrupted DDL/rotation scenarios | Cross-app/global privileges denied; recovery preserves existing data |

**Exit gate:** the selected engine provisions an isolated database used by a real
Laravel app; actual grants, secret handling, rotation and partial-failure behavior
pass on the VM. Only tested engine/version/data-movement capabilities are advertised.

### Phase 19 — Backup, restore and recovery
- Status: planned; requires a separate disposable restore VM and key-custody decision.
- Depends on: 15, 16, 17; include 18 for managed databases.

#### Backup scope and trust

Extend the existing backup-policy/history UI with real backup artifacts and verified
restore evidence. A simulated backup never satisfies retention, migration recovery
or upgrade safety. Define separate panel/control-state, application-data and database
scopes, plus a coordinated full-hostnur recovery set; this is not a promise to image
every operating-system file on the VPS.

Required recovery inventory: panel SQLite/config and APP_KEY; admin/MFA/setup state;
agent registry/execution journal and any credential encryption keys; app/revision/
deployment identity manifests; persistent uploads; supported DB dumps; source/DB
credential versions; certificates/ACME account keys; generated-service/template
versions. Derived config can be rebuilt from verified metadata where documented.

Agree encryption/key custody and the first destination (restricted local storage
plus an operator-controlled off-host copy/provider). Use established authenticated
encryption and an independently recoverable key/password, never custom crypto.
Do not place the only decryption key inside the encrypted backup it unlocks or rely
solely on a panel APP_KEY that was lost with the server. Record rotation/recovery
procedure without committing live keys. Backups are never public web downloads.

#### Consistent backup workflow

1. Validate policy/resource scope, approvals, destination access, capacity, retention
   and declared consistency level. Serialize conflicting backup/restore/upgrade work.
2. Establish a short consistent control-state checkpoint and prevent deployments/
   secret rotations from mixing revisions during capture. Use SQLite online-backup
   or a tested coordinated checkpoint, not blind copying of a live DB/WAL file.
3. Capture app data with its documented quiescence/snapshot policy; use engine-native
   consistent dumps for managed databases. Hot-copying mutable uploads plus a later
   DB dump does not automatically give cross-resource application consistency.
4. Write a versioned manifest: scope, resource IDs/revisions, OS/engine/provider/
   schema versions, capture times, consistency policy, sizes and integrity hashes.
5. Stream bounded content into protected staging, encrypt/authenticate, finalize
   atomically and transfer to the approved destination. Redact progress and keep
   dump/key material out of ordinary task events and failed-job payloads.
6. Verify artifact integrity/authentication and destination durability before
   marking the backup completed. Mark restore-tested separately: successful upload
   or checksum alone is not proof that recovery works.
7. Prune only eligible verified backups under the policy. Preserve a last known-good
   recovery set and backups pinned by upgrades/restores. A failed new backup must
   not delete the last useful one.

Handle disk-full, permission/network loss, cancellation, incomplete manifests,
checksum failure and process interruption with explicit partial/failed states.
Unverified temporary data is cleaned safely without deleting completed artifacts.
Retry uploads using the same artifact identity; do not recapture silently under
the same identity with different content.

#### Restore and operator recovery

Restore begins on a separate supported clean VM. Verify artifact/key, compatibility,
expected scope and destructive target approval before stopping services or extracting.
Bound archive entries/size and reject traversal, absolute paths, unsafe links/devices
and unexpected owners. Never extract arbitrary archive paths as root.

Put the target panel/worker/agent in maintenance; recreate managed OS identities
without UID conflicts; restore control state and keys consistently; reconcile logical
host/resource identity with the new machine/boot identity. Handle old journal entries
as recovered/needs-reconciliation, not pending work to execute automatically.
Restore shared data/DBs/secret versions, regenerate managed configs and validate them,
then verify administrator access/MFA, app health, TLS, workers and schedules before
enabling normal traffic. Do not duplicate the live source host's schedules or outbound
jobs during a restore drill; use isolated DNS/network/test destinations.

The runbook states what is recoverable, data-loss window, required keys/artifacts,
version limitations, expected downtime, how to recover without a functioning web
panel, and how to safely fail a restore before/after partial data replacement.

| Slice | Deliverable | Required evidence |
|---|---|---|
| P19.1 Recovery inventory/policy | Scope/consistency/retention and key-custody decision | Every secret/identity needed for recovery has a documented recoverable source |
| P19.2 Real artifact creation | Consistent DB/data capture, manifest and authenticated encryption | Live WAL/mutable-data cases and key-loss errors handled correctly |
| P19.3 Destination/retention | Verified transfer, pinning, prune and failure cleanup | Network/disk failure preserves previous recovery sets |
| P19.4 Restore orchestration | Safe extraction, identity/schema/config reconciliation | No root path escape, unexpected replay or host identity confusion |
| P19.5 Separate-VM drill/runbook | Full restore plus scoped restore examples | Real admin/MFA, data, app, TLS and service health recovered |

**Exit gate:** a separately provisioned VM restores a genuine encrypted recovery set
and passes the documented checks. Evidence includes artifact ID, versions, key access
method, consistency limits and observed recovery outcome; no fake/untested backup
can be represented as restore-ready.

### Phase 20 — Packages, installer, signed releases and upgrades
- Status: planned; domain, license, signing identity and distribution hosting need decisions.
- Depends on: 2, 16, 19; signing/domain/hosting decisions required.

#### Packaging scope and release inputs

Produce two coordinated Debian packages, `hostnur` and `hostnur-agent`, for the
validated Ubuntu 24.04 amd64 platform. The panel package declares a compatible
agent range and supported PHP dependencies. Database/runtime modules have explicit
optional dependencies; installing the panel must not install every hosted stack.
Include phase 18's backup/provisioning gates if managed databases are advertised.

Resolve official domain, repository visibility/licensing, release-signing method,
verification identity and first package host before publishing. Current GitHub
hosting does not imply public installer assets are already available. Never publish
placeholder installation commands as a working service.

Proposed packaging files: `packaging/debian/{hostnur,hostnur-agent}/`, service/socket
units, reviewed maintainer scripts, bootstrap installer, release manifest/verification
instructions, and separate CI jobs for build/sign/install/upgrade VM tests. Use
versioned templates and map release versions to Debian ordering deliberately.

#### Build artifacts and installed ownership

- Build Rust against the actual target ABI, not an arbitrary newer build host.
  Validate shared-library requirements on Ubuntu 24.04. Commit dependency locks
  and record build toolchain/source commit, artifact hashes and dependencies.
- Package the panel with production Composer dependencies and prebuilt Vite assets.
  Node/npm/Rust/Composer development tooling and dev-only Boost/test packages are
  not required to run an installed panel. Test the production-only package boot.
- Exclude development `.env`, app keys, SQLite data, credentials, caches, logs and
  fixture-only writable state. Templates/examples are not live secrets.
- Agent config/registry/journal and root-loaded templates remain protected; panel
  PHP-FPM/worker runs under its dedicated unprivileged identity. Hosted users and
  Nginx do not gain agent-socket access through a broad shared group.
- Use production PHP-FPM/Nginx/systemd services, not `artisan serve` as a public
  production web server. Document package locations, state dirs, owners/modes,
  ports/socket access, queue worker and scheduler separately from local port 9000.
- Keep generated encryption keys with preserved runtime state, not an expendable
  packaged default. If existing encrypted state has lost its key, require recovery;
  do not regenerate a replacement and silently make stored secrets unusable.

#### Installer and secure onboarding

The bootstrap detects OS/architecture, privilege and existing-host conflicts;
installs verified versioned packages; creates service identities/state with correct
permissions; initializes keys/schema once; checks agent/panel readiness; and prints
local/SSH-tunnel setup instructions. Default beta target is a clean supported VPS.
Existing unmanaged services/configuration cause an explained refusal or explicitly
agreed migration path, not silent adoption or overwriting.

Use a small reviewable HTTPS bootstrap and trusted signature/package metadata,
with pinned verification identity/key and documented trust bootstrap. Hashes alone
do not authenticate artifacts. No cloning/executing the latest development branch
as root or putting private-download tokens in URLs/command output.

First-run administration uses the existing local bootstrap command under the panel
identity. No default password, public registration or unauthenticated internet setup
page. Initial panel access is loopback/tunnel until the operator deliberately
configures domain/TLS/access policy. Package scripts must not assume piped stdin
can securely provide interactive passwords or print generated login credentials.

Maintainer scripts are idempotent and narrowly scoped. Test first install, rerun,
interrupted configuration and missing prerequisites. Remove/purge behavior must
document and preserve app data/recovery keys unless a separate explicit destructive
decommission action is approved; package uninstall is not permission to drop DBs.

#### Coordinated upgrade and recovery

1. Preflight panel/agent/protocol/schema compatibility, state/key access, disk space
   and an actual verified recovery set. Block unsupported version jumps/downgrades.
2. Stop accepting new mutations, drain or journal active operations, pause worker/
   scheduler and take a consistent control-state backup. Keep hosted apps running
   where the upgrade does not require their interruption.
3. Install the coordinated packages, preserve config/secrets and migrate panel/
   registry schema under documented locks. Do not rewrite live app data as a side
   effect of a panel upgrade or rotate keys implicitly.
4. Start agent, verify protocol/registry readiness, then panel/worker, routing and
   scheduler. Check authenticated access/MFA and representative app/task health.
   Resume pending intents only through reconciliation, not automatic journal replay.
5. On failure, keep the system in a truthful maintenance/recovery state. A schema
   downgrade is not necessarily reversible by installing an old binary; use the
   tested restore path and state the data-loss/downtime boundary. Never run a blind
   destructive migration rollback to make an upgrade appear successful.

#### Signing, publication and release validation

Pipeline: version tag -> component/contract checks -> build once -> production
packages/assets -> checksums and authenticated signatures -> test exact candidate
artifacts on clean/upgrade/restore VMs -> approved release publication. Do not rebuild
different artifacts after the version that passed tests. Keep signing credentials
or workload identity outside untrusted PR jobs; specify issuer/repository/workflow/
tag identity or pinned key that users must verify and define rotation/revocation.

GitHub Releases can host initial artifacts; use signed APT repository metadata
for routine updates before wider adoption. CI passing is not an automatic decision
to publish a public beta; phase 21's complete release gate still applies.

| Slice | Deliverable | Required evidence |
|---|---|---|
| P20.1 Distribution/ownership decisions | Package layout, versions/dependencies, domain/signing/license | Supported matrix and verification trust are explicit |
| P20.2 Production package builds | Compiled agent, no-dev panel/assets and units | Installed packages boot without build tools or development secrets |
| P20.3 Bootstrap/onboarding | Verified installer, OS checks, local admin setup and idempotency | Clean VM install/rerun/interruption keeps correct permissions and no public setup |
| P20.4 Signing/release CI | Exact artifact manifest, signatures and candidate publication | Tampered/wrong-identity artifacts are rejected; tested bytes equal released bytes |
| P20.5 Upgrade/recovery | Drain, backup, compatible migrations and failure handling | Existing app/data/key/MFA state survives; failed upgrade recovers on VM |
| P20.6 APT/update/uninstall lifecycle | Signed metadata, update/version policy and safe removal | Routine upgrade and removal do not silently destroy hosted resources |

**Exit gate:** verified packages install on a clean supported VM, upgrade a previous
supported version and recover from injected failure without losing panel/app/key
state. Ownership/socket isolation, real TLS and restore checks still hold. Published
verification/update/operator instructions match the exact tested artifacts.

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
| Authentication pipeline | First-party Fortify **v1.41.0 installed/locked** (session 008, approved) for session/TOTP/recovery; features enabled per slice | Resolved; revisit at P2.3+ |
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

### Session 028 — Phase 13 Git source lifecycle, public + private HTTPS (2026-10-13)

- Implemented **P13.1–P13.3** (and the failure-cleanup part of P13.5) for the
  chosen auth scope. P13.1: `agent/src/git/policy.rs` — HTTPS-only URLs, no
  credentials in URL, loopback/link-local/metadata (and default RFC1918) refused,
  optional host allowlist + custom port, and `git check-ref-format`-style ref
  validation distinguishing full-commit ids from branch/tag names and rejecting
  option-like/revision expressions. P13.2: `checkout.rs` runs a fixed `git` as the
  application UID/GID with a hardened, minimal environment (`GIT_CONFIG_NOSYSTEM`,
  `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`, disabled SSH/askpass,
  `GIT_ALLOW_PROTOCOL=https`), a deadline and capped output; it fetches with
  `--depth 1`, resolves `FETCH_HEAD` to a full commit, detaches, and removes only
  the incomplete staging tree on failure. P13.3: `credentials.rs` delivers a token
  via a transient app-owned `GIT_ASKPASS` helper + `0600` token file removed on
  drop; the value never enters the URL/argv/config/errors.
- SSH deploy keys (P13.4) deferred per the user's auth decision. P13.6 (real source
  results through panel tasks) deferred to phase 15.
- Checks (Lima Ubuntu 24.04 arm64 VM): `cargo fmt`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo build --locked`, `cargo test --locked` —
  **131 tests passed**. Root-only `tests/git_vm.rs` (ignored by default) passed:
  checkout as the app user produced an app-owned tree, pinned a 40-hex commit, and
  cleaned up a failed staging tree.
- Blocked/deferred: no live private-repo clone (offline VM); amd64/CI remains the
  only amd64 signal; panel integration is phase 15.
- Next slice: **phase 14** (PHP/Laravel provider and transactional Nginx routing),
  which needs the approved runtime/package and web-server policies ratified.

### Session 027 — Phase 12 application isolation (P12.1–P12.5) (2026-10-13)

- Completed **all of phase 12**. P12.1: managed-application registry
  (`managed_applications` table) and namespace policy (`agent/src/provision/policy.rs`):
  `hn_<12 hex>`, fixed `/opt/hostnur/apps/<uuid>/...` layout, lifecycle
  `reserved -> account_created -> layout_prepared -> ready` (+ `failed`/`drifted`);
  `ProvisioningService::reserve` refuses account-name collisions and unmanaged
  accounts/paths (host state injected via a `HostProbe` so policy is testable
  without root). P12.2: `accounts.rs` creates no-login, no-home, no-socket-group
  accounts via allowlisted `groupadd`/`useradd` and reads the identity back from
  NSS; the process primitive now clears supplementary groups
  (`setgroups([])`) before `setgid`/`setuid` in a `pre_exec` child. P12.3:
  `safe_fs.rs` resolves every managed path with
  `openat2(RESOLVE_BENEATH|NO_SYMLINKS|NO_MAGICLINKS)` from one trusted base fd,
  creates the verified mode/ownership tree and publishes files with an atomic
  rename. P12.4: idempotent `reconcile` resumes partial states, re-verifies layout,
  heals loosened modes and detects identity drift. P12.5: root-run VM isolation
  harness.
- Dependencies: added `libc` (openat2 + identity drop); `nix` features extended
  (`process`, `signal`, `hostname`). Recorded in `docs/agent-development.md`.
- Checks (Lima Ubuntu 24.04 arm64 VM): `cargo fmt`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo build --locked`, `cargo test --locked` —
  **121 tests passed**. Root-only `#[ignore]` harnesses run directly as root:
  `provision_vm` (two apps, distinct UIDs, `shared` `0750` app-owned, `secrets`
  `0700`, `releases` root-owned `0755`; A reads its own secret but is denied B's;
  symlinked parent refused) and `process_runner`
  (`runs_as_target_identity_without_extra_groups`) both passed.
- Blocked/deferred: still arm64-only locally (amd64 is CI-only); the hardened
  systemd unit continues to run the agent as `hostnur` — running privileged
  provisioning through the service is revisited when deployment handlers ship
  (phase 14/15); full openat2 tests are Linux-only (CI covers them).
- Next slice: **phase 13** (real public/private Git source lifecycle), which needs
  confirmation of supported Git providers/auth modes.

### Session 026 — Phase 11 transport/execution boundary, part 1 (P11.1–P11.4) (2026-10-13)

- Implemented **P11.1–P11.4** of phase 11 in the `agent/` crate. P11.1: Rust mirror
  of the `contracts/v1` request envelope and response DTOs with strict
  (`deny_unknown_fields`) parsing, a canonical UUID type, the embedded operation
  catalog, and 4-byte big-endian length-prefixed UTF-8 JSON framing (zero/oversized/
  truncated frames rejected before allocation). Cross-language fixture test in
  `agent/tests/contract_fixtures.rs` proves Rust agrees with the PHP-validated
  valid/invalid fixtures and their manifest error codes. P11.2: Unix socket
  endpoint (refuses symlink substitution, restrictive mode, stale-socket rebind) and
  kernel `SO_PEERCRED` peer-UID allowlist checked *before* any message is read;
  `agent/tests/transport_boundary.rs` proves unauthorized peers, substituted sockets,
  partial frames and invalid requests never reach dispatch. P11.3: SQLite-backed
  agent-owned execution journal (acceptance-before-execution, idempotency dedupe,
  same-key/different-intent `conflict`, durable events with cursors, restart
  survival) plus a managed-resource registry; `agent/tests/journal.rs`. P11.4:
  bounded process primitive (absolute program + argv array, `env_clear`, working
  dir, deadline, capped output, process-group SIGKILL on timeout) with a fixed
  program allowlist; `agent/tests/process_runner.rs`.
- Dependencies added (all MIT OR Apache-2.0 unless noted; no MSRV impact beyond the
  pin): `serde` 1.0.229, `serde_json` 1.0.154, `nix` 0.31.3 (MIT), `rusqlite` 0.40.2
  (`bundled` SQLite, C toolchain) and `getrandom` 0.4.3. Recorded in
  `docs/agent-development.md`.
- Checks (Lima Ubuntu 24.04 arm64 VM): `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked` and
  `cargo test --locked` — **92 tests passed** (54 unit + 8 CLI + 7 contract + 7
  journal + 7 process + 9 transport). Manual e2e: agent bound a `0660` UDS, accepted
  a peer-authorized framed request, and shut down cleanly on SIGTERM.
- P11.5 (user approved additive `agent.*` control operations): added
  `agent.capabilities`, `agent.inspect` (`execution_id`), `agent.cancel`
  (`execution_id`) to `contracts/v1/catalog.json` with fixtures; implemented the
  Rust `AgentHandler` dispatch (control ops + read-only `server.snapshot`; mutations
  return `unsupported_operation`) and the PHP `RealAgentClient` +
  `UnixSocketAgentTransport` (length-prefixed, bounds-checked, fail-closed) wired
  through `AgentClientFactory` so `real` mode never falls back to fake, plus
  `hostnur.agent.socket_path`/timeout config and a `config/hostnur.php` agent block.
- P11.6: added live read-only observation (`observe.rs`: load, memory, uptime,
  root-disk usage, CPU count from `/proc`/`statvfs`) and systemd packaging
  (`packaging/systemd/hostnur-agent.service`, `agent.toml.example`). Verified on the
  Lima Ubuntu 24.04 VM: service active as a dedicated `hostnur` user, socket
  `mode=660 owner=hostnur:hostnur`; a group member receives `mode=real` capabilities;
  an unrelated user is denied; `systemctl restart` recreates the socket and serves.
- Checks: Rust `fmt`/`clippy -D warnings`/`build --locked`/`test --locked` — **100
  tests passed** (56 unit + 8 CLI + 7 contract + 6 dispatch + 7 journal + 7 process +
  9 transport). PHP (host, PHP 8.4/Composer 2.9.5): `php artisan test --compact` —
  **254 tests / 1042 assertions passed**; `vendor/bin/pint`, `composer check:syntax`,
  `composer validate` (known license warning) passed. Local panel setup used
  project-local `composer install` + `npm ci`/`npm run build` (assets are required
  for HTTP tests).
- Blocked/deferred: real GitHub Actions run still pending (local-only commits);
  host integration ran on arm64 (amd64 is CI-only); a live PHP↔Rust end-to-end test
  was not run (PHP is not installed in the VM) — both sides are covered by the
  shared contract/fixtures and their own framing tests.
- All committed locally: `f3a7cf4`, `f792506`, `3daba42`, `40fa8e8`, `ec8d695`
  (P11.5), plus the P11.6 commit. Next slice: **phase 12** (Linux application
  isolation), which needs a disposable amd64 VM.

### Session 025 — Phase 10 Rust foundation (P10.1–P10.5) (2026-10-13)

- Completed **all of phase 10** in the prepared agent environment: a dedicated
  **Lima** Ubuntu 24.04 VM (arm64 native; the amd64 deployment target is validated
  in CI). P10.1: installed/verified rustc/cargo **1.99.0** (stable, released
  2026-10-01), rustup 1.29.1, gcc 13.3.0, make 4.3; confirmed the stored
  `rust-toolchain.toml` pin/MSRV (1.99.0, edition 2024) is valid and left it
  unchanged; target decision recorded. P10.2: created the independent
  `agent/` crate (`hostnur-agent`, lib + bin, `Cargo.lock` committed) with
  `--version`/`--help`, exit codes and an `HOSTNUR_AGENT_CONFIG`/`--config` path;
  the panel needs no Rust to run. P10.3: typed TOML configuration with strict
  parsing — unknown/missing/wrong-type/unsupported-version/relative-path/`..`/
  out-of-range settings fail with stable codes (`config_not_found`,
  `config_parse`, `config_invalid`) and never echo values; `check-config` has no
  side effects. P10.4: SIGTERM/SIGINT lifecycle with controlled shutdown, idle
  non-root `run` (no socket, no privileged handler) and bounded single-line JSON
  logs with a fixed field allowlist (no config/env/credential dumping). P10.5:
  `.github/workflows/agent.yml` (Rust 1.99.0, fmt/clippy `-D warnings`/build/test/
  binary smoke on x86_64-unknown-linux-gnu), `docs/agent-development.md` and
  `agent/.gitignore`.
- Dependencies (all MIT OR Apache-2.0, no native build, no MSRV impact beyond the
  pin): `toml` 1.1.8, `signal-hook` 0.4.5, dev `tempfile` 3.27.0. Crate has no
  `license` field (project license still an open decision) and `publish = false`.
- Checks (in the Lima VM): `cargo build --locked`, `cargo test --locked` —
  **34 tests passed** (26 unit + 8 CLI integration), `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings` all passed. Manual
  lifecycle: `run` exited 0 on SIGTERM with `agent.starting`/`agent.ready`/
  `agent.stopped` logs and created no state paths; `run` with a missing config
  exited 2 with `config_not_found`. No current-host Rust/system changes; web panel
  untouched.
- Blocked/deferred (recorded, not claimed): the agent workflow cannot be observed
  until the branch is pushed (this session committed locally only); privileged
  host/socket/systemd/install checks need a disposable Ubuntu 24.04 **amd64** VM
  (phase 11+); x86_64 binary execution/packaging is phase 20.
- Next slice: **P11.1** (cross-language contract) from phase 11.

### Session 024 — Detailed planning for phases 10–20 (2026-10-13)

- User requested reading only `plan.md`, expanding each phase separately and
  saving it before continuing, with planning only and no implementation.
- Read the current plan's completed web milestone, contracts, task semantics and
  phase dependencies. Preserve their implementation history and blocked checks.
- Expanded and saved each phase separately in order: 10, 11, 12, 13, 14, 15, 16,
  17, 18, 19 and 20. Each includes approach, boundaries, resource/state handling,
  small implementation slices, failure tests and exit evidence requirements.
- Proposed files/commands remain future implementation guidance, not executed work.
- This session changes only `plan.md`; no application, agent, environment, VM,
  dependency or package changes and no application test/CI reruns are required.
- Verified all eleven expanded phase headings, numbered slices and exit gates;
  `git diff --check` passed and only `plan.md` is modified. Publish this planning
  checkpoint; implementation still starts at P10.1 in the prepared environment.

### Session 023 — Phase 9 web milestone verification (P9.1–P9.5) (2026-10-13)

- Completed **all of phase 9** (P9.1–P9.5) and the web milestone. P9.1: added
  `WebRegressionTest` covering the web acceptance matrix (guest/ordinary/stale-MFA
  gates, recent-auth, Livewire authorization, ID tampering, escaped task output,
  secret non-rendering, job payload hygiene). P9.2: added `QueueInterruptionTest`
  (outbox redispatch, concurrent-lease no double-submit, unknown-not-success with no
  replay, accurate cancellation). P9.3: escaped-output/secret-leak assertions plus a
  recorded blocked real-browser review. P9.4: authored `docs/agent-integration.md`
  (operation → schema → resource → permission/approval → recovery map) and augmented
  `contracts/v1/README.md` with limits, capabilities, idempotency, cancellation and
  unknown-state handling. P9.5: authored `docs/web-development.md`,
  `docs/authentication.md` and `docs/web-milestone.md`, and updated `plan.md` phases
  1–9 to complete with a final resume card.
- Phase-9 acceptance: full `php artisan test --compact` — **244 tests / 998
  assertions passed** (13 new phase-9 tests). `vendor/bin/pint`,
  `composer check:syntax`, `composer check:style`, `composer validate` (known
  license warning), `composer check-platform-reqs`, `composer audit`, `npm audit`,
  `npm run build`, `route:list`, `view:cache` and guest HTTP smokes all passed. The
  local DB gained no new schema this session. CI passes both PHP 8.3 and 8.4.
- Blocked/deferred (recorded, not claimed): real browser keyboard/screen-reader and
  375 px/desktop review; disposable-VM install/systemd/TLS/upgrade/restore/isolation
  gates; Rust toolchain. Rust begins only when the user supplies/resumes the agent
  environment (phase 10), then phase 11 transport/version negotiation.
- Files: `tests/Feature/{WebRegressionTest,QueueInterruptionTest}.php`,
  `docs/{web-development,authentication,agent-integration,web-milestone}.md`,
  `contracts/v1/README.md`, `plan.md`; both READMEs and `AGENTS.md`.
- Feature commit `4ec19da` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38372991483>,
  verified with `gh run watch 38372991483 --exit-status`.
- Next slice: **P10.1** (Rust foundation) from section 9.12/phase 10, deferred until
  the agent environment is available; the web milestone is otherwise complete.

### Session 022 — Phase 8 server administration (P8.1–P8.6) (2026-10-13)

- Completed **all of phase 8** (P8.1–P8.6). Added the server-administration
  migration (applied locally as batch 9) for `scheduled_jobs`, `certificates`,
  `database_requests`, `backup_policies` and `backups` plus models and enums
  (`ServiceAction`, `DatabaseEngine`, `BackupScope`, `CertificateSource`,
  `RequestState`). Added `ServerService`, `ScheduleService`,
  `CertificateService`, `DatabaseService`, `BackupService` and a shared
  `TaskService::requestApproved` (UI confirmation + recent auth = approval), all
  reusing the one task pipeline.
- P8.1: read-only service/firewall snapshot plus allowlisted service-control and
  package requests (IDs resolved from the snapshot/catalog, never typed names or
  arbitrary flags), closed when the agent is unavailable. P8.2: desired schedules
  with validated cron/timezone/bounded command reference; saving never installs
  cron/systemd. P8.3: HTTP-01 issuance requests (rejecting wildcards) and
  user-supplied certificate ingestion validated locally with OpenSSL for key match,
  validity and hostname coverage, with encrypted write-only keys. P8.4: app-scoped
  database intent with panel-generated credentials (never an admin password/SQL).
  P8.5: backup policies plus simulated backups that are never labelled recoverable
  (no restore). P8.6: shared `x-ui.request-status`/`x-ui.capability-notice`
  components, task links and server-side gates.
- Also enriched `ServerSnapshot` with an optional read-only `firewall` summary and
  updated the catalog contract (`run_migrations`, certificate provider, database
  request, backup policy parameters). Added server `AuditAction` cases. UI:
  `ServerOverview`, `ScheduleIndex`, `CertificateIndex`, `DatabaseIndex`,
  `BackupIndex` under `layouts.panel` with the nav Server group and recent-auth
  routes. No root execution, no panel `.env`/secret export, no Rust.
- Checks: full `php artisan test --compact` — **231 tests / 944 assertions passed**
  (10 new phase-8 tests). `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `php artisan migrate`, `route:list`, `view:cache` and a guest HTTP smoke all
  passed. The user's port 9000 server was not restarted; the local DB gained only
  the additive server-administration tables.
- Feature commit `7635290` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38371962845>,
  verified with `gh run watch 38371962845 --exit-status`.
- Next slice: **P9.1** (complete web regression) from section 9.11.

### Session 021 — Phase 7 deployment intent/operations (P7.1–P7.5) (2026-10-13)

- Completed **all of phase 7** (P7.1–P7.5). Added the `deployments` migration
  (applied locally as batch 8) + `Deployment` model and `DeploymentState` enum;
  `App\Actions\Deployments\DeploymentPreflight` (panel-only checks: archived,
  provider/runtime, repository, git ref, SSH credential, health path, agent
  capability, domain note); and `App\Services\DeploymentService` (deploy, restart,
  rollback, selectable releases, task-derived refresh). Added `run_migrations` to
  the `application.deploy` catalog contract and deployment `AuditAction` cases.
- P7.1 preflight fails before submission for missing source/health/SSH credential/
  capability and passes for a ready app in fake mode. P7.2: the operations page
  (`/panel/applications/{id}/operations`, recent-auth) previews the non-secret plan
  and mode, fingerprints the plan, invalidates confirmation when the intent
  changes, and submits once (idempotency key dedupes double clicks). P7.3:
  deployment state is derived from the linked task, stays labelled `simulated` in
  fake mode, and unknown is never success; progress/logs come from the redacted
  task events. P7.4: restart and rollback request tasks through the same pipeline;
  rollback only accepts a known successful release of the same application and
  revoked authorization fails execution. P7.5: worker/scheduler values come from
  the immutable revision and the page states that migrations are opt-in app intent
  and code rollback is not database rollback; the panel never runs Composer/Git/
  Artisan against hosted source.
- Files: `database/migrations/2026_10_13_040000_create_deployments_table.php`,
  `app/Models/Deployment.php`, `app/Enums/DeploymentState.php`,
  `app/Actions/Deployments/DeploymentPreflight.php`,
  `app/Services/DeploymentService.php`,
  `app/Exceptions/DeploymentPreflightException.php`,
  `app/Livewire/Panel/{ApplicationOperations,DeploymentIndex,DeploymentShow}.php`
  + views, `routes/web.php`, `components/panel/nav.blade.php`,
  `application-show.blade.php`, `app/Enums/AuditAction.php`,
  `contracts/v1/catalog.json`; test `tests/Feature/DeploymentTest.php`; updated
  both READMEs and `AGENTS.md`.
- Checks: full `php artisan test --compact` — **221 tests / 898 assertions passed**
  (10 new phase-7 tests). `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `php artisan migrate`, `route:list`, `view:cache` and a guest HTTP smoke all
  passed. No host provisioning, no PHP process execution against hosted source, no
  Rust. The user's port 9000 server was not restarted.
- Feature commit `5fcb986` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38370400996>,
  verified with `gh run watch 38370400996 --exit-status`.
- Next slice: **P8.1** (managed services/packages/firewall views) from section 9.10.

### Session 020 — Phase 6 application inventory/source/secrets (P6.1–P6.5) (2026-10-13)

- Completed **all of phase 6** (P6.1–P6.5). Added the `applications`,
  `application_revisions`, `source_credentials`, `application_secrets` and
  `application_domains` migration (applied locally as batch 7) and Eloquent models;
  `Provider`/runtime allowlists in `config/hostnur.php`; the
  `ApplicationPolicy`; support validators `GitRepositoryUrl` (parser-based,
  rejects embedded credentials/local/ext schemes/query-fragment/custom ports/
  option-like or traversal paths) and `Hostname` (ASCII/punycode, label/suffix
  rules, wildcard/scheme/port/path rejection); and the single write path
  `App\Services\ApplicationService` (create, new immutable configuration
  revisions, source, credential store/rotate, secret set/delete, domain add/
  remove, archive).
- Security properties implemented and tested: UUID identity with unique immutable
  slug; immutable revisions so an active task's revision cannot mutate; bounded
  validated settings with relative-path enforcement; Git credentials and env
  secrets stored with encrypted casts, hidden from JSON/serializers/snapshots and
  never written to audit metadata; write failure preserves the previous value;
  normalized globally-unique domains; archiving refused while an application has
  active tasks and otherwise retaining all history. Added application `AuditAction`
  cases and the `ApplicationConflictException`. UI: `ApplicationIndex` (create +
  list) and `ApplicationShow` (configuration/source/credentials/secrets/domains/
  archive) under `layouts.panel`, plus the navigation link.
- Files: the application migration, `app/Models/{Application,ApplicationRevision,
  SourceCredential,ApplicationSecret,ApplicationDomain}.php`,
  `app/Enums/{SourceCredentialType,DomainHttpsPolicy,SourceStatus}.php`,
  `app/Policies/ApplicationPolicy.php`, `app/Support/{GitRepositoryUrl,Hostname}.php`,
  `app/Services/ApplicationService.php`,
  `app/Exceptions/ApplicationConflictException.php`,
  `app/Livewire/Panel/{ApplicationIndex,ApplicationShow}.php` + views,
  `routes/web.php`, `components/panel/nav.blade.php`, `config/hostnur.php`,
  `app/Enums/AuditAction.php`; tests
  `tests/Unit/{GitRepositoryUrlTest,HostnameTest}.php` and
  `tests/Feature/ApplicationInventoryTest.php`; updated both READMEs and `AGENTS.md`.
- Checks: full `php artisan test --compact` — **211 tests / 861 assertions passed**
  (49 new phase-6 tests). `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `php artisan migrate`, `route:list` and `view:cache` all passed. The user's port
  9000 server was not restarted; the local DB gained only the additive application
  tables. No host provisioning, no network check on save, no Rust.
- Feature commit `b405b68` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38343245245>,
  verified with `gh run watch 38343245245 --exit-status`.
- Next slice: **P7.1** (deployment schema/preflight) from section 9.9.

### Session 019 — Phase 5 UI shell/dashboard/task visibility (P5.1–P5.4) (2026-10-13)

- Completed **all of phase 5** (P5.1–P5.4). P5.1: added
  `resources/views/layouts/panel.blade.php` with a labelled navigation shell
  (`resources/views/components/panel/nav.blade.php`), a skip link, flash status,
  and reusable components `x-ui.button`, `x-ui.panel`, `x-ui.badge`,
  `x-ui.task-status`, `x-ui.empty-state`, `x-ui.stat` and a keyboard-operable,
  labelled `x-ui.confirm` dialog (used for MFA disable/regeneration). All
  authenticated panel pages now use `layouts.panel`.
- P5.2: task list gained status/operation filters and paging; task detail loads
  events with a bounded cursor plus "load more" and polls (`wire:poll.5s`) only
  while active; audit list gained action/actor filters and a new
  `/panel/audit/{event}` detail page. P5.3: the dashboard reads a single
  short-cached (`hostnur.dashboard.cache_seconds`) adapter snapshot and shows CPU,
  memory, disk, load, uptime, hostname/OS, managed services, active tasks and
  recent failures, with a persistent fake-mode banner, `—` for unavailable values
  plus the agent reason, and stale labelling; `ServerSnapshot` gained optional
  `hostname`/`os`.
- P5.4: responsive tables wrapped in `overflow-x-auto`, captions and `scope`
  headers, skip link, `aria-current` navigation, `role`/`aria-live` status text,
  and an `aria-modal`/`aria-labelledby` confirmation dialog; status is conveyed by
  text as well as colour. Added `[x-cloak]` and broadened Tailwind `@source` roots.
- Files: `resources/views/layouts/panel.blade.php`,
  `resources/views/components/panel/nav.blade.php`,
  `resources/views/components/ui/*`, updated panel Livewire components and views
  (`Dashboard`, `TaskIndex`, `TaskShow`, `AuditIndex`, new `AuditShow`,
  `SecuritySettings`, `ConfirmPassword`, `MfaEnrollment`), `routes/web.php`,
  `resources/css/app.css`, `config/hostnur.php`, `app/Agent/Dto/ServerSnapshot.php`,
  `contracts/v1/` server-snapshot schema/fixture, and tests
  `tests/Feature/{PanelShellTest,TaskVisibilityTest,DashboardTest,AccessibilityTest}.php`
  plus `tests/Support/InteractsWithPanel.php`; updated both READMEs and `AGENTS.md`.
- Checks: full `php artisan test --compact` — **161 tests / 765 assertions passed**
  (17 new phase-5 tests). `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `route:list`, `view:cache` and a real guest HTTP smoke (`/panel*` redirect to
  `/login`, `/login` 200) all passed. The user's port 9000 server was not started
  and no migration was needed. No Rust.
- Feature commit `7db8121` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38335876307>,
  verified with `gh run watch 38335876307 --exit-status`.
- Follow-up fix (same session): the dashboard cached the `ServerSnapshot` DTO
  object, which Laravel 13's database cache store unserialized into
  `__PHP_Incomplete_Class` (its `serializableClasses` allowlist is empty). The
  dashboard now caches `ServerSnapshot::toArray()` plain data and rebuilds the DTO
  via `fromArray`; added a database-store dashboard test that would catch the
  regression. Commit `dcb25ba`; CI passed
  (<https://github.com/maelzx/hostnur/actions/runs/38337001830>). Cache/config/view
  cleared on the running dev server.
- Next slice: **P6.1** (application schema/inventory/policy) from section 9.8.

### Session 018 — Phase 4 tasks/queue/audit (P4.1–P4.6) (2026-10-13)

- Completed **all of phase 4** (P4.1–P4.6) over the existing agent contract.
  P4.1: added `TaskStatus` (central transition rules), `TaskEventLevel`,
  `AuditAction`, `AuditOutcome` enums; `Task`/`TaskEvent`/`AuditEvent` models and
  the `tasks`/`task_events`/`audit_events` migration (applied locally as batch 6),
  with history-preserving nullable FKs and append-only audit.
- P4.2: `App\Services\TaskService::request/approve/deny` plus `App\Services\
  AuditRecorder`; dedupe on actor + idempotency key with payload fingerprint
  conflict detection, recent-auth and approval gating for non-read-only operations.
  P4.3: `App\Jobs\RunTask` (carries only the task UUID), short conditional lease
  claim/release, dispatch-after-commit and `hostnur:tasks:reconcile` (scheduled
  every minute) for the pending outbox and expired leases.
- P4.4: `TaskService::poll` inspects the adapter, `TaskStatus::fromOperation()` maps
  adapter statuses, events are deduplicated by sequence, bounded by
  `hostnur.tasks.max_events_per_task` and redacted/truncated by `App\Support\
  Redactor` (structured allowlist + secret patterns). P4.5: cancel (local vs
  agent-acknowledged), retry as a new linked task, unknown/timeout/failed
  outcomes, expired-approval return to `awaiting_approval`, and revoked-role
  submissions failing closed with `permission_denied`.
- P4.6: `App\Listeners\AuthenticationEventSubscriber` records Laravel/Fortify
  login/MFA/recovery events (never secrets); explicit audits added for password
  change, bootstrap and CLI recovery; read-only paginated history at
  `/panel/tasks`, `/panel/tasks/{id}` and `/panel/audit` behind
  auth + administrator + MFA.
- Files: `app/Enums/{TaskStatus,TaskEventLevel,AuditAction,AuditOutcome}.php`,
  `app/Models/{Task,TaskEvent,AuditEvent}.php`, the task/audit migration,
  `app/Services/{TaskService,AuditRecorder}.php`, `app/Support/Redactor.php`,
  `app/Jobs/RunTask.php`, `app/Console/Commands/ReconcileTasks.php`,
  `app/Listeners/AuthenticationEventSubscriber.php`, `app/Exceptions/*`,
  `app/Livewire/Panel/{TaskIndex,TaskShow,AuditIndex}.php` + views, routes,
  `routes/console.php`, `config/hostnur.php`, `app/Providers/AppServiceProvider.php`
  and the security/bootstrap/recovery commands; tests in `tests/Unit/
  TaskStatusTest.php` and `tests/Feature/{TaskRequestTest,TaskExecutionTest,
  TaskAuditUiTest}.php`; updated both READMEs and `AGENTS.md`.
- Checks: full `php artisan test --compact` — **144 tests / 687 assertions
  passed** (30 new phase-4 tests). `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `php artisan migrate`, `route:list`, `view:cache` and
  `hostnur:tasks:reconcile` (0 dispatched, 0 leases) all passed; a real guest HTTP
  smoke redirected `/panel/tasks` and `/panel/audit` to `/login`. The user's port
  9000 server was not started; the local DB gained only the additive task/audit
  tables. No Rust.
- Feature commit `9deae38` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38335015107>,
  verified with `gh run watch 38335015107 --exit-status`.
- Next slice: **P5.1** (protected panel shell and components) from section 9.7.

### Session 017 — P3.5 contract regression (2026-10-12)

- Implemented **P3.5** and completed phase 3. Added a shared fixture runner
  (`tests/Support/ContractFixtures`) and a minimal JSON Schema (2020-12 subset)
  checker (`tests/Support/JsonSchemaValidator`); added
  `contracts/v1/fixtures/invalid/manifest.json` mapping each invalid fixture to its
  expected error code, and `contracts/v1/README.md` compatibility notes (version,
  envelope, responses, error codes, modes/failure behavior, fake scenarios). The
  agent-contract suite now validates every valid fixture against its schema and
  parses it through the DTOs, and drives every invalid fixture through the expected
  machine error code.
- Phase-3 exit checks pass: PHP validates the contracts, `unavailable` fails closed,
  `fake` is local/testing-only and visibly `mode=fake`, and later flows can depend
  on `App\Contracts\AgentClient`. No network/host/exec calls; no new dependency
  (schemas are checked by the small test-only validator); no Rust.
- Files: `contracts/v1/README.md`, `contracts/v1/fixtures/invalid/manifest.json`,
  `tests/Support/ContractFixtures.php`, `tests/Support/JsonSchemaValidator.php`,
  `tests/Unit/AgentContractTest.php`; updated both READMEs and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Unit/AgentContractTest.php` —
  **8 tests / 108 assertions passed**. Full `php artisan test --compact` —
  **114 tests / 598 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit` and `npm run build`
  all passed; the manifest and fixtures parse. The user's port 9000 server and
  database were not touched.
- Feature commit `a691275` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38333551101>,
  verified with `gh run watch 38333551101 --exit-status`.
- Next slice: **P4.1** (panel task persistence and lifecycle) from section 9.6.

### Session 016 — P3.4 durable fake execution (2026-10-12)

- Implemented **P3.4**: replaced the fake adapter's `unsupported_operation` stubs
  with durable simulation. Added migration `2026_10_12_170000_...`
  (`fake_agent_executions`, applied locally as batch 5), the
  `FakeAgentExecution` model, the `FakeScenario` enum and
  `FakeExecutionSimulator`, which derives status/progress/events/result/error from
  the persisted record + scenario + controlled clock (no worker, no sleeps).
  `FakeAgentClient::submit` is idempotent on the idempotency key (unique index plus
  conflict detection), `inspect` returns the derived snapshot or
  `execution_unknown`, and `cancel` returns requested/unsupported (read-only)/
  already_finished and flips to `cancelled` on inspection. Capabilities list the
  catalog operations with `mode=fake`; the production mode guard is still enforced.
  Added `AgentContractException::{conflict,executionUnknown}` and
  `config('hostnur.fake.{scenario,duration_seconds,timeout_seconds}')`.
- Files: the new migration, `app/Models/FakeAgentExecution.php`,
  `app/Enums/FakeScenario.php`, `app/Agent/FakeExecutionSimulator.php`,
  `app/Agent/FakeAgentClient.php`, `app/Agent/AgentClientFactory.php`,
  `app/Agent/AgentContractException.php`, `config/hostnur.php`,
  `tests/Unit/FakeAgentTest.php` (removed the unused fake capability fixture);
  updated both READMEs and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Unit/FakeAgentTest.php` —
  **12 tests / 35 assertions passed**. Full `php artisan test --compact` —
  **122 tests / 564 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`
  and `php artisan migrate`/`migrate:status` all passed. The user's port 9000
  server was not started; the local DB gained only the additive fake-execution
  table. No Rust.
- Feature commit `2ee01a5` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38332243837>,
  verified with `gh run watch 38332243837 --exit-status`.
- Next slice: **P3.5** (contract regression runner and compatibility notes), the
  final phase-3 item.

### Session 015 — P3.3 read-only fake snapshots (2026-10-12)

- Implemented **P3.3**: added `FakeAgentClient`, bound for `AgentMode::Fake` in
  `AgentClientFactory` after the guard check. It returns fixture-driven
  `CapabilitySnapshot` and `ServerSnapshot` values labelled `mode=fake` with
  `sampled_at` from the app clock (freezable in tests), reads only
  `contracts/v1/fixtures/fake/`, and makes no network/host/shell/`exec` calls.
  Extracted `AgentModeGuard` (shared by the factory and the fake adapter) so fake
  mode is rejected in production at both bind and use time. Added
  `ServerSnapshot::isStale()` so fresh fake, stale and offline data are
  distinguishable; submit/inspect/cancel fail closed with `unsupported_operation`
  until P3.4.
- Added `contracts/v1/fixtures/fake/{capability,server-snapshot}.json`; no schema
  change (reuses the capability/server-snapshot schemas). No migration, no auth
  change, no durable fake state, no Rust.
- Files: `app/Agent/FakeAgentClient.php`, `app/Agent/AgentModeGuard.php`,
  `app/Agent/AgentClientFactory.php`, `app/Agent/Dto/ServerSnapshot.php`,
  `contracts/v1/fixtures/fake/*`, `tests/Unit/FakeAgentTest.php`, updated
  `tests/Unit/AgentAvailabilityTest.php`; updated both READMEs and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Unit/AgentAvailabilityTest.php
  tests/Unit/FakeAgentTest.php` — **14 tests / 42 assertions passed**. Full
  `php artisan test --compact` — **116 tests / 551 assertions passed**.
  `vendor/bin/pint`, `composer check:syntax`, `composer check:style`,
  `composer validate` (known license warning), `composer check-platform-reqs`,
  `composer audit`, `npm audit` and `npm run build` all passed; all contract JSON
  fixtures parse. The user's port 9000 server and database were not touched.
- Feature commit `c54d991` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38274611714>,
  verified with `gh run watch 38274611714 --exit-status`.
- Next slice: **P3.4** (durable fake execution) from section 9.5.

### Session 014 — P3.2 unavailable adapter and mode guard (2026-10-12)

- Implemented **P3.2**: added `App\Contracts\AgentClient` (capabilities/snapshot/
  submit/inspect/cancel), the `UnavailableAgentClient` (reports
  `AgentMode::Unavailable`, no capabilities/observations, throws
  `AgentUnavailableException` for submit/inspect/cancel), and `AgentClientFactory`
  selecting the adapter from `config('hostnur.agent_mode')`. `real` resolves to the
  unavailable adapter with a phase-11 reason; `fake` is guarded by
  `assertFakeModeAllowed()` (local/testing only) and currently reports "not
  implemented yet" rather than silently falling back; unknown modes are rejected.
  Added `AgentConfigurationException`, the `CancellationStatus` enum, and the
  `ServerSnapshot`/`CancellationReceipt` DTOs needed by the interface.
- Bound `AgentClient` through the factory in `AppServiceProvider`, added
  `contracts/v1` schemas/fixtures for server snapshot and cancellation receipt, and
  a `max_observations` limit. No migration, no auth change, no persistent fake
  state (P3.3/P3.4), no Rust.
- Files: `app/Contracts/AgentClient.php`, `app/Agent/UnavailableAgentClient.php`,
  `app/Agent/AgentClientFactory.php`, `app/Agent/Exceptions/*`,
  `app/Agent/Dto/{ServerSnapshot,CancellationReceipt}.php`,
  `app/Enums/CancellationStatus.php`, `app/Providers/AppServiceProvider.php`,
  `config/hostnur.php`, `contracts/v1/schemas/*`, `contracts/v1/fixtures/valid/*`,
  `tests/Unit/AgentAvailabilityTest.php`, updated `tests/Unit/AgentContractTest.php`;
  updated both READMEs and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Unit/AgentAvailabilityTest.php
  tests/Unit/AgentContractTest.php` — **24 tests / 94 assertions passed**. Full
  `php artisan test --compact` — **110 tests / 529 assertions passed**.
  `vendor/bin/pint`, `composer check:syntax`, `composer check:style`,
  `composer validate` (known license warning), `composer check-platform-reqs`,
  `composer audit`, `npm audit` and `npm run build` all passed. The user's port 9000
  server and database were not touched.
- Feature commit `fdf2beb` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38273972491>,
  verified with `gh run watch 38273972491 --exit-status`.
- Next slice: **P3.3** (read-only fake snapshots) from section 9.5.

### Session 013 — P3.1 agent DTOs/catalog/schema (2026-10-12)

- Implemented **P3.1**: the typed agent contract with no adapter yet. Added backed
  enums `AgentMode`, `OperationStatus`, `ResourceKind` and `ErrorCode`; DTOs
  `ResourceRef`, `OperationRequest`, `OperationReceipt`, `OperationSnapshot`,
  `CapabilitySnapshot` and `AgentError`; `OperationCatalog` loading
  `contracts/v1/catalog.json`; `OperationRequestValidator` (strict envelope keys,
  UUID/type checks, protocol allowlist, operation allowlist, resource/parameter
  schemas, message/parameter bounds) and `AgentContractException` carrying the
  machine error code.
- Added versioned contract files under `contracts/v1/`: JSON Schemas
  (`operation-request`, `operation-receipt`, `operation-snapshot`,
  `capability-snapshot`), `catalog.json` for the 12 initial operations and
  valid/invalid fixtures. Extended `config/hostnur.php` with `agent_mode`
  (default `unavailable`), `protocol_version`, `contracts_path` and `limits`, bound
  `OperationCatalog` as a singleton, and added `contracts/**` to the CI path filter.
- Files: `app/Enums/*`, `app/Agent/**`, `config/hostnur.php`,
  `app/Providers/AppServiceProvider.php`, `contracts/v1/**`,
  `tests/Unit/AgentContractTest.php`, `.github/workflows/web-panel.yml`; updated
  both READMEs and `AGENTS.md`. No migration, no auth change and no adapter/fake
  (those are P3.2–P3.4); no Rust.
- Focused checks: `php artisan test --compact tests/Unit/AgentContractTest.php` —
  **16 tests / 66 assertions passed**. Full `php artisan test --compact` —
  **102 tests / 501 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit` and `npm run build`
  all passed. The user's port 9000 server and database were not touched.
- Feature commit `07f9d09` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38273266956>,
  verified with `gh run watch 38273266956 --exit-status`.
- Next slice: **P3.2** (unavailable adapter and mode-selection guard) from section
  9.5.

### Session 012 — P2.6 local recovery (2026-10-11)

- Implemented **P2.6**: `hostnur:admin:recover`, a deliberate interactive Artisan
  command (no web route) that resets an existing administrator password. It refuses
  non-interactive input, unknown emails and ordinary accounts (no creation or
  promotion), requires an explicit confirmation, validates the new password with
  the bootstrap policy (min 12, max 72 bytes, no null bytes, confirmed) via hidden
  input, and never prints secrets.
- In one transaction the command re-checks the `is_admin` target, stores the new
  hashed password, increments `mfa_version` and deletes the account's stored
  sessions (guarded by a `sessions.user_id` check). MFA columns and the
  `panel_setup` marker are untouched, so MFA is never silently cleared and setup
  cannot be reopened.
- Documented the recovery runbook and ordered MFA lost-code handling in
  `panel/README.md` (recovery code -> CLI password recovery with MFA preserved ->
  re-enroll only if the device and all codes are lost). Updated `AGENTS.md` and the
  root README; phase 2 is now complete.
- Files: `app/Console/Commands/RecoverAdministrator.php`,
  `tests/Feature/RecoverAdministratorTest.php`, `panel/README.md`,
  `panel/AGENTS.md`, root `README.md`.
- Focused checks: `php artisan test --compact tests/Feature/RecoverAdministratorTest.php`
  — **8 tests / 65 assertions passed**. Full `php artisan test --compact` —
  **86 tests / 435 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `artisan list`/`help` all passed. No new migration, dependency or web route; the
  user's local database and port 9000 server were not touched.
- Feature commit `88c2dcd` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38272127148>,
  verified with `gh run watch 38272127148 --exit-status`.
- Next slice: **P3.1** (agent DTOs/catalog/schema) from section 9.5, the first
  phase-3 item. Phase 2 exit criteria (setup/login/recovery, no public
  registration or default password, unauthenticated users cannot invoke
  administrative actions) are met.

### Session 011 — P2.5 recent-auth and security settings (2026-10-11)

- Implemented **P2.5**: server-side recent-password confirmation and a security
  settings screen. Added `config/hostnur.php` with `recent_auth_timeout` (300s)
  and aligned `auth.password_timeout` to 300s so Laravel's `password.confirm`
  middleware uses the same window. Added `App\Services\RecentAuthentication`
  storing `auth.password_confirmed_at` plus an MFA-generation-bound
  `auth.password_confirmed_mfa_version`, and `EnsureRecentAuthentication`
  (alias `recent-auth`) that redirects to the confirmation screen.
- New `/user/confirm-password` Livewire screen (`password.confirm`, name matches
  Fortify/Laravel redirects) sets the recent-auth evidence. `config/fortify.php`
  now enables `confirmPassword => true` so Fortify's own `/user/two-factor-*`
  endpoints also require recent password, closing that bypass.
- New `/panel/security` page (`auth + administrator + mfa + recent-auth`) with:
  change password (bootstrap-equivalent policy), disable MFA, and regenerate
  recovery codes. Changing the password or disabling MFA increments `mfa_version`,
  clears the confirmation and signs out the session (invalidating other sessions
  via the generation check); new recovery codes are flashed and rendered once.
  Each mutation re-checks recent auth in-component in addition to the route and
  persistent Livewire middleware. `EnsureRecentAuthentication` is registered as
  persistent Livewire middleware alongside the existing gates.
- Files: `config/hostnur.php`, `config/auth.php`, `config/fortify.php`,
  `app/Services/RecentAuthentication.php`,
  `app/Http/Middleware/EnsureRecentAuthentication.php`,
  `app/Livewire/Panel/ConfirmPassword.php`,
  `app/Livewire/Panel/SecuritySettings.php` and their views,
  `app/Providers/AppServiceProvider.php`, `bootstrap/app.php`, `routes/web.php`,
  `.env.example`, dashboard/login views, `tests/Feature/SecuritySettingsTest.php`;
  updated both READMEs and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Feature/SecuritySettingsTest.php`
  — **17 tests / 55 assertions passed**. Full `php artisan test --compact` —
  **78 tests / 370 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `route:list` and `view:cache` all passed. Real HTTP smoke on an isolated port
  confirmed guests are redirected to `/login` for `/`, `/panel`,
  `/panel/security`, `/user/confirm-password` and `/two-factor-challenge`. No new
  migration and no local database change were required; the user's port 9000
  server was not started.
- Deferred and recorded: the deliberate local CLI password/MFA recovery runbook
  (P2.6) and atomic recovery-code consumption under concurrency (not required
  sequentially). Requiring recent password before initial MFA provisioning remains
  a possible hardening follow-up; enrollment already requires the account password
  at login.
- Feature commit `1ede09d` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38271393744>,
  verified with `gh run watch 38271393744 --exit-status`.
- Next slice: **P2.6** (local recovery and runbook) from section 9.4.

### Session 010 — P2.4 MFA login challenge (2026-10-11)

- Implemented **P2.4**: the generation-bound TOTP/recovery challenge and the
  `/panel` MFA gate. Added migration `2026_10_11_160000_...` for `mfa_version`
  (unsigned integer, default 0), applied locally as batch 4, and cast it as
  integer. Re-added Fortify's `RedirectsIfTwoFactorAuthenticatable` to
  `config/fortify.php` `pipelines.login` so a confirmed administrator is sent to
  the challenge instead of completing the session.
- Registered the app-owned GET `two-factor-challenge` route
  (`App\Livewire\Auth\MfaChallenge`, `guest:web`, name `two-factor.login`) because
  `views => false` means Fortify does not register it. The screen rejects a request
  without a started challenge and renders a TOTP field plus an optional recovery
  code; Fortify still owns POST `two-factor.login.store`. Added a named
  `two-factor` limiter (5/min per challenged user + IP).
- Added `App\Http\Middleware\EnsureMfaVerified` (alias `mfa`) and split the panel
  routes: `/panel/security/mfa` stays `auth + administrator` (exempt to avoid
  loops) while `/panel` now also requires `mfa`. The gate sends an unconfirmed
  administrator to enrollment and logs out a confirmed administrator whose
  generation-bound proof is missing or stale.
- Session proof is set only after a valid factor: an event listener on
  `ValidTwoFactorAuthenticationCodeProvided` stores `mfa_verified` (user id +
  `mfa_version`) before the session regenerates, and successful enrollment
  confirmation stores the same proof. The gate requires the proof to match the
  current user id and `mfa_version`, so rotating `mfa_version` (password/MFA
  change, later slices) invalidates old sessions. Registered `EnsureAdministrator`
  and `EnsureMfaVerified` via `Livewire::addPersistentMiddleware` so Livewire
  update requests re-run the same route gates.
- Files: new migration, `app/Livewire/Auth/MfaChallenge.php` + view,
  `app/Http/Middleware/EnsureMfaVerified.php`,
  `app/Providers/FortifyServiceProvider.php`, `app/Providers/AppServiceProvider.php`,
  `app/Livewire/Auth/MfaEnrollment.php`, `config/fortify.php`, `bootstrap/app.php`,
  `routes/web.php`, `tests/Feature/MfaChallengeTest.php`; updated the P2.2
  authentication tests for the new MFA-gated behavior, both READMEs, `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Feature/MfaChallengeTest.php`
  — **13 tests / 54 assertions passed**. Full `php artisan test --compact` —
  **61 tests / 315 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `php artisan migrate`, `route:list` and `view:cache` all passed. Real HTTP smoke
  on an isolated port confirmed `/`, `/panel` and `/two-factor-challenge` redirect
  guests to `/login` and `/login` returns 200. The user's port 9000 server and
  database rows were not modified beyond the additive `mfa_version` column.
- Deferred and recorded: recent-password confirmation for sensitive changes and
  MFA recovery/session revocation (P2.5), and a deliberate local CLI
  password/MFA recovery runbook (P2.6). Recovery-code consumption is single-use
  sequentially; concurrent reuse is not yet proven atomic.
- Feature commit `c2144d6` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38270389076>,
  verified with `gh run watch 38270389076 --exit-status`.
- Next slice: **P2.5** (recent-auth/security settings) from section 9.4.

### Session 009 — P2.3 MFA enrollment (2026-10-11)

- Implemented **P2.3** using the already-installed Fortify two-factor actions
  (`EnableTwoFactorAuthentication`, `ConfirmTwoFactorAuthentication`) rather than
  new cryptography. Added migration `2026_10_11_150000_...` for encrypted
  `two_factor_secret`, `two_factor_recovery_codes` and `two_factor_confirmed_at`;
  applied it locally as batch 3. Added Fortify's `TwoFactorAuthenticatable` trait
  to `User`, `two_factor_confirmed_at` datetime cast, and hid the secret/recovery
  columns from serialization.
- Enabled Fortify's `two-factor-authentication` feature with `confirm => true`,
  `confirmPassword => false` and a single-step `window => 1`. The secret and
  recovery codes are encrypted at rest by Fortify's encrypter (ciphertext text
  columns, no double model cast). `hasEnabledTwoFactorAuthentication()` is false
  until a valid authenticator code confirms the configuration.
- **Deferred login enforcement deliberately.** Because enabling the feature would
  normally add `RedirectsIfTwoFactorAuthenticatable` to the login pipeline before
  P2.4's challenge screen exists, `config/fortify.php` now defines an explicit
  `pipelines.login` (`CanonicalizeUsername`, `AttemptToAuthenticate`,
  `PrepareAuthenticatedSession`) without the two-factor redirect. P2.4 adds the
  redirect + challenge + `/panel` MFA gate; until then an enrolled administrator
  still signs in with a password only and `/panel` remains non-MFA-gated.
- New `App\Livewire\Auth\MfaEnrollment` full-page screen at
  `/panel/security/mfa` (route `panel.security.mfa`, `auth:web` + `administrator`)
  re-authorizes the database administrator in render and every mutation. It shows
  the locally generated QR SVG (bacon-qr-code, no external service), the setup key
  and eight recovery codes only while the secret is unconfirmed; a six-digit
  fixed-width code (leading zeroes preserved, `digits:6`) confirms enrollment.
  After confirmation the page and model expose no secret or codes (shown once).
  Dashboard now reports MFA status and links to the screen.
- Files: `app/Livewire/Auth/MfaEnrollment.php`,
  `resources/views/livewire/auth/mfa-enrollment.blade.php`, `config/fortify.php`,
  `app/Models/User.php`, `routes/web.php`,
  `resources/views/livewire/panel/dashboard.blade.php`, the new migration and
  `tests/Feature/MfaEnrollmentTest.php`; updated both READMEs and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Feature/MfaEnrollmentTest.php`
  — **12 tests / 43 assertions passed**. Full `php artisan test --compact` —
  **48 tests / 259 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit`, `npm run build`,
  `php artisan migrate`, `route:list` and `view:cache` all passed. The user's port
  9000 server was not started; the local database only received the additive
  nullable columns.
- Deferred and recorded for later slices: recent-password confirmation before
  showing provisioning data (P2.5), the generation-bound challenge and `/panel`
  MFA gate (P2.4), atomic recovery-code consumption verification and requiring
  recent auth to regenerate codes (P2.4/P2.5). Fortify's own `/user/two-factor-*`
  HTTP endpoints remain reachable by the single authenticated administrator; the
  UI uses the actions directly and no non-admin can authenticate.
- Feature commit `a991456` pushed; remote CI **passed** for PHP 8.3/Node 24 and
  PHP 8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38147195019>,
  verified with `gh run watch 38147195019 --exit-status`.
- Next slice: **P2.4** (MFA login challenge) from section 9.4.

### Session 008 — P2.2 login/logout foundation (2026-10-11)

- User approved adding the project-local `laravel/fortify` dependency (the P2.2
  checkpoint from section 9.4). Installed **v1.41.0** with `^1.41` and committed
  the lock; verified from Packagist metadata that it requires PHP `^8.2` and
  `illuminate/support ^11.0|^12.0|^13.0` (the local `composer show -a` text showed
  stale constraints, so the lockfile resolution and Packagist JSON were used).
- Implemented the P2.2 slice: app-owned GET `/login` Livewire screen plus Fortify's
  POST login/logout endpoints, a guest/administrator-gated `/panel` placeholder and
  the root redirect. Only Fortify's base login/logout routes are active; its
  optional features array is empty and `views => false` prevents unimplemented
  Fortify login/two-factor/password-confirm view routes from being registered.
- `Fortify::authenticateUsing` authenticates only an existing `is_admin` account;
  unknown email, ordinary account and wrong password all fail with the same generic
  `auth.failed` message. Email is trimmed and lowercased to match the bootstrap
  command. No remember-me; Fortify regenerates the session ID after login and
  invalidates the session plus regenerates the CSRF token on logout.
- Named `login` limiter: 5 attempts/minute per normalized email + IP and 30/minute
  per IP, configured in `App\Providers\FortifyServiceProvider`; `EnsureAdministrator`
  re-reads the database user each request so role removal takes effect immediately.
- Files: `config/fortify.php`; `app/Providers/FortifyServiceProvider.php` (registered
  in `bootstrap/providers.php`); `app/Http/Middleware/EnsureAdministrator.php`
  (alias in `bootstrap/app.php`); `app/Http/Controllers/HomeController.php`;
  `app/Livewire/Auth/Login.php`, `app/Livewire/Panel/Dashboard.php` and their views;
  rewrote `routes/web.php`; removed the obsolete public `App\Livewire\Foundation`
  component/view; updated `README.md` and `AGENTS.md`.
- Focused checks: `php artisan test --compact tests/Feature/AuthenticationTest.php`
  — **14 tests / 60 assertions passed**. Full `php artisan test --compact` —
  **36 tests / 216 assertions passed**. `vendor/bin/pint`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning),
  `composer check-platform-reqs`, `composer audit`, `npm audit` and `npm run build`
  passed. `php artisan route:list` confirmed app `login`/`panel.dashboard` plus
  Fortify `login.store`/`logout`; `view:cache` succeeded. Real HTTP smoke on an
  isolated port: `/` and `/panel` returned 302 to `/login`, `/login` returned 200
  with a CSRF token. The user's port 9000 server and database were not touched.
- CSRF is verified the way the phase requires: the framework skips CSRF under the
  testing environment, so a production-like test flips the app env and asserts a
  419 for login/logout POSTs without a token. MFA is not implemented yet, so the
  `/panel` placeholder is deliberately not MFA-gated and exposes no operations;
  Livewire-update middleware persistence is deferred until a component mutates state.
- Feature commit `f8352d8` pushed; remote CI **passed** for PHP 8.3/Node 24 and PHP
  8.4/Node 24 (locked installs, static gates, asset build, migration,
  template/queue and tests): <https://github.com/maelzx/hostnur/actions/runs/38146321207>,
  verified with `gh run watch 38146321207 --exit-status`.
- Next slice: **P2.3** (MFA enrollment) from section 9.4. Phase 2 remains in progress.

### Session 007 — Detailed DeepSeek handoff (2026-10-11)

- User requested a lower-cost development handoff and detailed implementation
  planning through phase 9, immediately before Rust development.
- Expanded section 9 with current-state inventory, low-cost session protocol,
  implementation boundaries, proposed file/data maps, slice-by-slice tasks,
  security/interaction rules, tests and acceptance gates for every web phase.
- This session changes documentation only. Application code remains the bootstrap
  implementation tested in CI run `38142904116` on commit `3939afd`.
- Next development item: **P2.2**, subject to the small authentication-package
  decision described in section 9.4. Preserve the existing bootstrap and port 9000.
- Reviewed document consistency/phase dependencies and added the auth dependency
  checkpoint to section 6. Verified all nine detailed phase subsections and
  `git diff --check`; only `plan.md` and the README handoff link changed. Commit/push
  this handoff; unchanged application tests/assets require no additional run.

### Session 006 — One-time administrator bootstrap (2026-10-11)

- Implemented `hostnur:admin:bootstrap`, interactive name/email plus hidden
  password/confirmation with visible fallback disabled; there is no password flag.
- Added `is_admin` default false/non-mass-assignable and durable `panel_setup`
  marker. Conditional marker claim and account save occur in one DB transaction;
  rejection, exceptions or cancelled model saves roll back the claim/account.
- Refuses existing accounts/completed or missing state/non-interactive creation.
  Deleting an administrator does not reopen setup. No default credentials or
  public web setup route; login/MFA remain upcoming and port stays 9000.
- Validates trimmed name, normalized RFC email, confirmation, password minimum
  12 characters, maximum 72 bytes and no null bytes; stores passwords hashed and
  does not claim the email has been verified. Error output excludes raw exceptions
  and entered secrets.
- Local focused suite passed: `php artisan test --compact
  tests/Feature/BootstrapAdministratorTest.php` — 20 tests / 153 assertions.
  Full `php artisan test --compact` — 22 tests / 155 assertions.
- Tested validation boundaries, refusal/repeat/deletion/missing state, hashed
  storage, role mass-assignment prevention, creation exception/cancellation
  rollback, hidden-input settings and interleaved competing setup/account writes
  against isolated SQLite. No real administrator created by tests.
- Passed `vendor/bin/pint --dirty --format agent`, `composer check:syntax`,
  `composer check:style`, `composer validate` (known license warning), and Git
  whitespace checks. No new tools or project dependencies were installed.
- Applied `php artisan migrate --no-interaction` to local panel DB. Verified with
  `php artisan migrate:status --no-interaction`, `php artisan db:show --counts
  --no-interaction` and command `--help`: migration ran, one setup-state row exists,
  zero users, and no password/force option. User must select credentials interactively.
- Updated panel/root README and project guidance. Feature commit `3939afd` pushed.
- Remote CI **passed** for PHP 8.3/Node 24 and PHP 8.4/Node 24, including locked
  installs, static gates, asset build, new migration, template/queue checks and tests:
  <https://github.com/maelzx/hostnur/actions/runs/38142904116>.
- Verified with `gh run watch 38142904116 --exit-status --interval 30`. Bootstrap
  slice is complete; phase 2 remains in progress for login/MFA and recovery work.
- Publication: save/push this documentation-only checkpoint. No application code
  changes follow the successful CI run.

### Session 005 — Web-only CI and static checks (2026-10-11)

- Added `.github/workflows/web-panel.yml`: Ubuntu 24.04 PHP 8.3/8.4 matrix,
  Node 24, Composer 2.10.3, locked installs, audits, syntax/style gates, asset build,
  isolated SQLite initialization/migrations, view compilation, queue startup/tests.
- Triggers: pushes to `main` and pull requests affecting `panel/**`, this workflow
  or `.nvmrc`, plus manual dispatch. Read-only workflow token, pinned action commits,
  timeout and per-ref concurrency are configured; there are no Rust jobs.
- Added Composer `check:syntax` and `check:style` commands with existing tools.
  Syntax checks tracked PHP; style runs Pint then rejects uncommitted PHP drift.
- User reports running the local panel on port 9000. Updated run instructions and
  committed `.env.example` to 9000; user's local `.env` and running server untouched.
- Local validation passed: `composer validate` (known license warning),
  `composer check:syntax`, `composer check:style`, `composer check-platform-reqs`,
  `composer audit`, `php artisan test --compact` (2 tests / 2 assertions),
  `npm ci --no-fund`, `npm audit`, `npm run build`, and `git diff --check`.
- Parsed workflow YAML with already installed PyYAML and checked triggers, matrix
  and permissions. GitHub Actions is enabled on the repository. No local tools
  installed/upgraded and no direct dependency constraints changed.
- First CI run on `40356b3` failed at clean frontend installation, after both PHP
  versions passed Composer/platform and syntax/style gates:
  <https://github.com/maelzx/hostnur/actions/runs/38119646698>.
- CI's npm 11.19.0 exposed missing optional React and bundled WASI lock entries.
  Regenerated the lock from scratch with existing npm 11.6.2 in a clean temporary
  Git worktree (no node_modules), applied its minimal diff and retained root
  dependency constraints. No npm/runtime upgrade or dependency-policy bypass.
- `npm ci --no-fund` and `npm run build` passed with the repaired lock in both the
  current panel and clean worktree; installation audits reported no advisories.
- Workflow commit `40356b3` and repaired-lock commit `6e0a998` were pushed.
- Final remote result: **success**, both PHP 8.3/Node 24 and PHP 8.4/Node 24 jobs,
  including all dependency/static/build/migration/template/queue/test stages:
  <https://github.com/maelzx/hostnur/actions/runs/38119851396>.
- Verified using `gh run watch 38119851396 --exit-status --interval 30`. This is
  actual PHP 8.3 and 8.4 execution, not emulated Composer platform resolution.
- Phase 1 is complete. Strict Composer validation still emits the known license
  warning; it is not claimed as passing and the release license decision remains.
- Publication: save/push this documentation-only handoff checkpoint; code tested
  by the successful run is unchanged. Temporary lock-repair worktree was removed
  after inspection; `git worktree list` contains only the main workspace.

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

**The web milestone (phases 1–9) and the phase-10 Rust foundation are complete.**
The panel is independently runnable and tested; the agent crate builds, tests and
runs unprivileged in the Lima VM. See `docs/web-milestone.md` and
`docs/agent-development.md` for passed gates, blocked checks and acceptance maps.

**Phases 11–13 are complete for their chosen scopes** (sessions 026–028). The next
work is **phase 14 (PHP/Laravel provider and transactional Nginx routing)**, which
needs the approved runtime/package and web-server policies ratified first (native
Ubuntu 24.04 PHP 8.3/PHP-FPM baseline, approved package-install primitive, and the
managed Nginx/FPM template rules). Continue the agent in the Lima VM (`agent/`)
with the root-run harness for privileged slices; the PHP panel runs on the host.
Local web work continues on port 9000 with `HOSTNUR_AGENT_MODE=fake`. Recorded
follow-ups: live PHP↔Rust end-to-end, amd64 VM evidence, and live private-repo
clone.

Push the phase-10 commit to observe the agent CI workflow; a real CI run is still
outstanding. The remaining genuine web gates (real browser keyboard/screen-reader
and 375 px/desktop review; disposable-VM install/systemd/TLS/upgrade/restore/
isolation) are unchanged and recorded in `docs/web-milestone.md`.

### Current blockers

- No blocker to local web work; the full suite and static gates pass locally and on
  both CI runtimes. Prior Composer-tool diagnostic findings remain recorded, and the
  shared tool is not to be changed under the user's environment constraint.
- Strict Composer validation awaits a project license decision; the normal
  metadata check passes with that warning. PHP 8.3/8.4 CI now passes.
- Rust is available in the prepared Lima Ubuntu 24.04 **arm64** VM; the pinned
  1.99.0 toolchain was verified there. Phases 10 and 11 are complete locally. The
  agent CI workflow's real run is pending a push (sessions committed locally only).
- Disposable Ubuntu 24.04 **amd64** VM access is still required for genuine amd64
  install/systemd/TLS/upgrade/restore/isolation gates; local phase-12 evidence is
  arm64 with amd64 covered only by the CI build target.
- A live PHP↔Rust cross-language end-to-end test (panel over the socket to the real
  agent) is a recorded follow-up; PHP is not installed in the VM.
- Phase 14 awaits ratification of the runtime/package and web-server policies:
  the native Ubuntu 24.04 PHP 8.3/PHP-FPM baseline, the internal approved
  package-install allowlist (Nginx/FPM/extensions), and the managed Nginx/FPM
  template boundaries. SSH deploy keys (P13.4) remain deferred by decision.
- Official domain, Git authentication details, database engine, release signing,
  and license await the decision checkpoints listed in section 6.

## 9. Detailed web-only implementation playbook for DeepSeek

### 9.1 Resume card, cost control and starter prompt

#### What actually exists

| Item | Existing implementation / evidence |
|---|---|
| Web foundation | `panel/`; Laravel 13.35.0, Livewire 4.4.7, Tailwind 4.3.3, Vite 8.3.4 |
| Current browser entry | `routes/web.php`: `/` redirects guests to `login` and admins to `/panel`; `layouts.panel` shell with `App\Livewire\Panel` pages (Dashboard, TaskIndex/Show, AuditIndex/Show, ApplicationIndex/Show, ApplicationOperations, DeploymentIndex/Show, ServerOverview, ScheduleIndex, CertificateIndex, DatabaseIndex, BackupIndex, SecuritySettings, ConfirmPassword) and `App\Livewire\Auth` guest/MFA pages |
| Layout/assets | `resources/views/layouts/app.blade.php`, `resources/css/app.css`, `vite.config.js`; Livewire supplies Alpine |
| Database | SQLite; database sessions/cache/queue; baseline user/cache/job migrations plus MFA, fake execution and task/audit migrations |
| Administrator bootstrap | `app/Console/Commands/BootstrapAdministrator.php`; `hostnur:admin:bootstrap` |
| Bootstrap state | `2026_10_11_131611_add_administrator_bootstrap_state.php`; `users.is_admin` defaults false; `panel_setup` row 1 records completion |
| Authentication | `laravel/fortify` v1.41.0; admin-only login/logout, `EnsureAdministrator`, MFA enrollment (`/panel/security/mfa`), generation-bound challenge (`/two-factor-challenge`), `EnsureMfaVerified` gate on `/panel`, recent-auth (`/panel/security`, `EnsureRecentAuthentication`) and CLI recovery (`hostnur:admin:recover`) |
| User model | `app/Models/User.php`; hashed password cast, boolean admin cast, Fortify two-factor trait, hidden secret/recovery, integer `mfa_version`; admin flag excluded from mass assignment |
| Agent contract | `app/Agent/` DTOs, `OperationCatalog`, `OperationRequestValidator`, `AgentClient` interface, `UnavailableAgentClient`, `AgentModeGuard`, fixture-driven `FakeAgentClient` and durable `FakeExecutionSimulator` over `fake_agent_executions`; `../contracts/v1/` schemas/catalog/fixtures/notes; `tests/Support` fixture + JSON Schema runner |
| Panel UI | `layouts.panel` shell, `components/panel/nav`, `components/ui/*` (button/panel/badge/task-status/empty-state/stat/confirm), dashboard adapter states, filtered task/audit screens with cursor polling |
| Tasks/queue/audit | `app/Services/TaskService` + `AuditRecorder`, `app/Jobs/RunTask`, `hostnur:tasks:reconcile` (scheduled), `tasks`/`task_events`/`audit_events`, `App\Listeners\AuthenticationEventSubscriber`, `/panel/tasks`, `/panel/tasks/{id}`, `/panel/audit`, `/panel/audit/{id}` |
| Applications | `applications`/`application_revisions`/`source_credentials`/`application_secrets`/`application_domains`, `App\Services\ApplicationService` + `ApplicationPolicy`, `App\Support\{GitRepositoryUrl,Hostname}`, `/panel/applications` + `/{id}` (config/source/credential/secret/domain/archive) |
| Deployments | `deployments` + `App\Services\DeploymentService` + `DeploymentPreflight`, `/panel/applications/{id}/operations` (recent-auth wizard) and `/panel/deployments` + `/{id}`; task-derived state/release, restart/rollback |
| Server admin | `scheduled_jobs`/`certificates`/`database_requests`/`backup_policies`/`backups` + `App\Services\{ServerService,ScheduleService,CertificateService,DatabaseService,BackupService}`; `/panel/server`, `/panel/schedules`, `/panel/certificates`, `/panel/databases`, `/panel/backups` (recent-auth), `x-ui.request-status`/`x-ui.capability-notice` |
| Tests | 20 bootstrap, 14 authentication, 12 MFA-enrollment, 13 MFA-challenge, 17 security-settings, 8 recovery, 8 agent-contract, 8 agent-availability, 12 fake-agent, 4 task-status, 7 task-request, 11 task-execution, 8 task-audit/UI, 5 panel-shell, 5 task-visibility, 3 dashboard, 4 accessibility, 20 Git-URL, 15 hostname, 14 application-inventory, 10 deployment, 10 server-administration, 9 web-regression, 4 queue-interruption and 2 scaffold tests; 244 tests / 998 assertions at handoff |
| CI | `.github/workflows/web-panel.yml`; PHP 8.3/8.4, Node 24, actual runtime/static/build/migration/test gates; watches `panel/**` and `contracts/**` |
| Last application CI | Session 022 code passed: <https://github.com/maelzx/hostnur/actions/runs/38371962845>, code commit `7635290` |
| Current local URL | `http://127.0.0.1:9000`; committed example uses port 9000 |
| Remaining work | Web milestone complete; phase 10 Rust foundation deferred until the agent environment is supplied |

Do not scaffold again, regenerate the app key, reset the database, create test
users in the real DB, reinstall Boost, or install Rust. The last inspection found
zero real users, but the user may have subsequently run bootstrap. Inspect state
read-only when necessary; never assume the account needs recreating. Phases 1–9
(the complete web milestone: authentication, agent contract, durable
tasks/queue/audit, panel UI, application inventory/source/secrets, deployments,
server administration and the phase-9 verification/handoff) are complete. The next
work is the deferred Rust agent (phase 10+), which requires a user-supplied
environment.

#### Low-cost session protocol

1. Read this resume card, `panel/AGENTS.md`, the relevant applicable guidance and
   the chosen phase subsection. Search for the slice ID rather than rereading the
   entire plan, original requirements, generated framework guidance and vendor tree.
2. Check Git status. Inspect the few existing files the slice touches. Treat local
   `.env`, database contents, running port 9000 server and unrelated edits as user work.
3. Select one slice ID from the tables below. Aim for one coherent behavior change,
   its meaningful tests and a small number of production files. Split a large table
   item if necessary; phase completion is not a one-session requirement.
4. Write a short active-slice note in section 8 before substantial work. Implement
   directly using installed APIs and existing patterns. Do not build speculative
   abstractions, add decorative placeholder features, or explore unrelated packages.
5. Run the focused tests first. Run the full existing suite once before finishing
   a behavior change; rerun only when later edits/failures justify it. Build assets
   only for frontend changes. Run dependency audits when dependency changes warrant
   them; CI already performs the complete routine checks.
6. Record files/behavior changed, exact checks/outcomes, unresolved issues and next
   slice ID. Keep the result compact. Preserve detailed design decisions in this
   phase subsection or linked docs rather than repeating them in every session log.
7. Inspect status/diff/history, stage intended files, commit/push and verify remote.
   Prefer one implementation commit per session. After push, inspect CI once and
   report its result; record that result at the start of the next planned commit.
   A separate documentation checkpoint is justified for a phase gate or meaningful
   decision, not automatically after every small change.
8. On failure, read the failing step and relevant error window, fix the cause and
   rerun the affected checks. Do not repeatedly fetch full successful CI logs or
   reinstall the same dependencies hoping for a different result. If an environment
   blocker remains, save a precise handoff and the next diagnostic action.

Do not generate tests that only assert a method was called or copy its implementation.
Test observable authorization, persisted state, validation, recovery and UI output.
No whole-repository audits or screenshot runs are necessary for every reversible edit.
The total plan may be long; each development session should load a small excerpt.

#### Copy/paste starter prompt

> Continue Hostnur using `plan.md` section 9.1 and the detailed subsection for the
> next slice named in section 8. Read `panel/AGENTS.md` and applicable guidance.
> Current code already has Laravel/Livewire/Tailwind, SQLite, passing PHP 8.3/8.4
> CI and one-time CLI admin bootstrap. Do not redo that work. Use existing tools,
> local port 9000, and keep Rust deferred. Implement exactly one small web slice
> with meaningful tests, update the handoff with the next slice ID, commit and
> push. Report changed behavior, test results, commit and CI status concisely.
> Proposed names in section 9 are implementation guidance; check for existing
> equivalents before creating files. Ask only about material unresolved decisions.

### 9.2 Shared implementation approach and invariants

#### Application boundaries

Use a conventional Laravel monolith: Blade/Livewire renders UI; Laravel policies,
middleware and application actions validate/authorize changes; Eloquent persists
configuration/tasks; queued jobs invoke an `AgentClient` interface. Rust later
implements the other side of that interface. No separate SPA, browser terminal,
root shell wrapper, remote VPS connector or module marketplace in these phases.

Suggested structure as features are introduced, not empty folders to scaffold now:

```text
panel/app/Actions/Auth/                 Small orchestration for auth-specific changes
panel/app/Actions/Tasks/                Request, approve, retry and cancel task intents
panel/app/Actions/Applications/         Save/archive app configuration; request deployment
panel/app/Contracts/AgentClient.php     Interface only, independent of transport
panel/app/Agent/                        DTOs, operation catalog, fake/unavailable adapters
panel/app/Enums/                        Backed enums for states/modes/credential types
panel/app/Http/Middleware/              Administrator, MFA and recent-authentication gates
panel/app/Livewire/                     Class-based full pages, following existing Foundation
panel/app/Jobs/                         Jobs containing task IDs, not plaintext secrets
panel/app/Models/                       Resource/task/audit records
panel/app/Policies/                     App/resource/task permissions
panel/app/Services/                     Focused secret, validation and redaction helpers
panel/resources/views/                 Shared layouts, Blade components and Livewire views
panel/config/hostnur.php                Panel-owned limits and agent mode configuration
contracts/v1/                          Operation schemas, catalog and valid/invalid fixtures
```

Create only classes actually needed by the current slice. Avoid a generic repository
layer on top of Eloquent, an event bus, multi-tenant framework or a DI abstraction for
every class. Use explicit PHP types and Laravel's installed v13 APIs. Keep PHP 8.3
compatibility. Use Artisan generators where appropriate and commit incremental
migrations; do not rewrite migrations already applied to the user's DB.

#### Universal authorization rule

Guest -> authenticate. Authenticated non-admin -> deny. Admin lacking confirmed
MFA/current MFA session proof -> enrollment/challenge only. Sensitive mutation ->
also require recent password confirmation and a clear operation confirmation.
This chain is enforced on the server, not only by navigation or disabled buttons.
No operation becomes permitted simply because the configured agent is fake.

Use route middleware plus policies/actions. Livewire requests can hydrate modified
properties: re-authorize inside each mutation, resolve IDs from the DB, and never
trust public properties such as `is_admin`, actor ID, task status, agent mode,
capabilities or approval flags. Persist custom middleware for Livewire updates
using the installed Livewire API where required; test subsequent update requests,
not only the initial GET. A locked ID property helps integrity but is not permission.

Keep one owner for each DOM region. Blade/Livewire is the default. Add HTMX only
for a separately owned flow with a concrete advantage and the same CSRF/auth rules;
there is no requirement to add HTMX solely to make it appear in `package.json`.

#### Desired state versus observed state

- Desired state is user configuration: domain, Git ref, runtime, schedule, etc.
- Observed state is evidence from an adapter: sampled metrics, execution outcome,
  active release, service health. It includes mode, time and resource identity.
- Simulation never changes a record to falsely claim a real site, user, certificate
  or database exists. Display a distinct simulated result/observation with source
  `fake`; real applied-state fields remain empty until genuine agent evidence.
- Offline, unsupported, stale and failed are different conditions. Do not turn
  unavailable data into zero CPU, healthy services or a successful deployment.

#### Proposed initial limits

Use configurable, documented values, not magic numbers spread across components.
Suggested first values: 1 MiB protocol message, 64 KiB event chunk, 2 MiB retained
output per task, 255-character display names, 254-character emails, 100 environment
variables per app, 64 KiB per secret input, 50 rows per paginated table. Bound both
number of entries and total bytes. Test the boundary and above-boundary cases.

Task polling: roughly 2–5 seconds while active, stop on terminal states; dashboard
read-only refresh: roughly 15–30 seconds, pause on hidden tabs where feasible.
These are engineering defaults to validate, not performance promises. SQLite should
not receive a permanent metric-history row on each poll. Keep writes/transactions
short; do not hold a DB lock while waiting for an agent/network response.

### 9.3 Phase 1 — Existing foundation and maintenance obligations

**Status: complete. Do not redo.** Preserve the documented locked dependency tree
and current `.npmrc` script/audit policy. PHP platform emulation in Composer is
8.3.0; actual 8.3 and 8.4 jobs already passed. The complete baseline commands are
in `panel/README.md` and the workflow, not a new tool-install checklist.

| Slice | State | Existing deliverable |
|---|---|---|
| P1.1 Toolchain baseline | Done | `docs/development-toolchain.md`; installed-tool constraints |
| P1.2 Web scaffold | Done | Laravel, Livewire page/layout, SQLite/queue defaults and lockfiles |
| P1.3 Web CI/static gates | Done | PHP 8.3/8.4 CI, syntax/style gates, audits/build/migration/test checks |

Important maintenance details:

- Root route is currently public **because it is only an informational scaffold**.
  Replace its behavior when introducing authentication, without exposing future
  authenticated dashboard data through that public route.
- `composer check:syntax` uses tracked PHP; stage intended new files before running.
  `composer check:style` runs Pint then checks unstaged PHP drift. Run the prescribed
  formatter and review both staged and unstaged changes before committing.
- `.env`, key material, SQLite, dependency folders and generated assets are ignored.
  Preserve the user's current `APP_KEY`; encrypted data will depend on it.
- The npm lock was repaired in a clean worktree after CI found missing bundled/peer
  entries. If clean install fails again, diagnose the lock; do not update Node/npm,
  use `--force`/peer-dependency bypasses, or switch package managers as a shortcut.
- CI paths cover `panel/**`, its workflow and `.nvmrc`. Documentation-only changes
  outside those paths do not require another identical application test run.
- Project license is still undecided. Normal Composer validation passes with its
  visible warning; do not invent a license or claim strict validation is passing.

Acceptance for future foundation changes: reproducible install, migration, tests,
asset build and both runtime jobs pass; browser assets load; local instructions
stay accurate. No production installer claim follows from this phase.

### 9.4 Phase 2 — Administrator authentication, MFA and recovery

**Status: P2.6 done; phase 2 complete.** Existing bootstrap is the only account-creation
path. Preserve its command, password policy, `panel_setup` marker and atomic tests.
Do not promote existing ordinary accounts, add web registration, seed credentials
or reset that marker when building login/recovery.

#### Authentication implementation decision

Recommended approach: Laravel's session guard and first-party **Laravel Fortify**
for the login/logout/password-confirmation/TOTP/recovery pipeline, with Hostnur
Blade/Livewire screens and custom administrator/MFA gates. Reuse established
authentication/TOTP logic rather than writing cryptography or creating a second
competing login backend. Fortify **v1.41.0** is installed and locked as of session
008 (dependency approved by the user); session 009 enabled the two-factor feature
with an explicit `pipelines.login` that deliberately defers the two-factor redirect
to P2.4, and `views => false` keeps unimplemented Fortify view routes unregistered.

Before adding it, confirm this one project-local dependency with the user, following
the repository dependency-approval rule. Inspect its actual Laravel 13 compatibility
and installed APIs; commit the resulting Composer lock. No global tool installation
or complete starter-kit replacement is needed. If approval is not available, keep
MFA-sensitive operations closed and record the precise blocker; do not invent a
home-grown TOTP algorithm to avoid the dependency decision.

Fortify should own authentication POST endpoints if selected. Livewire can render
screens and settings, but the same login must not also be independently implemented
as a Livewire mutation with different authorization/rate-limit rules. Native POST
forms with `@csrf` are acceptable within Blade/Livewire layouts. Never populate
password inputs with old submitted values or stored hashes.

#### Proposed files and routes

- `config/fortify.php`, `app/Providers/FortifyServiceProvider.php`, provider registered
  in `bootstrap/providers.php`; explicitly enable only approved authentication features.
- `resources/views/auth/{login,two-factor-challenge,confirm-password}.blade.php` and
  `app/Livewire/Auth/MfaEnrollment.php` / matching view where reactive settings help.
- `app/Http/Middleware/EnsureAdministrator.php`, `EnsureMfaVerified.php` and, when
  needed, `EnsureRecentAuthentication.php`; aliases in `bootstrap/app.php`.
- `app/Actions/Auth/` only for actual Hostnur orchestration/custom Fortify actions.
- Auth feature tests, using factories in isolated SQLite, never the real admin DB.
- Guest screens: GET login plus login POST. Logout is POST only. Enrollment,
  challenge, password confirmation and security settings are authenticated but
  carefully exempted from the full-MFA gate to avoid redirect loops.
- App screens under `/panel` require auth + administrator + confirmed MFA session.
  Root `/` eventually redirects guests to login and authorized users to the panel.
  Registration, public setup and email-reset routes remain absent unless explicitly
  planned later; current mail driver logs mail and is not a recovery-email service.

#### Authentication states and security flow

1. Normalize email consistently with bootstrap (trim/lowercase), validate bounded
   input and attempt credentials only for `is_admin=true` users. Failure uses a
   generic message for unknown email, wrong password and ordinary account.
2. Rate-limit password attempts by normalized email + IP, and add an IP-wide limit.
   Initial recommendation: 5 failed attempts/minute per identity/IP plus 30/minute
   per IP; configure named limiters and test expiry. Do not put passwords in keys.
3. Rotate session ID after password authentication. No remember-me feature until
   its MFA/session implications are intentionally supported and tested.
4. If MFA is unconfirmed, permit only enrollment, logout and required security
   confirmation. If confirmed, require TOTP or a recovery code before app access.
   Never equate `auth()->check()` with full administrator authorization.
5. Bind MFA session proof to user ID and a persisted `mfa_version`/generation. Gate
   against the current DB state. Password/MFA recovery or secret rotation must
   invalidate old proof; do not rely on a permanent `session('mfa_ok')` boolean.
6. Logout clears guard state, invalidates the session and regenerates CSRF token.
   POST logout without CSRF must not succeed. Do not redirect to arbitrary URLs
   supplied by the request; use named same-origin routes/intended-path validation.
7. Use HttpOnly, SameSite and production HTTPS cookie settings. Development on
   port 9000 can remain HTTP locally; never implement a blanket insecure production
   exception. Do not require verified email for the bootstrap account yet: the
   command deliberately does not claim email verification and mail is not configured.

MFA schema: add incremental migration for the selected Fortify fields
`two_factor_secret`, `two_factor_recovery_codes`, `two_factor_confirmed_at`, plus
Hostnur `mfa_version` if required for revocation. Use TEXT for encrypted values.
Hide them from model serialization and snapshots. Follow the selected library's
encryption conventions; do not double-encrypt values that it already encrypts.
Use the existing app key, and record that recovery depends on key custody.

Enrollment must confirm a valid code before setting `two_factor_confirmed_at`.
Provisioning secrets/QR information are available only to the current admin with
recent password confirmation, not public URLs, logs or external QR web services.
Treat TOTP input as a fixed-width digit string so leading zeroes are preserved.
Use a narrowly bounded TOTP clock window. Test wrong, expired and replayed codes;
persist a last-accepted counter if the selected library does not prevent replay.
Recovery codes are shown once, consumed once and regenerated only after strong
reauthentication; consumption must be atomic so concurrent requests cannot reuse
one code. Check actual Fortify behavior and add a transactional custom action if
needed. Do not assume package defaults satisfy all of these concurrency requirements.

Recent-authentication recommendation: password confirmation within 5 minutes,
bound to the same user/session/MFA generation. Sensitive changes include password,
MFA reset/code regeneration, credential replacement, destructive resource requests
and backup/private-key changes. A recent-auth token is server-generated evidence,
not a checkbox or a Livewire public boolean.

#### Slices and acceptance

| Slice | Deliverable | Main proof |
|---|---|---|
| P2.1 CLI bootstrap | Done; preserve existing implementation | Existing 20 tests and passing CI |
| P2.2 Login/logout foundation | Dependency decision, views/endpoints, admin gate, throttling, session rotation, restricted post-login state | Wrong/ordinary users rejected; genuine admin can reach restricted auth state; logout invalidates session; no app access before MFA |
| P2.3 MFA enrollment | Encrypted secret, local provisioning display, code confirmation, recovery-code creation | Unconfirmed secret grants no access; secret never serialized/logged; valid confirmation advances state |
| P2.4 MFA login challenge | TOTP/recovery challenge, generation-bound session proof, persistent Livewire middleware | Password-only/forged/stale proof denied; valid factor permits app access; replay/reuse rejected |
| P2.5 Recent-auth/security settings | Password confirmation/change, MFA/code rotation, session revocation | Stale confirmation rejected; recovery/rotation invalidates old sessions/proof |
| P2.6 Local recovery and runbook | Deliberate CLI password recovery for an existing admin; documented MFA lost-code handling | Cannot create/promote accounts or reopen bootstrap; secrets hidden; relevant sessions invalidated |

P2.6 recovery is not a public endpoint. A password-recovery CLI command must target
an existing admin explicitly, ask for confirmation/new hidden password, preserve
the setup marker and preserve MFA unless a separately designed, explicit local
MFA-reset procedure is chosen. Never clear MFA silently as a side effect of changing
password. Recovery tests cover wrong targets, cancellation, failed writes and stale
sessions. Keep a record of authentication/security events for the phase 4 auditor;
do not log passwords, codes, recovery strings or provisioning secrets.

Test CSRF explicitly: Laravel's normal testing environment may bypass CSRF checks.
A passing feature suite alone is not proof; use a production-like middleware test
or real HTTP check that demonstrates missing-token rejection. Also test role removal
after login and Livewire hydration attempts with forged IDs/privilege properties.
Phase exit: correct bootstrap/login/MFA/recovery flows and server-side access gates
pass, including denied paths. No root/agent capability is introduced here.

### 9.5 Phase 3 — Agent contract, unavailable mode and deterministic simulation

**Status: P3.5 done; phase 3 complete.** Typed DTOs, the operation catalog, strict
request validation, `contracts/v1/` schemas/fixtures/notes, the `AgentClient`
interface, the default unavailable adapter, the mode guard, a deterministic fake
adapter with durable submit/inspect/cancel, and the shared contract-regression
runner all exist. Phase 4 (tasks, queue and audit) is next.

**Goal:** make every later web flow depend on a small typed interface, not Rust,
shell commands, HTTP calls to arbitrary hosts or ad-hoc fake values in components.
Start with `unavailable` as the default in `config/hostnur.php`. `HOSTNUR_AGENT_MODE`
can opt into `fake` only in local/testing environments. Reject fake mode in production
both when binding the adapter and when executing a task; do not silently fall back.
`real` must report unavailable until phase 11 actually implements transport.

#### Interface shape and data contracts

Proposed `AgentClient` responsibilities:

- `capabilities(): CapabilitySnapshot` — protocol versions, mode, operation support,
  module/runtime identifiers, sample time and availability reason.
- `snapshot(): ServerSnapshot` — bounded read-only metric/service observation.
- `submit(OperationRequest): OperationReceipt` — accepts intent; acceptance is not
  completion. Include execution/request IDs and supported idempotency semantics.
- `inspect(string $executionId): OperationSnapshot` — durable state, bounded progress,
  event sequence and result/error. Unknown execution is explicit.
- `cancel(string $executionId): CancellationReceipt` — requested/unsupported/already
  finished; never promises rollback or reports cancellation before confirmation.

The exact method syntax can evolve, but keep these responsibilities and test all
callers against the same interface. Mutation parameters are operation-specific,
not an arbitrary dictionary that can become a generic root executor later.

Request envelope example, with UUIDs generated by the panel:

```json
{
  "protocol_version": 1,
  "request_id": "6c9dc595-fc9c-4c13-8c72-ed9a489ab9e7",
  "idempotency_key": "cf36a770-2abf-42e6-83cf-a0f042bcfc3c",
  "operation": "application.restart",
  "resource": {"kind": "application", "id": "224065ac-4bc7-4ba4-a06d-8c8bca950c32"},
  "parameters": {},
  "timeout_seconds": 60
}
```

Response/error envelopes must echo correlation/version, distinguish accepted from
terminal outcome, and include a stable machine code with a safe user message.
Initial error codes: `agent_unavailable`, `unsupported_operation`,
`unsupported_protocol`, `invalid_parameters`, `resource_not_found`, `conflict`,
`timeout`, `execution_unknown`, `permission_denied`, `operation_failed`.
Do not include raw process environments, request secrets or full exception traces.

Store `contracts/v1/` JSON Schemas (a declared dialect, e.g. 2020-12), operation
catalog and fixtures. Reject extra envelope/parameter keys, invalid UUIDs, wrong
types, oversized bytes/arrays and unsupported versions. PHP can initially enforce
this with Laravel validation/explicit DTO parsing and an operation allowlist;
keep schemas and fixtures consistent in contract tests. A validation library is a
separate dependency decision, not permission to invent unvalidated payloads.

#### Initial operation catalog

| Operation | Target / key inputs | Read-only? / retry rule |
|---|---|---|
| `server.snapshot` | Current single server; no arbitrary host | Read; bounded retry |
| `application.deploy` | App UUID, immutable configuration revision and release/source intent | Mutation; no blind replay after unknown outcome |
| `application.restart` | Managed app UUID | Mutation; capability-declared retry only |
| `application.rollback` | App UUID and known deployment/release UUID | Mutation; approved supported code rollback only |
| `service.control` | Managed service ID and enum start/stop/restart | Mutation; no raw systemd unit name |
| `package.install` | Approved catalog package ID | Mutation; not arbitrary APT flags/repository URLs |
| `schedule.apply` | Schedule UUID/revision, app UUID | Mutation; logical owner is managed app |
| `domain.apply` | Managed domain UUID/revision, app UUID | Mutation; not arbitrary Nginx text/path |
| `certificate.issue` | Managed domain UUID, approved HTTP-01 provider | Mutation; no wildcard DNS-01 claim |
| `certificate.install` | Domain UUID, protected certificate/key references | Mutation; secret references, not plaintext keys |
| `database.provision` | Database-request UUID/revision, supported engine ID | Mutation; no arbitrary SQL/root DB password |
| `backup.create` | Backup-policy UUID/revision and bounded resource selection | Mutation; no arbitrary filesystem path |

Define each operation only as its web phase needs it. Mark capabilities not yet
supported; a catalog entry is not a claim that the Rust handler exists. Linux
paths/users/units are future registry-resolved resources, never caller-trusted roots.
Lifecycle `submit/inspect/cancel` is separate from the domain operation name.

#### Fake adapter requirements

Fixtures are checked-in, deterministic and secret-free. Scenarios: accepted/running/
success, validation error, operation failure, timeout, lost receipt/unknown execution,
cancel accepted/unsupported and offline. No internet, package install, filesystem
provisioning, `exec`, shell or real daemon calls. Use a controlled clock, not sleeps
in tests. Fake capabilities list only scenarios that the adapter actually implements.

For execution inspection across queue/browser processes, use a small persisted
simulation record keyed by execution/request ID, or derive progress deterministically
from persisted request/scenario/time. An in-memory PHP array alone is insufficient.
Duplicate submissions with the same identity must refer to the same simulated
execution. Every snapshot/receipt/task includes `mode=fake`; UI displays it.

| Slice | Deliverable | Proof |
|---|---|---|
| P3.1 DTOs/catalog/schema | Versioned request, receipt, state/error and capability types | Valid fixtures pass; invalid/excess/unknown input fails |
| P3.2 Unavailable adapter/binding | Safe default and explicit mode-selection guard | Default/offline rejects mutations; production fake selection denied |
| P3.3 Read-only fake snapshots | Realistic stable fixtures with time/mode | No network/host calls; fake/stale/offline distinguishable |
| P3.4 Durable fake execution | Submit/inspect/cancel, scenario/clock control | Same request reuses execution; later process can inspect progress |
| P3.5 Contract regression | Shared valid/invalid fixture runner and compatibility notes | Unsupported version/capability fails clearly; schemas/DTOs aligned |

Phase exit: tests can run the web panel with fake or unavailable adapters while
production has no functioning privileged path. Actual socket/framing/Rust contract
proof belongs to phase 11, not this phase's mocked checks.

### 9.6 Phase 4 — Durable tasks, approvals, audit and queue reconciliation

**Status: P4.1–P4.6 done; phase 4 complete.** Tasks/events/audit persistence,
`TaskService` (request/approve/deny/cancel/retry/run/poll), `RunTask` leases,
`hostnur:tasks:reconcile`, redaction/limits, the auth audit subscriber and the
read-only history UI exist. Phase 5 (panel shell/dashboard) is next.

**Goal:** persist intent and history independently of the HTTP request. Use one
task service/action path for all UI mutations and later AI requests. A component
must not call `AgentClient::submit()` directly and bypass tasks/authorization.

#### Proposed persistence

| Record | Minimum fields / indexes |
|---|---|
| `tasks` | UUID PK; actor ID; operation; resource kind/UUID; config revision; mode; status; idempotency key; non-secret parameters/reference snapshot; payload fingerprint; requested/queued/started/finished times; deadline; execution ID; lease token/expiry; cancellation-requested time; safe error code/message |
| `task_events` | ID or UUID; task UUID; monotonic sequence unique per task; type/level; bounded redacted message/metadata; recorded time; index task+sequence |
| `audit_events` | Immutable event UUID; actor identity; action/resource/task IDs; allowlisted non-secret metadata; outcome/time; indexes time and resource |
| Approval fields/record | Task UUID; approving actor; confirmation/MFA generation; approved payload fingerprint; approved/expiry times; optional denial reason code |

Use FK/null/archive behavior that preserves history after resources/accounts change.
Actor deletion or loss of admin role prevents queued privileged execution, but
does not erase who originally requested it. Audit is append-only in application
code; do not claim it is tamper-proof against an OS/DB administrator.

Store only secret **references and versions** in task/job payloads. Encryption of
an entire payload is not a substitute for redaction or limiting what belongs there.
Queue jobs contain task UUIDs. Laravel `failed_jobs` otherwise serializes payloads,
so never dispatch decrypted environments, tokens, TLS keys or passwords.

#### Lifecycle and worker algorithm

States: `pending`, `awaiting_approval`, `queued`, `running`, `succeeded`, `failed`,
`cancelled`, `unknown`. Define allowed transitions centrally in a backed enum/helper.
Terminal states cannot be overwritten by a late poll. `cancel_requested_at` is an
intent, not a terminal state. Unknown completion is not the same as failure.

Request action:

1. Re-resolve actor/resource and enforce admin, MFA, resource policy, capability
   and operation-specific input rules. Consequential requests require recent auth
   and approval of the exact immutable payload/revision, not a previous version.
2. Create task intent plus audit event in a short DB transaction. Deduplicate on
   actor/idempotency key; the same key with a different fingerprint is a conflict,
   never silently an unrelated existing task. Actor ID comes from session.
3. Dispatch a job after commit. Account for a crash between commit and enqueue:
   pending intent acts as an outbox, and a scheduled/CLI reconciler redispatches
   undispatched work. Duplicate queue jobs must be harmless.

Worker:

The authorization/approval checks below apply before a **new mutation submission**
or retry. Once an operation is accepted, continue system-owned read-only inspection
and reconciliation even if the requesting actor loses access or approval expires.
Do not abandon accepted work or move it back to `awaiting_approval` merely because
a polling job ran later; stale authority must not permit a new mutation either.

1. Claim task/lease with a conditional DB update; another job seeing a current
   lease exits. Recheck actor role, resource existence/revision, mode/capability and
   unexpired approval. Revoked role/changed MFA or edited config cannot inherit
   stale authority. Expired approval goes back to `awaiting_approval`.
2. Submit with stable request/idempotency identity, outside a DB transaction.
   Persist execution receipt before subsequent polling. Agent acceptance alone
   never becomes `succeeded`.
3. Inspect execution, append deduplicated bounded/redacted events and update status
   using valid transitions. Release/requeue for later polls rather than holding
   a PHP worker or DB lock in a long sleep loop.
4. Separate queue-delivery retries from operation retries. `release()` consumes
   Laravel job attempts; set bounded `retryUntil`/tries/deadline deliberately so
   a legitimate running operation is not failed after one poll. Keep worker timeout
   below queue `retry_after` and lease expiry consistent with both.
5. Lost receipt/timeout after submission -> query/reconcile with the same identity;
   if unsupported, mark `unknown` and require investigation. Never resubmit a
   destructive operation simply because an HTTP response was lost.
6. Cancellation of pending work can be local; running cancellation requires adapter
   acknowledgement. Already-finished/unsupported requests report their actual state.
   Cancel is not undo. Retrying a failed operation creates a new linked task with
   fresh authorization/approval unless its contract explicitly permits safe reuse.

Add a small reconciliation command such as `hostnur:tasks:reconcile` and schedule
it through Laravel scheduling during development. Explain `schedule:work` versus
production cron/systemd later. It handles pending outbox entries and expired leases,
not arbitrary task-state rewriting. Test the decision matrix with a controlled clock.

Approval recommendation: 10-minute validity, single approved fingerprint and current
MFA generation. Reauth/approval at request time does not justify delayed execution
after identity or configuration changes. Enforce this in the action **and** worker.

Redaction: do not generate logs containing secrets in the first place. Structured
allowlists plus a central redactor handle known values/names before persistence;
never render raw output as HTML. Limit total output and append a truncation event.
Avoid storing the raw unredacted copy in another exception/audit/debug field.

| Slice | Deliverable | Proof |
|---|---|---|
| P4.1 Schema/models/states | Tasks/events/audit and enum transitions | Relationships/history preserved; illegal/terminal transitions rejected |
| P4.2 Authorized intent/approval | Request action, dedupe/fingerprint and audit | Tampered actor/resource/approval denied; double-click returns same intent |
| P4.3 Worker/outbox/leases | Job containing UUID, conditional claim and dispatch reconciliation | Pending intent survives enqueue failure; duplicate jobs do not double-submit |
| P4.4 Progress/log persistence | Inspect adapter, cursor/sequence, redaction and output limits | Refresh sees persisted history; duplicates/oversized/secrets handled |
| P4.5 Failure/cancel/retry | Unknown/timeout, expired approval, lost lease, cancellation and linked retry | No blind destructive replay; role/config revocation stops execution |
| P4.6 Audit UI and auth integration | Paginated read-only history, auth event subscriber | Guest/non-admin denied; login/MFA/credential actions attributed without secrets |

Phase exit: meaningful queue/process-interruption behavior passes using the durable
fake adapter. Real cross-process agent recovery remains a later VM gate.

### 9.7 Phase 5 — Shared UI shell, dashboard and task visibility

**Status: P5.1–P5.4 done; phase 5 complete.** The protected shell/navigation,
shared components, dashboard adapter states, filtered task/audit screens with
cursor polling and the responsive/accessibility pass exist. Phase 6 (application
inventory/source/secrets) is next.

Reuse the current system-font/neutral-slate/indigo styling as the starting direction;
build an operations workspace, not a marketing homepage. Use shared Blade components
for buttons, form fields/errors, status badges, empty states, panels, tables, pagination
and confirmation dialogs. Native semantic controls, clear labels, visible focus,
keyboard operation, adequate contrast and mobile wrapping are required.

Proposed pages/components: authenticated app layout with navigation; dashboard;
task list/detail; audit list/detail; agent availability summary. Navigation groups:
Overview, Applications, Tasks, Server, Security/Settings. Add links only when their
routes exist; explicitly unsupported features show an explanatory state, not a
button that silently does nothing. Local CLI bootstrap/login views remain separate.

Dashboard data flow: server-side authorize -> short-cached adapter snapshot -> typed
view data. Use a latest sample, not an unbounded time-series writer. Cards: CPU, RAM,
disk, uptime/OS, managed services, active tasks and recent failures. Each has units,
sample time and a readable unavailable/stale reason. Fake mode adds a persistent
simulation banner and marks values; disconnected metrics use `—`, not zero.

Task detail uses event-sequence cursor pagination, not full log reload on every poll.
Escape output, preserve readable whitespace, handle truncation and long lines. Poll
while active, stop on terminal states and avoid background-tab noise. A browser
refresh/reopen reconstructs state from the DB. Confirmation dialogs show resource,
operation and actual mode; they do not expose secrets or imply approval by opening.

| Slice | Deliverable | Proof |
|---|---|---|
| P5.1 Protected shell/components | Authenticated layout/navigation and reusable controls | Access gates hold on routes/Livewire updates; no broken navigation |
| P5.2 Task/audit screens | Filters, paging, detail/events and active polling | Only authorized records visible; reload/cursor/truncation work |
| P5.3 Dashboard adapter states | Fake/offline/stale/healthy sample presentation | No fake/live ambiguity or unavailable-as-zero values |
| P5.4 Responsive/accessibility pass | Small-screen tables/forms, focus, errors, keyboard dialogs | No horizontal page overflow; accessible names/status not color-only |

Test view states/authorization through feature/Livewire tests. Use manual browser
inspection at roughly 375 px and desktop width for real layout behavior. Record
what was actually inspected; rendered HTML tests are not screenshot/accessibility
certification. Add charts only when they provide value and their data is bounded.

### 9.8 Phase 6 — Application inventory, source configuration and secret storage

**Status: P6.1–P6.5 done; phase 6 complete.** Application inventory with immutable
configuration revisions, encrypted Git credentials and environment secrets, and
normalized domains/archive exist; host state remains unapplied. Phase 7
(deployment intent/operations) is next.

**Goal:** manage durable desired configuration safely, without source checkout,
Linux user creation or privileged deployment. First provider is `php-laravel`.
Version options come from a catalog/capability response; do not advertise arbitrary
PHP releases because the local panel happens to run on 8.4. Initial Ubuntu-native
runtime planning can use 8.3; broader managed runtimes remain later providers.

#### Proposed resource schema

| Record | Core fields / behavior |
|---|---|
| `applications` | UUID; display name; unique immutable slug; provider/runtime/version; desired config/revision; archive time; observed-state/mode/time separate |
| Application source | App UUID; normalized repository URL; Git ref; nullable credential UUID; source status `not_checked`; no plaintext auth in URL |
| `source_credentials` | UUID; label; type HTTPS token/SSH deploy key; encrypted material; optional public-key/host-key metadata; version/rotation time; secret fields hidden |
| `application_secrets` | UUID; app UUID; unique variable name per app; encrypted TEXT value; version; rotation time; value never returned by list/edit serializers |
| App domains | UUID; app UUID; normalized unique hostname; desired HTTPS policy; observed routing/certificate state and provenance |
| Revision snapshot | Immutable UUID/number, app UUID, non-secret deploy settings plus secret/credential version references; task/deployment links |

Use normal Eloquent relationships and policies. Archive rather than silently drop
apps with task/history records. If an operation is active, refuse conflicting
archive/config replacement or create a distinct future revision; never rewrite
the configuration being executed. Resource names are display data; UUIDs define
identity for the future managed-resource registry.

Secret implementation: use Laravel encryption/encrypted casts with the existing
app key, TEXT columns, hidden attributes and dedicated write actions. Do not put
stored decrypted values in Livewire public properties, edit-form defaults, debug
logs, model JSON, audit metadata or job payloads. Forms show presence/last rotation
and empty replacement inputs, with explicit replace/delete semantics. Empty value
and deletion are different operations. Clear replacement input after submission.

Decrypt only in a narrow service where a real future transport requires it; fake
execution needs presence/version, not secret material. App key loss prevents
decryption and must produce a safe failure, not empty-secret fallback. Key rotation
and encrypted off-host backup design remain later recovery work; record dependency
on the current key and do not regenerate it during development.

Validation rules:

- Names bounded/nonempty; slugs lowercase `[a-z0-9-]`, unique and not later changed
  into arbitrary host paths. Runtime/provider choices are allowlisted IDs.
- Environment keys follow `[A-Za-z_][A-Za-z0-9_]*`, are unique per app, and values/
  count/aggregate bytes are bounded. They affect the **hosted app**, never the panel's
  `.env`, agent mode, filesystem roots or security configuration.
- Git URLs allow planned HTTPS or verified-key SSH forms. Reject embedded HTTP
  credentials, `file:`, `ext:`, local paths, unsupported schemes, control characters,
  option-like/ambiguous syntax and query/fragment credential tricks. Normalize with
  a parser, not shell execution. Branch/tag/commit selectors are bounded and are
  later resolved/pinned by the agent. SSH host-key verification is a recorded
  requirement; do not disable it to make private Git easier.
- No network fetch/test occurs merely because an admin saves a URL. Show `not
  checked`; a fake connectivity result is labeled simulated. DNS/SSRF/path/build
  safety is independently revalidated by the later agent.
- Domains are normalized ASCII/punycode hostnames, bounded by DNS label/name limits,
  unique, and exclude scheme/path/port/control characters. Validate duplicates after
  normalization. Initial HTTP-01 cannot issue wildcard certificates.
- Persistent directories and health-check paths are logical relative settings in
  an app namespace, not arbitrary absolute Linux paths or caller-selected hosts.
- Composer/Artisan/build/start settings are application-user intent, never panel
  shell execution. Prefer defined templates/structured arguments; custom commands,
  if stored, remain untrusted app-level data and stay out of audit/error output.

| Slice | Deliverable | Proof |
|---|---|---|
| P6.1 App schema/inventory/policy | App list/create/show/archive and default provider | Unique immutable identity; unauthorized/tampered IDs and active-work conflicts denied |
| P6.2 Configuration/revisions | Runtime, build/worker/scheduler, health/persistent settings | Bounded validated config; revision used by active task cannot mutate |
| P6.3 Git metadata/credentials | Public/private source forms, protected references/rotation | Unsafe URLs rejected; credentials encrypted/masked; no network-on-save |
| P6.4 Environment secret forms | Write-only replacement/delete actions and presence metadata | No decrypted values in snapshots/JSON/audit/jobs; failure preserves old value |
| P6.5 Domains and archive behavior | Domain relation, normalized conflicts, safe archive/history | Duplicate/malformed domains rejected; history retained; no host provisioning |

Tests include punctuation/control/path tricks, cross-resource ID tampering, stored
XSS values rendered escaped, concurrent uniqueness/conflicting edits, and decrypt
failure. Inspect Livewire responses as well as initial HTML for secret exposure.
Phase exit: real panel CRUD/configuration and encrypted storage work; host state
remains unapplied and the UI communicates that accurately.

### 9.9 Phase 7 — Deployment intent, application operations and logs

**Status: P7.1–P7.5 done; phase 7 complete.** Deployment intent/preflight, the
confirmed wizard, task-derived progress/history and restart/rollback over the task
pipeline exist; no live application/certificate/service is claimed. Phase 8
(server administration views) is next.

Build a deployment wizard around shared actions/tasks, not a second queue/backend.
Steps: choose app/source revision -> validate prerequisites -> show non-secret plan
and mode -> recent-auth/operation confirmation -> persist approved intent -> task
detail/progress. Source preparation/build/service/Nginx work occurs only through the
adapter; the PHP panel must not run Composer/Git/Artisan against hosted source.

Proposed `deployments` record: UUID, app UUID, immutable revision/task UUID, requested
Git ref and later resolved commit, requested runtime, mode, state, release reference,
health summary, previous supported release and timestamps. Secrets are references.
States distinguish requested/simulated/real failure and success. A real active-release
pointer cannot be replaced by a successful fake deployment.

Preflight covers app not archived, supported provider/runtime/operation capability,
complete Git/credential references, domain conflicts, health settings and any
existing-database configuration. It checks panel configuration, not actual remote
repository reachability or package availability. Show what remains agent-dependent.

Migration policy: explicitly select whether to run migrations and confirm potential
data impact. Code rollback is not database rollback. Rollback UI permits only a
known compatible app-owned release and documents shared uploads/database limitations.
No automatic reversing of migrations, deleting uploads, rebuilding a release in
place or accepting arbitrary user-supplied release-directory paths.

Restart/rollback/worker operations reauthorize in their actions, require relevant
capability and stable managed IDs, create tasks and return their persisted status.
An unavailable adapter blocks execution clearly. Fake success/failure/timeout/logs
exercise UI and task state with a persistent simulation label. Preserve non-secret
request/history even when a later capability disappears.

| Slice | Deliverable | Proof |
|---|---|---|
| P7.1 Deployment schema/preflight | App-owned revision and deployment intent validation | Missing credential/capability/health settings fail before submission |
| P7.2 Wizard/confirmation | Preview, recent-auth, fingerprint approval and single-submit | Edited intent invalidates approval; double-click creates one task |
| P7.3 Durable progress/history | Phase progress, redacted logs, elapsed/error/cancel states | Refresh resumes; fake outcomes remain labeled; unknown is not success |
| P7.4 Restart/rollback UI | Managed operation actions and known-release selection | Wrong-app/unsupported release and stale authorization denied |
| P7.5 Laravel workers/scheduler policy | Desired worker definitions and migration/rollback wording | App-user intent only; no PHP panel process execution; persistent-data limits clear |

Minimum end-to-end fake flow: authenticated/MFA admin creates config, confirms
deployment, closes browser, reopens task, sees deterministic result/logs, requests
restart and supported simulated rollback. Run failure/timeout/cancel variants too.
No live public application, certificate or systemd service is claimed by this phase.

### 9.10 Phase 8 — Server administration views and desired configuration

**Status: P8.1–P8.6 done; phase 8 complete.** Service/package/firewall views,
scheduled jobs, domains/TLS, database requests and backup policies exist through the
same validated workflow; unavailable mode stays closed and host state remains
unapplied. Phase 9 (web milestone verification and Rust handoff) is next.

Use the same capability catalog, policies, secret handling, approvals and task
pipeline for every feature. Each small resource gets a focused model/form/action
only when needed. Treat this phase as several independent slices, not one giant
"server settings" component with hundreds of untyped public properties.

| Slice | Data/UI approach | Required denial/failure behavior |
|---|---|---|
| P8.1 Managed services/packages/firewall | Read adapter-owned service IDs/status, approved package catalog, read-only firewall summary | Reject arbitrary unit/package/flags; offline/stale is explicit; no firewall mutations |
| P8.2 Scheduled jobs | Schedule UUID, app UUID, desired expression/timezone/command reference, enabled flag/revision; execution history | Validate expression/timezone/limits; no root owner; saving does not install cron/systemd |
| P8.3 Domains/TLS settings | Domain routing intent, HTTP-01 provider, certificate expiry/status; protected user-supplied cert/key references | No DNS-01/wildcard issuance claim; failures retain desired history; secrets never public |
| P8.4 Database requests | Engine catalog, app-scoped DB/user intent, protected credentials, provisioning task references | No arbitrary SQL/admin DB passwords; unavailable/unsupported engines cannot provision |
| P8.5 Backup policies/requests | Resource scope, schedule, retention and destination credential reference; task/history/restore prerequisites | No fake backup labeled recoverable; no export of app keys/plain secrets; no real restore claim |
| P8.6 Unified request/error views | Shared confirmations, active task links, capability/version errors and history | State/permissions enforced server-side; no unsupported-button bypass or alternate executor |

Service controls resolve IDs from a managed snapshot, not unit names typed by a
caller. Package selection is a catalog ID; repository installation/upgrade policy
belongs to the later agent/package phase. Fake catalog entries are labeled samples.
The installed panel dependencies are not the OS package catalog.

Schedules may use existing cron-expression support for parsing, but do not run the
saved command in Laravel's own scheduler. Laravel scheduling runs only panel task
maintenance; hosted schedules execute later under the application's Linux user.
Bound execution duration/output/overlap policy. Cron validity is not command safety.

Certificate ingestion can use existing OpenSSL inspection to validate public
certificate/private-key matching, size, validity and domain metadata. Keep key/
passphrase encrypted/write-only and upload storage private with bounded file size.
Prevent unauthenticated Livewire upload access and never give a public key-file URL.
Do not emit parser contents in errors or fetch remote issuer/AIA URLs from input.
Issuance/activation/renewal are tasks, not inline shell/ACME calls from PHP.

Database engine choice needs user confirmation before real provisioning. Web
configuration can remain catalog-driven with unsupported options disabled. Never
use the panel's SQLite credentials/data as a hosted application's database.

Backups/restore: phase 8 configures policy and simulated requests only. A "backup"
record carries mode and verification status. Real consistency, encryption-key
custody, restore onto another VM and disk-full handling belong to phase 19. Do not
add a shortcut that zips the panel `.env` into a publicly downloadable archive.

Phase exit: authenticated admin can configure/request all agreed views through the
same validated workflow in fake mode; unavailable/unsupported mode remains closed.
Every screen states what is configured versus simulated versus actually observed.

### 9.11 Phase 9 — Web milestone verification and Rust handoff

**Status: P9.1–P9.5 done; phase 9 and the web milestone complete.** Regression and
interruption tests, the escaped-output/authorization checks, the contract/resource
map and the operator/developer handoff documents exist. Rust begins only in phase
10 when the user supplies the agent environment.

**Goal:** finish the independently runnable web panel and leave an exact agent
contract/acceptance map, not a collection of optimistic dashboard screenshots.
Stop after this gate. Rust starts only when the user supplies/resumes its environment.

| Slice | Deliverable | Acceptance |
|---|---|---|
| P9.1 Complete web regression | Auth/MFA, CRUD, task, secret, simulation/offline/capability and error tests | Happy and denied/error flows pass on PHP 8.3 and 8.4 |
| P9.2 Interruption/queue checks | Worker/browser restart, outbox/lease recovery, unknown execution, cancel/race cases | No lost durable intent, duplicate privileged intent or false terminal success |
| P9.3 Browser/accessibility review | Login/MFA, dashboard, forms, task logs at small/desktop widths | Keyboard/focus/errors work; escaped output; no secret leak or layout overflow |
| P9.4 Contract/resource-map freeze | Schemas, operation list, fixtures, adapter call map and unsupported features | PHP contract fixtures reproducible; known gaps explicitly recorded |
| P9.5 Operator/developer handoff | Setup, worker/scheduler, auth recovery, fake/offline modes, CI and next-agent work | Fresh environment can run web tests/UI without Rust or host mutation |

#### Web acceptance matrix

- Guest cannot access app/resource/task/audit or privileged Livewire actions.
- Ordinary user and admin with absent/stale MFA/recent-auth proof cannot bypass gates.
- Bootstrap remains one-time; password/MFA recovery does not create/promote accounts
  or reopen setup. Recovery-code use/rotation and role change revoke stale authority.
- App CRUD persists revisions and secret references; unsafe identifiers/URLs/domains
  fail safely. Credential/key replacement never leaks prior values or corrupts them.
- Direct HTTP/Livewire ID/status/actor/capability tampering cannot change authority.
- Task double-submit/duplicate jobs/retries preserve one approved immutable intent.
- Queued work rechecks actor/MFA/config/capability/approval expiry before mutation.
- Browser disconnect and queue worker interruption leave durable inspectable history.
- Lost receipt/unknown execution does not cause blind replay; cancellation is accurate.
- Logs and audit are bounded/escaped/redacted, including Laravel failed-job payloads.
- Fake mode is explicit, reproducible and rejected in production; it cannot mark real
  applied state. Default unavailable mode never executes a privileged operation.
- Livewire middleware persists for update requests; real CSRF rejection has evidence.
- SQLite polling/write behavior is bounded. Measure basic contention using concurrent
  panel requests/workers only in a disposable development test DB, not the user's data.
- Both runtime CI jobs pass using committed lockfiles. No Rust tools/host installer
  are needed. No feature is advertised as actually provisioning the Linux server.

#### Files/documents to deliver at the gate

Create these only when the underlying implementation is ready:

- `docs/web-development.md`: exact local setup, port 9000, mode selection, queue and
  scheduler commands, migration/key handling and focused/full test commands.
- `docs/authentication.md`: bootstrap, login/MFA states, recovery procedures, session
  revocation and known deployment prerequisites (HTTPS/cookies/time synchronization).
- `docs/agent-integration.md`: each PHP method and operation -> request/response
  schema -> fixtures -> resource ID source -> permission/approval -> recovery behavior.
- `contracts/v1/README.md`: protocol/dialect version, errors, size/time limits,
  capabilities, idempotency semantics, cancellation and unknown-state handling.
- `docs/web-milestone.md`: passed gates, exact CI run/commit, manual checks actually
  performed, remaining host/VM gates, pending user decisions and the first Rust slice.
- `plan.md`: phase 1–9 status, unresolved blockers and concise final resume card.

The future Rust implementer should be able to take one operation fixture and see
its exact input validation, managed-resource lookup requirement, expected errors,
idempotency boundary, privilege/user context, bounded output and rollback limitations.
They must independently validate these; the PHP schema is not the root trust boundary.

Phase 9 is complete only after the web acceptance matrix and actual CI/manual checks
have evidence. Public beta still requires later genuine agent/installer/upgrade/TLS/
restore and isolation validation on supported VMs. Preserve those gates in phases
10 onward; do not relabel this web milestone as a functioning whole-VPS product.

### 9.12 Decision checkpoints and economical order of execution

Resolve material decisions just before they matter:

| When | Decision to confirm | Independent progress allowed |
|---|---|---|
| P2.2 | First-party Fortify project-local dependency and compatible version | Route/view/access-gate design; never implement ad-hoc MFA crypto |
| P3.1 | Final initial operation shape/limits if implementation reveals a conflict | DTO/fixture tests and unavailable adapter |
| P6.3 | Required Git providers and HTTPS-token/SSH-key authentication modes | Public source metadata, secret-storage helper and reference model |
| P8.3 | User-supplied certificate input/passphrase requirements | HTTP-01 domain configuration and offline/fake TLS views |
| P8.4 / later phase 18 | MariaDB versus PostgreSQL first managed engine | Catalog-driven UI with no real provisioning claims |
| P9 gate | Actual remaining browser/test environment access | Automated PHP/contract/task checks and precise blocked-check record |
| Later phases | Product domain, signing/hosting, license, disposable VM/agent environment | Web-only work is not blocked by these earlier than necessary |

Recommended sequence: P2.2–P2.6 -> P3.1–P3.5 -> P4.1–P4.6 -> P5.1–P5.3 ->
P6.1–P6.5 -> P7.1–P7.5 -> P8.1–P8.6 -> P5.4/P9.1–P9.5 final review. Accessibility
and authorization are built into each feature; the final pass verifies the assembled
product rather than delaying them. Keep the fake end-to-end workflow coherent
before adding another administration screen. Stop at phase 9.
