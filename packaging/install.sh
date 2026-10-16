#!/usr/bin/env bash
#
# Hostnur bootstrap installer (interim, curl-and-bash).
#
# Installs the Hostnur agent (privileged systemd service) and the panel served by
# nginx + PHP-FPM under a dedicated unprivileged user, from a published release
# bundle. Idempotent: re-running updates the artifacts and reloads services.
#
# Usage:
#   curl -fsSL https://github.com/maelzx/hostnur/releases/latest/download/install.sh | sudo bash
#   ... | sudo bash -s -- --domain panel.example.com --email you@example.com
#
# Environment overrides:
#   HOSTNUR_VERSION   release tag (default: latest)
#   HOSTNUR_BASE_URL  override the download base URL (e.g. a private mirror)
#
# NOTE: this is the interim installer. Signed Debian/APT packaging and coordinated
# upgrades are a later phase.

set -euo pipefail

HOSTNUR_REPO="${HOSTNUR_REPO:-maelzx/hostnur}"
HOSTNUR_VERSION="${HOSTNUR_VERSION:-}"
HOSTNUR_ASSET="hostnur-linux-x86_64.tar.gz"

PHP_VERSION="8.3"
INSTALL_ROOT="/opt/hostnur"
PANEL_DIR="/opt/hostnur-panel"
CONTRACTS_DIR="/opt/hostnur/contracts"
CONFIG_DIR="/etc/hostnur"
AGENT_SOCKET="/run/hostnur/agent.sock"
AGENT_STATE="/var/lib/hostnur/agent"
AGENT_BIN="/usr/local/bin/hostnur-agent"
PANEL_USER="hostnur-panel"
PANEL_GROUP="hostnur-panel"
ACCESS_GROUP="hostnur"
PANEL_POOL="hostnur-panel"
PANEL_SOCK="/run/php/hostnur-panel.sock"
DOMAIN=""
EMAIL=""
ENABLE_TLS=1

log() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[warn]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

usage() { sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0; }

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

if [ -n "${HOSTNUR_BASE_URL:-}" ]; then
  BASE="$HOSTNUR_BASE_URL"
elif [ -n "$HOSTNUR_VERSION" ]; then
  BASE="https://github.com/${HOSTNUR_REPO}/releases/download/${HOSTNUR_VERSION}"
else
  BASE="https://github.com/${HOSTNUR_REPO}/releases/latest/download"
fi

# --- fetch + verify ----------------------------------------------------------
log "Downloading Hostnur bundle from ${BASE}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
curl -fsSL "${BASE}/${HOSTNUR_ASSET}" -o "$WORK/${HOSTNUR_ASSET}" \
  || die "could not download ${HOSTNUR_ASSET} (private repo? set HOSTNUR_BASE_URL / HOSTNUR_VERSION)."
if curl -fsSL "${BASE}/SHA256SUMS" -o "$WORK/SHA256SUMS" 2>/dev/null; then
  log "Verifying checksum"
  ( cd "$WORK" && grep " ${HOSTNUR_ASSET}\$" SHA256SUMS | sha256sum -c - ) || die "checksum verification failed."
else
  warn "no SHA256SUMS published; continuing without checksum verification."
fi

# --- prerequisites -----------------------------------------------------------
log "Installing prerequisites"
for _ in $(seq 1 60); do
  fuser /var/lib/apt/lists/lock /var/lib/dpkg/lock-frontend >/dev/null 2>&1 || break
  sleep 5
done
apt-get update -qq
apt-get install -y -qq nginx php${PHP_VERSION}-fpm php${PHP_VERSION}-cli php${PHP_VERSION}-sqlite3 \
  php${PHP_VERSION}-mbstring php${PHP_VERSION}-xml php${PHP_VERSION}-curl php${PHP_VERSION}-zip \
  php${PHP_VERSION}-intl composer git unzip sqlite3 rsync curl ca-certificates python3 >/dev/null

# --- service identities ------------------------------------------------------
log "Creating service identities"
groupadd -f "$ACCESS_GROUP"
if ! id -u "$PANEL_USER" >/dev/null 2>&1; then
  useradd --system --user-group --no-create-home --shell /usr/sbin/nologin "$PANEL_USER"
fi
usermod -aG "$ACCESS_GROUP" "$PANEL_USER"
PANEL_UID="$(id -u "$PANEL_USER")"

