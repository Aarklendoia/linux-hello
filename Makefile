.PHONY: build test release check clean help install-pam

help:
@echo "Linux Hello - Makefile commands"
@echo ""
@echo "  make build       - Build in debug mode"
@echo "  make release     - Build optimized release"
@echo "  make test        - Run all unit tests"
@echo "  make check       - Run cargo check (fast)"
@echo "  make clean       - Clean build artifacts"
@echo "  make fmt         - Format code (cargo fmt)"
@echo "  make lint        - Check code style (cargo clippy)"
@echo "  make docs        - Build documentation"
@echo "  make daemon      - Run daemon in debug mode"
@echo "  make camera-test - Test camera capture"
@echo "  make install-pam - Install PAM module (sudo)"
@echo ""

build:
cargo build --all

release:
TMPDIR=/home/edtech/tmp cargo build --all --release

test:
TMPDIR=/home/edtech/tmp cargo test --all --lib

check:
cargo check --all

clean:
cargo clean

fmt:
cargo fmt --all

lint:
cargo clippy --all -- -D warnings

docs:
cargo doc --no-deps --all --open

daemon:
cargo run -p linux_hello_cli -- daemon --debug

camera-test:
cargo run -p linux_hello_cli -- camera --duration 5

install-pam: release
@echo "Installing PAM module..."
sudo cp target/release/libpam_linux_hello.so /lib/security/pam_linux_hello.so
@echo "Done! Module installed at /lib/security/pam_linux_hello.so"
