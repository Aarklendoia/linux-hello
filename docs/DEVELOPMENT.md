# Linux Hello Development

Quick guide to contributing to the project.

## ⚡ Quick Start

```bash
# Clone and setup
git clone https://github.com/Aarklendoia/linux-hello.git
cd linux-hello
make dev-setup

# Build and test
make build
make test
make lint
```

## 📋 Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Debian/Ubuntu
- Dependencies: `make dev-setup`

## 🏗️ Project Structure

```
linux-hello/
├── hello_daemon/          # Facial recognition service
├── hello_camera/          # Camera capture and processing
├── hello_face_core/       # Recognition algorithms
├── linux_hello_cli/       # Command-line interface
├── linux_hello_config/    # Configuration GUI (QML)
├── pam_linux_hello/       # PAM module
├── debian/                # Debian packaging
├── .github/
│   ├── workflows/         # GitHub Actions CI/CD
│   └── ISSUE_TEMPLATE/    # Issue templates
├── Makefile               # Dev commands
├── CONTRIBUTING.md        # Contribution guide
├── RELEASE.md             # Release process
└── CI_CD_INFRASTRUCTURE.md # CI/CD documentation
```

## 🚀 Main Commands

```bash
# Development
make build          # Build in debug
make release        # Build optimized
make test           # Run the tests
make check          # Quick check (no compile)
make fmt            # Format the code
make lint           # Lint with clippy
make audit          # Check for vulnerabilities

# Debian
make debian         # Build the packages
make deb-install    # Install the packages locally
make deb-clean      # Clean up Debian artifacts

# Documentation
make docs           # Generate and open the docs

# Debug
make daemon         # Run the daemon in debug
make camera-test    # Test the camera
```

## 🔍 Typical Workflow

```bash
# 1. Create a branch
git checkout -b feature/my-feature

# 2. Make changes
# Edit files...

# 3. Test
make test
make lint

# 4. Commit
git add -A
git commit -m "feat: Clear description"

# 5. Push and create a PR
git push origin feature/my-feature
# Create a PR on GitHub
```

## 📦 Debian Packaging

The project uses the **Debian 3.0 (quilt)** format.

### Generate the packages

```bash
make debian
# Packages in ../
ls ../*.deb
```

### Create a patch

```bash
# Create and apply a patch
quilt new fix-name.patch
quilt add debian/rules
# Edit the file...
quilt refresh

# List the patches
quilt series
```

## 🔄 Automatic CI/CD

The GitHub Actions workflows run automatically:

- **build-debian.yml**: Builds the packages
- **test.yml**: Runs the tests
- **quality.yml**: Linting and security
- **docs.yml**: Generates the documentation

See [CI_CD_INFRASTRUCTURE.md](CI_CD_INFRASTRUCTURE.md) for more details.

## 📝 Code Conventions

### Rust

```rust
// Doc comments for public APIs
/// Brief description.
///
/// Longer explanation if needed.
pub fn my_function() {}

// Format with rustfmt
cargo fmt --all

// Lint with clippy
cargo clippy --all -- -D warnings
```

### Commits

Format: `<type>: <description>`

Types:
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `style:` Formatting
- `refactor:` Refactoring
- `perf:` Performance
- `test:` Tests
- `chore:` Maintenance

Example:
```bash
git commit -m "feat: Add face enrollment API"
```

## 🧪 Tests

```bash
# All tests
cargo test --all

# Specific test
cargo test --lib my_test

# With output
cargo test -- --nocapture

# Benchmark
cargo bench --all
```

## 📚 Documentation

The Rust documentation is generated automatically:

```bash
# Generate and open
make docs

# Read a specific crate
cargo doc --open --document-private-items
```

## 🐛 Debugging

```bash
# Build with symbols
RUSTFLAGS="-g" cargo build

# Run under a debugger
rust-gdb ./target/debug/hello-daemon

# Run with detailed logs
RUST_LOG=debug ./target/debug/hello-daemon
```

## 🔐 Security

```bash
# Check for vulnerable dependencies
cargo audit

# Update dependencies
cargo update

# Outdated
cargo outdated
```

## 📖 More Info

- [CONTRIBUTING.md](CONTRIBUTING.md) - Full contribution guide
- [RELEASE.md](RELEASE.md) - Release process
- [CI_CD_INFRASTRUCTURE.md](CI_CD_INFRASTRUCTURE.md) - CI/CD documentation
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) - Community rules

## ❓ Questions?

- Open a discussion on GitHub
- Create an issue
- Check the documentation

Thank you for contributing! 🎉
