# Hostnur

A modular, native-first VPS control panel built with Laravel, Livewire, Tailwind
CSS, selective HTMX, and a separate Rust privileged agent.

Hostnur is in the web-panel foundation stage. The initial target is Ubuntu 24.04
LTS on amd64, with PHP/Laravel as the first hosted application workflow.

## Project documents

- [Implementation plan and current handoff](plan.md): decisions, phases, acceptance
  criteria, blockers, and the next small implementation slice.
- [Development toolchain](docs/development-toolchain.md): selected versions,
  prerequisites, inspection results, and setup checkpoints.
- [Web panel setup and checks](panel/README.md): local install, run and verification
  commands for the runnable Laravel/Livewire scaffold.
- [Agent development](docs/agent-development.md): the Rust agent crate, prepared
  environment, toolchain/target decisions and commands.
- [Original requirements](vps-control-panel-plan.md): source product specification;
  subsequent confirmed decisions are recorded in `plan.md`.

## Development status

`panel/` now contains a runnable Laravel 13 / Livewire 4 foundation with Tailwind 4,
SQLite and database-backed queues. Local setup/run instructions are in
[`panel/README.md`](panel/README.md); the selected local port is **9000**.
Web-only GitHub Actions and syntax/style gates pass on PHP 8.3/8.4 and Node 24.
Phase 1 through phase 9 are complete: the full web-only milestone, including
authentication, the typed agent contract (unavailable + deterministic fake
adapters), durable panel tasks with database queue/leases/approvals/audit, the
operations shell with dashboard and task/audit history, application
inventory/source/secrets, deployment intent/preflight/wizard and
server-administration views. The panel is independently runnable and tested; see
[`docs/web-milestone.md`](docs/web-milestone.md) and
[`panel/README.md`](panel/README.md). Phase 10 is also complete: the independent
Rust `agent/` crate (foundation scaffold with typed configuration, diagnostics,
lifecycle and logging; no privileged handlers yet) builds/tests under rustc 1.99.0
in the prepared environment. See [`docs/agent-development.md`](docs/agent-development.md).
Run outcomes and the session handoff are recorded in `plan.md`.

For DeepSeek continuation, start with **`plan.md` section 9.1**, then the selected
phase's detailed slice. Section 9 provides the complete web-only implementation
handoff through phase 9, including file/data maps, tests and acceptance gates.

Web development comes first; Rust development is deferred to a prepared environment
after the web milestone. Current development uses the existing runtimes/tools.

The root page redirects guests to login and administrators to a restricted
`/panel` placeholder that shows the agent as disconnected and offers no
operations. Later web workflows will use clearly labeled local/test fixtures. Real
host operations require agent integration and VM validation. There is no released
installer yet.
