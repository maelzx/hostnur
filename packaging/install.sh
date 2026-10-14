#!/usr/bin/env bash
#
# Hostnur bootstrap installer (interim, curl-and-bash).
#
# Installs the Hostnur agent and panel on a clean Ubuntu 24.04 amd64 host from a
# published release bundle. Idempotent: re-running updates the artifacts and
# reloads services. It refuses to overwrite unrelated services/configuration.
#
# Usage:
#   curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh | sudo bash
#   ... | sudo bash -s -- --domain panel.example.com --email you@example.com
#
# Environment overrides:
#   HOSTNUR_VERSION   release tag (default: latest)
#   HOSTNUR_BASE_URL  override the download base URL (e.g. a private mirror)
#
# NOTE: this is the interim installer. Signed Debian/APT packaging and a
# PHP-FPM-based panel service are a later phase; the panel is served by a small
# systemd unit behind nginx here.

set -euo pipefail

HOSTNUR_REPO="${HOSTNUR_REPO:-maelzx/hostnur}"
HOSTNUR_VERSION="${HOSTNUR_VERSION:-}"
HOSTNUR_ASSET="hostnur-linux-x86_64.tar.gz"

INSTALL_ROOT="/opt/hostnur"
PANEL_DIR="/opt/hostnur-panel"
CONTRACTS_DIR="/opt/hostnur/contracts"
CONFIG_DIR="/etc/hostnur"
AGENT_SOCKET="/run/hostnur/agent.sock"
AGENT_STATE="/var/lib/hostnur/agent"
AGENT_BIN="/usr/local/bin/hostnur-agent"
PANEL_PORT="8080"
DOMAIN=""
EMAIL=""
ENABLE_TLS=1

log() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[warn]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

usage() {
  sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
}

while [ $# -gt 0 ]; do
  case "$1" in
    --domain) DOMAIN="${2:-}"; shift 2 ;;
    --email) EMAIL="${2:-}"; shift 2 ;;
    --version) HOSTNUR_VERSION="${2:-}"; shift 2 ;;
    --no-tls) ENABLE_TLS=0; shift ;;
    --help|-h) usage ;;
    *) die "unknown argument: $1" ;;
  esac
done

[ "$(id -u)" -eq 0 ] || die "run as root (use sudo)."
. /etc/os-release
[ "${ID:-}" = "ubuntu" ] || die "only Ubuntu is supported (found ${ID:-unknown})."
[ "$(uname -m)" = "x86_64" ] || die "only x86_64 is supported (found $(uname -m))."
command -v apt-get >/dev/null || die "apt-get is required."

export DEBIAN_FRONTEND=noninteractive

# --- download base URL -------------------------------------------------------
if [ -n "${HOSTNUR_BASE_URL:-}" ]; then
  BASE="$HOSTNUR_BASE_URL"
elif [ -n "$HOSTNUR_VERSION" ]; then
  BASE="https://github.com/${HOSTNUR_REPO}/releases/download/${HOSTNUR_VERSION}"
else
  BASE="https://github.com/${HOSTNUR_REPO}/releases/latest/download"
fi

# --- fetch + verify the bundle ----------------------------------------------
log "Downloading Hostnur bundle from ${BASE}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
curl -fsSL "${BASE}/${HOSTNUR_ASSET}" -o "$WORK/${HOSTNUR_ASSET}" \
  || die "could not download ${HOSTNUR_ASSET} (private repo? set HOSTNUR_BASE_URL / HOSTNUR_VERSION)."
if curl -fsSL "${BASE}/SHA256SUMS" -o "$WORK/SHA256SUMS" 2>/dev/null; then
  log "Verifying checksum"
  ( cd "$WORK" && grep " ${HOSTNUR_ASSET}\$" SHA256SUMS | sha256sum -c - ) \
    || die "checksum verification failed."
else
  warn "no SHA256SUMS published; continuing without checksum verification."
fi

# --- prerequisites -----------------------------------------------------------
log "Installing prerequisites"
# Wait out any first-boot apt lock.
for _ in $(seq 1 60); do
  fuser /var/lib/apt/lists/lock /var/lib/dpkg/lock-frontend >/dev/null 2>&1 || break
  sleep 5
done
apt-get update -qq
apt-get install -y -qq nginx php8.3-fpm php8.3-cli php8.3-sqlite3 php8.3-mbstring php8.3-xml \
  php8.3-curl php8.3-zip php8.3-intl composer git unzip sqlite3 rsync curl ca-certificates >/dev/null

# --- extract -----------------------------------------------------------------
log "Installing files under ${INSTALL_ROOT}"
mkdir -p "$INSTALL_ROOT" "$CONFIG_DIR" "${AGENT_STATE}" "$(dirname "$AGENT_SOCKET")"
tar xzf "$WORK/${HOSTNUR_ASSET}" -C "$WORK"
install -m 0755 "$WORK/hostnur/bin/hostnur-agent" "$AGENT_BIN"
install -m 0644 "$WORK/hostnur/packaging/hostnur-agent.service" /etc/systemd/system/hostnur-agent.service
rm -rf "$CONTRACTS_DIR"; mkdir -p "$CONTRACTS_DIR"
cp -a "$WORK/hostnur/contracts/." "$CONTRACTS_DIR/"
chmod 0755 "$INSTALL_ROOT" "$(dirname "$AGENT_SOCKET")"

# --- agent config ------------------------------------------------------------
if [ ! -f "$CONFIG_DIR/agent.toml" ]; then
  log "Writing ${CONFIG_DIR}/agent.toml"
  cat > "$CONFIG_DIR/agent.toml" <<EOF
