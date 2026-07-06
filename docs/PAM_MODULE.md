# Linux Hello PAM Module

PAM authentication module for Linux Hello - enables facial authentication via PAM for login, sudo, screenlock, etc.

## Building

```bash
cargo build -p pam_linux_hello --release
```

The compiled module will be at `target/release/libpam_linux_hello.so`

## Installation

### System installation

```bash
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/
```

### Or local test installation

Use the full path in the PAM configuration to test without root privileges.

## Automatic Activation

Installing `libpam-linux-hello` also installs `linux-hello-pam-autoconfigure.timer`,
a systemd system timer (runs shortly after boot, then every ~5 minutes) that
configures PAM for **sudo** automatically, as soon as any local user has
enrolled at least one face — no manual step needed. It edits `sudo`,
`sudo-i`, `su`, `su-l`, and `polkit-1`, using the same idempotent, backed-up
insertion logic as `install-pam.sh` (see `pam-lib.sh`). The `sudo`/`sudo-i`/
`su`/`su-l` lines are configured with the [`confirm`](#available-options)
option — `polkit-1` isn't, since it already has its own confirmation dialog.
It's safe on multi-user machines: the module already falls back to the password for any
user with no enrolled face, so a single system-wide activation is correct
regardless of who has actually enrolled.

**Screenlock doesn't use PAM at all.** Unlocking the screen with your face is
handled entirely by `hello-daemon`'s own watcher: it polls
`org.freedesktop.ScreenSaver` over D-Bus, and on a face match while the
screen is locked, unlocks it directly via `loginctl unlock-session` (see
`hello_daemon/src/screenlock.rs`). This needed no PAM configuration to begin
with — earlier revisions of this project tried inserting a
`pam_linux_hello.so context=screenlock` line into a KDE-specific PAM service
file (`kde-screenlocker`, or `kde` on older setups), but current KDE Plasma
(6.x) doesn't ship or use either of those by default (its actual PAM service
is named `kscreenlocker`, with no file present unless the distro ships one),
so that approach never did anything in practice — removed rather than fixed,
since the watcher already solves this without it.

