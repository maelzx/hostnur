# Disposable public test host — rebuild runbook

This documents the throwaway DigitalOcean droplet used to validate **phase 16
(real TLS)** and the **socket-driven deploy pipeline** (phase 15/16). The droplet
is destroyed between sessions; this file rebuilds it from scratch.

Last used: 2026-10-13 (session 032). Repo commits through `a376ef6`
(local-only, not pushed). Dev VM: Lima instance `hostnur-rust` (arm64) used to
cross-build the amd64 agent.

## Inputs (fill in when recreating)

| Item | Value used |
|---|---|
| Droplet | Ubuntu 24.04 LTS, **amd64**, ≥1 vCPU / 1 GB RAM (2 GB swap added) |
| Public IP | `178.128.220.132` (changes on recreate) |
| Panel hostname (DNS A → droplet, DNS-only) | `hostnur-p16.kom.my` |
| App hostname (DNS A → droplet, DNS-only) | `assetkom.hostnur-p16.kom.my` |
| Let's Encrypt email | `maelzx@duck.com` |
| SSH user | `root` (DO default) |
| SSH private key | `~/.ssh/hostnur_p16_ed25519` (Mac) |
| SSH public key to authorize | `ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPvPJ7FS5rXbUL40OqbtFo3YlrHQHDeiV+kuEzFx1R4S hostnur-p16-test` |
| Repo | `https://github.com/maelzx/hostnur` (local `/Users/ismail/Documents/projek gila/hostnur`) |

Add the public key at droplet creation (DO "SSH Keys"), or append it to
`~/.ssh/authorized_keys` after first login. The private key is **not** in the
repo; if lost, generate a new pair (`ssh-keygen -t ed25519`) and add its `.pub`.

Set a shell helper for the rest of this doc:

```bash
export HOST=root@<DROPLET_IP>
export KEY=~/.ssh/hostnur_p16_ed25519
ssh -i "$KEY" -o BatchMode=yes "$HOST" 'echo ok'
```

## 1. Baseline (swap, firewall, packages)

```bash
ssh -i "$KEY" "$HOST" 'bash -s' <<'EOF'
set -e
export DEBIAN_FRONTEND=noninteractive
# 2 GB swap (1 GB RAM is tight for composer/npm)
if [ ! -f /swapfile ]; then
  fallocate -l 2G /swapfile; chmod 600 /swapfile; mkswap /swapfile >/dev/null; swapon /swapfile
  grep -q /swapfile /etc/fstab || echo "/swapfile none swap sw 0 0" >> /etc/fstab
fi
apt-get update -qq
apt-get install -y -qq nginx php8.3-fpm php8.3-cli php8.3-sqlite3 php8.3-mbstring php8.3-xml \
  php8.3-curl php8.3-zip php8.3-intl composer git unzip sqlite3 rsync curl ca-certificates certbot \
  software-properties-common >/dev/null
# PHP 8.4 (external repo, operator opt-in) for AssetKOM
add-apt-repository -y ppa:ondrej/php >/dev/null 2>&1
apt-get update -qq
apt-get install -y -qq php8.4-fpm php8.4-cli php8.4-sqlite3 php8.4-mbstring php8.4-xml \
  php8.4-curl php8.4-zip php8.4-intl php8.4-gd php8.4-mysql >/dev/null 2>&1
# Node 20 (frontend builds)
curl -fsSL https://deb.nodesource.com/setup_20.x | bash - >/dev/null 2>&1
apt-get install -y -qq nodejs >/dev/null 2>&1
# Firewall
ufw allow OpenSSH >/dev/null; ufw allow 80/tcp >/dev/null; ufw allow 443/tcp >/dev/null; ufw --force enable >/dev/null
EOF
```

Verify: `php8.4 -v`, `node -v`, `nginx -v`, `certbot --version`, `swapon --show`.

## 2. Build the agent for amd64 (from the arm64 Lima VM)

