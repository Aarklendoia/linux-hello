#!/bin/sh
# Writes a Gentoo overlay holding sys-auth/linux-hello-<version>.ebuild,
# built from the linux-hello.ebuild template next to this script (#193).
#
# Usage: packaging/gentoo/make-overlay.sh OUTDIR
#
# Run it on a Gentoo system (or container) with network access: it needs
# cargo, curl, sha256sum, app-portage/pycargoebuild and ::gentoo's license
# mapping. It doesn't write the Manifest, which needs the distfiles: run
# `ebuild OUTDIR/sys-auth/linux-hello/*.ebuild manifest` afterwards. Set
# CRATE_DISTDIR to Portage's DISTDIR (`portageq distdir`) to keep the crates
# and the models this downloads there, so that step doesn't fetch them again.
#
# Cargo.lock isn't committed, so this resolves one from the committed tree
# (`git archive HEAD`, so untracked files and a local Cargo.lock are left
# out) and pins its crates in CRATES.
#
# The models pack is checked against the sha256 hello_face_core/build.rs
# pins before the Manifest can record it, so the overlay can't silently
# pick up a re-cut upstream asset.

set -eu

if [ $# -ne 1 ]; then
  echo "Usage: $0 OUTDIR" >&2
  exit 1
fi
OUTDIR="$1"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)"
if [ -z "$VERSION" ]; then
  echo "Could not read the workspace version from Cargo.toml" >&2
  exit 1
fi
MODEL_PACK_URL="https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip"
MODEL_PACK_SHA256="57d31b56b6ffa911c8a73cfc1707c73cab76efe7f13b675a05223bf42de47c72"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "==> Resolving Cargo.lock for linux-hello $VERSION"
mkdir "$WORK/src"
git -C "$ROOT" archive HEAD | tar -x -C "$WORK/src"
cargo generate-lockfile --manifest-path "$WORK/src/Cargo.toml"

CRATE_DISTDIR="${CRATE_DISTDIR:-$WORK/distdir}"
mkdir -p "$CRATE_DISTDIR"

echo "==> Fetching and checking the models pack"
MODELS_ZIP="$CRATE_DISTDIR/linux-hello-insightface-buffalo_sc-0.7.zip"
curl -fsSL -o "$MODELS_ZIP.tmp" "$MODEL_PACK_URL"
actual="$(sha256sum "$MODELS_ZIP.tmp" | cut -d' ' -f1)"
if [ "$actual" != "$MODEL_PACK_SHA256" ]; then
  rm -f "$MODELS_ZIP.tmp"
  echo "Models pack checksum mismatch (got $actual, expected $MODEL_PACK_SHA256)" >&2
  exit 1
fi
mv "$MODELS_ZIP.tmp" "$MODELS_ZIP"

PKGDIR="$OUTDIR/sys-auth/linux-hello"
EBUILD="$PKGDIR/linux-hello-$VERSION.ebuild"
mkdir -p "$PKGDIR" "$OUTDIR/profiles" "$OUTDIR/metadata" "$OUTDIR/licenses"
echo linux-hello > "$OUTDIR/profiles/repo_name"
# ::guru for sci-libs/onnxruntime, which ::gentoo doesn't package.
cat > "$OUTDIR/metadata/layout.conf" <<EOF
masters = gentoo guru
thin-manifests = true
sign-manifests = false
EOF
cp "$ROOT/packaging/gentoo/metadata.xml" "$PKGDIR/"
cp "$ROOT/packaging/gentoo/licenses/"* "$OUTDIR/licenses/"
cp "$ROOT/packaging/gentoo/linux-hello.ebuild" "$EBUILD"

echo "==> Filling CRATES and the crate licenses"
# pycargoebuild wants the workspace's members, not its root.
pycargoebuild --no-config --inplace "$EBUILD" --distdir "$CRATE_DISTDIR" \
  "$WORK/src/hello_camera" "$WORK/src/hello_face_core" "$WORK/src/hello_daemon" \
  "$WORK/src/pam_linux_hello" "$WORK/src/linux_hello_cli" "$WORK/src/linux_hello_config"

echo "==> Wrote $EBUILD"
