#!/bin/sh
# Writes the two sources packaging/rpm/linux-hello.spec builds from into
# OUTDIR (#192), under fixed names since the OBS package fetches them from
# the latest GitHub release (packaging/rpm/_service):
#   - linux-hello-source.tar.gz: `git archive HEAD`, with a
#     linux-hello-<version>/ prefix;
#   - linux-hello-vendor.tar.xz: vendor/, .cargo/config.toml and Cargo.lock
#     from debian/scripts/prepare-offline-build.sh (the vendored crates), so
#     the build runs offline as OBS requires. The ONNX models aren't
#     included (#211).
#
# Usage: packaging/rpm/make-sources.sh OUTDIR
#
# Needs network access, git, rustup and jq (see
# prepare-offline-build.sh; RUST_TOOLCHAIN defaults to stable here, since
# openSUSE Tumbleweed and Fedora ship a recent cargo).

set -eu

if [ $# -ne 1 ]; then
  echo "Usage: $0 OUTDIR" >&2
  exit 1
fi
mkdir -p "$1"
OUTDIR="$(cd "$1" && pwd)"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)"
P="linux-hello-$VERSION"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "==> linux-hello-source.tar.gz ($P)"
git -C "$ROOT" archive --format=tar.gz --prefix="$P/" -o "$OUTDIR/linux-hello-source.tar.gz" HEAD

echo "==> linux-hello-vendor.tar.xz"
mkdir "$WORK/$P"
git -C "$ROOT" archive HEAD | tar -x -C "$WORK/$P"
(cd "$WORK/$P" && RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}" ./debian/scripts/prepare-offline-build.sh > /dev/null)
tar -C "$WORK/$P" -cJf "$OUTDIR/linux-hello-vendor.tar.xz" vendor .cargo Cargo.lock

ls -l "$OUTDIR/linux-hello-source.tar.gz" "$OUTDIR/linux-hello-vendor.tar.xz"