The dev VM is `hostnur-rust`; the repo is mounted at the same path.

```bash
limactl shell hostnur-rust -- bash -lc '
  . "$HOME/.cargo/env"
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq gcc-x86-64-linux-gnu
  rustup target add x86_64-unknown-linux-gnu
  cd "/Users/ismail/Documents/projek gila/hostnur/agent"
  export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
  cargo build --locked --release --target x86_64-unknown-linux-gnu'
```

Binary: `agent/target/x86_64-unknown-linux-gnu/release/hostnur-agent`.

## 3. Install the agent on the host

```bash
BIN="/Users/ismail/Documents/projek gila/hostnur/agent/target/x86_64-unknown-linux-gnu/release/hostnur-agent"
scp -i "$KEY" "$BIN" "$HOST:/usr/local/bin/hostnur-agent"   # stop the service first if replacing
scp -i "$KEY" "/Users/ismail/Documents/projek gila/hostnur/packaging/systemd/hostnur-agent.service" "$HOST:/etc/systemd/system/"
ssh -i "$KEY" "$HOST" 'bash -s' <<'EOF'
set -e
mkdir -p /etc/hostnur /opt/hostnur/apps
chmod 755 /opt/hostnur/apps
cat > /etc/hostnur/agent.toml <<'CFG'
version = 1
socket_path = "/run/hostnur/agent.sock"
state_dir = "/var/lib/hostnur/agent"
log_level = "info"
socket_mode = 432
connection_read_timeout_seconds = 5
allowed_peer_uids = [0]
php_versions = ["8.3", "8.4"]
CFG
chmod 755 /usr/local/bin/hostnur-agent; chmod 644 /etc/hostnur/agent.toml
systemctl daemon-reload; systemctl enable --now hostnur-agent
sleep 1; systemctl is-active hostnur-agent; ls -l /run/hostnur/agent.sock
EOF
```

Notes: the unit runs the agent as **root** (needed for UID switching + config
writes); `allowed_peer_uids = [0]` because the panel is run as root on this test
box (production uses a dedicated unprivileged panel identity). To replace the
binary while it runs, `systemctl stop hostnur-agent` first (otherwise `scp` fails
with `ETXTBSY`), then `systemctl start hostnur-agent`.

## 4. Install the panel on the host

```bash
# Sync the panel (prebuilt assets included) and the contract
rsync -az --delete -e "ssh -i $KEY -o BatchMode=yes" \
  --exclude vendor --exclude node_modules --exclude .git --exclude 'storage/logs/*' \
  --exclude database/database.sqlite --exclude .env \
  "/Users/ismail/Documents/projek gila/hostnur/panel/" "$HOST:/opt/hostnur-panel/"
rsync -az -e "ssh -i $KEY -o BatchMode=yes" \
  "/Users/ismail/Documents/projek gila/hostnur/contracts/" "$HOST:/opt/contracts/"

ssh -i "$KEY" "$HOST" 'bash -s' <<'EOF'
set -e
cd /opt/hostnur-panel
export COMPOSER_ALLOW_SUPERUSER=1
composer install --no-dev --no-interaction --prefer-dist --no-progress
cat > .env <<'ENV'
APP_NAME=Hostnur
APP_ENV=production
APP_KEY=
APP_DEBUG=false
APP_URL=https://hostnur-p16.kom.my
LOG_CHANNEL=stack
DB_CONNECTION=sqlite
SESSION_DRIVER=database
QUEUE_CONNECTION=database
CACHE_STORE=database
HOSTNUR_AGENT_MODE=real
HOSTNUR_AGENT_SOCKET=/run/hostnur/agent.sock
HOSTNUR_CONTRACTS_PATH=/opt/contracts/v1
ENV
php artisan key:generate --force
touch database/database.sqlite
php artisan migrate --force
# Trust the local nginx reverse proxy so Laravel sees HTTPS
python3 - <<'PY'
p="/opt/hostnur-panel/bootstrap/app.php"; s=open(p).read()
if "trustProxies" not in s:
    s=s.replace("        ]);\n    })", "        ]);\n        $middleware->trustProxies(at: \"*\");\n    })",1)
    open(p,"w").write(s)
PY
php artisan view:clear; php artisan config:clear
# Test service (php artisan serve behind nginx). Test-only: runs as root.
cat > /etc/systemd/system/hostnur-panel.service <<'UNIT'
[Unit]
Description=Hostnur panel (temporary test)
After=network.target
[Service]
Type=simple
WorkingDirectory=/opt/hostnur-panel
ExecStart=/usr/bin/php artisan serve --host 127.0.0.1 --port 8080
Restart=on-failure
[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload; systemctl enable --now hostnur-panel
EOF
```

