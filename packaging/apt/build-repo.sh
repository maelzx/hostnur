#!/usr/bin/env bash
#
# Build a flat, optionally-signed APT repository from the built .deb packages.
# Run on Debian/Ubuntu (needs dpkg-scanpackages or apt-ftparchive, gzip).
#
# Usage:
#   packaging/apt/build-repo.sh --debs dist --out public [--key KEYID] \
#       [--origin Hostnur] [--label Hostnur] [--codename stable]
#
# With --key, Release is signed (Release.gpg + InRelease) using the given GPG
# key id; without it the repository is built unsigned (with a warning) so the CI
# can validate assembly before signing keys are configured.
set -euo pipefail

DEBS="dist"
OUT="public"
KEY=""
ORIGIN="Hostnur"
LABEL="Hostnur"
CODENAME="stable"
COMPONENT="main"
ARCH="amd64"

while [ $# -gt 0 ]; do
    case "$1" in
        --debs) DEBS="${2:-}"; shift 2 ;;
        --out) OUT="${2:-}"; shift 2 ;;
        --key) KEY="${2:-}"; shift 2 ;;
        --origin) ORIGIN="${2:-}"; shift 2 ;;
        --label) LABEL="${2:-}"; shift 2 ;;
        --codename) CODENAME="${2:-}"; shift 2 ;;
        -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
done

[ -d "$DEBS" ] || { echo "debs dir not found: $DEBS" >&2; exit 1; }

POOL="$OUT/pool/$COMPONENT"
DIST="$OUT/dists/$CODENAME/$COMPONENT"
mkdir -p "$POOL" "$DIST/binary-$ARCH"

cp "$DEBS"/*.deb "$POOL/"

gen_index() {
    if command -v apt-ftparchive >/dev/null 2>&1; then
        ( cd "$OUT" && apt-ftparchive packages "pool/$COMPONENT" > "dists/$CODENAME/$COMPONENT/binary-$ARCH/Packages" )
    elif command -v dpkg-scanpackages >/dev/null 2>&1; then
        ( cd "$OUT" && dpkg-scanpackages "pool/$COMPONENT" /dev/null > "dists/$CODENAME/$COMPONENT/binary-$ARCH/Packages" )
    else
        echo "need apt-ftparchive or dpkg-scanpackages" >&2
        exit 1
    fi
    gzip -kf "$DIST/binary-$ARCH/Packages"
}

gen_release() {
    if command -v apt-ftparchive >/dev/null 2>&1; then
        ( cd "$OUT" && apt-ftparchive \
            -o "APT::FTPArchive::Release::Origin=$ORIGIN" \
            -o "APT::FTPArchive::Release::Label=$LABEL" \
            -o "APT::FTPArchive::Release::Suite=$CODENAME" \
            -o "APT::FTPArchive::Release::Codename=$CODENAME" \
            -o "APT::FTPArchive::Release::Architectures=$ARCH" \
            -o "APT::FTPArchive::Release::Components=$COMPONENT" \
            release "dists/$CODENAME" > "dists/$CODENAME/Release" )
    else
        {
            printf 'Origin: %s\nLabel: %s\nSuite: %s\nCodename: %s\nArchitectures: %s\nComponents: %s\n' \
                "$ORIGIN" "$LABEL" "$CODENAME" "$CODENAME" "$ARCH" "$COMPONENT"
            ( cd "$DIST" && printf 'MD5Sum:\n' && find binary-$ARCH -type f -printf ' %s %p\n' )
        } > "$OUT/dists/$CODENAME/Release"
    fi
}

gen_index
gen_release

if [ -n "$KEY" ]; then
    gpg --batch --yes --armor --detach-sign --local-user "$KEY" -o "$OUT/dists/$CODENAME/Release.gpg" "$OUT/dists/$CODENAME/Release"
    gpg --batch --yes --clearsign --local-user "$KEY" -o "$OUT/dists/$CODENAME/InRelease" "$OUT/dists/$CODENAME/Release"
    echo "Signed repository with key $KEY"
else
    echo "WARNING: no --key given; repository is UNSIGNED and not safe to publish" >&2
fi

echo "Repository assembled at $OUT"
