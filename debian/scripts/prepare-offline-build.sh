#!/bin/sh
# Run this locally, with network access, before building a Launchpad source
# upload (debuild -S -sa). NEVER run as part of debian/rules — Launchpad's
# build farm has no general internet access, so everything this script
# fetches must already be sitting in the working directory by the time
# dpkg-source tars it up.
#
# It does two things dpkg-buildpackage would otherwise do over the network:
#   1. cargo vendor: pulls every crates.io/git dependency into vendor/, and
#      writes .cargo/config.toml to point cargo at it instead of the registry.
#   2. Pre-fetches the ONNX models into the same XDG path build.rs already
#      checks first (hello_face_core/build.rs's `present()` check) — so the
#      offline build finds them already there and never tries to download.
#
# IMPORTANT: vendor with the SAME cargo version the target Ubuntu series
# ships (check with `rmadison -u ubuntu cargo`), not whatever's locally
# "stable". A newer cargo vendoring the tree can silently omit
# Cargo.toml.orig companion files that an OLDER cargo needs at build time
# to verify a vendored crate's checksum against Cargo.lock — cargo then
# fails offline with "failed to calculate checksum of: vendor/<crate>/
# Cargo.toml.orig: No such file or directory". Confirmed by building
# linux-hello 1.1.0 for resolute (26.04, cargo 1.93.1): vendoring with a
# local 1.96.1 produced zero .orig files; re-vendoring with
# `rustup toolchain install 1.93.1` first fixed it (431 .orig files, all
# needed). Set RUST_TOOLCHAIN to the version to vendor with; defaults to
# "stable", which is very likely wrong for an older LTS target — pass the
# real one explicitly:
#   RUST_TOOLCHAIN=1.93.1 ./debian/scripts/prepare-offline-build.sh
#
# See docs/LAUNCHPAD.md for how this fits into the release process.

set -eu

RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

cd "$(dirname "$0")/../.."
REPO_ROOT="$(pwd)"

echo "==> Removing target/ (must not exist when dpkg-source tars up the tree)"
# debian/source/options deliberately doesn't use --tar-ignore for this: it
# matches "target" as a path component ANYWHERE, which also strips
# vendor/cc/src/target/ (a real subdirectory of the vendored `cc` crate,
# not a build artifact) — confirmed the hard way against a real Launchpad
# build. Physically removing target/ is the only reliable option.
rm -rf target

echo "==> Vendoring Cargo dependencies into vendor/ (toolchain: $RUST_TOOLCHAIN)"
if [ "$RUST_TOOLCHAIN" = "stable" ]; then
  echo "    WARNING: no RUST_TOOLCHAIN given, using 'stable' — check the target" >&2
  echo "    series' cargo version first (rmadison -u ubuntu cargo) and pass" >&2
  echo "    RUST_TOOLCHAIN=<version> explicitly if it differs." >&2
fi
rustup toolchain install "$RUST_TOOLCHAIN" > /dev/null 2>&1 || true
rm -rf vendor .cargo
cargo "+$RUST_TOOLCHAIN" vendor vendor > /tmp/cargo-vendor-config.toml.tmp
mkdir -p .cargo
cat /tmp/cargo-vendor-config.toml.tmp > .cargo/config.toml
rm -f /tmp/cargo-vendor-config.toml.tmp
echo "    $(du -sh vendor | cut -f1) in vendor/, $(find vendor -name '*.orig' | wc -l) .orig files"

echo "==> Disabling cargo's per-file checksum verification for vendored crates"
# dpkg-source's native-tarball builder has a hardcoded exclude list (VCS
# control files, backup/swap files — .git, .gitignore, .svn, CVS, *.orig,
# DEADJOE, ...) that cannot be turned off via debian/source/options. Some
# vendored crate, somewhere, will always have a test fixture or metadata
# file that happens to match one of those generic names (hit this for
# real with vendor/unicode-ident/tests/fst/.gitignore) — dpkg-source
# silently drops it from the tarball, and cargo's offline build then
# fails verifying that crate's per-file checksums against
# .cargo-checksum.json. This is a known, standard conflict between
# dpkg-source and `cargo vendor`; the documented Debian Rust-packaging
# fix is to blank out each vendored crate's "files" checksum map so
# cargo only trusts the vendor directory as-is instead of re-verifying
# every individual file (the "package" checksum, verified against
# Cargo.lock, is untouched).
find vendor -maxdepth 2 -name ".cargo-checksum.json" |
  while IFS= read -r f; do
    jq '.files = {}' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
  done

echo "==> Pre-fetching ONNX models"
MODELS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/linux-hello/models"
mkdir -p "$MODELS_DIR"
MODEL_PACK_URL="https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip"
ZIP_PATH="$(mktemp -t linux-hello-buffalo_sc-XXXXXX.zip)"

if [ -f "$MODELS_DIR/det_500m.onnx" ] && [ -f "$MODELS_DIR/w600k_mbf.onnx" ]; then
  echo "    already present in $MODELS_DIR, skipping download"
else
  curl -L --silent --max-time 60 --output "$ZIP_PATH" "$MODEL_PACK_URL"
  unzip -o -j "$ZIP_PATH" det_500m.onnx w600k_mbf.onnx -d "$MODELS_DIR"
  rm -f "$ZIP_PATH"
  echo "    fetched into $MODELS_DIR"
fi

cat <<EOF

Ready. From this same working directory (with vendor/, .cargo/config.toml,
and $MODELS_DIR populated), debian/rules will now build with
'cargo build --offline'. Proceed with the dch / debuild -S -sa / dput cycle
from docs/LAUNCHPAD.md.

Nothing here is meant to be committed to git — vendor/ and .cargo/ are
regenerated per release right before packaging (see .gitignore).
EOF
