#!/bin/bash
# Linux Hello Development Build Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_done() {
    echo -e "${GREEN}✓${NC} $1"
}

# Check for required tools
print_step "Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    echo "Error: cargo is not installed"
    echo "Install Rust from https://rustup.rs"
    exit 1
fi

if ! command -v dpkg-buildpackage &> /dev/null; then
    echo "Error: dpkg-buildpackage is not installed"
    echo "Run: sudo apt-get install build-essential debhelper"
    exit 1
fi

print_done "Prerequisites OK"

# Cargo checks
print_step "Running Rust checks..."
cargo fmt --check
print_done "Code formatting OK"

cargo clippy --all-targets --all-features -- -D warnings
print_done "Clippy checks passed"

cargo test --lib --quiet
print_done "Unit tests passed"

# Build Debian packages
print_step "Building Debian packages..."
dpkg-buildpackage -us -uc -b 2>&1 | tail -10

# Find generated packages
print_step "Generated packages:"
ls -lh ../*.deb 2>/dev/null | awk '{print $9, "(" $5 ")"}'

print_done "Build complete!"
echo ""
echo "Next steps:"
echo "  1. Install: sudo dpkg -i ../*.deb && sudo apt-get install -yf"
echo "  2. Test: systemctl --user status hello-daemon"
echo "  3. Run GUI: linux-hello-config"