## 5. DNS + TLS for the panel

Point DNS A records (DNS-only, no Cloudflare proxy): `hostnur-p16.kom.my` and the
app hostname → droplet IP. Open ports 80/443 (UFW + any DO cloud firewall).

```bash
ssh -i "$KEY" "$HOST" 'bash -s' <<'EOF'
set -e
# Managed include dir for the agent-rendered app sites
mkdir -p /etc/nginx/hostnur-sites
echo "include /etc/nginx/hostnur-sites/*.conf;" > /etc/nginx/conf.d/hostnur-sites.conf
# Panel TLS vhost (proxies to artisan serve on 127.0.0.1:8080)
mkdir -p /var/www/hostnur-acme/.well-known/acme-challenge
cat > /etc/nginx/sites-available/hostnur-panel <<'NGINX'
server {
    listen 80;
    server_name hostnur-p16.kom.my;
    location ^~ /.well-known/acme-challenge/ { root /var/www/hostnur-acme; }
    location / { return 301 https://$host$request_uri; }
}
server {
    listen 443 ssl;
    server_name hostnur-p16.kom.my;
    ssl_certificate /etc/letsencrypt/live/hostnur-p16.kom.my/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/hostnur-p16.kom.my/privkey.pem;
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;
    }
}
NGINX
ln -sf /etc/nginx/sites-available/hostnur-panel /etc/nginx/sites-enabled/hostnur-panel
rm -f /etc/nginx/sites-enabled/default
# Temporary HTTP-only block so HTTP-01 can run before certs exist
cat > /etc/nginx/sites-available/hostnur-acme <<'NGINX'
server {
    listen 80;
    server_name hostnur-p16.kom.my;
    location ^~ /.well-known/acme-challenge/ { root /var/www/hostnur-acme; }
    location / { return 200 "ok\n"; }
}
NGINX
ln -sf /etc/nginx/sites-available/hostnur-acme /etc/nginx/sites-enabled/hostnur-acme
# Temporarily disable the TLS vhost until the cert exists
rm -f /etc/nginx/sites-enabled/hostnur-panel
nginx -t && systemctl reload nginx
# Issue the cert (staging first), then production
certbot certonly --webroot -w /var/www/hostnur-acme -d hostnur-p16.kom.my \
  --staging --agree-tos --non-interactive --email maelzx@duck.com
certbot certonly --webroot -w /var/www/hostnur-acme -d hostnur-p16.kom.my \
  --server https://acme-v02.api.letsencrypt.org/directory --force-renewal \
  --agree-tos --non-interactive --email maelzx@duck.com
# Enable the panel TLS vhost, drop the ACME-only block
rm -f /etc/nginx/sites-enabled/hostnur-acme
ln -sf /etc/nginx/sites-available/hostnur-panel /etc/nginx/sites-enabled/hostnur-panel
nginx -t && systemctl reload nginx
EOF
```

Renewal is handled automatically by certbot's systemd timer.

## 6. First administrator (interactive — needs a real TTY)

