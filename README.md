# Hostnur

A modular, native-first VPS control panel: a Laravel/Livewire web panel plus a
separate privileged **Rust agent**. It deploys and manages applications on a
single Ubuntu 24.04 LTS (amd64) host using native Linux users, systemd, Nginx and
PHP-FPM — first-class support is currently **PHP/Laravel**.

## Install (interim, curl-and-bash)

On a clean Ubuntu 24.04 **amd64** VM/VPS, as root:

```bash
curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh \
  | sudo bash -s -- --domain panel.example.com --email you@example.com
```

Omit `--domain`/`--email` to skip TLS (panel reachable on `http://127.0.0.1:8080`
via an SSH tunnel). Then create the first administrator:

```bash
cd /opt/hostnur-panel && php artisan hostnur:admin:bootstrap
```

See [`packaging/README.md`](packaging/README.md) for the private-repo download
flow, flags and security notes. Signed Debian/APT packaging and coordinated
upgrades are deferred; releases are produced by
[`.github/workflows/release.yml`](.github/workflows/release.yml) and installed by
the attached `install.sh`.

## What it can do today

- **Administrator onboarding, login, MFA and recovery**; single-operator, admin-only.
- **Applications**: CRUD, source/credential/secret/domain configuration with
  revisions, and an unprivileged **deploy** pipeline (Git checkout pinned to a
  commit, Composer and Vite builds as the app user, `.env` handling, `key:generate`,
  `storage:link`, opt-in migrations) behind an agent-owned `current` release
  pointer with **health-gated activation and rollback**.
- **Isolation**: per-app Linux user + `/opt/hostnur/apps/<uuid>` layout, an
  `openat2`-based safe writer, per-app PHP-FPM pool and Nginx vhost.
- **TLS**: Let's Encrypt HTTP-01 issuance/renewal via a managed `certbot` route,
  plus supplied-certificate install; served and verified with a real handshake.
- **Databases**: managed **MariaDB** per app (isolated principal + grants,
  generated credential stored write-only by the agent, injected into the app's
  deployment env, rotation, bounded export/import, delete).
- **Services/packages/schedules**: allowlisted service control, approved package
  install, app-owned systemd schedule timers (full cron → `OnCalendar`),
  read-only firewall snapshot.
- Durable **tasks/audit** with a database queue, leases and approvals.

Validation was done on a disposable DigitalOcean Ubuntu 24.04 droplet, including
deploying a real Laravel app (**AssetKOM**) on MariaDB with a production TLS cert.

## Current limitations (honest)

- **Status: early / pre-beta.** Phases 1–18 are implemented and VM-validated;
  release packaging, backup/restore and hardening are not finished.
- The panel's own web service is an interim `php artisan serve` unit behind Nginx;
  a PHP-FPM-based panel service and a dedicated non-root panel identity are
  deferred.
- **Migrations run when the deploy requests them**; the stricter "recovery
  prerequisite before migrations" gate is a recorded follow-up.
- **No backup/restore** (phase 19 deferred) and **no signed APT repo / upgrades**
  (phase 20 partly deferred — the interim installer substitutes).
- **PHP/Laravel only** hosted runtime; Python/Node/Go/etc. are deferred.
- Single host; no multi-server, RBAC, module marketplace or AI features.
- The privileged agent runs as root and resolves actions from its own registry;
  fine-grained hardening is pending.

## Project documents

- [Implementation plan and handoff](plan.md): decisions, phases, acceptance
  criteria, blockers and the next slice.
- [Packaging and installation](packaging/README.md): release workflow, installer,
  private-repo flow.
- [Agent development](docs/agent-development.md): the Rust agent crate,
  environment, toolchain/target decisions and commands.
- [Agent integration map](docs/agent-integration.md): operation → schema → resource
  → approval → recovery.
- [Web panel setup and checks](panel/README.md): local install, run and verification.
- [Web milestone](docs/web-milestone.md) and
  [authentication](docs/authentication.md).
- [Disposable test host runbook](docs/p16-test-host.md): rebuild the throwaway
  host used for TLS/deploy validation.
- [Development toolchain](docs/development-toolchain.md): selected versions and
  checkpoints.
- [Original requirements](vps-control-panel-plan.md): source product specification;
  confirmed decisions are recorded in `plan.md`.

## Repository layout

```text
panel/       Laravel + Livewire + Tailwind web panel
agent/       Rust privileged agent crate
contracts/   Versioned protocol schemas, catalog and fixtures
packaging/   systemd units and the interim installer
docs/        Architecture, development and operator documentation
plan.md      Roadmap, decisions and session handoff
```

## Development

The panel runs independently of the agent using a deterministic fake adapter
(`HOSTNUR_AGENT_MODE=fake`); the agent builds and tests under the pinned Rust
toolchain (1.99.0). See [`panel/README.md`](panel/README.md) and
[`docs/agent-development.md`](docs/agent-development.md). The web panel is
local-first (port 9000); real host operations require the agent on a Linux host.
