# Linux Hello

[![Tests](https://github.com/Aarklendoia/linux-hello/actions/workflows/test.yml/badge.svg)](https://github.com/Aarklendoia/linux-hello/actions/workflows/test.yml)
[![Build Debian Packages](https://github.com/Aarklendoia/linux-hello/actions/workflows/build-debian.yml/badge.svg)](https://github.com/Aarklendoia/linux-hello/actions/workflows/build-debian.yml)
[![Quality](https://github.com/Aarklendoia/linux-hello/actions/workflows/quality.yml/badge.svg)](https://github.com/Aarklendoia/linux-hello/actions/workflows/quality.yml)
[![Latest release](https://img.shields.io/github/v/release/Aarklendoia/linux-hello)](https://github.com/Aarklendoia/linux-hello/releases/latest)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange.svg)](https://www.rust-lang.org)
[![Launchpad PPA](https://img.shields.io/badge/PPA-linux--hello-orange)](https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello)

A Windows Hello-style facial recognition authentication system for Linux, integrating with PAM (`sudo`, screen unlock) via a D-Bus daemon and a KDE Plasma configuration GUI.

**Keywords:** face recognition Linux, facial authentication Linux, biometric login Ubuntu Debian, Windows Hello alternative Linux, PAM face unlock, sudo face authentication, KDE Plasma face unlock, screen unlock face recognition, ONNX face detection, open source biometric authentication.

## What is Linux Hello?

Linux Hello lets you unlock `sudo` prompts and your screen lock by looking at your webcam, the same way Windows Hello or macOS's biometric prompts work — except it's free, open source, and runs entirely on your machine (no cloud, no telemetry).

- **You don't need to be a developer to use it.** If you're comfortable installing a `.deb` file and running one or two commands in a terminal, you can set it up in a few minutes — see [Quick start](#quick-start-for-everyday-users) below.
- **It never locks you out.** If the camera fails, the light is bad, or your face isn't recognized, Linux Hello simply falls back to your normal password — it only ever adds a faster option, it never replaces or breaks your existing login.
- **It works alongside `sudo` and your screen lock**, not instead of them — nothing about your existing password-based login changes unless you choose to enroll your face.

## Quick start (for everyday users)

Requirements: a Debian- or Ubuntu-based Linux distribution, a webcam, and a terminal.

1. **Install** the `linux-hello` package — it pulls in everything needed (the daemon, the ONNX models, PAM integration, and the CLI):

   - **Ubuntu 26.04 LTS (resolute)**, via the [Launchpad PPA](https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello) (simplest — `apt` handles updates too):

     ```bash
     sudo add-apt-repository ppa:aarklendoia-edtech/linux-hello
     sudo apt update
     sudo apt install linux-hello
     ```

   - **Other Debian/Ubuntu versions**: download the `.deb` files from the [Releases page](https://github.com/Aarklendoia/linux-hello/releases/latest) into one directory, then:

     ```bash
     sudo apt install ./*.deb
     ```

2. **Enroll your face** (replace `$(id -u)` with your own if enrolling for another user):

   ```bash
   linux-hello enroll $(id -u) --context sudo --samples 3
   ```

3. **Try it**: open a terminal and run `sudo -k && sudo -v` — look at your webcam, and it should authenticate you without asking for a password.

That's it. To also enable face unlock on your login/lock screen, see [docs/QUICKSTART.md](docs/QUICKSTART.md); to manage enrolled faces through a graphical settings app instead of the CLI, see [Graphical app (GUI)](#graphical-app-gui) below.

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

- `linux-hello` — metapackage; depends on the four packages below. Install this one for a complete, working setup
- `linux-hello-daemon` — the daemon binary + PAM module library, systemd user service (depends on `linux-hello-models`)
- `linux-hello-models` — the ONNX models (SCRFD-500M detector + ArcFace MobileNetV3 embedder)
- `linux-hello-tools` — the `linux-hello` CLI (depends on `linux-hello-daemon`)
- `libpam-linux-hello` — wires the PAM module into `sudo`/screenlock (depends on `linux-hello-daemon` and `linux-hello-tools`)
- `linux-hello-gui` — the Kirigami configuration app (depends on `linux-hello-daemon`); **not** pulled in by the `linux-hello` metapackage since it drags in Qt6/Kirigami, which headless/server installs don't need — install it separately if you want the graphical settings app

```bash
sudo apt install ./linux-hello_*.deb ./linux-hello-daemon_*.deb ./linux-hello-tools_*.deb \
  ./libpam-linux-hello_*.deb ./linux-hello-models_*.deb
# or, simplest, if you downloaded every .deb into one directory:
sudo apt install ./*.deb
```

Also available via a [Launchpad PPA](https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello) for Ubuntu 26.04 LTS — see [Quick start](#quick-start-for-everyday-users) above.

See [docs/DEBIAN_PACKAGE.md](docs/DEBIAN_PACKAGE.md) for build instructions and package details.

## Building from source

Requires Rust ≥ 1.93 — check with `rustc --version`; if your distro's package is older (Ubuntu's `rustc` package can lag), install a current toolchain with [rustup](https://rustup.rs/) instead.

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

## Graphical app (GUI)

For enrolling and managing faces without the terminal, there's an optional Kirigami/KDE settings app.

1. **Install it** — it's a separate package (not pulled in by the `linux-hello` metapackage, since it depends on Qt6/Kirigami):

   ```bash
   sudo apt install linux-hello-gui
   ```

2. **Launch it** — either from your application launcher (search for **"Linux Hello"**), or from a terminal:

   ```bash
   linux_hello_config
   ```

From there you can enroll a new face, see how many faces are registered, and delete any of them — the daemon status shown on the home screen reflects whether `hello-daemon` is actually running.

A third card lets you enable or disable face auth on the SDDM login screen (used when switching users) with a single click, prompting for authentication via `pkexec`. This needs **both** `linux-hello-gui` (above) **and** `libpam-linux-hello` installed — the latter isn't pulled in automatically by `linux-hello-gui` either, so on a GUI-only install the card shows as unavailable:

```bash
sudo apt install libpam-linux-hello
```

`libpam-linux-hello` is already included if you installed the `linux-hello` metapackage. This is deliberately opt-in rather than automatic — see [docs/PAM_MODULE.md](docs/PAM_MODULE.md#sddm-login-screen) for why.

## Security notes

**You can never be locked out.** Every PAM line this project installs uses
`auth sufficient` — if the camera fails, the face isn't recognized, or
anything else goes wrong, PAM falls through to the normal password prompt.
Face recognition only ever adds a faster option; it never replaces or gates
your existing login. Calls are also bounded by a timeout, so a stuck camera
can't hang a login attempt.

**Nothing leaves the machine.** All processing (face detection, embedding
extraction, matching) runs locally via ONNX Runtime — no cloud service, no
telemetry, no network calls involved in authentication itself.

- **Storage**: `~/.local/share/linux-hello/faces.db` (user mode) or
  `/var/lib/linux-hello/` (root/system mode), restricted permissions. A user
  can only enroll/manage their own face over D-Bus; root can manage any
  user's.
- **`sudo`/`su`**: activates automatically once you enroll a face, with a
  `confirm` prompt — a successful face match still requires an explicit
  `[y/N]` before access is granted, guarding against an accidental grant
  from just being visible to the camera.
- **SDDM (login screen)**: deliberately **not** part of automatic
  activation, and off by default even after installing everything. Enabling
  it starts a new root-owned, always-on, pre-authentication-reachable
  listener (`hello-daemon-system.service`) — a meaningfully larger change to
  the machine's attack surface than the on-demand `sudo`/screenlock path, so
  it's always an explicit, separate opt-in: `sudo install-pam.sh --enable-sddm`,
  or the GUI's home-screen toggle (itself gated behind a real `pkexec`
  prompt). See [docs/PAM_MODULE.md](docs/PAM_MODULE.md#sddm-login-screen)
  for the full reasoning and its documented residual risks (e.g. a minor
  response-timing side channel).
- **GUI ↔ backend channel**: the configuration GUI talks to its own backend
  over a local HTTP server on loopback. Every request must carry a random
  token (generated fresh per launch, written to a `0600` file only the
  launching user can read) — without it, any other local process could
  otherwise read your enrolled-face list or, worse, trigger the SDDM
  `pkexec` prompt on your behalf.

## Documentation

Further docs live in [`docs/`](docs/): architecture and D-Bus/PAM design, GUI architecture, internationalization, screenlock integration, CI/CD infrastructure, command reference, development and release process, and [publishing to Launchpad](docs/LAUNCHPAD.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

[GPL-3.0-or-later](LICENSE).