**Not covered by the automatic timer — the SDDM login screen.**
`linux-hello-pam-autoconfigure` deliberately never touches `/etc/pam.d/sddm`.
Login-screen support does exist (see [SDDM (Login Screen)](#sddm-login-screen)
below) but stays manual/opt-in only, via `install-pam.sh` — it starts a new,
always-on, root-owned, pre-authentication-reachable listener, which is
enough of a change to a machine's attack surface that it shouldn't happen
silently just because a face was enrolled.

**Known limitation.** User enrollment is detected by scanning `/etc/passwd`
directly (no `getent`/NSS lookup) — accounts served only via `systemd-homed`
(not present as real `/etc/passwd` lines) won't be detected by the automatic
timer, though PAM auth for those users still works fine once configured by
some other means.

**Opting out.** Run `sudo ./install-pam.sh --remove` to disable and revert;
this creates `/etc/linux-hello/pam-disabled`, which the automatic timer
checks and respects — it will not re-enable PAM config while that marker
exists. Running `install-pam.sh` again (without `--remove`) clears the
marker and re-enables automatic activation. `install-pam.sh --status` shows
both the current PAM configuration state and whether automatic activation is
enabled or opted out.

## SDDM (Login Screen)

Unlike sudo/screenlock, the login screen has no active session yet for the
user being authenticated, so the usual per-user `hello-daemon` (and its
per-user `/run/hello-pam/<uid>.socket`) can't help. Instead, installing
`libpam-linux-hello` also installs `hello-daemon-system.service` — a
**separate, minimal, root-owned, always-on listener**, started at boot,
that binds a fixed socket (`/run/hello-pam/system.socket`) with no D-Bus
surface and no ability to enroll or delete faces, only to verify. When
`pam_linux_hello` runs with `context=sddm`, it connects to this socket
instead of the per-user one; the listener resolves the target username's
home directory directly (root can read any home) and checks
`~/.local/share/linux-hello/users/<uid>/` for enrolled faces, without ever
creating anything there.

**This capability is opt-in only**, via `sudo ./install-pam.sh` (which both
inserts the `pam_linux_hello.so context=sddm` line into `/etc/pam.d/sddm`
and enables `hello-daemon-system.service`) or reverted together via
`install-pam.sh --remove`. It is deliberately **not** part of automatic
activation (`linux-hello-pam-autoconfigure` never touches `sddm` or this
service) — starting a new pre-authentication-reachable root listener is a
large enough change to a machine's attack surface that it should never
happen silently just because someone enrolled a face for sudo/screenlock.

Security notes:

- The socket is mode `0600` and only accepts connections whose peer UID is
  `0` — verified on a live Kubuntu 26.04/SDDM system that the process
  actually performing `/etc/pam.d/sddm` authentication (`sddm-helper`) runs
  as root. Non-root connections are dropped immediately, before any read.
- A non-blocking, cross-process file lock (`/run/lock/linux-hello-camera.lock`)
  serializes camera access between this listener and any per-user daemon
  that might be capturing at the same moment (e.g. fast user switching); on
  contention, verification fails fast with a distinct "camera busy" reason
  rather than silently degrading to blank frames.

Known limitations (accepted, not solved):

- If a target user's home directory lives on storage not mounted/decrypted
  until *after* a successful PAM session phase (network home, per-user
  encryption), the pre-auth read sees an empty or inaccessible home and
  falls back to password — safe, but unhelpful.
- Response time differs measurably between "no such user," "user exists but
  never enrolled," and "enrolled → camera capture happens" (roughly 1-5s).
  This is a mild timing side-channel: someone at the greeter (or scripting
  repeated attempts) could infer which local accounts have enrolled a face.
  A constant-time-floor response is a possible future hardening, not
  implemented today.
- Raw `/etc/passwd` parsing (no `getent`/NSS) won't resolve
  `systemd-homed`-only accounts, same as the automatic timer's limitation
  above.
- No visual integration with the SDDM greeter itself (e.g. a live camera
  preview) — feedback is text-only via the PAM conversation, matching the
  existing sudo/screenlock experience.

## PAM Configuration

### Basic format

```text
auth [sufficient|required] /path/to/libpam_linux_hello.so [options]
```

### Available options

- `context=<context>`: Authentication context (login, sudo, screenlock, sddm, test, etc.) [default: default]
- `timeout_ms=<ms>`: Timeout in milliseconds for capture [default: 5000]
- `similarity_threshold=<0.0-1.0>`: Similarity threshold [default: 0.6]
- `confirm`: After a face match, prompt "Confirm? [y/N]" and require an
  explicit `y` before granting access, instead of granting immediately.
  Guards against an accidental grant from someone merely being visible to
  the camera while a prompt is open. Enabled by default on the
  automatically-configured `sudo`/`sudo-i`/`su`/`su-l` lines (see
  [Automatic Activation](#automatic-activation)); not applied to
  `screenlock` (meant to stay hands-free) or `polkit`/`sddm` (which already
  have their own confirmation UI). No timeout of its own — PAM's
  conversation API doesn't support one — so a confirmation prompt can wait
  as long as the surrounding context already waits for password entry
  today; this is not a new class of risk.
- `debug`: Enable debug logs

### Usage Examples

#### Login (SDDM/GDM)

```bash
# /etc/pam.d/sddm
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sddm timeout_ms=5000
auth include common-auth
```

#### Sudo

```bash
# /etc/pam.d/sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000
@include common-auth
```

#### Screenlock (generic PAM-based greeter)

Not needed on current KDE Plasma — see [Automatic Activation](#automatic-activation)
above for how screen unlocking actually works there (no PAM involved). This
is a reference example for a greeter that authenticates through a real
PAM service file (e.g. GNOME's, or an older KDE `kde-screenlocker`/`kde`):

```bash
# /etc/pam.d/<your-screenlock-service>
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=screenlock timeout_ms=3000
auth required pam_permit.so
```

## PAM Return Codes

- `PAM_SUCCESS`: Authentication succeeded (face recognized)
- `PAM_AUTH_ERR`: Authentication failed (face not recognized or system error)
- `PAM_SYSTEM_ERR`: System error (daemon unavailable, etc.)
- `PAM_IGNORE`: Module cannot authenticate (debug mode)

## Recommended Contexts and Thresholds

Similarity thresholds vary depending on the context:

| Context | Default Threshold | Recommendation |
| ---------- | ------------------ | --- |
| login | 0.65 | Strict |
| sddm | 0.65 | Strict |
| sudo | 0.70 | Very strict |
| screenlock | 0.60 | Moderate |
| test | 0.50 | Permissive (test) |

## System Dependencies

The PAM module requires:

- D-Bus session bus running
- Linux Hello daemon running (`hello-daemon`)
- Faces enrolled for the user

## Testing

### Direct D-Bus test (without PAM)

```bash
# Start the daemon
./target/debug/hello-daemon --debug &

# Enroll a face
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.RegisterFace \
  string:'{"user_id":1000,"context":"test","timeout_ms":5000,"num_samples":3}'

# Verify the face
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Verify \
  string:'{"user_id":1000,"context":"test","timeout_ms":5000}'
```

### PAM Test with pamtester

```bash
# See the project sources for the full test script
./test-pam-full.sh
```

## Security

The PAM module implements:

- Verification based on the user's UID
- Access to the D-Bus session daemon (session isolation)
- Structured logs for auditing
- Timeouts to prevent blocking

## Troubleshooting

### "The name com.linuxhello.FaceAuth was not provided by any .service files"

The Linux Hello daemon is not running. Start it:

```bash
./target/debug/hello-daemon
```

### "Unable to retrieve UID for user"

The user doesn't exist or `getpwnam` is unavailable. Check with:

```bash
id username
```

### Module doesn't compile

Make sure the Rust dependencies are up to date:

```bash
cargo update -p hello_daemon -p pam_linux_hello
```

## Architecture

```text
Login/Sudo/Screenlock
         |
         v
   PAM Stack
         |
         v
   pam_linux_hello.so
         |
         v
    D-Bus session
         |
         v
  hello-daemon
         |
         v
 Camera + Face Matching
```

## Current Limitations

- Uses simulated camera (virtual frames)
- No multi-face support per probe
- Global timeout for capture+matching
- No persistent logging

## Future Improvements

- [ ] Real camera integration (V4L2/PipeWire)
- [ ] Real machine learning (ONNX/TensorFlow)
- [ ] Multi-modal support (IR, Depth)
- [ ] Polkit for sudo without PAM
- [ ] REST API in addition to D-Bus
- [ ] Persistent database (sqlite)