# --- extract -----------------------------------------------------------------
log "Installing files under ${INSTALL_ROOT}"
mkdir -p "$INSTALL_ROOT" "$CONFIG_DIR" "$AGENT_STATE" "$(dirname "$AGENT_SOCKET")"
tar xzf "$WORK/${HOSTNUR_ASSET}" -C "$WORK"
install -m 0755 "$WORK/hostnur/bin/hostnur-agent" "$AGENT_BIN"
install -m 0644 "$WORK/hostnur/packaging/hostnur-agent.service" /etc/systemd/system/hostnur-agent.service
rm -rf "$CONTRACTS_DIR"; mkdir -p "$CONTRACTS_DIR"
cp -a "$WORK/hostnur/contracts/." "$CONTRACTS_DIR/"

# --- agent config ------------------------------------------------------------
if [ ! -f "$CONFIG_DIR/agent.toml" ]; then
  log "Writing ${CONFIG_DIR}/agent.toml"
  cat > "$CONFIG_DIR/agent.toml" <<EOF
version = 1
socket_path = "${AGENT_SOCKET}"
state_dir = "${AGENT_STATE}"
log_level = "info"
socket_mode = 432
socket_group = "${ACCESS_GROUP}"
connection_read_timeout_seconds = 5
allowed_peer_uids = [${PANEL_UID}]
php_versions = ["8.3", "8.4"]
EOF
  chmod 0644 "$CONFIG_DIR/agent.toml"
else
  log "Keeping existing ${CONFIG_DIR}/agent.toml"
fi

systemctl daemon-reload
systemctl enable --now hostnur-agent

# --- panel -------------------------------------------------------------------
log "Installing panel to ${PANEL_DIR}"
mkdir -p "$PANEL_DIR"
if [ -f "$PANEL_DIR/.env" ]; then
  rsync -a --delete \
    --exclude '.env' --exclude 'database/database.sqlite' --exclude 'storage/logs/*' \
    --exclude 'bootstrap/cache/*.php' \
    "$WORK/hostnur/panel/" "$PANEL_DIR/"
else
  cp -a "$WORK/hostnur/panel/." "$PANEL_DIR/"
  ( cd "$PANEL_DIR"
    APP_KEY="base64:$(openssl rand 32 | base64)"
    cat > .env <<EOF
APP_NAME=Hostnur
APP_ENV=production
APP_KEY=${APP_KEY}
APP_DEBUG=false
APP_URL=$([ -n "$DOMAIN" ] && echo "https://${DOMAIN}" || echo "http://127.0.0.1")
LOG_CHANNEL=stack
DB_CONNECTION=sqlite
SESSION_DRIVER=database
QUEUE_CONNECTION=database
CACHE_STORE=database
HOSTNUR_AGENT_MODE=real
HOSTNUR_AGENT_SOCKET=${AGENT_SOCKET}
HOSTNUR_CONTRACTS_PATH=${CONTRACTS_DIR}/v1
EOF
    php artisan key:generate --force >/dev/null
    touch database/database.sqlite
    php artisan migrate --force
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

# Panel is owned by the dedicated unprivileged identity; storage/cache writable.
chown -R "$PANEL_USER:$PANEL_GROUP" "$PANEL_DIR"
find "$PANEL_DIR/storage" "$PANEL_DIR/bootstrap/cache" -type d -exec chmod 0775 {} + 2>/dev/null || true
if [ -f "$PANEL_DIR/database/database.sqlite" ]; then
  chown "$PANEL_USER:$PANEL_GROUP" "$PANEL_DIR/database/database.sqlite"
fi

# --- PHP-FPM pool for the panel ---------------------------------------------
log "Configuring PHP-FPM pool ${PANEL_POOL}"
cat > "/etc/php/${PHP_VERSION}/fpm/pool.d/${PANEL_POOL}.conf" <<EOF
[${PANEL_POOL}]
user = ${PANEL_USER}
group = ${PANEL_GROUP}
listen = ${PANEL_SOCK}
listen.owner = www-data
listen.group = www-data
listen.mode = 0660
pm = dynamic
pm.max_children = 5
pm.start_servers = 1
pm.min_spare_servers = 1
pm.max_spare_servers = 2
pm.max_requests = 500
chdir = ${PANEL_DIR}
catch_workers_output = yes
php_admin_value[error_log] = /var/log/php-fpm-${PANEL_POOL}.log
EOF
systemctl restart "php${PHP_VERSION}-fpm"

# --- nginx -------------------------------------------------------------------
log "Configuring nginx"
mkdir -p /etc/nginx/hostnur-sites /var/www/hostnur-acme/.well-known/acme-challenge
[ -f /etc/nginx/conf.d/hostnur-sites.conf ] || echo "include /etc/nginx/hostnur-sites/*.conf;" > /etc/nginx/conf.d/hostnur-sites.conf
rm -f /etc/nginx/sites-enabled/default

write_panel_site() {
  if [ "$1" = "on" ]; then
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
    root ${PANEL_DIR}/public;
    index index.php;
    location / { try_files \$uri \$uri/ /index.php?\$query_string; }
    location ~ \.php\$ {
        include snippets/fastcgi-php.conf;
        fastcgi_pass unix:${PANEL_SOCK};
        fastcgi_param HTTPS on;
    }
    location ~ /\.(?!well-known) { deny all; }
}
EOF
  else
    cat > /etc/nginx/sites-available/hostnur-panel <<EOF
server {
    listen 80 default_server;
    server_name _;
    root ${PANEL_DIR}/public;
    index index.php;
    location / { try_files \$uri \$uri/ /index.php?\$query_string; }
    location ~ \.php\$ {
        include snippets/fastcgi-php.conf;
        fastcgi_pass unix:${PANEL_SOCK};
    }
    location ~ /\.(?!well-known) { deny all; }
}
EOF
  fi
}

