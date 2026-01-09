# GitHub Actions CI/CD Workflows

This directory contains the CI/CD pipeline configuration for the Linux Hello project.

## Workflows

### build.yml
Builds Debian packages on every push and pull request.

**Triggers:**
- Push to `main` or `develop`
- Pull requests to `main` or `develop`
- Tags matching `v*` pattern

**Actions:**
1. Installs Debian build dependencies
2. Runs `dpkg-buildpackage` to create .deb packages
3. Uploads artifacts for 30 days
4. Creates GitHub release with packages when tagged

**Artifacts:**
- `debian-packages/`: Contains all built .deb files

### test.yml
Tests package installation and verifies functionality.

**Triggers:**
- Push to `main` or `develop`
- Pull requests to `main` or `develop`

**Tests:**
1. Builds packages
2. Installs all packages with `dpkg`
3. Verifies binaries are present
4. Checks icon and .desktop files are installed
5. Verifies daemon service is available

### lint.yml
Runs code quality checks and linting.

**Triggers:**
- Push to `main` or `develop`
- Pull requests to `main` or `develop`

**Checks:**
- Rust: `cargo clippy`, `cargo test`, `cargo fmt`
- Shell: `shellcheck` on scripts
- Debian: Control file syntax, changelog format

## Release Process

To create a release:

```bash
# Update version in debian/changelog
dch -i "New release"

# Commit changes
git add debian/changelog
git commit -m "Release: v1.0.0"

# Create tag
git tag v1.0.0

# Push
git push origin main --tags
```

The CI/CD pipeline will automatically:
1. Build packages
2. Run tests
3. Create GitHub release
4. Attach built packages

## Local Development

For local testing without triggering CI:

```bash
# Run the dev-build script
./scripts/dev-build.sh

# Or manually
cargo test --lib
cargo clippy -- -D warnings
cargo fmt --check
dpkg-buildpackage -us -uc -b
```

## Dependencies

The workflows use:
- `actions/checkout@v4`: Repository checkout
- `actions/upload-artifact@v4`: Artifact upload
- `actions/cache@v3`: Build caching
- `softprops/action-gh-release@v1`: Release creation
- `dtolnay/rust-toolchain@stable`: Rust toolchain

All dependencies are pinned to specific versions for security and reproducibility.
