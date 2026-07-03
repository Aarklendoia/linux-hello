# Useful Commands - Linux Hello Project

## 🏗️ Build & Compilation

### Release Build (Optimized)

```bash
cargo build --release
```

**Result**: Optimized binaries in `target/release/`
**Time**: ~52 seconds
**Binaries**:

- `hello-daemon` - PAM and D-Bus daemon
- `linux-hello` - CLI tool
- `linux-hello-config` - GUI (Iced/Wayland)
- `libpam_linux_hello.so` - PAM module

### Debug Build (Development)

```bash
cargo build
```

**Result**: Binaries with debug symbols
**Time**: ~1-2 minutes
**Advantage**: Faster to compile, better debugging

### Quick Check (No Linking)

```bash
cargo check
cargo check --release
```

**Time**: <1 second
**Use**: Check for errors quickly without compiling

---

## 🧪 Tests

### All Tests

```bash
cargo test --release
# Result: 35 tests, all ✅
```

### Tests for a Specific Crate

```bash
cargo test --release -p hello_daemon
cargo test --release -p linux_hello_config
cargo test --release -p hello_face_core
```

### A Single Test

```bash
cargo test --release preview::tests::test_get_display_data_with_frame
cargo test --release camera::tests::test_start_capture_stream
```

### With Output (Not Captured)

```bash
cargo test --release -- --nocapture
```

### Code Coverage (Llvm-cov)

```bash
cargo tarpaulin --release
```

---

## 📦 Installation

### Local Binaries

```bash
# Release build
cargo build --release

# In target/release/
./target/release/hello-daemon    # Start the daemon
./target/release/linux-hello     # CLI client
./target/release/linux-hello-config  # GUI
```

### Debian Package (Phase B)

```bash
# Generated in debian/
dpkg -i libpam-linux-hello_*.deb
dpkg -i linux-hello_*.deb
dpkg -i linux-hello-daemon_*.deb
dpkg -i linux-hello-tools_*.deb
```

---

## 🚀 Running

### Daemon

```bash
# Terminal 1 - Start the daemon
./target/release/hello-daemon

# With debug logs
RUST_LOG=debug ./target/release/hello-daemon
```

### GUI (KDE/Wayland)

```bash
# Terminal 2 - Start the GUI
./target/release/linux-hello-config
```

### CLI Client

```bash
# Terminal 2 - Test via D-Bus
./target/release/linux-hello \
  --user testuser \
  --timeout 5000 \
  start-capture
```

---

## 📊 Benchmarks

### Performance

```bash
# Build release
time cargo build --release
# → ~52 seconds

# Run tests
time cargo test --release
# → ~2-3 minutes total

# Check only
time cargo check
# → <1 second

# Test specific
time cargo test --release camera::
# → ~0.2 seconds
```

---

## 🔍 Debugging

### View Detailed Warnings

```bash
cargo check 2>&1 | grep "warning:"
```

### View Detailed Errors

```bash
cargo build 2>&1 | grep "error:"
cargo check 2>&1 | grep "error:"
```

### Clippy (Advanced Lint)

```bash
cargo clippy --release
cargo clippy --fix
```

### Document & Open Docs

```bash
cargo doc --open
```

### View Dependencies

```bash
cargo tree
cargo outdated
```

---

## 📝 Documentation

### Generate the Docs

```bash
cargo doc --release --no-deps
```

### Generate and Open

```bash
cargo doc --open
```

### View the Doctests

```bash
cargo test --doc
```

---

## 🧹 Cleanup

### Remove the Artifacts

```bash
cargo clean
```

### Remove the Logs

```bash
rm -rf target/
```

### Remove the Built Packages

```bash
rm -rf debian/linux-hello*/
rm -rf debian/libpam*/
```

---

## 📋 Useful Daily Commands

### Quick Check & Test (Development)

```bash
# Less than 5 seconds
cargo check && cargo test --lib
```

### Full Build & Test

```bash
# About 55 seconds
cargo build --release && cargo test --release
```

### Fix Compiler Warnings

```bash
cargo fix --allow-dirty
cargo fix --allow-dirty --release
```

### Update Dependencies

```bash
cargo update
cargo outdated
```

---

## 🐛 Advanced Debugging

### With GDB

```bash
rust-gdb ./target/release/hello-daemon
# In gdb:
# (gdb) b main
# (gdb) run
# (gdb) n
```

