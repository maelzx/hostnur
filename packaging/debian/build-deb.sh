#!/usr/bin/env bash
#
# Build the hostnur-agent and hostnur Debian packages from a staged release
# bundle. Run on Linux (needs dpkg-deb/coreutils `install`).
#
# Usage:
#   packaging/debian/build-deb.sh --version 0.1.0 --bundle /path/to/stage/hostnur [--out dist]
#
# The bundle is the `hostnur/` tree produced by the release workflow:
#   bin/hostnur-agent   compiled agent binary
#   panel/              panel with vendor + built assets
#   contracts/          versioned protocol schemas/catalog/fixtures
#
# Signed Debian/APT publishing and real-host validation are handled separately
# (see packaging/apt and .github/workflows/packages.yml).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION=""
BUNDLE=""
OUT="dist"

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="${2:-}"; shift 2 ;;
        --bundle) BUNDLE="${2:-}"; shift 2 ;;
        --out) OUT="${2:-}"; shift 2 ;;
        -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
done

[ -n "$VERSION" ] || { echo "--version is required" >&2; exit 1; }
[ -d "$BUNDLE" ] || { echo "bundle not found: $BUNDLE" >&2; exit 1; }
[ -x "$BUNDLE/bin/hostnur-agent" ] || { echo "bundle is missing bin/hostnur-agent" >&2; exit 1; }

DEB_DIR="$REPO_ROOT/packaging/debian"
SYSTEMD_DIR="$REPO_ROOT/packaging/systemd"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT"

render_control() {
    sed "s/__VERSION__/${VERSION}/g" "$1" > "$2"
}

copy_maintainer_scripts() {
    local src="$1" dst="$2"
    cp "$src/conffiles" "$dst/conffiles"
    for script in postinst prerm postrm; do
        install -m 0755 "$src/$script" "$dst/$script"
    done
}

build_agent() {
    local root="$WORK/hostnur-agent"
    install -D -m 0755 "$BUNDLE/bin/hostnur-agent" "$root/usr/local/bin/hostnur-agent"
    install -d -m 0755 "$root/opt/hostnur/apps"
    install -D -m 0644 "$SYSTEMD_DIR/hostnur-agent.service" "$root/lib/systemd/system/hostnur-agent.service"
    install -D -m 0644 "$DEB_DIR/hostnur-agent/agent.toml" "$root/etc/hostnur/agent.toml"
    mkdir -p "$root/DEBIAN"
    render_control "$DEB_DIR/hostnur-agent/DEBIAN/control" "$root/DEBIAN/control"
    copy_maintainer_scripts "$DEB_DIR/hostnur-agent/DEBIAN" "$root/DEBIAN"
    dpkg-deb --build --root-owner-group "$root" "$OUT/hostnur-agent_${VERSION}_amd64.deb"
}

build_panel() {
    local root="$WORK/hostnur"
    mkdir -p "$root/opt/hostnur"
    cp -a "$BUNDLE/panel" "$root/opt/hostnur-panel"
    cp -a "$BUNDLE/contracts" "$root/opt/hostnur/contracts"
    install -D -m 0644 "$SYSTEMD_DIR/hostnur-panel-worker.service" "$root/lib/systemd/system/hostnur-panel-worker.service"
    install -D -m 0644 "$SYSTEMD_DIR/hostnur-panel-scheduler.service" "$root/lib/systemd/system/hostnur-panel-scheduler.service"
    install -D -m 0644 "$SYSTEMD_DIR/hostnur-panel-scheduler.timer" "$root/lib/systemd/system/hostnur-panel-scheduler.timer"
    install -D -m 0644 "$DEB_DIR/hostnur/hostnur-panel.nginx" "$root/etc/nginx/sites-available/hostnur-panel"
    install -D -m 0644 "$DEB_DIR/hostnur/hostnur-sites.conf" "$root/etc/nginx/conf.d/hostnur-sites.conf"
    install -D -m 0644 "$REPO_ROOT/packaging/hostnur-panel.logrotate" "$root/etc/logrotate.d/hostnur-panel"
    install -D -m 0644 "$DEB_DIR/hostnur/hostnur-panel.fpm.conf" "$root/etc/php/8.3/fpm/pool.d/hostnur-panel.conf"
    install -D -m 0644 "$DEB_DIR/hostnur/panel-tls.conf.example" "$root/etc/hostnur/panel-tls.conf.example"
    mkdir -p "$root/DEBIAN"
    render_control "$DEB_DIR/hostnur/DEBIAN/control" "$root/DEBIAN/control"
    copy_maintainer_scripts "$DEB_DIR/hostnur/DEBIAN" "$root/DEBIAN"
    dpkg-deb --build --root-owner-group "$root" "$OUT/hostnur_${VERSION}_amd64.deb"
}

build_agent
build_panel

echo "Built packages:"
ls -l "$OUT"/*.deb