```bash
PASS="$(openssl rand -base64 30 | tr -dc 'A-Za-z0-9' | head -c 20)"
printf 'Hostnur Admin\nadmin@kom.my\n%s\n%s\n' "$PASS" "$PASS" \
  | ssh -tt -i "$KEY" "$HOST" 'cd /opt/hostnur-panel && php artisan hostnur:admin:bootstrap; echo EXIT=$?'
# -> email admin@kom.my, password $PASS (write it down)
```

Log in at `https://hostnur-p16.kom.my` (enroll MFA on first login).

## 7. Deploy an app (AssetKOM) end-to-end over the socket

Register the app's source/domain with the agent, then deploy, then poll
`agent.inspect`. In production this is driven by the panel's Deploy button (which
now calls `agent.register_application` before submitting). For a quick check,
`/root/probe.py` on the host does it directly:

```python
# /root/probe.py  (run as root; peer uid 0 is allowed)
import socket, struct, json, time, uuid
SOCK="/run/hostnur/agent.sock"; APP="aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"
def call(p):
    s=socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.connect(SOCK)
    b=json.dumps(p).encode(); s.sendall(struct.pack(">I",len(b))+b); s.settimeout(120)
    pre=s.recv(4); n=struct.unpack(">I",pre)[0]; buf=b""
    while len(buf)<n: buf+=s.recv(n-len(buf))
    return json.loads(buf.decode())
def req(op, params, res=None):
    r={"protocol_version":1,"request_id":str(uuid.uuid4()),"idempotency_key":str(uuid.uuid4()),"operation":op,"parameters":params}
    if res: r["resource"]=res
    return r
print(call(req("agent.register_application", {"application":APP,"repository_url":"https://github.com/maelzx/AssetKOM.git","server_name":"assetkom.hostnur-p16.kom.my","provider":"php-laravel","runtime":"php","runtime_version":"8.4"})))
r=call(req("application.deploy", {"revision":"99999999-9999-4999-8999-999999999999","ref":"main","run_migrations":True}, {"kind":"application","id":APP}))
ex=r["execution_id"]
for _ in range(180):
    s=call(req("agent.inspect", {"execution_id":ex})); st=s.get("status")
    print(st, (s.get("events") or [{}])[-1].get("message",""))
    if st in ("succeeded","failed","cancelled","timed_out"): break
    time.sleep(5)
```

The pipeline (as the app user): clone → `composer install` → `npm ci && npm run
build` → `.env` from `.env.example` → `key:generate` → SQLite → `migrate` →
`storage:link` → PHP-FPM pool (by runtime version) → Nginx route rooted at
`<app>/current/public` → atomic `current` switch → health check.

## 7b. Deploy through the panel itself (`hostnur:smoke`, session 052)

Prefer this to the socket probe in §7: it populates the panel DB and exercises
panel → task → preflight → agent exactly as the UI does (only login/MFA is
skipped). Run as the panel identity so the socket peer check matches:

```bash
cd /opt/hostnur-panel && sudo -u hostnur-panel php artisan hostnur:smoke \
  --repo=https://github.com/<owner>/<app>.git --ref=main \
  --hostname=smoke.hostnur-p16.kom.my --php=8.4 \
  --database --migrate --certificate --rollback --force
```

It prints each task's agent events, then a pass/FAIL table, and exits non-zero
on the first failure. `--rollback` deploys twice and rolls back to the first
release (checks agent release ids end to end); `--certificate` needs DNS for the
hostname. A queue worker running alongside is harmless (tasks are leased). The
created application stays in the panel for inspection; archive it afterwards.

## 8. Make AssetKOM actually usable (remaining)

Deploying gets the app serving, but to *log in and use it*:

1. **DNS**: A record for the app hostname → droplet IP (DNS-only).
2. **First admin**: AssetKOM has no default login; create one as the app user:
   `sudo -u hn_aaaaaaaabbbb /usr/bin/php8.4 <release>/artisan admin:create admin@yourorg.com`
   (service identity is `hn_<12 hex of app uuid>`; release dir under `/opt/hostnur/apps/<uuid>/current`).