version = 1
socket_path = "${AGENT_SOCKET}"
state_dir = "${AGENT_STATE}"
log_level = "info"
socket_mode = 432
connection_read_timeout_seconds = 5
allowed_peer_uids = [0]
php_versions = ["8.3"]
EOF
  chmod 0644 "$CONFIG_DIR/agent.toml"
else
  log "Keeping existing ${CONFIG_DIR}/agent.toml"
fi

systemctl daemon-reload
systemctl enable --now hostnur-agent

# --- panel -------------------------------------------------------------------
if [ -e "$PANEL_DIR/.env" ]; then
  log "Existing panel detected at ${PANEL_DIR}; updating application files only"
  rsync -a --delete \
    --exclude '.env' --exclude 'database/database.sqlite' --exclude 'storage/logs/*' \
    --exclude 'bootstrap/cache/*.php' \
    "$WORK/hostnur/panel/" "$PANEL_DIR/"
else
  log "Installing panel to ${PANEL_DIR}"
  mkdir -p "$PANEL_DIR"
  cp -a "$WORK/hostnur/panel/." "$PANEL_DIR/"
  ( cd "$PANEL_DIR"
    APP_KEY="base64:$(openssl rand 32 | base64)"
    cat > .env <<EOF
APP_NAME=Hostnur
APP_ENV=production
APP_KEY=${APP_KEY}
APP_DEBUG=false
APP_URL=$([ -n "$DOMAIN" ] && echo "https://${DOMAIN}" || echo "http://127.0.0.1:${PANEL_PORT}")
LOG_CHANNEL=stack
DB_CONNECTION=sqlite
SESSION_DRIVER=database
QUEUE_CONNECTION=database
CACHE_STORE=database
HOSTNUR_AGENT_MODE=real
HOSTNUR_AGENT_SOCKET=${AGENT_SOCKET}
HOSTNUR_CONTRACTS_PATH=${CONTRACTS_DIR}/v1
EOF
    php artisan migrate --force
    # Trust the local nginx proxy.
    python3 - <<'PY'
p="bootstrap/app.php"; s=open(p).read()
if "trustProxies" not in s:
    s=s.replace("        ]);\n    })", "        ]);\n        $middleware->trustProxies(at: \"*\");\n    })",1)
    open(p,"w").write(s)
PY
    php artisan config:clear >/dev/null 2>&1 || true
    php artisan view:clear >/dev/null 2>&1 || true
  )
fi

cat > /etc/systemd/system/hostnur-panel.service <<EOF
[Unit]
Description=Hostnur panel (interim web service)
After=network.target

[Service]
Type=simple
WorkingDirectory=${PANEL_DIR}
ExecStart=/usr/bin/php artisan serve --host 127.0.0.1 --port ${PANEL_PORT}
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable --now hostnur-panel

# --- nginx -------------------------------------------------------------------
mkdir -p /etc/nginx/hostnur-sites /var/www/hostnur-acme/.well-known/acme-challenge
[ -f /etc/nginx/conf.d/hostnur-sites.conf ] || echo "include /etc/nginx/hostnur-sites/*.conf;" > /etc/nginx/conf.d/hostnur-sites.conf

if [ -n "$DOMAIN" ] && [ "$ENABLE_TLS" -eq 1 ] && [ -n "$EMAIL" ]; then
  log "Configuring nginx + Let's Encrypt for ${DOMAIN}"
  apt-get install -y -qq certbot >/dev/null
  cat > /etc/nginx/sites-available/hostnur-acme <<EOF
server {
    listen 80;
    server_name ${DOMAIN};
    location ^~ /.well-known/acme-challenge/ { root /var/www/hostnur-acme; }
    location / { return 200 "ok\n"; }
}
EOF
  ln -sf /etc/nginx/sites-available/hostnur-acme /etc/nginx/sites-enabled/hostnur-acme
  nginx -t && systemctl reload nginx
  certbot certonly --webroot -w /var/www/hostnur-acme -d "$DOMAIN" \
    --agree-tos --non-interactive --email "$EMAIL" || warn "certificate issuance failed; panel remains on HTTP"
  cat > /etc/nginx/sites-available/hostnur-panel <<EOF
server {
    listen 80;
    server_name ${DOMAIN};
    location ^~ /.well-known/acme-challenge/ { root /var/www/hostnur-acme; }
    location / { return 301 https://\$host\$request_uri; }
}
server {
    listen 443 ssl;
    server_name ${DOMAIN};
    ssl_certificate /etc/letsencrypt/live/${DOMAIN}/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/${DOMAIN}/privkey.pem;
    location / {
        proxy_pass http://127.0.0.1:${PANEL_PORT};
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;
    }
}
EOF
  rm -f /etc/nginx/sites-enabled/hostnur-acme
else
  warn "no --domain/--email: panel available on http://127.0.0.1:${PANEL_PORT} (use an SSH tunnel)."
fi
rm -f /etc/nginx/sites-enabled/default
ln -sf /etc/nginx/sites-available/hostnur-panel /etc/nginx/sites-enabled/hostnur-panel 2>/dev/null || true
nginx -t && systemctl reload nginx

# --- firewall ----------------------------------------------------------------
if command -v ufw >/dev/null; then
  ufw allow OpenSSH >/dev/null 2>&1 || true
  ufw allow 80/tcp >/dev/null 2>&1 || true
  ufw allow 443/tcp >/dev/null 2>&1 || true
fi

# --- first administrator -----------------------------------------------------
if [ -t 0 ]; then
  log "Create the first administrator next:"
  echo "    cd ${PANEL_DIR} && php artisan hostnur:admin:bootstrap"
else
  log "Create the first administrator: cd ${PANEL_DIR} && php artisan hostnur:admin:bootstrap"
fi

log "Done. Agent: $(systemctl is-active hostnur-agent), panel: $(systemctl is-active hostnur-panel)"
