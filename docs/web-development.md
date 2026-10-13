# Web development guide

Local setup, run and verification for the Hostnur web panel. The panel is a
standard Laravel 13 monolith under `panel/`; it runs independently of any Rust
agent and never mutates the host in this milestone.

## Prerequisites

Use the existing runtimes/tools only: PHP 8.3+ (8.4 is used locally), Composer 2.x,
Node 24 and npm 11. SQLite is bundled. No Rust, Docker, QEMU or system packages are
required to build or test the web panel.

## Setup

```bash
cd panel
composer install --no-interaction
composer run post-root-package-install   # copies .env.example -> .env if absent
php artisan key:generate --no-interaction # only when APP_KEY is empty
php -r "file_exists('database/database.sqlite') || touch('database/database.sqlite');"
php artisan migrate --no-interaction
npm ci --no-fund
npm run build
```

Keep an existing `APP_KEY`: encrypted source credentials and application secrets
depend on it. `.env`, the SQLite file, vendor/node_modules and generated assets are
Git-ignored.

Create the one-time administrator (hidden input, no command-line password flag):

```bash
php artisan hostnur:admin:bootstrap
```

## Run

```bash
php artisan serve --host=127.0.0.1 --port=9000 --no-interaction
# separate terminals:
npm run dev
php artisan queue:work --no-interaction
php artisan schedule:work --no-interaction
```

Port 9000 is the selected local panel port. `schedule:work` runs only panel task
maintenance (`hostnur:tasks:reconcile`, every minute); it never runs hosted
application schedules.

## Agent mode

`HOSTNUR_AGENT_MODE` selects the adapter:

- `unavailable` (default): no capabilities/observations; every privileged request
  fails closed before execution.
- `fake`: deterministic simulated adapter, allowed only in `local`/`testing`
  (rejected in production), labelled `mode=fake` throughout the UI.
- `real`: reports unavailable until phase 11 implements transport.

`HOSTNUR_FAKE_SCENARIO` selects the fake execution outcome (`success`/`failure`/
`timeout`). `HOSTNUR_CONTRACTS_PATH` overrides the contract directory (default
`../contracts/v1`).

## Queue and reconciliation

Tasks are durable in `tasks`/`task_events`; jobs carry only the task UUID. A short
DB lease prevents duplicate submission. Recover a crashed worker or a commit that
never enqueued with:

```bash
php artisan hostnur:tasks:reconcile
```

It redispatches queued intent (outbox), polls running tasks and clears expired
leases. It does not rewrite arbitrary task state.

## Tests and static checks

```bash
php artisan test --compact
composer check:syntax      # stage new PHP files first (uses git ls-files)
composer check:style       # Pint + unstaged PHP drift check
composer validate          # known license warning
composer check-platform-reqs
composer audit
npm audit
npm run build
```

CI (`.github/workflows/web-panel.yml`) runs the same gates on PHP 8.3/Node 24 and
PHP 8.4/Node 24 with committed lockfiles. Actual run URLs are recorded in
`../plan.md`.

## Current scope

The web panel is a milestone, not a public beta: no Linux user/source checkout,
service, Nginx, certificate, database or backup is actually provisioned. Every
screen distinguishes configuration, simulation and observation.