3. **HTTPS** for the app hostname (the deployed site is HTTP-only until certs are issued for it).
4. **Production env + SMTP**: the seeded `.env` is AssetKOM's `APP_ENV=local`/`APP_DEBUG=true`;
   set `APP_ENV=production`, `APP_DEBUG=false`, `APP_URL=https://<app host>` and Mail settings
   (AssetKOM requires verified accounts).

## Gotchas learned

- On a freshly-created droplet, `apt` is briefly locked by first-boot
  `cloud-init`/unattended-upgrades; wait for `/var/lib/apt/lists/lock` to free
  before installing.
- `scp` over a running binary → `ETXTBSY`; stop the systemd unit first.
- The firewall probe uses `ufw`, a script that shells out to `iptables`/`sysctl`;
  the agent runs children with a cleared environment, so the probe must set `PATH`
  (already handled in `agent/src/firewall.rs`).
- `hostnur:admin:bootstrap` refuses non-interactive stdin; use `ssh -tt` and feed lines.
- The agent expects its managed Nginx include dir + the `conf.d` include (step 5).
- `bootstrap/app.php` needs `trustProxies('*')` when the panel is behind nginx (host-only patch).
- App asset URLs follow the request host, so browser rendering is fine; `APP_URL` still
  matters for CLI-generated URLs and emails.
- Deploys health-check `127.0.0.1:80` with the `Host` header, so DNS is **not** required
  for a deploy to succeed — only for browser access.
- Deleting the droplet removes everything; nothing on the dev machine needs cleanup
  except the Lima VM (kept) and the SSH key (kept at `~/.ssh/hostnur_p16_ed25519`).

## Appendix — session 047 panel/agent deploy (2026-10-16)

Panel domain `hostnur-p16.kom.my`, app domain `assetkom.hostnur-p16.kom.my` (both A →
droplet). Fast path used this session (beyond the base runbook):

- Install PHP 8.4 (`ppa:ondrej/php`) + Node 20 (nodesource) — Laravel builds need them.
- Agent `agent.toml`: `socket_group = "hostnur"`, `allowed_peer_uids = [0, <panel-uid>]`,
  `acme_email = "maelzx@duck.com"`, `backup_recipient`/`backup_dir` optional.
- **Dedicated panel identity** `hostnur-panel` (in group `hostnur`); FPM pool + worker/
  scheduler units run as it; chown `/opt/hostnur-panel` to it.
- **Critical**: the panel `.env` must include `APP_KEY=` (even empty) — `php artisan
  key:generate` only *replaces* an existing line. Easiest: set `APP_KEY=base64:$(openssl rand 32 | base64)`.
- Enable the managed-site include: `/etc/nginx/conf.d/hostnur-sites.conf` =
  `include /etc/nginx/hostnur-sites/*.conf;` (the installer/package do this; a hand-rolled
  deploy must too, or app vhosts won't load).
- Panel nginx: `:80` (ACME + 301 to the panel domain) + `:443` ssl; `/run/php/hostnur-panel.sock`.
- Worker + scheduler (as `hostnur-panel`): `hostnur-panel-worker` + `hostnur-panel-scheduler.timer`.
- Panel TLS: `php artisan hostnur:panel:setup-tls --domain hostnur-p16.kom.my --email …`.
- Deploy an app via the panel wizard, **or** (scripted validation) the agent socket:
  `agent.register_application` → `application.deploy {revision,ref,run_migrations,
  recovery_acknowledged}` → poll `agent.inspect`; DB via `agent.register_database` +
  `database.provision`; app TLS via `agent.register_certificate` + `certificate.issue`.
  Socket-driven deploys **bypass the panel DB** (reconcile or use the wizard).
