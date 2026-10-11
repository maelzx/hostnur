# Hostnur

A modular, native-first VPS control panel built with Laravel, Livewire, Tailwind
CSS, selective HTMX, and a separate Rust privileged agent.

Hostnur is in the foundation stage. The initial target is Ubuntu 24.04 LTS on
amd64, with PHP/Laravel as the first hosted application workflow.

## Project documents

- [Implementation plan and current handoff](plan.md): decisions, phases, acceptance
  criteria, blockers, and the next small implementation slice.
- [Development toolchain](docs/development-toolchain.md): selected versions,
  prerequisites, inspection results, and setup checkpoints.
- [Original requirements](vps-control-panel-plan.md): source product specification;
  subsequent confirmed decisions are recorded in `plan.md`.

## Development status

The panel and agent have not been scaffolded yet. Check the toolchain prerequisites
before creating `panel/` and `agent/`; runnable setup commands will be documented
alongside verified scaffolding. There is no released installer yet.
