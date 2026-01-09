# Linux Hello - Development Environment Setup

## Quick Start

### Prerequisites
- Ubuntu 24.04 LTS or Debian Bookworm
- Rust 1.70+
- Cargo

### Build from Source

```bash
# Clone repository
git clone https://github.com/edouard-martinez/linux-hello.git
cd linux-hello

# Install system dependencies
sudo apt-get install -y \
  build-essential \
  debhelper \
  quilt \
  libssl-dev \
  libpam0g-dev \
  pkg-config \
  libkf6config-dev \
  libkf6coreaddons-dev \
  libkf6guiaddons-dev \
  qml6-module-qtbase

# Build Debian packages
dpkg-buildpackage -us -uc -b

# Install packages
sudo dpkg -i ../linux-hello*.deb
sudo apt-get install -yf
```

### Build with Cargo (Development)

```bash
# Build binaries
cargo build --release

# Run tests
cargo test --lib

# Lint code
cargo clippy -- -D warnings
cargo fmt --check
```

## CI/CD

The project uses GitHub Actions for continuous integration:

- **build.yml**: Builds Debian packages on every push/PR
- **test.yml**: Tests package installation and functionality
- **lint.yml**: Runs Rust clippy, tests, and Debian lint checks

### Releases

Tags matching `v*` will automatically create GitHub releases with:
- Built Debian packages
- Source tarball
- Checksums

```bash
git tag v1.0.0
git push origin v1.0.0
```

## Debian Package Format

The project uses **Debian source format 3.0 (quilt)** for:
- Better patch management
- Compatible with modern Debian/Ubuntu
- Support for source tarball distribution
- Integration with dpkg-source tools

### Patches

Patches are stored in `debian/patches/` and managed with quilt:

```bash
# Create a new patch
quilt new fix-something.patch
quilt edit <file>
quilt refresh

# Apply patches
quilt push -a

# List applied patches
quilt series
```

## Documentation

See the following for more information:
- [README.md](../README.md) - Project overview
- [QUICKSTART.md](../QUICKSTART.md) - Getting started
- [INTEGRATION_GUIDE.md](../INTEGRATION_GUIDE.md) - Integration with PAM/KDE
