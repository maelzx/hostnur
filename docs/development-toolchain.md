# Development toolchain baseline

Selected on 2026-10-11 during phase 1, slice 1; revised in session 003 for web-first
development. Use installed tools without runtime/global-tool installation or
upgrades. Application dependency resolution and builds passed in session 004;
web CI is the next slice.
Rust prerequisites apply only to the later prepared agent environment.

## Selected versions

| Component | Baseline | Reason / enforcement checkpoint |
|---|---|---|
| PHP | Minimum 8.3; local 8.4 allowed | Laravel 13 supports PHP 8.3–8.5; Ubuntu 24.04 supplies PHP 8.3 |
| Laravel | 13.x | Current major; documented security support until 2028-03-17 |
| Livewire | 4.x | Its Composer metadata explicitly supports Laravel 13 |
| Tailwind | 4.x | Matches the Laravel 13 asset scaffold |
| Vite | 8.x with Laravel Vite plugin 3.x | Matches the current Laravel 13 asset scaffold |
| Composer | Existing 2.9.2 locally; intended 2.10.3+ in CI/releases | Do not upgrade shared tooling here; preserve diagnostic findings and verify actual dependency compatibility |
| Node.js | Existing 24.12.0 locally; patched 24.x for CI | `.nvmrc` selects the 24 line; no local runtime installation/upgrade |
| npm | Compatible with selected Node 24 | Commit the scaffold's `package-lock.json`; use `npm ci` thereafter |
| Rust/Cargo | Deferred baseline 1.99.0, edition 2024 | Stored pin in `rust-toolchain.toml`; recheck in prepared environment at phase 10 |
| Rust MSRV | Deferred initial 1.99 | Verify with future crate/dependencies in phase 10; not a web prerequisite |
| SQLite | Ubuntu-provided SQLite, accessed through PDO | Local 3.45.1 available; actual queue/locking behavior remains to be tested |

The selected Rust version is present in the official stable manifest dated
2026-10-01. The pin includes rustfmt and Clippy, but requires no action in the current
environment. Rust is a later **agent build** dependency; the web panel builds and
tests independently. An installed Hostnur agent will be a compiled executable.

PHP 8.3 is the initial production baseline, avoiding an extra PHP repository for
the panel on Ubuntu 24.04. PHP versions managed for hosted applications are a
separate runtime-provider decision. Use Composer's platform configuration at
PHP 8.3.0 during scaffolding to avoid accidentally locking PHP-8.4-only packages,
then test on actual PHP 8.3 and 8.4; emulated resolution is not runtime proof.

Node/npm are asset-build tools. Plan prebuilt assets in release packages so users
do not need Node merely to run the panel.

## Current web-development prerequisites

- PHP CLI with Laravel-required Ctype, cURL, DOM, Fileinfo, Filter, Hash, Mbstring,
  OpenSSL, PCRE, PDO, Session, Tokenizer, and XML support.
- PDO SQLite for panel persistence; SQLite3 and ZIP support for tooling; Sodium
  available for later secret-management work.
- PCNTL and POSIX in the CLI environment for Linux worker/process lifecycle work.
  Verify the PHP-FPM configuration separately when production services are added.
- Existing Composer 2.9.2; confirm package compatibility during scaffold resolution.
- Existing Node 24.12.0 and npm 11.6.2. `.nvmrc` is version metadata, not an
  instruction to install nvm or another Node version in this environment.
- Existing Git, SSH, cURL and unzip.
- Install only project-local Composer/npm dependencies for the panel. No runtime,
  system-package, global-tool installation, Composer self-update or key-setup changes.

Rust/Cargo/rustup, Clippy/rustfmt and C linker prerequisites belong to phase 10 in
the user's prepared agent environment. Native dependency additions there must be
based on actual selected crates, rather than blocking current web work.

Docker/QEMU are not required for initial local scaffolding. A disposable Ubuntu
24.04 amd64 VM is required later for real systemd, installer, TLS, upgrade, and
restore gates.

## Observed development environment

Inspection on 2026-10-11:

