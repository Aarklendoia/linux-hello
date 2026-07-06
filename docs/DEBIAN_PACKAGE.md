# Debian Package Build & Installation

## Package Information

The Linux Hello project builds 5 Debian packages:

| Package | Contents |
| --- | --- |
| `linux-hello` | Meta-package; depends on the packages below |
| `linux-hello-models` | The ONNX models (SCRFD-500M detector + ArcFace MobileNetV3 embedder) |
| `libpam-linux-hello` | PAM module, plus the automatic sudo activation timer and the opt-in SDDM system listener |
| `linux-hello-tools` | The `linux-hello` CLI |
| `linux-hello-gui` | The Kirigami configuration app |

## Building the Debian Package

### Prerequisites

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential debhelper dpkg-dev \
  libssl-dev libpam0g-dev libdbus-1-dev pkg-config unzip \
  qt6-base-dev qml6-module-qtcore qml6-module-qtquick \
  qml6-module-qtquick-layouts qml6-module-qtquick-controls
```

A Rust toolchain new enough for the pinned dependencies is also required
(check with `rustc --version`; if your distro's `rustc` is too old, install
a current one via [rustup](https://rustup.rs/) instead of apt's).

### Build Command

```bash
cd /path/to/linux-hello
dpkg-buildpackage -b -d --no-check-builddeps -us -uc
```

(Drop `-us -uc` if you have a GPG key configured and want a signed build.)

This creates the `.deb` files in the parent directory, e.g.:

```text
../linux-hello_<version>_amd64.deb
../linux-hello-models_<version>_all.deb
../libpam-linux-hello_<version>_amd64.deb
../linux-hello-tools_<version>_amd64.deb
../linux-hello-gui_<version>_amd64.deb
```

## Installation

```bash
sudo apt install ./linux-hello_<version>_amd64.deb
```

`apt` resolves the meta-package's dependencies and installs everything.
`linux-hello`'s postinst:

- Creates `~/.local/share/linux-hello` for the installing user
- Enables and starts the per-user `hello-daemon.service`
  (`systemctl --user`)
- Installs the KDE lock-screen QML overlay via `dpkg-divert`

It does **not** run an interactive wizard and does **not** touch PAM
configuration — see below.

## PAM Activation

- **sudo activates automatically**: `libpam-linux-hello` ships
  `linux-hello-pam-autoconfigure.timer`, which configures `/etc/pam.d/sudo`,
  `su`, and `polkit-1` as soon as any local user enrolls a face — no manual
  step. See [PAM_MODULE.md](PAM_MODULE.md#automatic-activation).
- **Screenlock unlocking needs no PAM configuration at all**: it's handled
  by `hello-daemon`'s own watcher, which unlocks the session directly via
  `loginctl` on a face match once you're locked out.
- **SDDM (login screen) stays opt-in**: run `sudo ./install-pam.sh` from a
  source checkout (or see [PAM_MODULE.md](PAM_MODULE.md#sddm-login-screen)
  for what it does) — this also starts a new root-owned system listener,
  which is enough of a change to a machine's attack surface that it's
  never enabled automatically.
- To disable everything and restore the original PAM files:
  `sudo ./install-pam.sh --remove`.

## Post-Installation

### Register a face

```bash
linux-hello enroll $(id -u) --context sudo --samples 3
```

### Test

```bash
sudo -k && sudo -v   # should authenticate with your face once enrolled
```

Screenlock: lock the screen and present your face to unlock.

## Uninstallation

```bash
sudo apt remove linux-hello
```

This stops/disables the daemon and the autoconfigure timer. It does
**not** revert `/etc/pam.d/*` changes — run `sudo ./install-pam.sh
--remove` first if you want the original PAM files restored.

## File Locations

```text
/usr/bin/
  hello-daemon                  # Per-user daemon
  hello-daemon-system           # SDDM system listener (opt-in)
  linux-hello                   # CLI
  linux_hello_config            # GUI
  linux-hello-pam-autoconfigure # Automatic PAM activation script

/lib/<multiarch>/security/
  pam_linux_hello.so             # PAM module (e.g. /lib/x86_64-linux-gnu/security/)

/usr/lib/systemd/user/
  hello-daemon.service

/usr/lib/systemd/system/
  linux-hello-pam-autoconfigure.timer
  linux-hello-pam-autoconfigure.service
  hello-daemon-system.service     # enabled only via install-pam.sh

/usr/share/linux-hello/models/
  det_500m.onnx
  w600k_mbf.onnx

/etc/linux-hello/
  config.toml.example

/usr/share/doc/linux-hello/
  README.md, QUICKSTART.md, INTEGRATION_GUIDE.md, PAM_MODULE.md, ...
```

## Configuration Files Modified

`linux-hello-pam-autoconfigure` and `install-pam.sh` back up any PAM file
before editing it: `/etc/pam.d/<service>.pre-linuxhello-<timestamp>`.
`install-pam.sh --remove` restores from the latest backup (or strips the
inserted lines if none exists) and writes `/etc/linux-hello/pam-disabled`
so automatic activation won't silently re-enable what was just removed.

## Troubleshooting

### Build issues

Missing build dependencies: see [Prerequisites](#prerequisites) above —
`sudo apt-get install -y <missing package>`.

### "Cannot start daemon"

```bash
systemctl --user restart hello-daemon
journalctl --user -u hello-daemon -n 50
```

### "PAM module not found"

```bash
sudo ./install-pam.sh --status
find /lib -name pam_linux_hello.so
```

### "Camera not detected"

```bash
ls -la /dev/video*
sudo usermod -a -G video $USER
```

### Restore PAM from backup manually

```bash
ls -la /etc/pam.d/*.pre-linuxhello-*
sudo cp /etc/pam.d/sudo.pre-linuxhello-<timestamp> /etc/pam.d/sudo
```

Or just run `sudo ./install-pam.sh --remove`.

## Package Dependencies

- `linux-hello`: `dbus`, `systemd`, `libonnxruntime1.23`, `linux-hello-models`
- `libpam-linux-hello`: `linux-hello`, `libpam-runtime`
- `linux-hello-gui`: `linux-hello`, `qml-qt6`, `qml6-module-org-kde-kirigami`,
  `qml6-module-qtcore`, `qml6-module-qtquick`,
  `qml6-module-qtquick-layouts`, `qml6-module-qtquick-controls`

Build-time only: `debhelper-compat`, `libssl-dev`, `libpam0g-dev`,
`libdbus-1-dev`, `pkg-config`, `unzip`, `qt6-base-dev`, and the QML modules
listed under [Prerequisites](#prerequisites).

## Notes

1. **Per-user daemon**: `hello-daemon` runs as a systemd *user* service, not
   system-wide. The only system-level, always-on component is the opt-in
   SDDM listener (`hello-daemon-system.service`).
2. **Password fallback always available**: every PAM line uses `auth
   sufficient` — biometric failure or an unavailable daemon always falls
   through to the normal password prompt.
3. **No root required for enrollment/verification** against the per-user
   daemon.

---

For more information, see:

- [README.md](../README.md) — project overview
- [QUICKSTART.md](QUICKSTART.md) — getting started
- [INTEGRATION_GUIDE.md](INTEGRATION_GUIDE.md) — detailed integration steps
- [PAM_MODULE.md](PAM_MODULE.md) — PAM configuration reference, including
  automatic activation and SDDM support