### With LLDB

```bash
lldb ./target/release/hello-daemon
# In lldb:
# (lldb) b main
# (lldb) r
# (lldb) n
```

### With Valgrind (Memory)

```bash
valgrind --leak-check=full ./target/release/hello-daemon
```

### Trace System Calls

```bash
strace ./target/release/hello-daemon
```

---

## 📦 Distribution

### Build Debian Package

```bash
# See Makefile
make build-debian

# Or manually
dpkg-deb --build debian/linux-hello debian/
```

### Check Package Contents

```bash
dpkg -c libpam-linux-hello_*.deb
dpkg -c linux-hello_*.deb
```

### Install from Debian

```bash
sudo dpkg -i *.deb
sudo apt-get install -f  # Fix dependencies
```

---

## 🔧 Configuration

### Debug Logging

```bash
RUST_LOG=debug cargo run --release
RUST_LOG=info,hello_daemon=debug cargo build
```

### Features Flag

```bash
# Build with optional features
cargo build --release --features "feature1,feature2"
```

---

## 📊 Code Statistics

### Count Lines of Code

```bash
# Rust only
find . -name "*.rs" -type f | xargs wc -l | tail -1

# Without dependencies
find . -path ./target -prune -o -name "*.rs" -type f -print | xargs wc -l
```

### View TODO Comments

```bash
grep -r "TODO\|FIXME\|XXX\|HACK" --include="*.rs" .
```

### Cyclomatic Complexity

```bash
cargo install cargo-cyclomatic
cargo cyclomatic
```

---

## 🎯 Git Workflow

### View Changes

```bash
git status
git diff
```

### Commit

```bash
git add .
git commit -m "Phase 3.3: Preview rendering implementation"
git push
```

### Tags

```bash
git tag -a v0.3.3 -m "Phase 3.3 Complete"
git push origin v0.3.3
```

---

## 🚨 Troubleshooting

### The project doesn't compile

```bash
# 1. Clean completely
cargo clean

# 2. Check dependencies
cargo update

# 3. Rebuild
cargo build --release
```

### Tests fail

```bash
# 1. Run a specific test
cargo test test_name -- --nocapture

# 2. View the logs
RUST_LOG=debug cargo test test_name

# 3. Check memory
valgrind --leak-check=full cargo test
```

### Stale build artifacts

```bash
# Clean the incrementals
cargo clean
cargo build --release

# Or just the problematic crate
cargo clean -p hello_daemon
cargo build -p hello_daemon --release
```

---

## 📚 Useful Resources

### Local Documentation

```bash
# Open the project docs
cargo doc --open

# Dependency docs
# https://docs.rs/ (web)
```

### Crates.io

- <https://crates.io/crates/iced> - GUI framework
- <https://crates.io/crates/zbus> - D-Bus bindings
- <https://crates.io/crates/v4l> - V4L2 camera
- <https://crates.io/crates/tokio> - Async runtime

---

## ⚙️ System Configuration (Linux)

### Install build dependencies

```bash
# Ubuntu/Debian
sudo apt-get install \
  build-essential \
  libssl-dev \
  pkg-config \
  libv4l-dev \
  libpam0g-dev \
  libwayland-dev

# Fedora
sudo dnf install \
  gcc \
  openssl-devel \
  pkg-config \
  libv4l-devel \
  pam-devel \
  wayland-devel
```

### Camera Permissions

```bash
# Add the user to the video group
sudo usermod -a -G video $USER

# Restart the session or:
newgrp video
```

### Permission for PAM

```bash
# Give the appropriate permissions to the PAM module
sudo chown root:root /usr/lib/x86_64-linux-gnu/libpam_linux_hello.so
sudo chmod 755 /usr/lib/x86_64-linux-gnu/libpam_linux_hello.so
```

---

## 📖 Commands by Scenario

### "I just want to check that everything compiles"

```bash
cargo check --release
```

### "I want to check all the tests"

```bash
cargo test --release
```

### "I want to build and run"

```bash
cargo build --release
./target/release/linux-hello-config
```

### "I want to create a Debian package"

```bash
make build-debian
# Or see debian/rules for more control
```

### "I want to debug a test"

```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### "I want to see if the code stays clean"

```bash
cargo fmt --check
cargo clippy --release
```

---

**Version**: 0.3.3
**Last updated**: 2026-01-XX
**For Phase**: 3.3 (Preview Rendering)
