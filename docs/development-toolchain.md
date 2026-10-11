# Development toolchain baseline

Selected on 2026-10-11, during phase 1, slice 1. These are project engineering
decisions; application dependency resolution, builds, and CI validation follow
in the scaffolding slice.

## Selected versions

| Component | Baseline | Reason / enforcement checkpoint |
|---|---|---|
| PHP | Minimum 8.3; local 8.4 allowed | Laravel 13 supports PHP 8.3–8.5; Ubuntu 24.04 supplies PHP 8.3 |
| Laravel | 13.x | Current major; documented security support until 2028-03-17 |
| Livewire | 4.x | Its Composer metadata explicitly supports Laravel 13 |
| Tailwind | 4.x | Matches the Laravel 13 asset scaffold |
| Vite | 8.x with Laravel Vite plugin 3.x | Matches the current Laravel 13 asset scaffold |
| Composer | 2.10.3 or newer compatible 2.x | Installed 2.9.2 has reported advisories; resolve before dependency installation |
| Node.js | Latest patched 24.x LTS | `.nvmrc` selects the 24 line; check exact patch at provisioning |
| npm | Compatible with selected Node 24 | Commit the scaffold's `package-lock.json`; use `npm ci` thereafter |
| Rust/Cargo | 1.99.0, stable, edition 2024 | Exact build toolchain pinned in `rust-toolchain.toml` |
| Rust MSRV | Initially 1.99 | Set `rust-version = "1.99"` in the future crate; do not promise an untested older compiler |
| SQLite | Ubuntu-provided SQLite, accessed through PDO | Local 3.45.1 available; actual queue/locking behavior remains to be tested |

The selected Rust version is present in the official stable manifest dated
2026-10-01. The pin includes rustfmt and Clippy. Rust is a **development/build**
dependency; an installed Hostnur agent will be a compiled executable.

PHP 8.3 is the initial production baseline, avoiding an extra PHP repository for
the panel on Ubuntu 24.04. PHP versions managed for hosted applications are a
separate runtime-provider decision. Use Composer's platform configuration at
PHP 8.3.0 during scaffolding to avoid accidentally locking PHP-8.4-only packages,
then test on actual PHP 8.3 and 8.4; emulated resolution is not runtime proof.

Node/npm are asset-build tools. Plan prebuilt assets in release packages so users
do not need Node merely to run the panel.

## Development prerequisites

- PHP CLI with Laravel-required Ctype, cURL, DOM, Fileinfo, Filter, Hash, Mbstring,
  OpenSSL, PCRE, PDO, Session, Tokenizer, and XML support.
- PDO SQLite for panel persistence; SQLite3 and ZIP support for tooling; Sodium
  available for later secret-management work.
- PCNTL and POSIX in the CLI environment for Linux worker/process lifecycle work.
  Verify the PHP-FPM configuration separately when production services are added.
- Composer meeting the version baseline, with working release-verification keys.
- Node 24 LTS and npm; an nvm user can run `nvm install` / `nvm use` at the repo root.
  nvm is optional; an equivalent Node installation is sufficient.
- rustup-managed Rust 1.99.0, Cargo, Clippy and rustfmt; C compiler/linker and make.
  Install Rust as the development user, using the official rustup instructions.
- Git, SSH, cURL and unzip. Add pkg-config/native development packages only when
  selected dependencies require them.

Docker/QEMU are not required for initial local scaffolding. A disposable Ubuntu
24.04 amd64 VM is required later for real systemd, installer, TLS, upgrade, and
restore gates.

## Observed development environment

Inspection on 2026-10-11:

| Check | Result |
|---|---|
| Host architecture | Linux x86_64 |
| PHP CLI | 8.4.26, `/usr/bin/php`; CLI configuration `/etc/php/8.4/cli/php.ini` |
| Composer | 2.9.2, `/usr/local/bin/composer`; below selected baseline |
| Node/npm | 24.12.0 / 11.6.2; Node patch currency still needs checking |
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

## Next setup checkpoint

1. Provision a verified Composer 2.10.3+ binary and release-verification keys;
   prefer a user-local installation over changing shared tooling. Follow the
   official installer verification procedure and run `composer diagnose` again.
2. Check the latest patched Node 24 release and use it for dependency installation.
3. Install the pinned Rust toolchain as the development user; verify versions and
   that rustfmt/Clippy are present. The pin alone does not install rustup.
4. Scaffold the Laravel 13 application and add Livewire 4 with stable constraints;
   retain Tailwind 4/Vite 8 from the compatible scaffold. Use PHPUnit initially,
   matching the Laravel scaffold; verify its PHP 8.3 compatibility during resolution.
5. Scaffold the independent Rust binary crate with edition 2024 and MSRV 1.99.
6. Commit application lockfiles and document successfully executed run/check
   commands. Verify panel boot, migrations, tests and asset build; verify agent
   build, tests, rustfmt and Clippy. Add CI in its own small slice if needed.

No tool installation, application dependency resolution, application build, or VM
validation was performed in the baseline slice. Phase 1 remains in progress.

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
