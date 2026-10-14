# Packaging and installation (interim)

Signed Debian/APT packaging and coordinated upgrades are deferred (plan phase 20
substitute). Until then, releases are produced by
[`.github/workflows/release.yml`](../.github/workflows/release.yml) and installed
with a **curl-and-bash** bootstrap script.

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
- `--version <tag>` / `HOSTNUR_VERSION` — install a specific release.
- `HOSTNUR_BASE_URL` — override the download base (private mirror / testing).

After install, create the first administrator:

```bash
cd /opt/hostnur-panel && php artisan hostnur:admin:bootstrap
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
- The panel's web service here is a small `php artisan serve` unit behind nginx;
  a PHP-FPM service and full ownership model are part of the deferred packaging
  phase. The agent runs as a privileged systemd service.
- Existing `/etc/hostnur/agent.toml` is preserved; the panel `.env` is preserved
  on re-run. Unmanaged Nginx sites are not deleted.
