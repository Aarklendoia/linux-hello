# Linux Hello

A Windows Hello-style facial recognition authentication system for Linux, integrating with PAM (`sudo`, screen unlock) via a D-Bus daemon and a KDE Plasma configuration GUI.

## How it works

```text
PAM (sudo / screenlock)
   │  auth sufficient pam_linux_hello.so
   ▼
pam_linux_hello.so  ──D-Bus──▶  hello-daemon (com.linuxhello.FaceAuth)
                                    │
                                    ├─ hello_camera   → capture a frame (V4L2)
                                    ├─ hello_face_core → detect face (SCRFD-500M) +
                                    │                    extract embedding (ArcFace MobileNetV3)
                                    │                    via ONNX Runtime (dynamic loading)
                                    └─ SQLite storage  → compare against enrolled embeddings
```

If no face is detected or the match fails, PAM falls back to the normal password prompt — face recognition is always additive, never a hard lock-out.

## Components

| Crate | Type | Role |
| --- | --- | --- |
| `hello_face_core` | lib | Face detection (SCRFD-500M), embedding extraction (ArcFace MobileNetV3), liveness check |
| `hello_camera` | lib | V4L2 camera abstraction |
| `hello_daemon` | lib + bin (`hello-daemon`) | D-Bus service (`com.linuxhello.FaceAuth`), SQLite storage, screenlock monitor, streaming preview |
| `pam_linux_hello` | cdylib (`pam_linux_hello.so`) | PAM module — calls the daemon over D-Bus to authenticate |
| `linux_hello_cli` | bin (`linux-hello`) | CLI for enrollment, verification and management |
| `linux_hello_config` | bin | Qt6/QML/Kirigami configuration GUI, with 10-language i18n |

Each crate builds and tests independently; see [docs/DESIGN.md](docs/DESIGN.md) (D-Bus/PAM design) for details.

## Installing from Debian packages

The project builds 6 `.deb` packages:

- `linux-hello` — meta-package (depends on the packages below)
- `linux-hello-daemon` — the daemon binary + systemd user service
- `linux-hello-models` — the ONNX models (SCRFD-500M detector + ArcFace MobileNetV3 embedder)
- `linux-hello-tools` — the `linux-hello` CLI
- `libpam-linux-hello` — the PAM module
- `linux-hello-gui` — the Kirigami configuration app

```bash
sudo apt install ./linux-hello_*.deb
```

See [docs/DEBIAN_PACKAGE.md](docs/DEBIAN_PACKAGE.md) for build instructions and package details.

## Building from source

Requires Rust ≥ 1.94 — check with `rustc --version`; if your distro's package is older (Ubuntu's `rustc` package can lag), install a current toolchain with [rustup](https://rustup.rs/) instead.

System dependencies (Debian/Ubuntu):

```bash
sudo apt install build-essential libssl-dev libpam0g-dev libdbus-1-dev \
  pkg-config unzip qt6-base-dev qml6-module-qtcore qml6-module-qtquick \
  qml6-module-qtquick-layouts qml6-module-qtquick-controls
```

Then:

```bash
cargo build --release

# Or build a single crate
cargo build -p hello_daemon --release
```

On first build, `hello_face_core`'s build script downloads the ONNX models into `~/.local/share/linux-hello/models/` (falls back to a stub detector if the download fails — set `LINUX_HELLO_NO_MODEL_DOWNLOAD=1` to skip it, e.g. in CI).

## Configuring PAM

```text
# /etc/pam.d/sudo
auth   sufficient   pam_linux_hello.so context=sudo confirm=true
auth   include      system-auth

# /etc/pam.d/kde (KScreenLocker)
auth   sufficient   pam_linux_hello.so context=screenlock timeout_ms=3000
auth   include      system-login
```

Module options: `context`, `timeout_ms`, `similarity_threshold`, `confirm`, `debug`. Full reference in [docs/PAM_MODULE.md](docs/PAM_MODULE.md) and [docs/INTEGRATION_GUIDE.md](docs/INTEGRATION_GUIDE.md).

## CLI usage

```bash
# Run the daemon (debug mode)
linux-hello daemon --debug

# Enroll a face
linux-hello enroll <uid> --context sudo --samples 3

# Verify
linux-hello verify <uid> --context sudo

# List / delete enrolled faces
linux-hello list <uid>
linux-hello delete <uid> [face_id]
```

## Security notes

- Storage: `~/.local/share/linux-hello/faces.db` (user mode) or `/var/lib/linux-hello/` (root/system mode), restricted permissions.
- A user can only manage their own face over D-Bus; root can manage any user.
- PAM calls are bounded by a timeout and always degrade to password authentication on failure.

## Documentation

Further docs live in [`docs/`](docs/): architecture and D-Bus/PAM design, GUI architecture, internationalization, screenlock integration, CI/CD infrastructure, command reference, development and release process.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

GPL-3.0-or-later.
