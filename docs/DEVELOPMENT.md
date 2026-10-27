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

```text
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
- **quality.yml**: Linting, security, and code coverage
- **fuzz.yml**: 60s fuzz smoke run on `hello_face_core`'s buffer-parsing code (see [Fuzzing](#-fuzzing) below)
- **docs.yml**: Generates the documentation

See [CI_CD_INFRASTRUCTURE.md](CI_CD_INFRASTRUCTURE.md) for more details.

## 🐛 Fuzzing

`hello_face_core/fuzz/` has cargo-fuzz targets for `rgb_liveness_score` and
`ir_liveness_score` — buffer/bounding-box-indexing code that has already had
real out-of-bounds-read bugs found and fixed (a truncated/mismatched camera
capture buffer). CI (`fuzz.yml`) runs each target for a 60s smoke test on
every PR touching `hello_face_core`; for a real fuzzing session, run for
longer locally:

```bash
rustup install nightly
cargo install cargo-fuzz
cd hello_face_core
cargo +nightly fuzz run rgb_liveness_score -- -max_total_time=300
cargo +nightly fuzz run ir_liveness_score -- -max_total_time=300
```

The fuzz targets build with `default-features = false` on `hello_face_core`
(no ONNX/tract), since the liveness module is plain byte-buffer math — no
model files or system ONNX Runtime needed to run them.

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
```

## ⏱️ Benchmarks

`hello_face_core/benches/pipeline.rs` times each stage a verification runs on
every camera frame, with the real models (it skips if they aren't installed),
and prints the peak memory of the process:

```bash
cargo bench -p hello_face_core
# Same, on a real photo (resized to 640x480), to also time a real detection:
LINUX_HELLO_BENCH_FRAME=face.jpg cargo bench -p hello_face_core
```

Reference, AMD Ryzen 7 5800X (CPU inference, synthetic frame):

```
stage                              min    median       p95       max
detection (SCRFD-500M)        15.67 ms  15.92 ms  16.16 ms  16.25 ms
embedding (ArcFace MBF)       25.21 ms  25.52 ms  26.54 ms  26.66 ms
IR liveness score              1.11 ms   1.13 ms   1.13 ms   1.17 ms
RGB liveness score             1.14 ms   1.15 ms   1.16 ms   1.22 ms

peak RSS: 109 MiB
```

A frame with a face costs detection + embedding + RGB liveness (~43 ms here).
A verification needs several consecutive matching frames, so the camera's frame
rate and its time to first frame usually dominate. Comparing machines or camera
setups is easier with this output pasted in the issue. Matching against stored
embeddings (a 512-float dot product each) is negligible next to these numbers.

### On a real camera

Every verification also logs where its time went, at `info` level
(`journalctl --user -u hello-daemon`, or `hello-daemon-system` for SDDM):

```
Loaded 2 stored embeddings in 0 ms
Capture timings: first frame 2664 ms, 3 frames (3 with a face), detection 21.0 ms/frame, embedding 27.2 ms/face, total 2919 ms, peak RSS 132 MiB
Face recognized after 2 consecutive matching frames (score=0.807) in 2919 ms
```

`Loaded … in` includes unsealing the embedding key from the TPM, which can take
seconds the first time. `first frame` runs from the start of the capture (camera
lock taken) to the first RGB frame, so it covers opening and starting the
camera. The lines above come from a Logitech Brio on the same machine. There,
the first frame accounts for 90 % of the verification: the RGB stream alone
delivers it in about 1.2 s, and starting the IR stream 300 ms later pushes it
to about 2.8 s.

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
