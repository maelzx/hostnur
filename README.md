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
- [Original requirements](vps-control-panel-plan.md): source product specification;
  subsequent confirmed decisions are recorded in `plan.md`.

## Development status

The next slice creates `panel/` using the existing PHP/Composer/Node/npm tools.
Web development comes first; Rust agent development is deferred to a prepared
environment after the web milestone. No additional runtimes or global tools will
be installed in the current environment. Runnable setup commands will accompany
verified scaffolding.

The panel will use explicitly labeled local/test agent fixtures while the real
agent is pending. Real host operations require later agent integration and VM
validation. There is no released installer yet.
