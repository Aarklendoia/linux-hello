# Contributing to Linux Hello

Thank you for contributing to Linux Hello! Here's how to proceed.

## Development Setup

### Prerequisites

- Rust 1.70+ (stable)
- Debian/Ubuntu with build-essential
- libssl-dev, libpam0g-dev, pkg-config
- Qt 6 development libraries
- Kirigami design system

### Setting Up the Environment

```bash
# Clone the repository
git clone https://github.com/Aarklendoia/linux-hello.git
cd linux-hello

# Install system dependencies
sudo apt-get install build-essential libssl-dev libpam0g-dev pkg-config \
  qt6-base-dev qml6-module-qtcore libkf6kirigami-dev

# Build the project
cargo build --release
```

## Contribution Process

### 1. Create a branch

```bash
git checkout -b feature/my-feature
# or
git checkout -b fix/my-fix
```

### 2. Make your changes

- Follow the code style (rustfmt)
- Write tests for new features
- Update the documentation

### 3. Test your code

```bash
# Tests
cargo test --all

# Linting
cargo clippy --all -- -D warnings

# Formatting
cargo fmt --all

# Dependency audit
cargo audit
```

### 4. Commit with clear messages

```bash
git commit -m "feat: Add authentication support"
```

Commit message format:

- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation
- `style:` for style changes
- `refactor:` for refactoring
- `perf:` for performance optimizations
- `test:` for tests
- `chore:` for maintenance tasks

### 5. Push and create a Pull Request

```bash
git push origin feature/my-feature
```

Then create a PR on GitHub. The CI/CD will automatically check:

- Tests pass
- Code is properly formatted
- Linters pass
- No vulnerabilities are present

## Debian Format

The project uses the Debian 3.0 (quilt) format for Debian packages.

### Structure

```
debian/
├── source/
│   └── format (3.0 quilt)
├── patches/ (if needed)
├── rules
├── control
├── postinst
└── ...
```

### Adding a patch

```bash
quilt new my-fix.patch
quilt add file-to-modify
# Modify the file
quilt refresh
```

## Debian Packages

To build locally:

```bash
cd linux-hello
dpkg-buildpackage -us -uc -b
```

The generated packages will be in the parent directory.

## Documentation

The documentation is generated with cargo-doc and available at:

- README.md - Introduction
- docs/QUICKSTART.md - Quick start guide
- docs/INTEGRATION_GUIDE.md - Integration into systems
- docs/PAM_MODULE.md - PAM module documentation

## Bug Reports

Create a GitHub issue with:

- Linux Hello version (`linux-hello --version`)
- Operating system and version
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs

## Code Conventions

### Rust

- Use `rustfmt` for formatting
- Follow clippy rules
- Write doc comments for public APIs
- Name variables explicitly

### QML

- Indentation: 4 spaces
- Name IDs in camelCase
- Group related properties

## License

By contributing, you agree that your code will be published under the same license as the project.

## Questions?

- Open a discussion on GitHub
- Create an issue for bugs
- Contact the maintenance team

Thank you for contributing!
