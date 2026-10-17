# Packaging and installation

Two install paths are provided:

- **Debian packages + APT repository** (`packaging/debian`, `packaging/apt`,
  [`.github/workflows/packages.yml`](../.github/workflows/packages.yml)) — builds
  `hostnur` and `hostnur-agent`, tests install/upgrade/purge on Ubuntu 24.04, and
  can publish a signed APT repository. This is the coordinated-upgrade path.
- **curl-and-bash interim installer** (`install.sh`,
  [`.github/workflows/release.yml`](../.github/workflows/release.yml)) — a single
  bundle for quick/self-serve installs.

Both install the same layout: the privileged agent (systemd, root) and the panel
under nginx + PHP-FPM as a dedicated unprivileged user. Live-host validation of
the packages (systemd services, real upgrades) is part of the disposable-VM phase.

## Debian packages

Built by `packaging/debian/build-deb.sh` from a release bundle:

```bash
packaging/debian/build-deb.sh --version 0.1.0 --bundle /path/to/stage/hostnur --out dist
```

- `hostnur-agent` — `/usr/bin/hostnur-agent`, `hostnur-agent.service`,
  `/etc/hostnur/agent.toml` (conffile), shared `hostnur` group, state dirs.
- `hostnur` — the panel under `/opt/hostnur-panel` + `/opt/hostnur/contracts`,
  nginx vhost and PHP-FPM pool (conffiles), queue-worker + scheduler units.
  `Depends` on nginx/PHP and `hostnur-agent (= version)` for coordinated versions.

Maintainer scripts create the identities, initialise `.env`/database once, point
the agent's `allowed_peer_uids` at the panel user, and reload services. The panel
SQLite database and `.env` are intentionally preserved on purge.

## HTTPS / TLS

TLS is configured by one idempotent command, `hostnur:panel:setup-tls`, used by
both install paths (and safe to re-run for renewals, domain changes or recovery):

```bash
sudo php /opt/hostnur-panel/artisan hostnur:panel:setup-tls \
  --domain panel.example.com --email you@example.com
```

It serves ACME challenges on `:80`, requests a Let's Encrypt certificate via the
webroot, writes the `:80 → :443` redirect + TLS vhost, and sets `APP_URL`.

- **Interim installer**: pass `--domain`/`--email` (or `--no-tls`) to configure TLS
  at install time; the installer calls the command.
- **Debian package**: copy `panel-tls.conf.example` to `/etc/hostnur/panel-tls.conf`
  with `DOMAIN=`/`EMAIL=` set *before* installing `hostnur`; the post-install script
  calls the command. Without the file the panel is served over HTTP, and TLS can be
  enabled later with the same command.

## APT repository

`packaging/apt/build-repo.sh` assembles a flat repository and (with `--key`) signs
it:

```bash
packaging/apt/build-repo.sh --debs dist --out public --key <gpg-key-id>
```

CI (`.github/workflows/packages.yml`) builds and tests the packages on every tag,
and publishes a signed repository to the `gh-pages` branch when `GPG_PRIVATE_KEY`
is configured as a repository secret.

## Interim bundle (later in this file)

## What the release pipeline produces

On a `v*` tag (or a manual `workflow_dispatch`), the workflow builds and attaches
to a GitHub Release:

- `hostnur-linux-x86_64.tar.gz` — the amd64 agent binary, the panel (production
  Composer dependencies + built Vite assets), the `contracts/` directory and the
  systemd unit.
- `install.sh` — the bootstrap installer.
- `SHA256SUMS` — checksums.

## Install (public repo)

```bash
curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh \
  | sudo bash -s -- --domain panel.example.com --email you@example.com
```

Without `--domain`/`--email` the panel is only reachable on
`http://127.0.0.1:8080` (use an SSH tunnel); TLS is skipped. Flags/env:

- `--domain <fqdn>` — serve the panel on this hostname with Let's Encrypt TLS.
- `--email <addr>` — ACME contact email (required for `--domain`).
- `--no-tls` — never request a certificate.
- `--php84` — also install PHP 8.4 (ondrej PPA) for applications that need it.
- `--version <tag>` / `HOSTNUR_VERSION` — install a specific release.
- `HOSTNUR_BASE_URL` — override the download base (private mirror / testing).

Node.js/npm is installed automatically (Laravel frontend builds). When `--email` is
given it is also written to `agent.toml` as `acme_email`, so the agent can issue
certificates for hosted applications (not just the panel).

After install, create the first administrator:

```bash
cd /opt/hostnur-panel && php artisan hostnur:admin:bootstrap
```

## Updating an existing install

**Interim installer** — re-run the same command (idempotent). It downloads the new
release, **stops the agent** before replacing its binary, updates the panel + agent +
contracts, runs `php artisan migrate`, clears caches, restarts the panel worker, and
reloads PHP-FPM/nginx. `.env`, the SQLite database and `agent.toml` are preserved.

```bash
# update to the latest release
curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh \
  | sudo bash -s -- --domain panel.example.com --email you@example.com
# or pin a version
curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh \
  | sudo bash -s -- --version v0.1.3 --domain panel.example.com --email you@example.com
```

Flags are not persisted between runs — pass the same `--domain`/`--email` (and
`--php84` if you used it) again. Panel migrations run automatically on update.

**Debian packages** (once the APT repository is published):

```bash
sudo apt update && sudo apt upgrade hostnur hostnur-agent
```

## Private repository (until the repo is public)

Release assets require authentication while the repo is private. Download the
bundle and installer with a token, then run the installer locally:

```bash
GH=maelzx/hostnur
curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" \
  "https://api.github.com/repos/$GH/releases/latest" > release.json
# resolve asset URLs, download hostnur-linux-x86_64.tar.gz + install.sh + SHA256SUMS,
# then: sudo bash install.sh --domain ... --email ...
```

Or point the installer at an authenticated mirror via `HOSTNUR_BASE_URL`.

## Security notes

- The installer verifies `SHA256SUMS` when published. Checksums alone are not a
  signature; authenticated signing is part of the deferred Debian/APT work.
- It installs only from the release bundle (no `git clone`, no build tools
  required on the target). Development tooling (Node/Rust/Composer dev) is not
  needed to run an installed panel.
- The panel is served by **nginx + PHP-FPM** under a dedicated unprivileged user
  (`hostnur-panel`); the agent runs as a privileged systemd service and exposes
  its socket to a dedicated group (`hostnur`) that the panel user joins, with a
  peer-UID allowlist. (`php artisan serve` is used only for local development.)
- Existing `/etc/hostnur/agent.toml` is preserved; the panel `.env` is preserved
  on re-run. Unmanaged Nginx sites are not deleted.
