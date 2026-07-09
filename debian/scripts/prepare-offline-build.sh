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
# See docs/LAUNCHPAD.md for how this fits into the release process.

set -eu

cd "$(dirname "$0")/../.."
REPO_ROOT="$(pwd)"

echo "==> Vendoring Cargo dependencies into vendor/"
rm -rf vendor .cargo
cargo vendor vendor > /tmp/cargo-vendor-config.toml.tmp
mkdir -p .cargo
cat /tmp/cargo-vendor-config.toml.tmp > .cargo/config.toml
rm -f /tmp/cargo-vendor-config.toml.tmp
echo "    $(du -sh vendor | cut -f1) in vendor/"

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
