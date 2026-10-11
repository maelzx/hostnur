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
- [Original requirements](vps-control-panel-plan.md): source product specification;
  subsequent confirmed decisions are recorded in `plan.md`.

## Development status

`panel/` now contains a runnable Laravel 13 / Livewire 4 foundation with Tailwind 4,
SQLite and database-backed queues. Local setup/run instructions are in
[`panel/README.md`](panel/README.md). The next slice adds web-only CI/static checks,
followed by administrator onboarding and authentication.

Web development comes first; Rust development is deferred to a prepared environment
after the web milestone. Current development uses the existing runtimes/tools.

The root page is a non-interactive scaffold and reports the agent as disconnected.
Later web workflows will use clearly labeled local/test fixtures. Real host
operations require agent integration and VM validation. There is no released
installer yet.
