# Hostnur

Hostnur is a self-hosted control panel for running applications on your own VPS.
It pairs a Laravel/Livewire web panel with a separate privileged Rust agent, and
hosts apps natively on the server using Linux users, systemd, Nginx and PHP-FPM.
It targets Ubuntu 24.04 LTS (amd64) and currently provisions PHP/Laravel apps.

## Requirements

- A fresh Ubuntu 24.04 LTS **amd64** server with root access.
- A domain pointed at the server (optional, for HTTPS).

## Install

```bash
curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh \
  | sudo bash -s -- --domain panel.example.com --email you@example.com
```

Flags:

- `--domain <fqdn>` — serve the panel on this hostname and request a Let's Encrypt certificate.
- `--email <addr>` — ACME contact email (required together with `--domain`).
- `--no-tls` — never request a certificate.

Without `--domain`/`--email`, TLS is skipped and the panel is only reachable on
`http://127.0.0.1:8080` (connect over an SSH tunnel).

## Create the first administrator

```bash
cd /opt/hostnur-panel && php artisan hostnur:admin:bootstrap
```

Then open the panel URL and sign in. Multi-factor authentication is enrolled on
first sign-in.