if [ -n "$DOMAIN" ] && [ "$ENABLE_TLS" -eq 1 ] && [ -n "$EMAIL" ]; then
  apt-get install -y -qq certbot >/dev/null
  # One implementation for TLS: the panel's own idempotent command (also used by
  # the Debian package post-install and for later renewals/domain changes).
  if php "$PANEL_DIR/artisan" hostnur:panel:setup-tls --domain "$DOMAIN" --email "$EMAIL"; then
    rm -f /etc/nginx/sites-enabled/hostnur-acme
  else
    warn "TLS setup failed; serving the panel over HTTP."
    write_panel_site off
  fi
else
  write_panel_site off
  [ -n "$DOMAIN" ] || warn "no --domain/--email: serving the panel on :80 (use an IP or add a domain + TLS later)."
fi

ln -sf /etc/nginx/sites-available/hostnur-panel /etc/nginx/sites-enabled/hostnur-panel
nginx -t && systemctl reload nginx

if systemctl list-unit-files hostnur-panel.service >/dev/null 2>&1; then
  systemctl disable --now hostnur-panel.service >/dev/null 2>&1 || true
  rm -f /etc/systemd/system/hostnur-panel.service
  systemctl daemon-reload
fi

# --- panel queue worker + scheduler -----------------------------------------
log "Configuring panel queue worker and scheduler"
cat > /etc/systemd/system/hostnur-panel-worker.service <<EOF
[Unit]
Description=Hostnur panel queue worker
After=network.target

[Service]
Type=simple
User=${PANEL_USER}
Group=${PANEL_GROUP}
WorkingDirectory=${PANEL_DIR}
ExecStart=/usr/bin/php artisan queue:work --sleep=3 --tries=1 --max-time=3600
Restart=always
RestartSec=5
StopWaitSecs=60

[Install]
WantedBy=multi-user.target
EOF

cat > /etc/systemd/system/hostnur-panel-schedule.service <<EOF
[Unit]
Description=Hostnur panel scheduler (schedule:run)

[Service]
Type=oneshot
User=${PANEL_USER}
Group=${PANEL_GROUP}
WorkingDirectory=${PANEL_DIR}
ExecStart=/usr/bin/php artisan schedule:run
EOF

cat > /etc/systemd/system/hostnur-panel-schedule.timer <<'EOF'
[Unit]
Description=Run the Hostnur panel scheduler every minute

[Timer]
OnCalendar=*-*-* *:*:00
AccuracySec=1s
Persistent=true

[Install]
WantedBy=timers.target
EOF

systemctl daemon-reload
systemctl enable --now hostnur-panel-worker.service
systemctl enable --now hostnur-panel-schedule.timer

# --- firewall ----------------------------------------------------------------
if command -v ufw >/dev/null; then
  ufw allow OpenSSH >/dev/null 2>&1 || true
  ufw allow 80/tcp >/dev/null 2>&1 || true
  ufw allow 443/tcp >/dev/null 2>&1 || true
fi

# --- first administrator -----------------------------------------------------
log "Create the first administrator: cd ${PANEL_DIR} && sudo -u ${PANEL_USER} php artisan hostnur:admin:bootstrap"
log "Done. Agent: $(systemctl is-active hostnur-agent), php-fpm: $(systemctl is-active php${PHP_VERSION}-fpm), nginx: $(systemctl is-active nginx)"