| Check | Result |
|---|---|
| Host architecture | Linux x86_64 |
| PHP CLI | 8.4.26, `/usr/bin/php`; CLI configuration `/etc/php/8.4/cli/php.ini` |
| Composer | 2.9.2, `/usr/local/bin/composer`; retained for local web work |
| Node/npm | 24.12.0 / 11.6.2; retained for local web work |
| PHP extensions | All prerequisites above available |
| SQLite / PDO | SQLite 3.45.1; PDO sqlite and mysql available |
| Rust/Cargo/rustup | Unavailable in PATH |
| C compiler/make | cc, gcc and make available |
| pkg-config | Unavailable; not yet a demonstrated dependency |
| Git/SSH/cURL/unzip | Available; Git 2.43.0 |
| Docker/QEMU | Unavailable |

`composer diagnose` confirmed Packagist/GitHub connectivity, platform settings,
Git, and free disk space. It also reported missing release-verification public
keys and 10 advisories affecting its bundled Composer/Symfony packages. This was
a diagnostic of the existing **tool**, not Hostnur application dependencies.
Do not describe it as a clean diagnostic or a Hostnur dependency audit.

These diagnostic findings remain on record. The user has chosen to keep existing
tools. Do not treat the intended newer CI/release Composer baseline as a mandatory
local installation step, or disable advisory checks to force package resolution.
If actual incompatibility blocks a check, record it and continue independent work.

## Next web-only checkpoint

The scaffold steps below were completed in session 004. Reproduction commands
are in [`panel/README.md`](../panel/README.md). The next active checkpoint is item 4,
web-only CI/static checks, followed by the web features in item 5.

1. Scaffold Laravel 13 in `panel/` with installed PHP/Composer, stable constraints,
   Livewire 4 and the compatible Tailwind 4/Vite scaffold. Use scaffold PHPUnit
   initially and verify PHP 8.3 compatibility during dependency resolution.
2. Configure Hostnur identity, SQLite and database-backed queues. Exclude local
   `.env`, credentials, SQLite state, vendor/node_modules and generated assets
   from source control; retain examples and lockfiles.
3. Verify panel boot, migrations, baseline tests and asset build using installed
   tools. Document exact commands and any blocked checks.
4. Add PHP 8.3/8.4 web CI with appropriate maintained tools in a subsequent small
   slice if needed. Rust jobs must not gate web-only progress.
5. Build authentication, PHP contracts/local test adapters, tasks and panel workflows
   through phases 2–9. Keep fixture results visibly simulated and unavailable
   production operations rejected server-side.

## Later agent checkpoint

At phase 10, switch to the prepared environment, recheck the Rust pin/MSRV, scaffold
the independent agent and add its own build/test/format/Clippy CI. Phase 11 adds
the actual transport and cross-language validation; later VM gates prove host
operations. The current machine does not need Rust, Docker, QEMU or new packages.

## Verified scaffold checkpoint (session 004)

- Locked Laravel 13.35.0, Livewire 4.4.7, Tailwind 4.3.3, Vite 8.3.4 and Laravel
  Vite plugin 3.2.0. Laravel Boost 2.10.3 is a project-local dev dependency required
  by the upstream scaffold's repository guidance.
- Existing PHP 8.4.26/Composer 2.9.2/Node 24.12.0/npm 11.6.2 resolved dependencies
  and passed application platform checks, two baseline tests, migrations, empty
  database queue-worker startup, template compilation and asset build.
- Application Composer/npm audits passed. `composer validate` passed with the
  missing-license warning; strict validation awaits the license decision.
- Local HTTP checks passed for the foundation page, health endpoint, Livewire
  JavaScript and built CSS. Actual PHP 8.3 validation awaits CI.
- No runtime/global tools were installed/upgraded; no real host-operation VM
  validation or browser visual automation has been performed.

## Source references

- [Laravel 13 release/support policy](https://laravel.com/docs/13.x/releases)
- [Laravel 13 scaffold Composer metadata](https://github.com/laravel/laravel/blob/13.x/composer.json)
- [Laravel 13 scaffold asset metadata](https://github.com/laravel/laravel/blob/13.x/package.json)
- [Livewire 4 installation](https://livewire.laravel.com/docs/4.x/installation)
- [Livewire 4 Composer compatibility](https://github.com/livewire/livewire/blob/4.x/composer.json)
- [Ubuntu 24.04 default PHP](https://packages.ubuntu.com/noble/php)
- [Rust stable release manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml)
- [Official rustup setup](https://www.rust-lang.org/tools/install)
- [Verified Composer installation](https://getcomposer.org/download/)

References describe upstream state at inspection time; dependency lockfiles and CI
will become the authoritative record of exact application versions.
