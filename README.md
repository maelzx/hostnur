# Hostnur

Hostnur is a self-hosted control panel for running applications on your own VPS.
It pairs a Laravel/Livewire web panel with a separate privileged Rust agent, and
hosts apps natively on the server using Linux users, systemd, Nginx and PHP-FPM.
It targets Ubuntu 24.04 LTS (amd64) and currently provisions PHP/Laravel apps.

Project status and next steps: [`STATUS.md`](STATUS.md).

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
- `--php84` — also install PHP 8.4 (for applications that need it).

Without `--domain`/`--email`, TLS is skipped and the panel is only reachable on
`http://<server-ip>:80` (open port 80; add a domain + TLS later).

## Create the first administrator

```bash
cd /opt/hostnur-panel && php artisan hostnur:admin:bootstrap
```

Then open the panel URL and sign in. Multi-factor authentication is enrolled on
first sign-in.
