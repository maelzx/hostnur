# Web milestone

Status: **phase 1–9 complete for the web-only milestone.** The panel is
independently runnable and tested without Rust or any host mutation. This is not a
public beta: no Linux user, source checkout, service, Nginx route, certificate,
database or backup is actually provisioned.

## Automated gates

- Full suite: `php artisan test --compact` — **244 tests / 998 assertions** (local
  PHP 8.4.26).
- Static: `composer check:syntax`, `composer check:style` (Pint), `composer
  validate` (known license warning), `composer check-platform-reqs`,
  `composer audit`, `npm audit`, `npm run build`.
- CI: GitHub Actions `web-panel.yml` on PHP 8.3/Node 24 and PHP 8.4/Node 24 with
  committed lockfiles. The phase-9 checkpoint passed at commit `4ec19da`
  (<https://github.com/maelzx/hostnur/actions/runs/38372991483>); the phase-8
  checkpoint passed at commit `7635290`
  (<https://github.com/maelzx/hostnur/actions/runs/38371962845>).

## Acceptance matrix → evidence

| Acceptance item | Evidence |
|---|---|
| Guest blocked from app/resource/task/audit and privileged Livewire actions | `WebRegressionTest`, per-feature suites |
| Ordinary/absent/stale MFA proof cannot bypass gates | `WebRegressionTest`, `PanelShellTest`, `MfaChallengeTest` |
| Bootstrap one-time; recovery cannot create/promote/reopen; rotation revokes | `BootstrapAdministratorTest`, `RecoverAdministratorTest`, `SecuritySettingsTest` |
| App CRUD persists revisions/secret refs; unsafe identifiers fail | `ApplicationInventoryTest`, `GitRepositoryUrlTest`, `HostnameTest` |
| Direct HTTP/Livewire ID/actor/capability tampering cannot change authority | `WebRegressionTest`, `TaskAuditUiTest`, `DeploymentTest` |
| Task double-submit/duplicate jobs/retry preserve one intent | `TaskRequestTest`, `TaskExecutionTest`, `QueueInterruptionTest` |
| Queued work rechecks actor/MFA/config/capability/approval | `TaskExecutionTest`, `DeploymentTest` |
| Worker interruption leaves durable history | `QueueInterruptionTest`, `TaskExecutionTest` |
| Lost receipt/unknown ≠ success; cancellation accurate | `QueueInterruptionTest`, `TaskExecutionTest`, `DeploymentTest` |
| Logs/audit bounded, escaped, redacted; jobs carry only task IDs | `TaskAuditUiTest`, `WebRegressionTest`, `Redactor` |
| Fake explicit/reproducible, rejected in production; unavailable closed | `AgentAvailabilityTest`, `FakeAgentTest`, `ServerAdministrationTest` |
| Livewire middleware persistence; real CSRF rejection | `MfaChallengeTest`, `AuthenticationTest`, `AppServiceProvider` |
| Both runtime CI jobs pass | CI run above |
| No Rust/installer required; nothing advertised as provisioning the host | `docs/web-development.md`, this document |

## Manual checks actually performed

- Real HTTP smoke on an isolated port at each slice: `/` and `/panel*` redirect
  guests to `/login`; `/login` returns 200; built Vite CSS is served.
- Local CLI: `hostnur:admin:bootstrap`, `hostnur:admin:recover`,
  `hostnur:tasks:reconcile` (0 dispatched / 0 expired leases on an idle DB), and
  `migrate`/`migrate:status` on the local SQLite database.

## Blocked / not performed

- **No real browser review** (keyboard traversal, screen-reader names, 375 px /
  desktop layout, focus order). Rendered-HTML accessibility assertions exist
  (`AccessibilityTest`) but are not a screenshot/a11y certification. A browser was
  not available in this environment; recorded as a blocked manual gate.
- **No disposable VM**: install/systemd/TLS/upgrade/restore/isolation gates remain
  deferred to phases 12–21.
- No Rust toolchain in this environment.

## Remaining gates and pending decisions

Host/VM phases 10–21 remain. Pending user decisions (section 6): official
panel/install/docs domain; Git provider auth modes; first managed database engine
(MariaDB vs PostgreSQL); release signing/hosting; project license; and a disposable
VM/agent environment for phase 11.

## Handoff documents

- [`web-development.md`](web-development.md) — setup, port 9000, modes, queue,
  scheduler, tests.
- [`authentication.md`](authentication.md) — bootstrap, login/MFA, recovery,
  revocation, prerequisites.
- [`agent-integration.md`](agent-integration.md) — operation → schema → resource →
  permission → recovery map.
- [`../contracts/v1/README.md`](../contracts/v1/README.md) — protocol, errors,
  limits, capabilities, idempotency, cancellation, unknown handling.

## First Rust slice

Phase 10: recheck `rust-toolchain.toml` pin/MSRV, scaffold the `agent/` crate in a
user-provided prepared environment, and begin phase 11 (protocol framing, version
negotiation and cross-language compatibility) against `contracts/v1` and the
`AgentClient` interface. The Rust agent must independently validate every input and
own the managed-resource registry; the PHP schema is not the trust boundary.
